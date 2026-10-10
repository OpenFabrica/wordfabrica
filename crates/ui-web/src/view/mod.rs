//! Leptos views for the web chrome. Each page slot (title bar, tab strip, ribbon, panes, status
//! bar, overlay) gets its own mount; all of them read the same [`Bridge`] state.

mod chrome;
mod dialogs;
mod file;
mod menus;
mod panes;
mod plugins;
mod ribbon;

use leptos::prelude::*;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::closure::Closure;
use wordfabrica_ui_egui::theme::Tokens;
use wordfabrica_ui_egui::{Action, tl};

use crate::bridge::Bridge;

/// The egui canvas element (see `apps/wordfabrica-web/index.html`).
pub const CANVAS_ID: &str = "wordfabrica_canvas";
/// Page elements the chrome mounts into.
pub const SLOTS: [&str; 7] = ["wc-title", "wc-tabs", "wc-ribbon", "wc-left", "wc-right", "wc-status", "wc-overlay"];

/// Mount every chrome part into its slot and install the page-level keyboard handling.
pub fn mount(b: &Bridge, document: &web_sys::Document) {
    let slot = |id: &str| document.get_element_by_id(id).and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok());
    // Each part re-renders when the interface language changes (strings come from `tl!`).
    macro_rules! part {
        ($id:expr, $view:path) => {
            if let Some(el) = slot($id) {
                let b = b.clone();
                leptos::mount::mount_to(el, move || {
                    let lang = b.select(|s| s.lang.clone());
                    move || {
                        // Read the memo (not just `track()`): a memo nobody has read is never computed,
                        // so it wouldn't follow the state and the part would never re-render.
                        let _ = lang.get();
                        $view(b.clone())
                    }
                })
                .forget();
            }
        };
    }
    part!("wc-title", chrome::title_bar);
    part!("wc-tabs", chrome::tab_strip);
    part!("wc-ribbon", ribbon::ribbon);
    part!("wc-left", panes::left);
    part!("wc-right", panes::right);
    part!("wc-status", chrome::status_bar);
    part!("wc-overlay", overlay);
    install_keys(b, document);
}

/// Dialogs, the File screen, open menus, and the theme (CSS variables on `<html>`).
fn overlay(b: Bridge) -> impl IntoView {
    let dark = b.select(|s| s.dark);
    Effect::new(move |_| apply_theme(dark.get()));
    view! {
        {file::file_screen(b.clone())}
        {dialogs::dialog_host(b.clone())}
        {menus::menu_host(b.clone())}
    }
}

impl Bridge {
    /// A memo of one part of the state: views using it update only when that part changes.
    pub fn select<T>(&self, f: impl Fn(&wordfabrica_ui_egui::ChromeState) -> T + Send + Sync + 'static) -> Memo<T>
    where
        T: PartialEq + Clone + Send + Sync + 'static,
    {
        let st = self.state.clone();
        Memo::new(move |_| st.with(|s| f(s)))
    }
}

/// Write the theme's colours as CSS variables (`--wc-…`) on the root element.
fn apply_theme(dark: bool) {
    let t = if dark { Tokens::dark() } else { Tokens::light() };
    let Some(root) = web_sys::window().and_then(|w| w.document()).and_then(|d| d.document_element()) else { return };
    let Ok(root) = root.dyn_into::<web_sys::HtmlElement>() else { return };
    let style = root.style();
    for (name, c) in [
        ("title-bar", t.title_bar),
        ("ribbon", t.ribbon),
        ("tab-strip", t.tab_strip),
        ("canvas", t.canvas),
        ("panel", t.panel),
        ("border", t.border),
        ("border-strong", t.border_strong),
        ("text", t.text),
        ("text-dim", t.text_dim),
        ("text-disabled", t.text_disabled),
        ("icon", t.icon),
        ("hover", t.hover),
        ("pressed", t.pressed),
        ("checked", t.checked),
        ("accent", t.accent),
        ("accent-text", t.accent_text),
        ("input", t.input),
        ("input-border", t.input_border),
        ("status-bar", t.status_bar),
        ("menu", t.menu),
        ("red", t.red),
        ("green", t.green),
    ] {
        let _ = style.set_property(&format!("--wc-{name}"), &css_color(c));
    }
    let _ = root.set_attribute("data-theme", if dark { "dark" } else { "light" });
}

/// `rgba(…)` for a colour.
pub fn css_color(c: egui::Color32) -> String {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    format!("rgba({r},{g},{b},{:.3})", a as f32 / 255.0)
}

/// An icon as inline SVG in the theme's colours (greyed out when `enabled` is false).
pub fn icon_svg(name: &str, px: f32, dark: bool, enabled: bool) -> String {
    let t = if dark { Tokens::dark() } else { Tokens::light() };
    let (c, a) = if enabled { (t.icon, t.accent) } else { (t.text_disabled, t.text_disabled) };
    wordfabrica_ui_egui::icons::svg(name, px, c, a)
}

/// An icon in fixed colours (white on the accent buttons).
pub fn icon_svg_in(name: &str, px: f32, c: egui::Color32) -> String {
    wordfabrica_ui_egui::icons::svg(name, px, c, c)
}

/// Keep focus where it is when a toolbar control is clicked with the mouse: the document's
/// selection and IME state stay put, and the click still fires.
pub fn keep_focus(ev: web_sys::MouseEvent) {
    ev.prevent_default();
}

/// The text of an `<input>` event.
pub fn input_value(ev: &web_sys::Event) -> String {
    ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()).map(|i| i.value()).unwrap_or_default()
}

/// Whether an `<input type=checkbox>` event's box is checked.
pub fn input_checked(ev: &web_sys::Event) -> bool {
    ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()).is_some_and(|i| i.checked())
}

/// Screen rectangle (left, bottom, width) of the element an event is on, for anchoring menus.
pub fn anchor_of(ev: &web_sys::MouseEvent) -> (f64, f64, f64) {
    let el = ev.current_target().and_then(|t| t.dyn_into::<web_sys::Element>().ok());
    match el {
        Some(el) => {
            let r = el.get_bounding_client_rect();
            (r.left(), r.bottom(), r.width())
        }
        None => (ev.client_x() as f64, ev.client_y() as f64, 0.0),
    }
}

/// Translate an English UI string at render time (owned, so views can keep it).
pub fn t(s: &str) -> String {
    tl!(s).to_string()
}

/// Whether DOM focus is in the document area (the egui canvas or its text input).
fn focus_in_document(target: Option<web_sys::Element>) -> bool {
    target.is_some_and(|el| el.id() == CANVAS_ID || el.closest("#wc-doc").ok().flatten().is_some())
}

/// Page-level keys: F6 moves between the document and the ribbon; Escape returns to the
/// document; with focus on a chrome button (not a text field), command shortcuts still work.
fn install_keys(b: &Bridge, document: &web_sys::Document) {
    let b = b.clone();
    let cb = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |ev: web_sys::KeyboardEvent| {
        let target = ev.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok());
        let in_doc = focus_in_document(target.clone());
        let key = ev.key();
        if key == "F6" {
            ev.prevent_default();
            if in_doc {
                focus_selector("#wc-tabs .wc-tab.active, #wc-tabs .wc-tab");
            } else {
                b.send(Action::FocusDocument);
            }
            return;
        }
        // Menus open without taking focus from the document, so Escape closes them from anywhere
        // (and egui doesn't see that Escape).
        if key == "Escape" && b.menu.with_untracked(Option::is_some) {
            b.menu.set(None);
            ev.prevent_default();
            ev.stop_propagation();
            return;
        }
        if in_doc {
            return;
        }
        let in_text = target.as_ref().is_some_and(|el| matches!(el.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT"));
        let in_dialog = target.as_ref().and_then(|el| el.closest("dialog, .wc-file").ok().flatten()).is_some();
        if key == "Escape" {
            if !in_dialog && !in_text {
                b.send(Action::FocusDocument);
            }
            return;
        }
        if in_text || in_dialog {
            return;
        }
        let command = if b.mac { ev.meta_key() } else { ev.ctrl_key() };
        let fkey = key.len() >= 2 && key.starts_with('F') && key.chars().skip(1).all(|c| c.is_ascii_digit());
        if !command && !fkey {
            return;
        }
        let Some(name) = crate::shortcut_name(&key, ev.ctrl_key(), ev.meta_key(), ev.alt_key(), ev.shift_key(), b.mac) else { return };
        if let Some(spec) = b.registry.by_shortcut(&name) {
            ev.prevent_default();
            b.send(Action::run(spec.id, serde_json::json!({})));
        }
    });
    let _ = document.add_event_listener_with_callback_and_bool("keydown", cb.as_ref().unchecked_ref(), true);
    cb.forget();
}

/// Focus the first element matching `selector`.
pub fn focus_selector(selector: &str) {
    let el = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.query_selector(selector).ok().flatten())
        .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok());
    if let Some(el) = el {
        let _ = el.focus();
    }
}
