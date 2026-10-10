//! Hosted document lifecycle. A single request is in flight; edits during it stay dirty.
use serde::de::DeserializeOwned;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wordfabrica_remote::{MAX_BYTES, Metadata, Snapshot, WriteRequest};
use wordfabrica_ui_egui::{HostedDocument, WordApp, i18n, tl};
use wordfabrica_web::sync::{Flight, acknowledges_current, status};

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
    /// The File tab was open last frame; opening it again refreshes the server list.
    browsing: bool,
    replies: Replies,
    unsaved: Rc<Cell<bool>>,
    _unload: gloo_events::EventListener,
    confirm_reload: bool,
    /// Access token of a third-party document link: (document id, token). Sent with every request
    /// for that document; kept in sessionStorage so a reload of the tab keeps working.
    token: Option<(String, String)>,
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
        let query = window.location().search().ok().unwrap_or_default();
        let param = |name: &str| query.trim_start_matches('?').split('&').find_map(|part| part.strip_prefix(name).map(str::to_owned));
        let load_id = param("doc=");
        let token = load_id.as_ref().and_then(|id| {
            let key = format!("wordfabrica-token:{id}");
            let storage = window.session_storage().ok().flatten();
            match param("t=") {
                Some(token) => {
                    if let Some(storage) = &storage {
                        let _ = storage.set_item(&key, &token);
                    }
                    Some((id.clone(), token))
                }
                None => storage.and_then(|s| s.get_item(&key).ok().flatten()).map(|token| (id.clone(), token)),
            }
        });
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
            browsing: false,
            replies: Rc::default(),
            unsaved,
            _unload: unload,
            confirm_reload: false,
            token,
        };
        if let Some(id) = load_id {
            remote.load(&id, ctx);
        }
        Some(remote)
    }

    pub fn busy_loading(&self) -> bool {
        self.loading || self.load_id.is_some()
    }

    /// Per-frame lifecycle: take UI requests, apply replies, autosave, then publish state to the UI.
    pub fn logic(&mut self, app: &mut WordApp, ctx: &egui::Context) {
        // Requests File ▸ Open left in the shared state during the previous frame.
        let (refresh, open) = app.hosted.as_mut().map(|h| (std::mem::take(&mut h.refresh), h.open.take())).unwrap_or_default();
        if refresh || (app.ui.backstage && !self.browsing) {
            self.refresh(app, ctx);
        }
        self.browsing = app.ui.backstage;
        if let Some(id) = open
            && !app.session.dirty
            && !self.saving
            && !self.busy_loading()
        {
            self.load(&id, ctx);
        }
        if self.epoch != app.document_epoch {
            self.epoch = app.document_epoch;
            self.id = uuid::Uuid::new_v4().to_string();
            // A new or opened file is the user's own document, not the linked one.
            app.session.read_only = false;
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
                    if let Some(hosted) = &mut app.hosted {
                        hosted.listing = false;
                        match result {
                            Ok(list) => {
                                hosted.documents =
                                    list.into_iter().map(|m: Metadata| HostedDocument { id: m.id, title: m.title, revision: m.revision }).collect();
                                hosted.list_error = None;
                            }
                            Err(e) => hosted.list_error = Some(e.message),
                        }
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
                            app.session.read_only = snapshot.read_only;
                            app.session.path = None;
                            self.observed = app.session.rev();
                            self.load_id = None;
                            self.error = None;
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
            self.save(app, ctx, manual);
        }
        self.unsaved.set(app.session.dirty || self.saving);
        if app.session.dirty || self.saving {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        self.publish(app);
    }

    /// Copy the save state into `app.hosted` for the status bar and File ▸ Open.
    fn publish(&self, app: &mut WordApp) {
        let dirty = app.session.dirty;
        let failed = self.error.is_some();
        if let Some(hosted) = &mut app.hosted {
            hosted.status = status(self.busy_loading(), self.saving, failed, dirty, self.revision, app.session.read_only);
            hosted.failed = failed;
            hosted.revision = self.revision;
            hosted.busy = self.saving || self.busy_loading();
        }
    }

    /// UI commands run after frame logic; update the leave guard and schedule their save work now.
    pub fn after_ui(&self, app: &WordApp, ctx: &egui::Context) {
        self.unsaved.set(app.session.dirty || self.saving || self.epoch != app.document_epoch);
        if self.unsaved.get() || self.requested.get() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    /// PUT the current snapshot; `commit` marks an explicit save (third-party documents call back).
    fn save(&mut self, app: &mut WordApp, ctx: &egui::Context, commit: bool) {
        if self.error.as_ref().is_some_and(|e| e.conflict) || app.session.read_only {
            return;
        }
        let title = app.title_stem();
        let request = WriteRequest { revision: self.revision, title, document: app.session.doc.clone(), commit };
        let body = match serde_json::to_string(&request) {
            Ok(body) if body.len() <= MAX_BYTES => body,
            _ => {
                self.error = Some(Failure { message: "Document is too large to save (16 MiB limit). Download a copy.".into(), conflict: false });
                return;
            }
        };
        let flight = Flight { id: self.id.clone(), epoch: self.epoch, local_revision: app.session.rev() };
        let url = format!("/api/documents/{}", self.id);
        let token = self.token_for(&self.id);
        self.saving = true;
        // Even an explicit save of a new blank document must be protected until acknowledged.
        app.session.dirty = true;
        self.error = None;
        self.last_save = js_sys::Date::now();
        let replies = self.replies.clone();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = request_json(&url, Some(body), token).await;
            replies.borrow_mut().push(Reply::Save(flight, result));
            ctx.request_repaint();
        });
    }

    fn load(&mut self, id: &str, ctx: &egui::Context) {
        if !wordfabrica_remote::valid_id(id) {
            self.error = Some(Failure { message: "Invalid document link.".into(), conflict: false });
            return;
        }
        self.loading = true;
        self.load_id = Some(id.into());
        self.error = None;
        let url = format!("/api/documents/{id}");
        let token = self.token_for(id);
        let replies = self.replies.clone();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = request_json(&url, None, token).await;
            replies.borrow_mut().push(Reply::Load(result));
            ctx.request_repaint();
        });
    }

    /// Fetch the server document list into `app.hosted`; one request at a time.
    fn refresh(&self, app: &mut WordApp, ctx: &egui::Context) {
        let Some(hosted) = &mut app.hosted else { return };
        if hosted.listing {
            return;
        }
        hosted.listing = true;
        let replies = self.replies.clone();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = request_json("/api/documents", None, None).await;
            replies.borrow_mut().push(Reply::List(result));
            ctx.request_repaint();
        });
    }

    /// Message bar shown only when a load or save needs the user: error text and recovery actions.
    pub fn message_bar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let load_failed = self.load_id.is_some() && !self.loading;
        if self.error.is_some() || load_failed {
            egui::Panel::top("server_message").show(ui, |ui| {
                if self.load_id.is_some() && !self.loading && ui.button(tl!("Back to editor")).clicked() {
                    self.load_id = None;
                    self.error = None;
                }
                if let Some(error) = &self.error {
                    let message = error.message.clone();
                    let conflict = error.conflict;
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(wordfabrica_ui_egui::theme::Tokens::get(&ctx).red, i18n::t(&message));
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

    /// The access token for document `id`, if it was opened through a third-party link.
    fn token_for(&self, id: &str) -> Option<String> {
        self.token.as_ref().filter(|(doc, _)| doc == id).map(|(_, token)| token.clone())
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
async fn request_json<T: DeserializeOwned>(url: &str, body: Option<String>, token: Option<String>) -> Result<T> {
    let failure = |message: String| Failure { message, conflict: false };
    let controller = web_sys::AbortController::new().map_err(|_| failure("Browser request unavailable".into()))?;
    let signal = controller.signal();
    let operation = async {
        let builder = if body.is_some() { gloo_net::http::Request::put(url) } else { gloo_net::http::Request::get(url) };
        let builder = builder.abort_signal(Some(&signal));
        let builder = match &token {
            Some(token) => builder.header("X-WordFabrica-Token", token),
            None => builder,
        };
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
                    401 => "This link has expired or is not valid. Open the document again from its app.".into(),
                    403 => "This document is open read-only. Changes are not saved.".into(),
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
