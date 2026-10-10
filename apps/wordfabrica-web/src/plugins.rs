//! Plugins over HTTP: fetch the server's plugins after the editor starts, and save tag edits.
//! Plugins are optional chrome: a failed fetch leaves the editor without them and is only logged;
//! a failed save is shown in the pane.
use std::future::Future;

use wordfabrica_remote::{MAX_PLUGIN_BYTES, MAX_PLUGINS, Plugin, TagCategory, TagsWrite};
use wordfabrica_ui_egui::{i18n, tl};
use wordfabrica_ui_web::Bridge;

/// Load the plugins and let the chrome save and reload them through this module.
pub fn install(bridge: Bridge) {
    let (saver, loader) = (bridge.clone(), bridge.clone());
    bridge.set_plugin_host(move |id, revision, categories| save(saver.clone(), id, revision, categories), move || load(loader.clone()));
    load(bridge);
}

/// Fetch `/api/plugins` and hand the result to the chrome.
fn load(bridge: Bridge) {
    wasm_bindgen_futures::spawn_local(async move {
        match fetch().await {
            Ok(plugins) => bridge.set_plugins(plugins),
            Err(e) => log::warn!("plugins not loaded: {e}"),
        }
    });
}

/// Why a request failed.
enum Failure {
    /// Someone saved newer tags (HTTP 409).
    Conflict,
    Other(String),
}

/// Save tags; on a conflict reload the latest tags so the user can redo the change on top.
fn save(bridge: Bridge, id: String, revision: u64, categories: Vec<TagCategory>) {
    wasm_bindgen_futures::spawn_local(async move {
        match put(&id, &TagsWrite { revision, categories }).await {
            Ok(plugin) => bridge.plugin_saved(plugin),
            Err(Failure::Conflict) => {
                bridge
                    .plugin_save_failed(tl!("Someone else changed these tags. They have been reloaded; please make your change again.").to_string());
                load(bridge);
            }
            Err(Failure::Other(e)) => bridge.plugin_save_failed(i18n::fmt(tl!("Tags not saved: {error}"), &[("error", &e)])),
        }
    });
}

/// The plugin list, bounded before parsing; the bridge validates each plugin.
async fn fetch() -> Result<Vec<Plugin>, String> {
    with_timeout(|signal| async move {
        let response = gloo_net::http::Request::get("/api/plugins").abort_signal(Some(&signal)).send().await.map_err(|e| e.to_string())?;
        if !response.ok() {
            return Err(format!("HTTP {}", response.status()));
        }
        let bytes = response.binary().await.map_err(|e| e.to_string())?;
        if bytes.len() > MAX_PLUGIN_BYTES.saturating_mul(MAX_PLUGINS) {
            return Err("plugin list too large".into());
        }
        serde_json::from_slice::<Vec<Plugin>>(&bytes).map_err(|e| e.to_string())
    })
    .await
}

/// A failed request: HTTP status (0 = no reply) and what went wrong.
struct HttpError(u16, String);

impl From<String> for HttpError {
    fn from(message: String) -> Self {
        HttpError(0, message)
    }
}

/// `PUT /api/plugins/{id}/tags`; the reply is the plugin as stored.
async fn put(id: &str, write: &TagsWrite) -> Result<Plugin, Failure> {
    let url = format!("/api/plugins/{id}/tags");
    let result = with_timeout(|signal| async move {
        let request = gloo_net::http::Request::put(&url).abort_signal(Some(&signal)).json(write).map_err(|e| HttpError(0, e.to_string()))?;
        let response = request.send().await.map_err(|e| HttpError(0, e.to_string()))?;
        if !response.ok() {
            return Err(HttpError(response.status(), format!("HTTP {}", response.status())));
        }
        let bytes = response.binary().await.map_err(|e| HttpError(0, e.to_string()))?;
        if bytes.len() > MAX_PLUGIN_BYTES.saturating_mul(2) {
            return Err(HttpError(0, "reply too large".into()));
        }
        serde_json::from_slice::<Plugin>(&bytes).map_err(|e| HttpError(0, e.to_string()))
    })
    .await;
    result.map_err(|HttpError(status, e)| if status == 409 { Failure::Conflict } else { Failure::Other(e) })
}

/// Run a request with a 10 s limit; on timeout the request is aborted.
async fn with_timeout<T, E: From<String>, F: Future<Output = Result<T, E>>>(request: impl FnOnce(web_sys::AbortSignal) -> F) -> Result<T, E> {
    let controller = web_sys::AbortController::new().map_err(|_| E::from("no AbortController".to_string()))?;
    let operation = request(controller.signal());
    futures_util::pin_mut!(operation);
    let timeout = gloo_timers::future::TimeoutFuture::new(10_000);
    futures_util::pin_mut!(timeout);
    match futures_util::future::select(operation, timeout).await {
        futures_util::future::Either::Left((result, _)) => result,
        futures_util::future::Either::Right(_) => {
            controller.abort();
            Err(E::from("timed out".to_string()))
        }
    }
}
