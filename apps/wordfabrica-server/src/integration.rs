//! Third-party integration: another system hands us a DOCX (by URL or upload), users edit it in
//! the browser, and every explicit save is exported to file storage and announced by a callback.
//! All calls from the third party are HMAC-signed with a shared secret (see `signing`).
use axum::{
    Json, Router,
    body::{Body, Bytes, to_bytes},
    extract::{FromRequest, Multipart, Path, Query, Request, State, rejection::QueryRejection},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::Notify;
use url::Url;
use wordfabrica_remote::{MAX_BYTES, Metadata, valid_id};

use crate::{
    blocking,
    links::{self, Claims, Mode},
    signing::{self, MAX_SKEW_SECS, SIGNATURE_HEADER, TIMESTAMP_HEADER},
    storage::{self, Files, MinioSettings},
    store::{Error, Integration, Store},
};

/// Integration configuration, read from the environment at startup.
#[derive(Clone)]
pub struct Settings {
    pub secret: Vec<u8>,
    /// Base URL users and third parties reach this server at, e.g. `https://docs.example.com`.
    pub public_url: Url,
    pub storage: StorageSettings,
    /// Lifetime of download links handed out in callbacks and status replies.
    pub download_ttl: Duration,
    /// Hosts the server may download documents from and call back; empty allows any host.
    pub allow_hosts: Vec<String>,
}

/// Which file storage backs the integration.
#[derive(Clone, Debug)]
pub enum StorageSettings {
    Local(PathBuf),
    Minio(MinioSettings),
}

// Settings get logged at startup; never print the shared secret.
impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settings")
            .field("public_url", &self.public_url.as_str())
            .field("storage", &self.storage)
            .field("download_ttl", &self.download_ttl)
            .field("allow_hosts", &self.allow_hosts)
            .finish_non_exhaustive()
    }
}

impl Settings {
    /// Read `WORDFABRICA_INTEGRATION_SECRET` and friends. Without a secret the integration is off (`None`).
    pub fn from_env(data_dir: &std::path::Path) -> Result<Option<Self>, String> {
        let var = |name: &str| std::env::var(name).ok().map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
        let Some(secret) = var("WORDFABRICA_INTEGRATION_SECRET") else { return Ok(None) };
        if secret.len() < 32 {
            return Err("WORDFABRICA_INTEGRATION_SECRET must be at least 32 bytes".into());
        }
        let public_url = var("WORDFABRICA_PUBLIC_URL").ok_or("WORDFABRICA_PUBLIC_URL is required when the integration is enabled")?;
        let public_url = Url::parse(&public_url).map_err(|e| format!("WORDFABRICA_PUBLIC_URL: {e}"))?;
        if !matches!(public_url.scheme(), "http" | "https") {
            return Err("WORDFABRICA_PUBLIC_URL must be an http(s) URL".into());
        }
        let storage = match var("WORDFABRICA_STORAGE").as_deref().unwrap_or("local") {
            "local" => StorageSettings::Local(var("WORDFABRICA_STORAGE_DIR").map_or_else(|| data_dir.join("files"), PathBuf::from)),
            "minio" => {
                let required = |name: &str| var(name).ok_or(format!("{name} is required when WORDFABRICA_STORAGE=minio"));
                StorageSettings::Minio(MinioSettings {
                    endpoint: required("WORDFABRICA_S3_ENDPOINT")?,
                    bucket: required("WORDFABRICA_S3_BUCKET")?,
                    access_key: required("WORDFABRICA_S3_ACCESS_KEY")?,
                    secret_key: required("WORDFABRICA_S3_SECRET_KEY")?,
                    region: var("WORDFABRICA_S3_REGION").unwrap_or_else(|| "us-east-1".into()),
                })
            }
            other => return Err(format!("WORDFABRICA_STORAGE must be local or minio, not {other:?}")),
        };
        let ttl = match var("WORDFABRICA_DOWNLOAD_TTL") {
            Some(v) => {
                v.parse::<u64>().ok().filter(|s| (60..=7 * 24 * 3600).contains(s)).ok_or("WORDFABRICA_DOWNLOAD_TTL must be 60..=604800 seconds")?
            }
            None => 3600,
        };
        let allow_hosts = var("WORDFABRICA_ALLOW_HOSTS")
            .map(|v| v.split(',').map(|h| h.trim().to_ascii_lowercase()).filter(|h| !h.is_empty()).collect())
            .unwrap_or_default();
        Ok(Some(Self { secret: secret.into_bytes(), public_url, storage, download_ttl: Duration::from_secs(ttl), allow_hosts }))
    }
}

/// Shared integration state: settings, file storage, HTTP clients and the callback wake-up.
pub struct Service {
    pub(crate) settings: Settings,
    pub(crate) files: Files,
    pub(crate) store: Arc<Store>,
    pub(crate) notify: Notify,
    fetcher: reqwest::Client,
    pub(crate) caller: reqwest::Client,
    /// First retry delay; doubles per attempt (tests shorten it).
    pub(crate) retry_base: Duration,
}

impl Service {
    /// Open file storage and build the HTTP clients.
    pub fn new(settings: Settings, store: Arc<Store>) -> Result<Arc<Self>, String> {
        let files = match &settings.storage {
            StorageSettings::Local(root) => Files::local(root)?,
            StorageSettings::Minio(minio) => Files::minio(minio)?,
        };
        // Every redirect hop is checked like the first URL, so an allowed host can't bounce us elsewhere.
        let hosts = settings.allow_hosts.clone();
        let policy = reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() >= 5 {
                attempt.error("too many redirects")
            } else if host_allowed(&hosts, attempt.url()) {
                attempt.follow()
            } else {
                attempt.stop()
            }
        });
        let fetcher =
            reqwest::Client::builder().redirect(policy).timeout(Duration::from_secs(15)).build().map_err(|e| format!("HTTP client: {e}"))?;
        // Callbacks never follow redirects: a signed body must only reach the configured URL.
        let caller = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| format!("HTTP client: {e}"))?;
        Ok(Arc::new(Self { settings, files, store, notify: Notify::new(), fetcher, caller, retry_base: Duration::from_secs(5) }))
    }

    /// Same as `new`, with a custom first retry delay.
    pub fn with_retry_base(settings: Settings, store: Arc<Store>, retry_base: Duration) -> Result<Arc<Self>, String> {
        let mut service = Self::new(settings, store)?;
        if let Some(service) = Arc::get_mut(&mut service) {
            service.retry_base = retry_base;
        }
        Ok(service)
    }

    pub(crate) fn allowed(&self, url: &Url) -> bool {
        host_allowed(&self.settings.allow_hosts, url)
    }

    /// Who may read or write document `id`: `None` for the editor's own documents; for third-party
    /// documents the claims of a valid access token (401 without one, 403 for writes with a view link).
    pub(crate) async fn authorize(&self, id: &str, token: Option<&str>, write: bool) -> Result<Option<Claims>, Error> {
        let (store, lookup) = (self.store.clone(), id.to_owned());
        if blocking(move || store.integration(&lookup)).await?.is_none() {
            return Ok(None);
        }
        let claims = token.and_then(|t| links::verify(&self.settings.secret, t, id, signing::now_secs())).ok_or(Error::Unauthorized)?;
        if write && claims.mode != Mode::Edit {
            return Err(Error::ReadOnly);
        }
        Ok(Some(claims))
    }

    /// After an explicit save by `user`: export DOCX, store it and queue the callback.
    /// Every step is idempotent, so a client retry is safe.
    pub(crate) async fn commit(&self, metadata: &Metadata, mut document: wordfabrica_doc::Document, user: &Claims) -> Result<(), Error> {
        let bytes = blocking(move || {
            document.ensure_nonempty();
            wordfabrica_docx::write(&document).map_err(|e| {
                eprintln!("DOCX export failed: {e}");
                Error::Storage
            })
        })
        .await?;
        self.files.put(&storage::saved_key(&metadata.id, metadata.revision), bytes).await?;
        let (store, m, user) = (self.store.clone(), metadata.clone(), user.clone());
        blocking(move || store.enqueue(&m.id, m.revision, &m.title, m.updated_at, (&user.user_id, &user.user_name))).await?;
        self.notify.notify_one();
        Ok(())
    }

    /// A download link for a saved revision: presigned for MinIO, server-signed for local storage.
    pub(crate) async fn download_url(&self, id: &str, revision: u64) -> Result<(String, u64), Error> {
        let ttl = self.settings.download_ttl;
        let expires = signing::now_secs().saturating_add(ttl.as_secs());
        if let Some(url) = self.files.presign(&storage::saved_key(id, revision), ttl).await? {
            return Ok((url.into(), expires));
        }
        let signature = signing::sign(&self.settings.secret, &[id.as_bytes(), revision.to_string().as_bytes(), expires.to_string().as_bytes()]);
        let url = self.public(&format!("api/integration/files/{id}/{revision}?expires={expires}&sig={signature}"))?;
        Ok((url, expires))
    }

    /// An absolute URL under the public base (which may itself carry a path prefix).
    fn public(&self, relative: &str) -> Result<String, Error> {
        let mut base = self.settings.public_url.clone();
        if !base.path().ends_with('/') {
            let path = format!("{}/", base.path());
            base.set_path(&path);
        }
        base.join(relative).map(String::from).map_err(|_| Error::Storage)
    }

    /// Download a document from the third party, bounded in size, time and redirects.
    async fn fetch(&self, url: &Url) -> Result<Vec<u8>, Error> {
        let mut response = self.fetcher.get(url.clone()).send().await.map_err(|e| fetch_error(&e))?;
        if !response.status().is_success() {
            eprintln!("document download: HTTP {}", response.status());
            return Err(Error::Fetch);
        }
        if response.content_length().is_some_and(|n| n > MAX_BYTES as u64) {
            return Err(Error::TooLarge);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|e| fetch_error(&e))? {
            if bytes.len().saturating_add(chunk.len()) > MAX_BYTES {
                return Err(Error::TooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}

fn fetch_error(error: &reqwest::Error) -> Error {
    eprintln!("document download: {error}");
    Error::Fetch
}

/// Only http(s) URLs with a host, and only allowed hosts when an allow list is configured.
fn host_allowed(hosts: &[String], url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some_and(|host| hosts.is_empty() || hosts.iter().any(|h| h.eq_ignore_ascii_case(host)))
}

/// Integration routes. Document routes need a signed request; the file route checks its own link signature.
pub(crate) fn routes(service: Arc<Service>) -> Router {
    let signed = Router::new()
        .route("/api/integration/documents", post(create))
        .route("/api/integration/documents/{id}", get(status))
        .route("/api/integration/documents/{id}/links", post(link))
        .route_layer(middleware::from_fn_with_state(service.clone(), verify));
    Router::new().route("/api/integration/files/{id}/{revision}", get(download)).merge(signed).with_state(service)
}

/// Reject requests whose `x-wordfabrica-signature` isn't HMAC(secret, "{timestamp}.{METHOD}.{path}.{body}").
/// The body is buffered (already capped by the router's body limit) so it can be signed, then restored.
async fn verify(State(service): State<Arc<Service>>, request: Request, next: Next) -> Response {
    let (parts, body) = request.into_parts();
    let header = |name: &str| parts.headers.get(name).and_then(|v| v.to_str().ok()).map(str::to_owned);
    let (Some(timestamp), Some(signature)) = (header(TIMESTAMP_HEADER), header(SIGNATURE_HEADER)) else {
        return Error::Unauthorized.into_response();
    };
    let fresh = timestamp.parse::<u64>().is_ok_and(|t| t.abs_diff(signing::now_secs()) <= MAX_SKEW_SECS);
    let Ok(bytes) = to_bytes(body, MAX_BYTES).await else { return Error::TooLarge.into_response() };
    let method = parts.method.as_str().as_bytes();
    let signed = signing::verify(&service.settings.secret, &[timestamp.as_bytes(), method, parts.uri.path().as_bytes(), &bytes], &signature);
    if !fresh || !signed {
        return Error::Unauthorized.into_response();
    }
    next.run(Request::from_parts(parts, Body::from(bytes))).await
}

/// What the third party tells us about a document. With JSON requests `url` says where to fetch it.
#[derive(Deserialize)]
struct CreateRequest {
    #[serde(default)]
    url: Option<String>,
    file_name: String,
    callback_url: String,
    key: String,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Created {
    pub id: String,
    pub revision: u64,
}

/// `POST /api/integration/documents`: JSON `{url,…}` (we download) or multipart `meta` + `file` (upload).
async fn create(State(service): State<Arc<Service>>, request: Request) -> Result<Json<Created>, Error> {
    let content_type = request.headers().get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").to_ascii_lowercase();
    let (meta, bytes) = if content_type.starts_with("multipart/form-data") {
        let mut multipart = Multipart::from_request(request, &()).await.map_err(|_| Error::Rejected("malformed multipart body"))?;
        let (mut meta, mut file) = (None, None);
        while let Some(field) = multipart.next_field().await.map_err(|_| Error::Rejected("malformed multipart body"))? {
            match field.name() {
                Some("meta") => meta = Some(field.bytes().await.map_err(|_| Error::Rejected("malformed multipart body"))?),
                Some("file") => file = Some(field.bytes().await.map_err(|_| Error::Rejected("malformed multipart body"))?),
                _ => {}
            }
        }
        let meta: CreateRequest = parse_meta(meta.ok_or(Error::Rejected("missing meta part"))?)?;
        (meta, file.ok_or(Error::Rejected("missing file part"))?.to_vec())
    } else {
        let body = to_bytes(request.into_body(), MAX_BYTES).await.map_err(|_| Error::TooLarge)?;
        let meta = parse_meta(body)?;
        let url = Url::parse(meta.url.as_deref().ok_or(Error::Rejected("url is required without a file upload"))?)
            .map_err(|_| Error::Rejected("url is not a valid URL"))?;
        if !service.allowed(&url) {
            return Err(Error::Rejected("url host is not allowed"));
        }
        let bytes = service.fetch(&url).await?;
        (meta, bytes)
    };
    let info = validate(&service, &meta)?;
    let title = match meta.title.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        Some(title) => title.to_owned(),
        None => stem(&info.file_name).chars().take(200).collect(),
    };
    let document =
        blocking(move || wordfabrica_docx::read(&bytes).map(|d| (d, bytes)).map_err(|_| Error::Rejected("not a readable DOCX file"))).await?;
    let (document, bytes) = document;
    let id = uuid::Uuid::new_v4().to_string();
    service.files.put(&storage::original_key(&id), bytes).await?;
    let (store, created_id) = (service.store.clone(), id.clone());
    let metadata = blocking(move || store.create_integration(&created_id, &title, document, &info)).await?;
    Ok(Json(Created { id: metadata.id, revision: metadata.revision }))
}

fn parse_meta(bytes: Bytes) -> Result<CreateRequest, Error> {
    serde_json::from_slice(&bytes).map_err(|_| Error::Rejected("invalid JSON: expected file_name, callback_url and key"))
}

/// Check the third party's fields and return the integration record to store.
fn validate(service: &Service, meta: &CreateRequest) -> Result<Integration, Error> {
    let name = meta.file_name.trim();
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    if name.is_empty() || name.chars().count() > 255 || name.chars().any(|c| c.is_control() || c == '/' || c == '\\') {
        return Err(Error::Rejected("file_name must be 1-255 characters without slashes"));
    }
    if !matches!(ext.as_deref(), Some("docx" | "docm" | "dotx")) {
        return Err(Error::Rejected("only .docx, .docm and .dotx files are supported"));
    }
    let key = meta.key.trim();
    if key.is_empty() || key.chars().count() > 200 || key.chars().any(char::is_control) {
        return Err(Error::Rejected("key must be 1-200 characters"));
    }
    if meta.title.as_deref().is_some_and(|t| t.trim().chars().count() > 200 || t.chars().any(char::is_control)) {
        return Err(Error::Rejected("title must be at most 200 characters"));
    }
    let callback = Url::parse(meta.callback_url.trim()).map_err(|_| Error::Rejected("callback_url is not a valid URL"))?;
    if !service.allowed(&callback) {
        return Err(Error::Rejected("callback_url must be http(s) on an allowed host"));
    }
    // Saved files are DOCX whatever came in (a .docm loses its macros, a .dotx becomes a document).
    let file_name = format!("{}.docx", stem(name));
    Ok(Integration { key: key.into(), file_name, callback_url: callback.into() })
}

fn stem(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Status {
    pub id: String,
    pub key: String,
    pub revision: u64,
    pub saved_revision: Option<u64>,
    pub callback_state: Option<String>,
    pub url: Option<String>,
    pub url_expires_at: Option<u64>,
}

/// `GET /api/integration/documents/{id}`: current revision, last explicit save and a fresh link to it.
async fn status(State(service): State<Arc<Service>>, Path(id): Path<String>) -> Result<Json<Status>, Error> {
    if !valid_id(&id) {
        return Err(Error::NotFound);
    }
    let (store, lookup) = (service.store.clone(), id.clone());
    let (info, (revision, saved)) =
        blocking(move || Ok((store.integration(&lookup)?.ok_or(Error::NotFound)?, store.integration_status(&lookup)?))).await?;
    let (url, url_expires_at) = match &saved {
        Some((saved_revision, _)) => {
            let (url, expires) = service.download_url(&id, *saved_revision).await?;
            (Some(url), Some(expires))
        }
        None => (None, None),
    };
    let (saved_revision, callback_state) = saved.map_or((None, None), |(r, s)| (Some(r), Some(s)));
    Ok(Json(Status { id, key: info.key, revision, saved_revision, callback_state, url, url_expires_at }))
}

/// Who the link is for and what they may do.
#[derive(Deserialize)]
struct LinkRequest {
    user_id: String,
    #[serde(default)]
    user_name: Option<String>,
    mode: Mode,
    /// Seconds; default 12 hours, at most 7 days.
    #[serde(default)]
    ttl: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Link {
    pub url: String,
    pub mode: Mode,
    pub expires_at: u64,
}

/// `POST /api/integration/documents/{id}/links`: an editor link for one user, to edit or only view.
async fn link(State(service): State<Arc<Service>>, Path(id): Path<String>, body: Bytes) -> Result<Json<Link>, Error> {
    if !valid_id(&id) {
        return Err(Error::NotFound);
    }
    let request: LinkRequest = serde_json::from_slice(&body)
        .map_err(|_| Error::Rejected("invalid JSON: expected user_id, mode (edit or view) and optional user_name, ttl"))?;
    let user_id = request.user_id.trim();
    if user_id.is_empty() || user_id.chars().count() > 200 || user_id.chars().any(char::is_control) {
        return Err(Error::Rejected("user_id must be 1-200 characters"));
    }
    let user_name = request.user_name.as_deref().map(str::trim).filter(|n| !n.is_empty()).unwrap_or(user_id);
    if user_name.chars().count() > 200 || user_name.chars().any(char::is_control) {
        return Err(Error::Rejected("user_name must be at most 200 characters"));
    }
    let ttl = request.ttl.unwrap_or(links::DEFAULT_TTL);
    if !(60..=links::MAX_TTL).contains(&ttl) {
        return Err(Error::Rejected("ttl must be 60..=604800 seconds"));
    }
    let (store, lookup) = (service.store.clone(), id.clone());
    blocking(move || store.integration(&lookup)).await?.ok_or(Error::NotFound)?;
    let expires_at = signing::now_secs().saturating_add(ttl);
    let claims = Claims { doc: id.clone(), user_id: user_id.into(), user_name: user_name.into(), mode: request.mode, exp: expires_at };
    let token = links::issue(&service.settings.secret, &claims);
    let url = service.public(&format!("?doc={id}&t={token}"))?;
    Ok(Json(Link { url, mode: request.mode, expires_at }))
}

#[derive(Deserialize)]
struct LinkQuery {
    expires: u64,
    sig: String,
}

/// `GET /api/integration/files/{id}/{revision}?expires&sig`: a saved DOCX behind a server-signed, expiring link.
async fn download(
    State(service): State<Arc<Service>>,
    Path((id, revision)): Path<(String, u64)>,
    query: Result<Query<LinkQuery>, QueryRejection>,
) -> Result<Response, Error> {
    let Ok(Query(link)) = query else { return Err(Error::Forbidden) };
    let (revision_text, expires_text) = (revision.to_string(), link.expires.to_string());
    let parts: [&[u8]; 3] = [id.as_bytes(), revision_text.as_bytes(), expires_text.as_bytes()];
    if !valid_id(&id) || link.expires < signing::now_secs() || !signing::verify(&service.settings.secret, &parts, &link.sig) {
        return Err(Error::Forbidden);
    }
    let (store, lookup) = (service.store.clone(), id.clone());
    let info = blocking(move || store.integration(&lookup)).await?.ok_or(Error::NotFound)?;
    let bytes = service.files.get(&storage::saved_key(&id, revision)).await?;
    let disposition = format!("attachment; filename=\"document.docx\"; filename*=UTF-8''{}", percent_encode(&info.file_name));
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_owned()),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        bytes,
    )
        .into_response())
}

/// RFC 3986 percent-encoding of everything but unreserved characters (for `filename*`).
fn percent_encode(text: &str) -> String {
    text.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}
