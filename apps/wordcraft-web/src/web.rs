//! The browser shell: web `Services`, drag-and-drop, and the eframe web runner.

use wasm_bindgen::JsCast as _;
use wordcraft_engine::Session;
use wordcraft_ui_egui::{Inbox, Services, WordApp};

const DOC_EXTS: &[&str] = &["docx", "docm", "dotx", "odt", "rtf", "txt", "md", "html", "htm", "json"];
const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp"];
const CANVAS_ID: &str = "wordcraft_canvas";
const LOADING_ID: &str = "wordcraft_loading";

pub fn start() {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            log::error!("no document");
            return;
        };
        let Some(canvas) = document.get_element_by_id(CANVAS_ID).and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok()) else {
            log::error!("missing <canvas id=\"{CANVAS_ID}\">");
            return;
        };
        let mut options = eframe::WebOptions::default();
        if query().contains("webgl")
            && let eframe::egui_wgpu::WgpuSetup::CreateNew(create) = &mut options.wgpu_options.wgpu_setup
        {
            create.instance_descriptor.backends = eframe::wgpu::Backends::GL;
        }
        let local = has_flag("local");
        // `?egui-chrome` keeps the old all-egui interface (the HTML slots are hidden by CSS).
        let egui_chrome = has_flag("egui-chrome");
        if egui_chrome && let Some(body) = document.body() {
            body.class_list().add_1("egui-chrome").ok();
        }
        if let Some(el) = document.get_element_by_id(LOADING_ID) {
            el.set_text_content(Some("正在加载文档字体… / Loading document fonts…"));
        }
        let font_error = !local && crate::fonts::load().await.is_err();
        let result = eframe::WebRunner::new()
            .start(
                canvas,
                options,
                Box::new(move |cc| {
                    if let Some(rs) = &cc.wgpu_render_state {
                        log::info!("wordcraft-web: wgpu backend {:?}", rs.adapter.get_info().backend);
                    }
                    // A browser tab has no window decorations to theme; eframe's web backend would
                    // only log that it can't.
                    cc.egui_ctx.options_mut(|o| o.sync_window_theme = false);
                    let inbox: Inbox = Inbox::default();
                    let doc = if query().contains("sample") { wordcraft_engine::sample::sample_document() } else { wordcraft_doc::Document::new() };
                    let mut app = WordApp::new(Session::new(doc), services(inbox.clone(), cc.egui_ctx.clone()));
                    let remote = if local { None } else { crate::remote::Remote::new(&cc.egui_ctx) };
                    let locale = crate::locale::Locale::new(&mut app);
                    app.autosave = remote.is_some();
                    if let Some(remote) = &remote {
                        app.hosted = Some(wordcraft_ui_egui::Hosted::default());
                        let requested = remote.requested.clone();
                        app.services.request_save = Some(Box::new(move || requested.set(true)));
                    }
                    let bridge = if egui_chrome { None } else { mount_chrome(&mut app, &cc.egui_ctx) };
                    // Plugins come from the server and only the HTML chrome shows them.
                    if let Some(bridge) = &bridge
                        && remote.is_some()
                    {
                        crate::plugins::load(bridge.clone());
                    }
                    Ok(Box::new(WebShell { app, inbox, remote, locale, font_error, title: String::new(), bridge }))
                }),
            )
            .await;
        if let Some(el) = document.get_element_by_id(LOADING_ID) {
            match result {
                Ok(()) => el.remove(),
                Err(e) => el.set_text_content(Some(&format!(
                    "{}: {e:?}. {}",
                    wordcraft_ui_egui::tl!("WordCraft failed to start"),
                    wordcraft_ui_egui::tl!("A browser with WebGPU or WebGL2 is required.")
                ))),
            }
        }
    });
}

fn query() -> String {
    web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default()
}

/// Whether the page URL has the bare query flag `name` (`?local`, `?egui-chrome`, …).
fn has_flag(name: &str) -> bool {
    query().trim_start_matches('?').split('&').any(|p| p == name)
}

/// Mount the HTML chrome (title bar, ribbon, panes, dialogs, File screen, status bar) around the
/// canvas and let egui draw only the document. Without the page slots, egui keeps its chrome.
fn mount_chrome(app: &mut WordApp, ctx: &egui::Context) -> Option<wordcraft_ui_web::Bridge> {
    let ctx = ctx.clone();
    let bridge = wordcraft_ui_web::Bridge::mount(app, move || ctx.request_repaint())?;
    app.html_chrome = wordcraft_ui_egui::HostedChrome::ALL;
    Some(bridge)
}

/// Wraps the app to read dropped files asynchronously (browsers can't read them synchronously)
/// and feed them through the inbox.
struct WebShell {
    app: WordApp,
    inbox: Inbox,
    remote: Option<crate::remote::Remote>,
    locale: crate::locale::Locale,
    font_error: bool,
    /// The browser tab title last written, so `document.title` only changes when it must.
    title: String,
    /// The HTML chrome, unless the page asked for egui's (`?egui-chrome`).
    bridge: Option<wordcraft_ui_web::Bridge>,
}

impl eframe::App for WebShell {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.locale.sync(&mut self.app);
        let dropped = ctx.input_mut(|i| std::mem::take(&mut i.raw.dropped_files));
        for f in dropped {
            let inbox = self.inbox.clone();
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let name = f.path().file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "dropped".into());
                match f.bytes_async().await {
                    Ok(bytes) => {
                        inbox.lock().unwrap_or_else(|e| e.into_inner()).push((name, bytes));
                        ctx.request_repaint();
                    }
                    Err(e) => log::error!("couldn't read dropped file {name}: {e}"),
                }
            });
        }
        if let Some(remote) = &mut self.remote {
            remote.logic(&mut self.app, ctx);
        }
        if !self.remote.as_ref().is_some_and(|r| r.busy_loading()) {
            // Clicks and edits from the HTML chrome run before the frame, like egui's own input.
            if let Some(bridge) = &self.bridge {
                bridge.apply_actions(&mut self.app);
            }
            self.app.logic(ctx);
        }
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.app.raw_input_hook(raw);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.locale.sync(&mut self.app);
        self.locale.notice(ui);
        if self.font_error {
            egui::Panel::top("font_notice").show(ui, |ui| {
                ui.label(wordcraft_ui_egui::tl!("Some server fonts could not load. Using available fonts; save your work and refresh to retry."));
            });
        }
        // Language lives in File ▸ Options and save state in the status bar; only problems get a bar.
        if let Some(remote) = &mut self.remote {
            remote.message_bar(ui);
        }
        if self.remote.as_ref().is_some_and(|r| r.busy_loading()) {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.label(wordcraft_ui_egui::tl!("Opening your saved document…"));
            });
        } else {
            self.app.ui(ui);
        }
        self.locale.sync(&mut self.app);
        if let Some(remote) = &self.remote {
            remote.after_ui(&self.app, ui.ctx());
        }
        // The HTML chrome follows the app: publish this frame's state once it's final.
        if let Some(bridge) = &self.bridge {
            bridge.publish(&mut self.app);
        }
        self.sync_title();
    }
}

impl WebShell {
    /// Show the open document's title in the browser tab (eframe ignores title commands on the web).
    /// While a saved document is still loading the tab keeps its current title.
    fn sync_title(&mut self) {
        if self.remote.as_ref().is_some_and(|r| r.busy_loading()) {
            return;
        }
        let title = self.app.window_title();
        if title == self.title {
            return;
        }
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            doc.set_title(&title);
        }
        self.title = title;
    }
}

fn services(inbox: Inbox, ctx: egui::Context) -> Services {
    let open_inbox = inbox.clone();
    Services {
        open_async: Some(Box::new(move |purpose: &str| {
            let inbox = open_inbox.clone();
            let ctx = ctx.clone();
            let dialog = if purpose == "picture" {
                rfd::AsyncFileDialog::new().add_filter("Pictures", IMAGE_EXTS)
            } else {
                rfd::AsyncFileDialog::new().add_filter("Documents", DOC_EXTS)
            };
            wasm_bindgen_futures::spawn_local(async move {
                let Some(file) = dialog.pick_file().await else {
                    return;
                };
                let bytes = file.read().await;
                inbox.lock().unwrap_or_else(|e| e.into_inner()).push((file.file_name(), bytes));
                ctx.request_repaint();
            });
        })),
        // Exports ask for a name; the browser decides where the download goes.
        pick_save: Some(Box::new(|name: &str| Some(name.to_string()))),
        download: Some(Box::new(|name: &str, bytes: &[u8]| {
            if let Err(e) = download(name, bytes) {
                log::error!("download of {name} failed: {e}");
            }
        })),
        inbox: Some(inbox),
        ..Default::default()
    }
}

/// Trigger a browser download of `bytes` named after the last component of `path`.
fn download(path: &str, bytes: &[u8]) -> Result<(), String> {
    let js = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "document".into());
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime_for(&name));
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).map_err(js)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js)?;
    let a: web_sys::HtmlAnchorElement = document.create_element("a").map_err(js)?.dyn_into().map_err(|_| "not an anchor")?;
    a.set_href(&url);
    a.set_download(&name);
    a.style().set_property("display", "none").map_err(js)?;
    let body = document.body().ok_or("no body")?;
    body.append_child(&a).map_err(js)?;
    a.click();
    a.remove();
    // Revoke after the click has been dispatched; the download keeps its own reference.
    let revoke = wasm_bindgen::closure::Closure::once_into_js(move || {
        web_sys::Url::revoke_object_url(&url).ok();
    });
    window.set_timeout_with_callback_and_timeout_and_arguments_0(revoke.unchecked_ref(), 10_000).map_err(js)?;
    Ok(())
}

fn mime_for(name: &str) -> &'static str {
    match name.rsplit('.').next().map(str::to_ascii_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("pdf") => "application/pdf",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("odt") => "application/vnd.oasis.opendocument.text",
        Some("rtf") => "application/rtf",
        Some("html") => "text/html",
        Some("md" | "txt") => "text/plain",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}
