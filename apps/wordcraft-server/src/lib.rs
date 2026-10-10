//! Same-origin HTTP delivery and document storage; no conversion or collaboration service.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod callbacks;
pub mod fonts;
pub mod integration;
pub mod plugins;
pub mod signing;
pub mod storage;
pub mod store;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{any, get},
};
use std::{path::PathBuf, sync::Arc};
use store::{Error, Store};
use wordcraft_remote::{MAX_BYTES, Metadata, Snapshot, WriteRequest};

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = match self {
            Self::Invalid => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict => StatusCode::CONFLICT,
            Self::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::Storage => StatusCode::SERVICE_UNAVAILABLE,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::Rejected(_) => StatusCode::BAD_REQUEST,
            Self::Fetch => StatusCode::BAD_GATEWAY,
        };
        (status, Json(serde_json::json!({"error": self.to_string()}))).into_response()
    }
}

/// Serve the built WASM app and an API on one origin. Deploy behind authentication for shared use.
pub fn router(store: Arc<Store>, web_dir: PathBuf) -> Router {
    router_with_fonts(store, web_dir, Arc::new(fonts::Catalog::default()))
}

/// Serve an administrator-provided immutable font catalog alongside the document API.
pub fn router_with_fonts(store: Arc<Store>, web_dir: PathBuf, fonts: Arc<fonts::Catalog>) -> Router {
    router_with_catalogs(store, web_dir, fonts, Arc::new(plugins::Catalog::default()))
}

/// Serve administrator-provided font and plugin catalogs alongside the document API.
pub fn router_with_catalogs(store: Arc<Store>, web_dir: PathBuf, fonts: Arc<fonts::Catalog>, plugins: Arc<plugins::Catalog>) -> Router {
    router_with_integration(store, web_dir, fonts, plugins, None)
}

/// The full server; with an integration service, third parties can register documents and get save callbacks.
pub fn router_with_integration(
    store: Arc<Store>,
    web_dir: PathBuf,
    fonts: Arc<fonts::Catalog>,
    plugins: Arc<plugins::Catalog>,
    integration: Option<Arc<integration::Service>>,
) -> Router {
    let plugin_routes = plugins::routes(plugins, store.clone());
    let integration_routes = integration.clone().map(integration::routes).unwrap_or_default();
    Router::new()
        .route("/api/health", get(|| async { Json(serde_json::json!({"status":"ok"})) }))
        .route("/api/documents", get(list))
        .route("/api/documents/{id}", get(load).put(save))
        .route("/api/{*path}", any(|| async { StatusCode::NOT_FOUND }))
        .fallback_service(tower_http::services::ServeDir::new(web_dir))
        .with_state(AppState { store, integration })
        .merge(fonts::routes(fonts))
        .merge(plugin_routes)
        .merge(integration_routes)
        .layer(DefaultBodyLimit::max(MAX_BYTES))
        .layer(tower_http::limit::RequestBodyLimitLayer::new(MAX_BYTES))
        .layer(middleware::from_fn(same_origin))
}

// Browsers may not read private documents or submit mutations from another origin. No CORS.
// Opening the editor from a link on another site is a top-level GET navigation of the app's
// pages: the other site can't read the response, so it's allowed; the API stays same-origin.
async fn same_origin(request: Request, next: Next) -> Response {
    let headers = request.headers();
    // Signed, expiring download links of third-party documents may also be opened from their site.
    let path = request.uri().path();
    let page_link = request.method() == axum::http::Method::GET
        && headers.get("sec-fetch-mode").is_some_and(|v| v == "navigate")
        && (!path.starts_with("/api/") || path.starts_with("/api/integration/files/"));
    if headers.get("sec-fetch-site").is_some_and(|v| v == "cross-site") && !page_link {
        return StatusCode::FORBIDDEN.into_response();
    }
    if let Some(origin) = headers.get(header::ORIGIN) {
        let host = headers.get(header::HOST).and_then(|h| h.to_str().ok()).unwrap_or("");
        let origin = origin.to_str().unwrap_or("");
        if origin != format!("http://{host}") && origin != format!("https://{host}") {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    let mut response = next.run(request).await;
    response.headers_mut().insert(header::CACHE_CONTROL, axum::http::HeaderValue::from_static("no-store"));
    response.headers_mut().insert(header::X_CONTENT_TYPE_OPTIONS, axum::http::HeaderValue::from_static("nosniff"));
    response
}

pub(crate) async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T, Error> + Send + 'static) -> Result<T, Error> {
    tokio::task::spawn_blocking(work).await.map_err(|_| Error::Storage)?
}
/// State of the document API: the store, plus the integration service when enabled.
#[derive(Clone)]
struct AppState {
    store: Arc<Store>,
    integration: Option<Arc<integration::Service>>,
}

async fn list(State(app): State<AppState>) -> Result<Json<Vec<Metadata>>, Error> {
    blocking(move || app.store.list()).await.map(Json)
}
async fn load(State(app): State<AppState>, Path(id): Path<String>) -> Result<Json<Snapshot>, Error> {
    blocking(move || app.store.get(&id)).await.map(Json)
}
/// Save a snapshot. An explicit save (`commit`) of a third-party document also exports and
/// stores DOCX and queues the callback; if that fails the client sees 503 and its retry of the
/// same snapshot is acknowledged idempotently and committed again.
async fn save(State(app): State<AppState>, Path(id): Path<String>, Json(request): Json<WriteRequest>) -> Result<Json<Metadata>, Error> {
    let document = (request.commit && app.integration.is_some()).then(|| request.document.clone());
    let store = app.store.clone();
    let metadata = blocking(move || store.put(&id, request)).await?;
    if let (Some(service), Some(document)) = (&app.integration, document) {
        service.commit(&metadata, document).await?;
    }
    Ok(Json(metadata))
}

#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod tests;
