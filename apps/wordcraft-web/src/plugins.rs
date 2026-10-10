//! Fetch the server's plugins after the editor starts. Plugins are optional chrome: any failure
//! leaves the editor without them and is only logged.
use wordcraft_remote::{MAX_PLUGIN_BYTES, MAX_PLUGINS, Plugin};

/// Load `/api/plugins` (10 s limit) and hand the result to the chrome.
pub fn load(bridge: wordcraft_ui_web::Bridge) {
    wasm_bindgen_futures::spawn_local(async move {
        match fetch().await {
            Ok(plugins) => bridge.set_plugins(plugins),
            Err(e) => log::warn!("plugins not loaded: {e}"),
        }
    });
}

/// The plugin list, bounded before parsing; the bridge validates each plugin.
async fn fetch() -> Result<Vec<Plugin>, String> {
    let controller = web_sys::AbortController::new().map_err(|_| "no AbortController".to_string())?;
    let signal = controller.signal();
    let operation = async {
        let response = gloo_net::http::Request::get("/api/plugins").abort_signal(Some(&signal)).send().await.map_err(|e| e.to_string())?;
        if !response.ok() {
            return Err(format!("HTTP {}", response.status()));
        }
        let bytes = response.binary().await.map_err(|e| e.to_string())?;
        if bytes.len() > MAX_PLUGIN_BYTES.saturating_mul(MAX_PLUGINS) {
            return Err("plugin list too large".into());
        }
        serde_json::from_slice::<Vec<Plugin>>(&bytes).map_err(|e| e.to_string())
    };
    futures_util::pin_mut!(operation);
    let timeout = gloo_timers::future::TimeoutFuture::new(10_000);
    futures_util::pin_mut!(timeout);
    match futures_util::future::select(operation, timeout).await {
        futures_util::future::Either::Left((result, _)) => result,
        futures_util::future::Either::Right(_) => {
            controller.abort();
            Err("timed out".into())
        }
    }
}
