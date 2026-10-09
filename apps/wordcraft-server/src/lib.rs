//! Same-origin HTTP delivery and document storage; no conversion or collaboration service.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod fonts;
pub mod store;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
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
    Router::new()
        .route("/api/health", get(|| async { Json(serde_json::json!({"status":"ok"})) }))
        .route("/api/documents", get(list))
        .route("/api/documents/{id}", get(load).put(save))
        .route("/api/{*path}", get(|| async { StatusCode::NOT_FOUND }))
        .fallback_service(tower_http::services::ServeDir::new(web_dir))
        .with_state(store)
        .merge(fonts::routes(fonts))
        .layer(DefaultBodyLimit::max(MAX_BYTES))
        .layer(tower_http::limit::RequestBodyLimitLayer::new(MAX_BYTES))
        .layer(middleware::from_fn(same_origin))
}

// Browsers may not read private documents or submit mutations from another origin. No CORS.
async fn same_origin(request: Request, next: Next) -> Response {
    let headers = request.headers();
    if headers.get("sec-fetch-site").is_some_and(|v| v == "cross-site") {
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

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T, Error> + Send + 'static) -> Result<T, Error> {
    tokio::task::spawn_blocking(work).await.map_err(|_| Error::Storage)?
}
async fn list(State(store): State<Arc<Store>>) -> Result<Json<Vec<Metadata>>, Error> {
    blocking(move || store.list()).await.map(Json)
}
async fn load(State(store): State<Arc<Store>>, Path(id): Path<String>) -> Result<Json<Snapshot>, Error> {
    blocking(move || store.get(&id)).await.map(Json)
}
async fn save(State(store): State<Arc<Store>>, Path(id): Path<String>, Json(request): Json<WriteRequest>) -> Result<Json<Metadata>, Error> {
    blocking(move || store.put(&id, request)).await.map(Json)
}

#[cfg(test)]
mod tests;
