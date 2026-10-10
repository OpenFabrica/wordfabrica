//! The link between the HTML chrome and `WordApp`: a queue of actions going in, a state snapshot
//! (and rendered preview images) coming out. HTML event handlers only push to the queue and ask
//! for a repaint; they never touch the app, so nothing can borrow it twice.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use leptos::prelude::*;
use wasm_bindgen::JsCast as _;
use wordcraft_engine::Registry;
use wordcraft_ui_egui::host::CommandInfo;
use wordcraft_ui_egui::{Action, ChromeState, WordApp};

use crate::model::Item;

/// A preview image the chrome wants (rendered by the engine, shown in an `<img>`).
#[derive(Clone, Debug, PartialEq)]
pub enum Preview {
    /// Font menu entry in its own face.
    Font(String),
    /// Styles gallery tile: style id, label (translated name), character style?
    Style {
        id: String,
        label: String,
        is_char: bool,
    },
    StyleSet(String),
    TableStyle(String),
    /// File ▸ New template thumbnail.
    Template(String),
}

/// Where an open menu comes from.
#[derive(Clone, Debug)]
pub enum MenuSource {
    Items(&'static [Item]),
    Fonts,
    Sizes,
    /// The status bar's document language menu.
    Language,
    /// The tab strip's Editing / Reviewing menu.
    EditMode,
}

/// The one dropdown menu that can be open, anchored under a control.
#[derive(Clone, Debug)]
pub struct OpenMenu {
    /// Identifies the control that opened it (clicking it again closes the menu).
    pub key: String,
    pub left: f64,
    pub top: f64,
    /// Minimum width (the anchor's width for combo lists).
    pub width: f64,
    pub source: MenuSource,
}

/// Shared handle used by every chrome view.
#[derive(Clone)]
pub struct Bridge {
    pub state: ArcRwSignal<ChromeState>,
    queue: Arc<Mutex<VecDeque<Action>>>,
    repaint: Arc<dyn Fn() + Send + Sync>,
    /// Rendered previews: key → `data:` URL.
    pub images: ArcRwSignal<HashMap<String, String>>,
    wanted: Arc<Mutex<Vec<(String, Preview)>>>,
    pub menu: ArcRwSignal<Option<OpenMenu>>,
    /// Font families for the font menu, and every command for search and tooltips.
    pub fonts: Arc<Vec<String>>,
    pub commands: Arc<Vec<CommandInfo>>,
    pub registry: Arc<Registry>,
    ids: Arc<Vec<&'static str>>,
    /// macOS shortcut notation (⌘) rather than Ctrl.
    pub mac: bool,
}

impl Bridge {
    /// Build the chrome around the canvas and mount it into the page. Returns `None` when the
    /// page has no chrome slots (the host then keeps egui's chrome).
    pub fn mount(app: &mut WordApp, repaint: impl Fn() + Send + Sync + 'static) -> Option<Bridge> {
        let document = web_sys::window()?.document()?;
        for id in crate::view::SLOTS {
            document.get_element_by_id(id)?;
        }
        let mac = web_sys::window().and_then(|w| w.navigator().platform().ok()).is_some_and(|p| p.to_ascii_lowercase().contains("mac"));
        let ids = crate::model::all_ids();
        let bridge = Bridge {
            state: ArcRwSignal::new(app.chrome_state(&ids)),
            queue: Arc::new(Mutex::new(VecDeque::new())),
            repaint: Arc::new(repaint),
            images: ArcRwSignal::new(HashMap::new()),
            wanted: Arc::new(Mutex::new(Vec::new())),
            menu: ArcRwSignal::new(None),
            fonts: Arc::new(app.previews.families()),
            commands: Arc::new(app.command_list()),
            registry: app.session.registry.clone(),
            ids: Arc::new(ids),
            mac,
        };
        crate::view::mount(&bridge, &document);
        Some(bridge)
    }

    /// Queue an action for the app and ask for a frame to apply it.
    pub fn send(&self, action: Action) {
        self.queue.lock().unwrap_or_else(|e| e.into_inner()).push_back(action);
        (self.repaint)();
    }

    /// Run a command, then give the keyboard back to the document (toolbar buttons).
    pub fn run(&self, id: &str, params: serde_json::Value) {
        self.send(Action::run(id, params));
        self.send(Action::FocusDocument);
    }

    /// Before a frame: apply what the chrome asked for.
    pub fn apply_actions(&self, app: &mut WordApp) {
        let actions: Vec<Action> = self.queue.lock().unwrap_or_else(|e| e.into_inner()).drain(..).collect();
        let focus = actions.iter().any(|a| matches!(a, Action::FocusDocument));
        for a in actions {
            app.apply(a);
        }
        if focus {
            focus_canvas();
        }
    }

    /// After a frame: publish the snapshot when it changed, and render a few wanted previews.
    pub fn publish(&self, app: &mut WordApp) {
        let st = app.chrome_state(&self.ids);
        if self.state.with_untracked(|s| *s != st) {
            self.state.set(st);
        }
        self.render_previews(app);
    }

    /// Whether command `id` is disabled right now (tracks the state).
    pub fn disabled(&self, id: &str) -> bool {
        self.state.with(|s| s.disabled.contains(id))
    }

    /// The preview image for `key`, or `None` while it's being rendered (it's queued).
    pub fn image(&self, key: &str, preview: impl FnOnce() -> Preview) -> Option<String> {
        if let Some(url) = self.images.with(|m| m.get(key).cloned()) {
            return Some(url);
        }
        let mut w = self.wanted.lock().unwrap_or_else(|e| e.into_inner());
        if !w.iter().any(|(k, _)| k == key) {
            w.push((key.to_string(), preview()));
            drop(w);
            (self.repaint)();
        }
        None
    }

    /// Tooltip text: label, shortcut, where the command lives, and whether it's available.
    pub fn tooltip(&self, label: &str, id: &str) -> String {
        let info = self.commands.iter().find(|c| c.id == id);
        let sc = info.map(|c| wordcraft_ui_egui::host::shortcut_display(&c.shortcut, self.mac)).unwrap_or_default();
        let mut s = if sc.is_empty() { label.to_string() } else { format!("{label} ({sc})") };
        if let Some(c) = info {
            let loc = wordcraft_ui_egui::i18n::location(&c.location);
            if !loc.is_empty() {
                s.push('\n');
                s.push_str(&loc);
            }
        }
        if self.state.with_untracked(|st| st.disabled.contains(id)) {
            s.push('\n');
            s.push_str(wordcraft_ui_egui::tl!("Not available right now"));
        }
        s
    }

    /// Shortcut text for a menu entry.
    pub fn shortcut(&self, id: &str) -> String {
        self.commands.iter().find(|c| c.id == id).map(|c| wordcraft_ui_egui::host::shortcut_display(&c.shortcut, self.mac)).unwrap_or_default()
    }

    /// Render up to a few queued previews per frame (rendering is real layout work).
    fn render_previews(&self, app: &mut WordApp) {
        let todo: Vec<(String, Preview)> = {
            let mut w = self.wanted.lock().unwrap_or_else(|e| e.into_inner());
            let n = w.len().min(6);
            w.drain(..n).collect()
        };
        if todo.is_empty() {
            return;
        }
        let ppp = web_sys::window().map(|w| w.device_pixel_ratio() as f32).unwrap_or(1.0).clamp(1.0, 3.0);
        let dark = self.state.with_untracked(|s| s.dark);
        let mut done = Vec::new();
        for (key, p) in todo {
            use wordcraft_ui_egui::previews as pv;
            let img = match &p {
                Preview::Font(name) => pv::font_image(name, ppp),
                Preview::Style { id, label, is_char } => pv::style_image(&app.session.doc, id, label, *is_char, ppp, dark),
                Preview::StyleSet(name) => pv::style_set_image(name, ppp),
                Preview::TableStyle(id) => pv::table_style_image(&app.session.doc, id, ppp),
                Preview::Template(name) => pv::template_image(name, 150.0 * ppp),
            };
            // A preview that can't render keeps an empty entry so it isn't retried every frame.
            let url = img.and_then(|img| data_url(img.size[0], img.size[1], &pv::straight_rgba(&img))).unwrap_or_default();
            done.push((key, url));
        }
        self.images.update(|m| {
            if m.len() > 800 {
                m.clear();
            }
            m.extend(done);
        });
        if !self.wanted.lock().unwrap_or_else(|e| e.into_inner()).is_empty() {
            (self.repaint)();
        }
    }
}

/// Encode straight RGBA pixels as a PNG `data:` URL through a detached canvas.
fn data_url(w: usize, h: usize, rgba: &[u8]) -> Option<String> {
    if w == 0 || h == 0 || rgba.len() != w.checked_mul(h)?.checked_mul(4)? {
        return None;
    }
    let (w32, h32) = (u32::try_from(w).ok()?, u32::try_from(h).ok()?);
    let document = web_sys::window()?.document()?;
    let canvas: web_sys::HtmlCanvasElement = document.create_element("canvas").ok()?.dyn_into().ok()?;
    canvas.set_width(w32);
    canvas.set_height(h32);
    let ctx: web_sys::CanvasRenderingContext2d = canvas.get_context("2d").ok()??.dyn_into().ok()?;
    let data = web_sys::ImageData::new_with_u8_clamped_array_and_sh(wasm_bindgen::Clamped(rgba), w32, h32).ok()?;
    ctx.put_image_data(&data, 0.0, 0.0).ok()?;
    canvas.to_data_url().ok()
}

/// Give DOM focus back to the egui canvas; egui then focuses its text input for typing and IME.
/// When focus is already in the document area (canvas or egui's text input) it stays put, so an
/// IME composition in progress isn't interrupted.
pub fn focus_canvas() {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
    let in_doc = document.active_element().is_some_and(|el| el.closest("#wc-doc").ok().flatten().is_some());
    if in_doc {
        return;
    }
    let canvas = document.get_element_by_id(crate::view::CANVAS_ID).and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok());
    if let Some(c) = canvas {
        let _ = c.focus();
    }
}
