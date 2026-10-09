//! Hosted document lifecycle. A single request is in flight; edits during it stay dirty.
use serde::de::DeserializeOwned;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wordcraft_remote::{MAX_BYTES, Metadata, Snapshot, WriteRequest};
use wordcraft_ui_egui::{WordApp, i18n, tl};
use wordcraft_web::sync::{Flight, acknowledges_current};

#[derive(Debug)]
struct Failure {
    message: String,
    conflict: bool,
}
type Result<T> = std::result::Result<T, Failure>;
enum Reply {
    List(Result<Vec<Metadata>>),
    Load(Result<Box<Snapshot>>),
    Save(Flight, Result<Metadata>),
}
type Replies = Rc<RefCell<Vec<Reply>>>;

pub struct Remote {
    pub requested: Rc<Cell<bool>>,
    id: String,
    revision: u64,
    epoch: u64,
    observed: u64,
    changed_at: f64,
    last_save: f64,
    saving: bool,
    loading: bool,
    load_id: Option<String>,
    error: Option<Failure>,
    list: Vec<Metadata>,
    show_list: bool,
    listing: bool,
    list_error: Option<String>,
    replies: Replies,
    unsaved: Rc<Cell<bool>>,
    _unload: gloo_events::EventListener,
    confirm_reload: bool,
}

impl Remote {
    pub fn new(ctx: &egui::Context) -> Option<Self> {
        let window = web_sys::window()?;
        let unsaved = Rc::new(Cell::new(false));
        let dirty = unsaved.clone();
        let unload = gloo_events::EventListener::new_with_options(
            &window,
            "beforeunload",
            gloo_events::EventListenerOptions::enable_prevent_default(),
            move |event| {
                use wasm_bindgen::JsCast as _;
                if dirty.get()
                    && let Some(event) = event.dyn_ref::<web_sys::BeforeUnloadEvent>()
                {
                    event.prevent_default();
                    event.set_return_value("");
                }
            },
        );
        let load_id = window
            .location()
            .search()
            .ok()
            .and_then(|q| q.trim_start_matches('?').split('&').find_map(|part| part.strip_prefix("doc=").map(str::to_owned)));
        let mut remote = Self {
            requested: Rc::new(Cell::new(false)),
            id: uuid::Uuid::new_v4().to_string(),
            revision: 0,
            epoch: 0,
            observed: 1,
            changed_at: js_sys::Date::now(),
            last_save: js_sys::Date::now(),
            saving: false,
            loading: false,
            load_id: load_id.clone(),
            error: None,
            list: Vec::new(),
            show_list: false,
            listing: false,
            list_error: None,
            replies: Rc::default(),
            unsaved,
            _unload: unload,
            confirm_reload: false,
        };
        if let Some(id) = load_id {
            remote.load(&id, ctx);
        }
        Some(remote)
    }

    pub fn busy_loading(&self) -> bool {
        self.loading || self.load_id.is_some()
    }

    pub fn logic(&mut self, app: &mut WordApp, ctx: &egui::Context) {
        if self.epoch != app.document_epoch {
            self.epoch = app.document_epoch;
            self.id = uuid::Uuid::new_v4().to_string();
            self.revision = 0;
            self.error = None;
            // Opening a local file or creating from a template creates a distinct server document.
            if app.session.doc.core.title.is_empty() {
                app.session.doc.core.title = app.title_stem();
            }
            app.session.path = None;
            app.session.dirty = true;
            self.set_url(None);
        }
        let replies = std::mem::take(&mut *self.replies.borrow_mut());
        for reply in replies {
            match reply {
                Reply::List(result) => {
                    self.listing = false;
                    match result {
                        Ok(list) => {
                            self.list = list;
                            self.list_error = None;
                        }
                        Err(e) => self.list_error = Some(e.message),
                    }
                }
                Reply::Load(result) => {
                    self.loading = false;
                    match result {
                        Ok(snapshot) => {
                            self.id = snapshot.metadata.id;
                            self.revision = snapshot.metadata.revision;
                            app.session.set_document(snapshot.document);
                            app.session.doc.core.title = snapshot.metadata.title;
                            app.session.path = None;
                            self.observed = app.session.rev();
                            self.load_id = None;
                            self.error = None;
                            self.show_list = false;
                            self.set_url(Some(&self.id));
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
                Reply::Save(flight, result) => {
                    self.saving = false;
                    if flight.id != self.id || flight.epoch != self.epoch {
                        continue;
                    }
                    match result {
                        Ok(metadata) => {
                            self.revision = metadata.revision;
                            if acknowledges_current(&flight, &self.id, self.epoch, app.session.rev()) {
                                app.session.dirty = false;
                            }
                            self.error = None;
                            self.set_url(Some(&self.id));
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
            }
        }
        let now = js_sys::Date::now();
        if self.observed != app.session.rev() {
            self.observed = app.session.rev();
            self.changed_at = now;
        }
        let manual = !self.saving && self.requested.replace(false);
        if !self.saving
            && !self.busy_loading()
            && (manual
                || (app.autosave && app.session.dirty && self.error.is_none() && (now - self.changed_at > 1500.0 || now - self.last_save > 10000.0)))
        {
            self.save(app, ctx);
        }
        self.unsaved.set(app.session.dirty || self.saving);
        if app.session.dirty || self.saving {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    /// UI commands run after frame logic; update the leave guard and schedule their save work now.
    pub fn after_ui(&self, app: &WordApp, ctx: &egui::Context) {
        self.unsaved.set(app.session.dirty || self.saving || self.epoch != app.document_epoch);
        if self.unsaved.get() || self.requested.get() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn save(&mut self, app: &mut WordApp, ctx: &egui::Context) {
        if self.error.as_ref().is_some_and(|e| e.conflict) {
            return;
        }
        let title = app.title_stem();
        let request = WriteRequest { revision: self.revision, title, document: app.session.doc.clone() };
        let body = match serde_json::to_string(&request) {
            Ok(body) if body.len() <= MAX_BYTES => body,
            _ => {
                self.error = Some(Failure { message: "Document is too large to save (16 MiB limit). Download a copy.".into(), conflict: false });
                return;
            }
        };
        let flight = Flight { id: self.id.clone(), epoch: self.epoch, local_revision: app.session.rev() };
        let url = format!("/api/documents/{}", self.id);
        self.saving = true;
        // Even an explicit save of a new blank document must be protected until acknowledged.
        app.session.dirty = true;
        self.error = None;
        self.last_save = js_sys::Date::now();
        let replies = self.replies.clone();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = request_json(&url, Some(body)).await;
            replies.borrow_mut().push(Reply::Save(flight, result));
            ctx.request_repaint();
        });
    }

    fn load(&mut self, id: &str, ctx: &egui::Context) {
        if !wordcraft_remote::valid_id(id) {
            self.error = Some(Failure { message: "Invalid document link.".into(), conflict: false });
            return;
        }
        self.loading = true;
        self.load_id = Some(id.into());
        self.error = None;
        let url = format!("/api/documents/{id}");
        let replies = self.replies.clone();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = request_json(&url, None).await;
            replies.borrow_mut().push(Reply::Load(result));
            ctx.request_repaint();
        });
    }

    fn refresh(&mut self, ctx: &egui::Context) {
        if self.listing {
            return;
        }
        self.listing = true;
        let replies = self.replies.clone();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = request_json("/api/documents", None).await;
            replies.borrow_mut().push(Reply::List(result));
            ctx.request_repaint();
        });
    }

    pub fn toolbar(&mut self, app: &mut WordApp, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        egui::Panel::top("server_documents").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                crate::locale::selector(app, ui);
                ui.separator();
                if ui.add_enabled(!self.busy_loading(), egui::Button::new(tl!("Documents"))).clicked() {
                    self.show_list = true;
                    self.refresh(&ctx);
                }
                let can_switch = !app.session.dirty && !self.saving && !self.busy_loading();
                if ui.add_enabled(can_switch, egui::Button::new(tl!("New document"))).on_disabled_hover_text(tl!("Save your changes first")).clicked()
                {
                    let _ = app.run("file.new", serde_json::json!({}));
                }
                if ui.add_enabled(!self.saving && !self.busy_loading(), egui::Button::new(tl!("Save to server"))).clicked() {
                    self.requested.set(true);
                }
                if ui.add_enabled(!self.busy_loading(), egui::Button::new(tl!("Download DOCX"))).clicked() {
                    let _ = app.run("file.saveAs", serde_json::json!({"path": format!("{}.docx", app.title_stem())}));
                }
                ui.separator();
                let mut title = app.title_stem();
                let response = ui.add_enabled(
                    !self.busy_loading(),
                    egui::TextEdit::singleline(&mut title).desired_width(160.0).char_limit(200).hint_text(tl!("Document title")),
                );
                if response.changed() && !title.trim().is_empty() {
                    let _ = app.run("file.properties", serde_json::json!({"title": title}));
                }
                let status = if self.busy_loading() {
                    "Opening document…"
                } else if self.saving {
                    "Saving…"
                } else if self.error.is_some() {
                    "Not saved"
                } else if app.session.dirty {
                    "Unsaved changes"
                } else if self.revision == 0 {
                    "New document · save to keep"
                } else {
                    "Saved to server"
                };
                ui.label(tl!(status));
                if self.revision > 0 {
                    ui.small(format!("v{}", self.revision));
                }
            });
            if self.load_id.is_some() && !self.loading && ui.button(tl!("Back to editor")).clicked() {
                self.load_id = None;
                self.error = None;
            }
            if let Some(error) = &self.error {
                let message = error.message.clone();
                let conflict = error.conflict;
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(wordcraft_ui_egui::theme::Tokens::get(&ctx).red, i18n::t(&message));
                    if conflict {
                        if ui.button(tl!("Save as new copy")).clicked() {
                            self.id = uuid::Uuid::new_v4().to_string();
                            self.revision = 0;
                            self.error = None;
                            self.requested.set(true);
                        }
                        if ui.button(tl!("Reload server version…")).clicked() {
                            self.confirm_reload = true;
                        }
                    } else if !self.loading && ui.button(tl!("Retry")).clicked() {
                        if let Some(id) = self.load_id.clone() {
                            self.load(&id, &ctx);
                        } else {
                            self.requested.set(true);
                        }
                    }
                });
            }
        });
        if self.show_list {
            let mut open = true;
            let mut selected = None;
            egui::Window::new(tl!("Server documents")).open(&mut open).default_width(460.0).show(&ctx, |ui| {
                if self.listing {
                    ui.label(tl!("Loading…"));
                }
                if let Some(error) = &self.list_error {
                    ui.label(i18n::t(error));
                }
                if app.session.dirty {
                    ui.label(tl!("Save your current changes before opening another document."));
                }
                if ui.button(tl!("Refresh")).clicked() {
                    self.refresh(&ctx);
                }
                if self.list.is_empty() && !self.listing {
                    ui.label(tl!("No saved documents yet."));
                }
                egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                    for item in &self.list {
                        if ui
                            .add_enabled(
                                !app.session.dirty && !self.saving && !self.loading,
                                egui::Button::new(format!("{}  ·  v{}", item.title, item.revision)),
                            )
                            .clicked()
                        {
                            selected = Some(item.id.clone());
                        }
                    }
                });
            });
            self.show_list = open;
            if let Some(id) = selected {
                self.load(&id, &ctx);
            }
        }
        if self.confirm_reload {
            egui::Window::new(tl!("Discard local changes?")).collapsible(false).show(&ctx, |ui| {
                ui.label(tl!("Reloading replaces your local changes with the saved server version."));
                ui.label(tl!("Download a copy first if you want to keep both versions."));
                if ui.button(tl!("Discard changes and reload")).clicked() {
                    self.confirm_reload = false;
                    self.load(&self.id.clone(), &ctx);
                }
                if ui.button(tl!("Keep editing")).clicked() {
                    self.confirm_reload = false;
                }
            });
        }
    }

    fn set_url(&self, id: Option<&str>) {
        if let Some(window) = web_sys::window()
            && let Ok(history) = window.history()
        {
            let url = id.map(|id| format!("?doc={id}")).unwrap_or_else(|| "/".into());
            let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&url));
        }
    }
}

/// Bound both fetch and body decoding; abort a timed-out request instead of leaving saves hung.
async fn request_json<T: DeserializeOwned>(url: &str, body: Option<String>) -> Result<T> {
    let failure = |message: String| Failure { message, conflict: false };
    let controller = web_sys::AbortController::new().map_err(|_| failure("Browser request unavailable".into()))?;
    let signal = controller.signal();
    let operation = async {
        let builder = if body.is_some() { gloo_net::http::Request::put(url) } else { gloo_net::http::Request::get(url) };
        let builder = builder.abort_signal(Some(&signal));
        let response = if let Some(body) = body {
            builder
                .header("Content-Type", "application/json")
                .body(body)
                .map_err(|_| failure("Could not prepare the save request.".into()))?
                .send()
                .await
        } else {
            builder.send().await
        }
        .map_err(|_| failure("Cannot reach server. Your changes remain here; retry or download a copy.".into()))?;
        if !response.ok() {
            return Err(Failure {
                message: match response.status() {
                    409 => "Another editor saved a newer version. Your changes are kept here.".into(),
                    404 => "Document not found on this server.".into(),
                    413 => "Document exceeds the 16 MiB limit. Download a copy.".into(),
                    _ => "Server rejected the request. Changes remain unsaved.".into(),
                },
                conflict: response.status() == 409,
            });
        }
        response.json::<T>().await.map_err(|_| failure("Invalid response from document server.".into()))
    };
    futures_util::pin_mut!(operation);
    let timeout = gloo_timers::future::TimeoutFuture::new(15_000);
    futures_util::pin_mut!(timeout);
    match futures_util::future::select(operation, timeout).await {
        futures_util::future::Either::Left((result, _)) => result,
        futures_util::future::Either::Right(_) => {
            controller.abort();
            Err(failure("Save request timed out. Retry or download a copy.".into()))
        }
    }
}
