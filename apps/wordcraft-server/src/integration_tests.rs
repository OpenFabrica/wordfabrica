//! Third-party integration: signed requests, upload/URL loading, commit on explicit save,
//! callback delivery with retries, download links and MinIO presigning.
use crate::{
    callbacks::{self, Payload},
    integration::{Created, Link, Service, Settings, Status, StorageSettings},
    links::TOKEN_HEADER,
    signing,
    storage::{Files, MinioSettings},
    store::Store,
};
use axum::{
    Router,
    body::{Body, Bytes, to_bytes},
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    routing::{get, post},
};
use std::{
    collections::VecDeque,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use tower::ServiceExt;
use wordcraft_doc::{Document, StoryRef};
use wordcraft_remote::{Metadata, Snapshot, WriteRequest};

const SECRET: &[u8] = b"0123456789abcdef0123456789abcdef";

fn settings(dir: &Path, allow_hosts: &[&str]) -> Settings {
    Settings {
        secret: SECRET.to_vec(),
        public_url: url::Url::parse("http://editor.test/office").unwrap(),
        storage: StorageSettings::Local(dir.join("files")),
        download_ttl: Duration::from_secs(3600),
        allow_hosts: allow_hosts.iter().map(|h| h.to_string()).collect(),
    }
}

struct Harness {
    app: Router,
    service: Arc<Service>,
    dir: tempfile::TempDir,
}

fn harness(allow_hosts: &[&str]) -> Harness {
    let dir = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::open(&dir.path().join("db")).unwrap());
    let service = Service::with_retry_base(settings(dir.path(), allow_hosts), store.clone(), Duration::ZERO).unwrap();
    let app = crate::router_with_integration(
        store,
        dir.path().into(),
        Arc::new(crate::fonts::Catalog::default()),
        Arc::new(crate::plugins::Catalog::default()),
        Some(service.clone()),
    );
    Harness { app, service, dir }
}

fn docx(text: &str) -> Vec<u8> {
    wordcraft_docx::write(&Document::from_text(text)).unwrap()
}

/// A request signed the way third parties must sign it.
fn signed(method: &str, path: &str, content_type: &str, body: Vec<u8>, timestamp: u64) -> Request<Body> {
    let ts = timestamp.to_string();
    let signature = signing::sign(SECRET, &[ts.as_bytes(), method.as_bytes(), path.as_bytes(), &body]);
    Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", content_type)
        .header(signing::TIMESTAMP_HEADER, ts)
        .header(signing::SIGNATURE_HEADER, signature)
        .body(Body::from(body))
        .unwrap()
}

const BOUNDARY: &str = "wordcraft-test-boundary";
fn multipart(meta: &str, file: Option<&[u8]>) -> Vec<u8> {
    let mut body = format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"meta\"\r\n\r\n{meta}\r\n").into_bytes();
    if let Some(file) = file {
        body.extend(format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"x.docx\"\r\n\r\n").as_bytes());
        body.extend(file);
        body.extend(b"\r\n");
    }
    body.extend(format!("--{BOUNDARY}--\r\n").as_bytes());
    body
}

fn upload(meta: &str, file: Option<&[u8]>) -> Request<Body> {
    signed("POST", "/api/integration/documents", &format!("multipart/form-data; boundary={BOUNDARY}"), multipart(meta, file), signing::now_secs())
}

const META: &str = r#"{"file_name":"合同 v1.docx","callback_url":"http://127.0.0.1:9/cb","key":"ext-42"}"#;

async fn json<T: serde::de::DeserializeOwned>(response: axum::response::Response) -> T {
    let bytes = to_bytes(response.into_body(), 64 * 1024 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&bytes)))
}

async fn create(h: &Harness, meta: &str, text: &str) -> Created {
    let response = h.app.clone().oneshot(upload(meta, Some(&docx(text)))).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}

/// Ask for an access link the way the third party does; `None` if the document has no integration.
async fn link(h: &Harness, id: &str, body: &str) -> (StatusCode, Option<Link>) {
    let path = format!("/api/integration/documents/{id}/links");
    let response = h.app.clone().oneshot(signed("POST", &path, "application/json", body.as_bytes().to_vec(), signing::now_secs())).await.unwrap();
    let status = response.status();
    let link = if status == StatusCode::OK { Some(json(response).await) } else { None };
    (status, link)
}

/// The token in an access link URL (`…&t=<token>`).
async fn token(h: &Harness, id: &str, mode: &str) -> Option<String> {
    let body = format!(r#"{{"user_id":"u-1","user_name":"张三","mode":"{mode}"}}"#);
    let (_, link) = link(h, id, &body).await;
    link.map(|l| l.url.split("&t=").nth(1).unwrap().to_owned())
}

/// Save as the editor does, with an edit link's token when the document has an integration.
async fn save(h: &Harness, id: &str, revision: u64, text: &str, commit: bool) -> axum::response::Response {
    let token = token(h, id, "edit").await;
    save_with(h, id, revision, text, commit, token.as_deref()).await
}

async fn save_with(h: &Harness, id: &str, revision: u64, text: &str, commit: bool, token: Option<&str>) -> axum::response::Response {
    let request = WriteRequest { revision, title: "合同 v1".into(), document: Document::from_text(text), commit };
    let mut request = Request::put(format!("/api/documents/{id}"))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&request).unwrap()))
        .unwrap();
    if let Some(token) = token {
        request.headers_mut().insert(TOKEN_HEADER, token.parse().unwrap());
    }
    h.app.clone().oneshot(request).await.unwrap()
}

/// Load as the editor does.
async fn load(h: &Harness, id: &str, token: Option<&str>) -> axum::response::Response {
    let mut request = Request::get(format!("/api/documents/{id}")).body(Body::empty()).unwrap();
    if let Some(token) = token {
        request.headers_mut().insert(TOKEN_HEADER, token.parse().unwrap());
    }
    h.app.clone().oneshot(request).await.unwrap()
}

/// Fetch a download link through the router (links point at the configured public URL).
async fn follow(h: &Harness, url: &str) -> axum::response::Response {
    let path = url.strip_prefix("http://editor.test/office").unwrap();
    h.app.clone().oneshot(Request::get(path).body(Body::empty()).unwrap()).await.unwrap()
}

#[tokio::test]
async fn requests_need_a_fresh_valid_signature() {
    let h = harness(&[]);
    let now = signing::now_secs();
    let body = multipart(META, Some(&docx("x")));
    let ct = format!("multipart/form-data; boundary={BOUNDARY}");
    let status = |request: Request<Body>| async { h.app.clone().oneshot(request).await.unwrap().status() };

    let mut unsigned = signed("POST", "/api/integration/documents", &ct, body.clone(), now);
    unsigned.headers_mut().remove(signing::SIGNATURE_HEADER);
    assert_eq!(status(unsigned).await, StatusCode::UNAUTHORIZED);
    let mut forged = signed("POST", "/api/integration/documents", &ct, body.clone(), now);
    forged.headers_mut().insert(signing::SIGNATURE_HEADER, "00".repeat(32).parse().unwrap());
    assert_eq!(status(forged).await, StatusCode::UNAUTHORIZED);
    let stale = signed("POST", "/api/integration/documents", &ct, body.clone(), now - signing::MAX_SKEW_SECS - 10);
    assert_eq!(status(stale).await, StatusCode::UNAUTHORIZED, "replayed long after");
    // Signed for one body or path, sent with another.
    let ts = now.to_string();
    let sig = signing::sign(SECRET, &[ts.as_bytes(), b"POST", b"/api/integration/documents", b"other body"]);
    let tampered = Request::post("/api/integration/documents")
        .header("content-type", &ct)
        .header(signing::TIMESTAMP_HEADER, &ts)
        .header(signing::SIGNATURE_HEADER, sig)
        .body(Body::from(body.clone()))
        .unwrap();
    assert_eq!(status(tampered).await, StatusCode::UNAUTHORIZED);
    let sig = signing::sign(SECRET, &[ts.as_bytes(), b"GET", b"/api/integration/documents/x", b""]);
    let moved = Request::get(format!("/api/integration/documents/{}", uuid::Uuid::new_v4()))
        .header(signing::TIMESTAMP_HEADER, &ts)
        .header(signing::SIGNATURE_HEADER, sig)
        .body(Body::empty())
        .unwrap();
    assert_eq!(status(moved).await, StatusCode::UNAUTHORIZED);
    assert_eq!(status(signed("POST", "/api/integration/documents", &ct, body.clone(), now)).await, StatusCode::OK);

    // Without a configured secret the integration does not exist.
    let dir = tempfile::tempdir().unwrap();
    let plain = crate::router(Arc::new(Store::open(&dir.path().join("db")).unwrap()), dir.path().into());
    let response = plain.oneshot(signed("POST", "/api/integration/documents", &ct, body, now)).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn uploads_open_in_the_editor_and_bad_input_is_rejected() {
    let h = harness(&[]);
    let created = create(&h, META, "第一稿 first draft").await;
    assert_eq!(created.revision, 1);
    assert!(h.dir.path().join(format!("files/originals/{}.docx", created.id)).is_file());
    let response = load(&h, &created.id, token(&h, &created.id, "edit").await.as_deref()).await;
    let snapshot: Snapshot = json(response).await;
    assert_eq!(snapshot.document.plain_text(StoryRef::Body), "第一稿 first draft");
    assert_eq!(snapshot.metadata.title, "合同 v1", "title defaults to the file stem");

    let rejected = |meta: &str, file: Option<Vec<u8>>| {
        let request = upload(meta, file.as_deref());
        let app = h.app.clone();
        async move { app.oneshot(request).await.unwrap().status() }
    };
    assert_eq!(rejected(META, Some(b"not a zip".to_vec())).await, StatusCode::BAD_REQUEST);
    assert_eq!(rejected(META, None).await, StatusCode::BAD_REQUEST, "missing file part");
    assert_eq!(rejected("{bad", Some(docx("x"))).await, StatusCode::BAD_REQUEST);
    assert_eq!(rejected(&META.replace(".docx", ".pdf"), Some(docx("x"))).await, StatusCode::BAD_REQUEST);
    assert_eq!(rejected(&META.replace("合同 v1", "../evil"), Some(docx("x"))).await, StatusCode::BAD_REQUEST);
    assert_eq!(rejected(&META.replace("http://127.0.0.1:9/cb", "ftp://x/cb"), Some(docx("x"))).await, StatusCode::BAD_REQUEST);
    assert_eq!(rejected(&META.replace("ext-42", ""), Some(docx("x"))).await, StatusCode::BAD_REQUEST);
    let mut zip_bomb_like = docx("x");
    zip_bomb_like.truncate(zip_bomb_like.len() / 2);
    assert_eq!(rejected(META, Some(zip_bomb_like)).await, StatusCode::BAD_REQUEST, "truncated DOCX never panics");
}

/// A stand-in third party: serves a DOCX, redirects, and records callbacks (answering with queued statuses).
#[derive(Default)]
struct ThirdParty {
    calls: Mutex<Vec<(HeaderMap, Bytes)>>,
    statuses: Mutex<VecDeque<u16>>,
}

async fn third_party() -> (Arc<ThirdParty>, String) {
    let state = Arc::new(ThirdParty::default());
    async fn callback(State(s): State<Arc<ThirdParty>>, headers: HeaderMap, body: Bytes) -> StatusCode {
        s.calls.lock().unwrap().push((headers, body));
        StatusCode::from_u16(s.statuses.lock().unwrap().pop_front().unwrap_or(200)).unwrap()
    }
    let app = Router::new()
        .route("/doc.docx", get(|| async { docx("来自三方 from a URL") }))
        .route("/missing.docx", get(|| async { StatusCode::NOT_FOUND }))
        .route("/cb", post(callback))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (state, base)
}

fn url_request(base: &str, path: &str) -> Request<Body> {
    let meta = format!(r#"{{"url":"{base}{path}","file_name":"a.docx","callback_url":"{base}/cb","key":"k1","title":"标题"}}"#);
    signed("POST", "/api/integration/documents", "application/json", meta.into_bytes(), signing::now_secs())
}

#[tokio::test]
async fn documents_load_from_a_url_on_allowed_hosts_only() {
    let (_, base) = third_party().await;
    let h = harness(&[]);
    let response = h.app.clone().oneshot(url_request(&base, "/doc.docx")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let created: Created = json(response).await;
    let response = load(&h, &created.id, token(&h, &created.id, "edit").await.as_deref()).await;
    let snapshot: Snapshot = json(response).await;
    assert_eq!(snapshot.document.plain_text(StoryRef::Body), "来自三方 from a URL");
    assert_eq!(snapshot.metadata.title, "标题");
    let response = h.app.clone().oneshot(url_request(&base, "/missing.docx")).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

    let fenced = harness(&["docs.example.com"]);
    let response = fenced.app.clone().oneshot(url_request(&base, "/doc.docx")).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST, "127.0.0.1 is not on the allow list");
}

#[tokio::test]
async fn explicit_saves_store_docx_and_call_back_once_per_revision() {
    let (party, base) = third_party().await;
    let h = harness(&[]);
    let meta = format!(r#"{{"file_name":"report.docm","callback_url":"{base}/cb","key":"ext-7"}}"#);
    let created = create(&h, &meta, "draft").await;
    let id = created.id.as_str();

    // Autosave: draft only.
    assert_eq!(save(&h, id, 1, "autosaved", false).await.status(), StatusCode::OK);
    assert_eq!(callbacks::deliver_due(&h.service).await, 0);
    assert!(!h.dir.path().join(format!("files/saved/{id}/2.docx")).exists());

    // Explicit save; the third party fails once, then accepts the retry.
    party.statuses.lock().unwrap().push_back(500);
    let response = save(&h, id, 2, "final 最终版", true).await;
    assert_eq!(response.status(), StatusCode::OK);
    let metadata: Metadata = json(response).await;
    assert_eq!(metadata.revision, 3);
    assert!(h.dir.path().join(format!("files/saved/{id}/3.docx")).is_file());
    // A retry of the same request (lost reply) commits again but never queues a second callback.
    assert_eq!(save(&h, id, 2, "final 最终版", true).await.status(), StatusCode::OK);
    assert_eq!(callbacks::deliver_due(&h.service).await, 0, "HTTP 500");
    assert_eq!(callbacks::deliver_due(&h.service).await, 1, "retried");
    assert_eq!(callbacks::deliver_due(&h.service).await, 0, "nothing left");

    let calls = party.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 2);
    let (headers, body) = &calls[1];
    let ts = headers[signing::TIMESTAMP_HEADER].to_str().unwrap();
    assert!(signing::verify(SECRET, &[ts.as_bytes(), body], headers[signing::SIGNATURE_HEADER].to_str().unwrap()));
    let payload: Payload = serde_json::from_slice(body).unwrap();
    assert_eq!((payload.event.as_str(), payload.id.as_str(), payload.key.as_str()), ("document.saved", id, "ext-7"));
    assert_eq!((payload.revision, payload.file_name.as_str()), (3, "report.docx"));
    assert_eq!((payload.user_id.as_str(), payload.user_name.as_str()), ("u-1", "张三"));
    let response = follow(&h, &payload.url).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers()["content-disposition"].to_str().unwrap().contains("report.docx"));
    let bytes = to_bytes(response.into_body(), 64 * 1024 * 1024).await.unwrap();
    assert_eq!(wordcraft_docx::read(&bytes).unwrap().plain_text(StoryRef::Body), "final 最终版");

    // Two saves before delivery: only the newest is announced.
    assert_eq!(save(&h, id, 3, "v4", true).await.status(), StatusCode::OK);
    assert_eq!(save(&h, id, 4, "v5", true).await.status(), StatusCode::OK);
    assert_eq!(callbacks::deliver_due(&h.service).await, 1);
    let calls = party.calls.lock().unwrap().clone();
    let last: Payload = serde_json::from_slice(&calls.last().unwrap().1).unwrap();
    assert_eq!((calls.len(), last.revision), (3, 5));

    let path = format!("/api/integration/documents/{id}");
    let response = h.app.clone().oneshot(signed("GET", &path, "application/json", vec![], signing::now_secs())).await.unwrap();
    let status: Status = json(response).await;
    assert_eq!((status.revision, status.saved_revision, status.callback_state.as_deref()), (5, Some(5), Some("done")));
    assert_eq!(follow(&h, status.url.as_deref().unwrap()).await.status(), StatusCode::OK);

    // The editor's own documents ignore `commit`.
    let own = "12345678-1234-1234-1234-123456789abc";
    assert_eq!(save(&h, own, 0, "mine", true).await.status(), StatusCode::OK);
    assert!(!h.dir.path().join(format!("files/saved/{own}")).exists());
}

#[tokio::test]
async fn callbacks_give_up_after_the_last_attempt() {
    let (party, base) = third_party().await;
    let h = harness(&[]);
    let created = create(&h, &format!(r#"{{"file_name":"a.docx","callback_url":"{base}/cb","key":"k"}}"#), "x").await;
    party.statuses.lock().unwrap().extend(std::iter::repeat_n(503, callbacks::MAX_ATTEMPTS as usize));
    assert_eq!(save(&h, &created.id, 1, "y", true).await.status(), StatusCode::OK);
    for _ in 0..callbacks::MAX_ATTEMPTS + 2 {
        assert_eq!(callbacks::deliver_due(&h.service).await, 0);
    }
    assert_eq!(party.calls.lock().unwrap().len(), callbacks::MAX_ATTEMPTS as usize);
    let path = format!("/api/integration/documents/{}", created.id);
    let status: Status = json(h.app.clone().oneshot(signed("GET", &path, "", vec![], signing::now_secs())).await.unwrap()).await;
    assert_eq!(status.callback_state.as_deref(), Some("failed"));
}

#[tokio::test]
async fn download_links_are_signed_and_expire() {
    let h = harness(&[]);
    let created = create(&h, META, "x").await;
    assert_eq!(save(&h, &created.id, 1, "y", true).await.status(), StatusCode::OK);
    let (url, _) = h.service.download_url(&created.id, 2).await.unwrap();
    assert_eq!(follow(&h, &url).await.status(), StatusCode::OK);
    assert_eq!(follow(&h, &url.replace("sig=", "sig=00")).await.status(), StatusCode::FORBIDDEN);
    let base = url.split('?').next().unwrap();
    assert_eq!(follow(&h, base).await.status(), StatusCode::FORBIDDEN, "no signature");
    let past = signing::now_secs() - 1;
    let sig = signing::sign(SECRET, &[created.id.as_bytes(), b"2", past.to_string().as_bytes()]);
    assert_eq!(follow(&h, &format!("{base}?expires={past}&sig={sig}")).await.status(), StatusCode::FORBIDDEN, "expired");
    // A third party may hand the link to a browser on its own site.
    let path = url.strip_prefix("http://editor.test/office").unwrap();
    let navigation = Request::get(path).header("sec-fetch-site", "cross-site").header("sec-fetch-mode", "navigate").body(Body::empty()).unwrap();
    assert_eq!(h.app.clone().oneshot(navigation).await.unwrap().status(), StatusCode::OK);
}

fn minio_settings(endpoint: &str, bucket: &str, access: &str, secret: &str) -> MinioSettings {
    MinioSettings {
        endpoint: endpoint.into(),
        bucket: bucket.into(),
        access_key: access.into(),
        secret_key: secret.into(),
        region: "us-east-1".into(),
    }
}

#[tokio::test]
async fn minio_links_are_presigned_path_style_urls() {
    let files = Files::minio(&minio_settings("http://127.0.0.1:9000", "docs", "minio", "minio12345")).unwrap();
    let url = files.presign("saved/abc/2.docx", Duration::from_secs(600)).await.unwrap().unwrap();
    assert_eq!(url.path(), "/docs/saved/abc/2.docx");
    assert!(url.query().unwrap().contains("X-Amz-Signature="));
    assert!(url.query().unwrap().contains("X-Amz-Expires=600"));
    assert!(!format!("{:?}", minio_settings("e", "b", "a", "top-secret")).contains("top-secret"));
}

/// Live check against a real MinIO; run with `cargo test -p wordcraft-server -- --ignored` after setting
/// `WORDCRAFT_TEST_MINIO_ENDPOINT/_BUCKET/_ACCESS_KEY/_SECRET_KEY` (the bucket must exist).
#[tokio::test]
#[ignore]
async fn minio_round_trip() {
    let var = |n: &str| std::env::var(format!("WORDCRAFT_TEST_MINIO_{n}")).unwrap();
    let files = Files::minio(&minio_settings(&var("ENDPOINT"), &var("BUCKET"), &var("ACCESS_KEY"), &var("SECRET_KEY"))).unwrap();
    let key = format!("test/{}.docx", uuid::Uuid::new_v4());
    files.put(&key, docx("minio")).await.unwrap();
    assert_eq!(files.get(&key).await.unwrap(), docx("minio"));
    let url = files.presign(&key, Duration::from_secs(60)).await.unwrap().unwrap();
    let bytes = reqwest::get(url).await.unwrap().error_for_status().unwrap().bytes().await.unwrap();
    assert_eq!(wordcraft_docx::read(&bytes).unwrap().plain_text(StoryRef::Body), "minio");
}

#[tokio::test]
async fn third_party_documents_open_only_through_access_links() {
    let h = harness(&[]);
    let created = create(&h, META, "secret text").await;
    let id = created.id.as_str();

    // The link: the editor page with the document and a token, valid for the requested time.
    let (status, issued) = link(&h, id, r#"{"user_id":"u-9","mode":"view","ttl":600}"#).await;
    assert_eq!(status, StatusCode::OK);
    let issued = issued.unwrap();
    assert!(issued.url.starts_with(&format!("http://editor.test/office/?doc={id}&t=")), "{}", issued.url);
    assert!(issued.expires_at.abs_diff(signing::now_secs() + 600) <= 5);
    for bad in [r#"{"user_id":"","mode":"edit"}"#, r#"{"user_id":"u","mode":"admin"}"#, r#"{"user_id":"u","mode":"edit","ttl":1}"#] {
        assert_eq!(link(&h, id, bad).await.0, StatusCode::BAD_REQUEST, "{bad}");
    }
    let unknown = uuid::Uuid::new_v4().to_string();
    assert_eq!(link(&h, &unknown, r#"{"user_id":"u","mode":"edit"}"#).await.0, StatusCode::NOT_FOUND);

    // No token, or a token for another document, a forged or an expired one: nothing.
    assert_eq!(load(&h, id, None).await.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(save_with(&h, id, 1, "x", false, None).await.status(), StatusCode::UNAUTHORIZED);
    let other = create(&h, META, "other").await;
    let other_token = token(&h, &other.id, "edit").await.unwrap();
    assert_eq!(load(&h, id, Some(&other_token)).await.status(), StatusCode::UNAUTHORIZED);
    let edit = token(&h, id, "edit").await.unwrap();
    let forged = format!("{}0", &edit[..edit.len() - 1]);
    assert_eq!(load(&h, id, Some(&forged)).await.status(), StatusCode::UNAUTHORIZED);
    let expired = crate::links::issue(
        SECRET,
        &crate::links::Claims {
            doc: id.into(),
            user_id: "u".into(),
            user_name: "u".into(),
            mode: crate::links::Mode::Edit,
            exp: signing::now_secs() - 1,
        },
    );
    assert_eq!(load(&h, id, Some(&expired)).await.status(), StatusCode::UNAUTHORIZED);

    // View links read, read-only; they can't save.
    let view = token(&h, id, "view").await.unwrap();
    let snapshot: Snapshot = json(load(&h, id, Some(&view)).await).await;
    assert!(snapshot.read_only);
    assert_eq!(save_with(&h, id, 1, "x", true, Some(&view)).await.status(), StatusCode::FORBIDDEN);
    // Edit links read and write.
    let snapshot: Snapshot = json(load(&h, id, Some(&edit)).await).await;
    assert!(!snapshot.read_only);
    assert_eq!(save_with(&h, id, 1, "edited", false, Some(&edit)).await.status(), StatusCode::OK);

    // Third-party documents are never listed; the editor's own documents still are, without tokens.
    let own = "12345678-1234-1234-1234-123456789abc";
    assert_eq!(save_with(&h, own, 0, "mine", false, None).await.status(), StatusCode::OK);
    assert_eq!(load(&h, own, None).await.status(), StatusCode::OK);
    let list: Vec<Metadata> = json(h.app.clone().oneshot(Request::get("/api/documents").body(Body::empty()).unwrap()).await.unwrap()).await;
    assert_eq!(list.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), [own]);
}
