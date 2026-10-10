//! Dialogs as HTML `<dialog>` modals with the same fields, labels and buttons as the egui ones
//! (`wordcraft_ui_egui::dialogs`). The form is a local copy of `ChromeState::dialog`; every button
//! sends the whole form back as [`Action::Dialog`], so `dialogs::press` runs the same commands
//! whichever front end shows the dialog.

use std::mem::discriminant;
use std::sync::atomic::{AtomicU32, Ordering};

use leptos::prelude::*;
use wasm_bindgen::JsCast as _;
use wordcraft_ui_egui::credits::{self, CONTRIBUTORS, MODELS, NameMode, SortKey, TOTAL_COMMITS};
use wordcraft_ui_egui::dialogs::{Dialog, DialogButton, command_hits, title};
use wordcraft_ui_egui::{Action, tl};

use super::{focus_selector, input_checked, input_value, t};
use crate::bridge::{Bridge, focus_canvas};

/// The open dialog (if any) as a modal `<dialog>`. It is rebuilt only when another kind of dialog
/// opens; updates to the same dialog (Find's message…) re-sync its form in place.
pub fn dialog_host(b: Bridge) -> impl IntoView {
    let kind = b.select(|s| s.dialog.as_ref().map(discriminant));
    move || {
        kind.get()?;
        let d = b.state.with_untracked(|s| s.dialog.clone())?;
        Some(shell(b.clone(), d))
    }
}

/// Read and write one field of one dialog variant (`None` / no-op on any other variant).
struct Lens<T> {
    get: fn(&Dialog) -> Option<T>,
    set: fn(&mut Dialog, T),
}

// Manual impls: a derive would demand `T: Copy`, but the two fn pointers are always copyable.
impl<T> Clone for Lens<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Lens<T> {}

/// A [`Lens`] onto `Dialog::$variant { $field }`; numbers travel as `f64` (u32 fields round).
macro_rules! lens {
    (str $v:ident . $f:ident) => {
        Lens::<String> {
            get: |d| match d {
                Dialog::$v { $f, .. } => Some($f.clone()),
                _ => None,
            },
            set: |d, x| {
                if let Dialog::$v { $f, .. } = d {
                    *$f = x;
                }
            },
        }
    };
    (bool $v:ident . $f:ident) => {
        Lens::<bool> {
            get: |d| match d {
                Dialog::$v { $f, .. } => Some(*$f),
                _ => None,
            },
            set: |d, x| {
                if let Dialog::$v { $f, .. } = d {
                    *$f = x;
                }
            },
        }
    };
    (f32 $v:ident . $f:ident) => {
        Lens::<f64> {
            get: |d| match d {
                Dialog::$v { $f, .. } => Some(f64::from(*$f)),
                _ => None,
            },
            set: |d, x| {
                if let Dialog::$v { $f, .. } = d {
                    *$f = x as f32;
                }
            },
        }
    };
    (u32 $v:ident . $f:ident) => {
        Lens::<f64> {
            get: |d| match d {
                Dialog::$v { $f, .. } => Some(f64::from(*$f)),
                _ => None,
            },
            // `as` saturates, and the field's range already clamped the value.
            set: |d, x| {
                if let Dialog::$v { $f, .. } = d {
                    *$f = x.round() as u32;
                }
            },
        }
    };
}

/// The open dialog's local form and the bridge its buttons talk to. Every access is `try_`: an
/// event can still arrive while the dialog's reactive scope is being torn down.
#[derive(Clone, Copy)]
struct Form {
    b: StoredValue<Bridge>,
    d: RwSignal<Dialog>,
    /// Bumped when the form changes from outside a field (state sync, radios, the zoom slider).
    /// Text and number fields re-read their value only then, so typing is never reformatted.
    epoch: RwSignal<u64>,
}

impl Form {
    /// Send `button` with the form as it is now; the app runs it and maybe closes the dialog.
    fn press(self, button: DialogButton) {
        let Some(dialog) = self.d.try_get_untracked() else { return };
        let _ = self.b.try_with_value(|b| b.send(Action::Dialog { dialog, button }));
    }

    /// A field's current value (tracked).
    fn get<T>(self, l: Lens<T>) -> Option<T> {
        self.d.try_with(|d| (l.get)(d)).flatten()
    }

    /// A field's value for a text box: re-read only when [`Form::refresh`] runs.
    fn shown<T>(self, l: Lens<T>) -> Option<T> {
        self.epoch.track();
        self.d.try_with_untracked(|d| (l.get)(d)).flatten()
    }

    /// Write a field.
    fn set<T>(self, l: Lens<T>, v: T) {
        let _ = self.d.try_update(|d| (l.set)(d, v));
    }

    /// Make text and number fields show the form's values again.
    fn refresh(self) {
        let _ = self.epoch.try_update(|e| *e = e.wrapping_add(1));
    }

    /// Whether the app still has a dialog open.
    fn open_in_app(self) -> bool {
        self.b.try_with_value(|b| b.state.with_untracked(|s| s.dialog.is_some())).unwrap_or(false)
    }
}

/// One dialog: title bar, the form for its kind, and its keyboard handling.
fn shell(b: Bridge, initial: Dialog) -> impl IntoView {
    let kind = discriminant(&initial);
    let name = if matches!(initial, Dialog::Find { .. }) { "find" } else { initial.name() };
    let remote = b.select(|s| s.dialog.clone());
    let f = Form { b: StoredValue::new(b), d: RwSignal::new(initial.clone()), epoch: RwSignal::new(0) };
    // The app answers a button with the form it kept (plus messages): that becomes the form.
    Effect::new(move |_| {
        if let Some(d) = remote.get()
            && discriminant(&d) == kind
        {
            f.d.set(d);
            f.refresh();
        }
    });
    // Give the keyboard back to the document once the dialog is gone: while the modal is still
    // in the page everything else is inert and `focus()` is ignored. If another dialog opened
    // instead, the canvas is inert again and keeps no focus.
    on_cleanup(|| set_timeout(focus_canvas, std::time::Duration::ZERO));
    let heading = Memo::new(move |_| f.d.with(|d| t(title(d))));
    let node = NodeRef::<leptos::html::Dialog>::new();
    Effect::new(move |_| {
        if let Some(el) = node.get() {
            if !el.open() {
                let _ = el.show_modal();
            }
            focus_first(&el);
        }
    });
    let body = body(f, &initial);
    view! {
        <dialog class=format!("wc-dialog wc-dlg-{name}") node_ref=node aria-labelledby="wc-dlg-title"
            on:cancel=move |ev: web_sys::Event| {
                ev.prevent_default();
                f.press(DialogButton::Cancel);
            }
            // The browser may close the dialog without a cancelable `cancel` event; tell the app.
            on:close=move |_| {
                if f.open_in_app() {
                    f.press(DialogButton::Cancel);
                }
            }
            on:keydown=move |ev| keydown(f, &ev)>
            <header class="wc-dlg-head">
                <h2 id="wc-dlg-title">{move || heading.get()}</h2>
                <button type="button" class="wc-dlg-x" aria-label=t("Close") title=t("Close")
                    on:click=move |_| f.press(DialogButton::Cancel)>"×"</button>
            </header>
            <div class="wc-dlg-body">{body}</div>
        </dialog>
    }
}

/// Escape cancels; Enter in a single-line field presses the dialog's default button.
fn keydown(f: Form, ev: &web_sys::KeyboardEvent) {
    // Keys that end an IME composition belong to the input method, not the dialog.
    if ev.is_composing() {
        return;
    }
    match ev.key().as_str() {
        "Escape" => {
            ev.prevent_default();
            f.press(DialogButton::Cancel);
        }
        "Enter" if ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()).is_some_and(|i| single_line(&i)) => {
            ev.prevent_default();
            if let Some(button) = default_button(f) {
                f.press(button);
            }
        }
        _ => {}
    }
}

/// Whether an `<input>` is a one-line text or number box (Enter submits; checkboxes don't).
fn single_line(i: &web_sys::HtmlInputElement) -> bool {
    matches!(i.type_().as_str(), "text" | "number" | "search" | "url")
}

/// What Enter does: Find Next in Find, the first hit in command search, OK elsewhere.
fn default_button(f: Form) -> Option<DialogButton> {
    let d = f.d.try_get_untracked()?;
    Some(match d {
        Dialog::Find { .. } => DialogButton::FindNext,
        Dialog::Commands { query } => {
            let id = f.b.try_with_value(|b| command_hits(&b.registry, &query, 1).first().map(|c| c.id.to_string())).flatten()?;
            DialogButton::Command(id)
        }
        _ => DialogButton::Ok,
    })
}

/// Focus the dialog's `data-autofocus` element, else its first field, else its first button,
/// selecting the text of a text box so typing replaces it.
fn focus_first(el: &web_sys::HtmlDialogElement) {
    let pick = |sel: &str| el.query_selector(sel).ok().flatten();
    let Some(target) = pick("[data-autofocus]").or_else(|| pick(".wc-dlg-body input, .wc-dlg-body select")).or_else(|| pick(".wc-dlg-body button"))
    else {
        return;
    };
    match target.dyn_into::<web_sys::HtmlInputElement>() {
        Ok(input) => {
            let _ = input.focus();
            if single_line(&input) {
                input.select();
            }
        }
        Err(other) => {
            if let Ok(h) = other.dyn_into::<web_sys::HtmlElement>() {
                let _ = h.focus();
            }
        }
    }
}

/// A fresh element id, so every `<label for>` points at its own field.
fn next_id() -> String {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    format!("wc-dlg-f{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

/// The value of an event's target (`<select>` included, which `input_value` doesn't cover).
fn target_value(ev: &web_sys::Event) -> String {
    ev.target().and_then(|t| js_sys::Reflect::get(&t, &"value".into()).ok()).and_then(|v| v.as_string()).unwrap_or_default()
}

/// A labelled one-line text field.
fn text_field(f: Form, label: &str, lens: Lens<String>) -> impl IntoView + use<> {
    let id = next_id();
    view! {
        <label for=id.clone()>{t(label)}</label>
        <input type="text" id=id spellcheck="false" autocomplete="off"
            prop:value=move || f.shown(lens).unwrap_or_default()
            on:input=move |ev| f.set(lens, input_value(&ev))/>
    }
}

/// Range, step and display of a number field.
#[derive(Clone, Copy)]
struct Num {
    min: f64,
    max: f64,
    step: f64,
    /// Fixed decimal places; 0 shows whole numbers bare and others with up to two places.
    decimals: usize,
    unit: &'static str,
}

const INCHES: Num = Num { min: -22.0, max: 22.0, step: 0.05, decimals: 2, unit: "\"" };
const POINTS: Num = Num { min: 0.0, max: 1584.0, step: 1.0, decimals: 0, unit: "pt" };

/// `v` as a number field shows it (see [`Num::decimals`]).
fn show_num(v: f64, decimals: usize) -> String {
    if !v.is_finite() {
        return String::new();
    }
    let s = if decimals > 0 { format!("{v:.decimals$}") } else { format!("{v:.2}").trim_end_matches('0').trim_end_matches('.').to_string() };
    // Tiny negatives round to "-0": show them as 0.
    match s.strip_prefix('-') {
        Some(rest) if rest.chars().all(|c| c == '0' || c == '.') => rest.to_string(),
        _ => s,
    }
}

/// A typed number clamped to the field's range; `None` for anything that isn't a finite number.
fn parse_num(s: &str, num: Num) -> Option<f64> {
    let v = s.trim().parse::<f64>().ok().filter(|v| v.is_finite())?;
    Some(v.max(num.min).min(num.max))
}

/// A number box (with its unit) for the field `id`. Typing updates the form as soon as the text
/// is a number; leaving the box shows the clamped value.
fn number_input(f: Form, id: String, lens: Lens<f64>, num: Num) -> impl IntoView {
    view! {
        <span class="wc-dlg-num">
            <input type="number" id=id min=num.min max=num.max step=num.step
                prop:value=move || f.shown(lens).map(|v| show_num(v, num.decimals)).unwrap_or_default()
                on:input=move |ev| {
                    if let Some(v) = parse_num(&input_value(&ev), num) {
                        f.set(lens, v);
                    }
                }
                on:change=move |_| f.refresh()/>
            {(!num.unit.is_empty()).then(|| view! { <span class="wc-dlg-unit">{num.unit}</span> })}
        </span>
    }
}

/// A labelled number field.
fn number_field(f: Form, label: &str, lens: Lens<f64>, num: Num) -> impl IntoView + use<> {
    let id = next_id();
    view! {
        <label for=id.clone()>{t(label)}</label>
        {number_input(f, id, lens, num)}
    }
}

/// A checkbox with its label.
fn check(f: Form, label: &str, lens: Lens<bool>) -> impl IntoView + use<> {
    view! {
        <label class="wc-dlg-check">
            <input type="checkbox" prop:checked=move || f.get(lens).unwrap_or(false) on:change=move |ev| f.set(lens, input_checked(&ev))/>
            <span>{t(label)}</span>
        </label>
    }
}

/// One radio button of `group`, setting the field to `value`.
fn radio<T: Copy + PartialEq + Send + Sync + 'static>(f: Form, group: &'static str, label: String, lens: Lens<T>, value: T) -> impl IntoView {
    view! {
        <label class="wc-dlg-check">
            <input type="radio" name=group prop:checked=move || f.get(lens) == Some(value)
                on:change=move |_| {
                    f.set(lens, value);
                    f.refresh();
                }/>
            <span>{label}</span>
        </label>
    }
}

/// A dialog button; `primary` is the filled default button.
fn button(f: Form, label: &str, button: DialogButton, primary: bool) -> impl IntoView + use<> {
    view! {
        <button type="button" class="wc-dlg-btn" class:primary=primary on:click=move |_| f.press(button.clone())>{t(label)}</button>
    }
}

/// The OK (named `ok`) / Cancel pair at the bottom right.
fn ok_cancel(f: Form, ok: &str) -> impl IntoView + use<> {
    view! { <div class="wc-dlg-buttons">{button(f, ok, DialogButton::Ok, true)}{button(f, "Cancel", DialogButton::Cancel, false)}</div> }
}

/// A single Close button (read-only dialogs).
fn close_only(f: Form) -> impl IntoView {
    view! { <div class="wc-dlg-buttons">{button(f, "Close", DialogButton::Ok, true)}</div> }
}

/// A section heading inside a dialog.
fn section(label: &str) -> impl IntoView + use<> {
    view! { <h3 class="wc-dlg-section">{t(label)}</h3> }
}

/// The form for the dialog's kind.
fn body(f: Form, initial: &Dialog) -> AnyView {
    match initial {
        Dialog::Font { .. } => view! {
            <div class="wc-dlg-grid">
                {text_field(f, "Font:", lens!(str Font.font))}
                {text_field(f, "Size:", lens!(str Font.size))}
                {text_field(f, "Font color:", lens!(str Font.color))}
                {number_field(f, "Spacing (pt):", lens!(f32 Font.spacing), Num { min: -20.0, max: 20.0, step: 0.1, decimals: 0, unit: "" })}
            </div>
            {section("Effects")}
            <div class="wc-dlg-checks">
                {check(f, "Bold", lens!(bool Font.bold))}
                {check(f, "Italic", lens!(bool Font.italic))}
                {check(f, "Underline", lens!(bool Font.underline))}
                {check(f, "Strikethrough", lens!(bool Font.strike))}
                {check(f, "Superscript", lens!(bool Font.sup))}
                {check(f, "Subscript", lens!(bool Font.sub))}
                {check(f, "Small caps", lens!(bool Font.small_caps))}
                {check(f, "All caps", lens!(bool Font.caps))}
                {check(f, "Hidden", lens!(bool Font.hidden))}
            </div>
            {ok_cancel(f, "OK")}
        }
        .into_any(),
        Dialog::Paragraph { .. } => view! {
            {section("General")}
            <div class="wc-dlg-grid">{align_select(f)}</div>
            {section("Indentation")}
            <div class="wc-dlg-grid wc-dlg-grid4">
                {number_field(f, "Left:", lens!(f32 Paragraph.left), INCHES)}
                {number_field(f, "Right:", lens!(f32 Paragraph.right), INCHES)}
                {number_field(f, "First line:", lens!(f32 Paragraph.first), INCHES)}
            </div>
            {section("Spacing")}
            <div class="wc-dlg-grid wc-dlg-grid4">
                {number_field(f, "Before:", lens!(f32 Paragraph.before), POINTS)}
                {number_field(f, "Line spacing:", lens!(f32 Paragraph.line), Num { min: 0.5, max: 5.0, step: 0.05, decimals: 2, unit: "" })}
                {number_field(f, "After:", lens!(f32 Paragraph.after), POINTS)}
            </div>
            {section("Line and Page Breaks")}
            <div class="wc-dlg-stack">
                {check(f, "Widow/Orphan control", lens!(bool Paragraph.widow))}
                {check(f, "Keep with next", lens!(bool Paragraph.keep_next))}
                {check(f, "Keep lines together", lens!(bool Paragraph.keep_lines))}
                {check(f, "Page break before", lens!(bool Paragraph.page_break))}
            </div>
            {ok_cancel(f, "OK")}
        }
        .into_any(),
        Dialog::Find { .. } => find_body(f).into_any(),
        Dialog::Goto { .. } => view! {
            <div class="wc-dlg-grid">{text_field(f, "Enter page number:", lens!(str Goto.page))}</div>
            {ok_cancel(f, "Go To")}
        }
        .into_any(),
        Dialog::InsertTable { .. } => {
            let count = |max: f64| Num { min: 1.0, max, step: 1.0, decimals: 0, unit: "" };
            view! {
                <div class="wc-dlg-grid">
                    {number_field(f, "Number of columns:", lens!(u32 InsertTable.cols), count(63.0))}
                    {number_field(f, "Number of rows:", lens!(u32 InsertTable.rows), count(1000.0))}
                </div>
                {ok_cancel(f, "OK")}
            }
            .into_any()
        }
        Dialog::PageSetup { .. } => {
            let margin = Num { min: 0.0, ..INCHES };
            let landscape = lens!(bool PageSetup.landscape);
            view! {
                {section("Margins")}
                <div class="wc-dlg-grid wc-dlg-grid4">
                    {number_field(f, "Top:", lens!(f32 PageSetup.top), margin)}
                    {number_field(f, "Bottom:", lens!(f32 PageSetup.bottom), margin)}
                    {number_field(f, "Left:", lens!(f32 PageSetup.left), margin)}
                    {number_field(f, "Right:", lens!(f32 PageSetup.right), margin)}
                </div>
                {section("Orientation")}
                <div class="wc-dlg-row" role="radiogroup" aria-label=t("Orientation")>
                    {radio(f, "wc-dlg-orientation", t("Portrait"), landscape, false)}
                    {radio(f, "wc-dlg-orientation", t("Landscape"), landscape, true)}
                </div>
                {ok_cancel(f, "OK")}
            }
            .into_any()
        }
        Dialog::Link { .. } => view! {
            <div class="wc-dlg-grid wc-dlg-wide">
                {text_field(f, "Text to display:", lens!(str Link.text))}
                {text_field(f, "Address:", lens!(str Link.url))}
            </div>
            {ok_cancel(f, "OK")}
        }
        .into_any(),
        // Name field to add a bookmark, then the document's bookmarks (click one to go there).
        Dialog::Bookmark { .. } => {
            let marks = f.b.try_with_value(|b| b.select(|s| s.bookmarks.clone()));
            let list = move || {
                marks
                    .map(|m| m.get())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|n| {
                        let label = n.clone();
                        view! {
                            <button type="button" class="wc-dlg-list-item" on:click=move |_| f.press(DialogButton::GotoBookmark(n.clone()))>{label}</button>
                        }
                    })
                    .collect_view()
            };
            view! {
                <div class="wc-dlg-grid">{text_field(f, "Bookmark name:", lens!(str Bookmark.name))}</div>
                <div class="wc-dlg-list" role="list" aria-label=t("Bookmarks")>{list}</div>
                {ok_cancel(f, "Add")}
            }
            .into_any()
        }
        Dialog::WordCount { stats } => {
            let rows = [
                ("Pages", "pages"),
                ("Words", "words"),
                ("Characters (no spaces)", "characters"),
                ("Characters (with spaces)", "charactersWithSpaces"),
                ("Paragraphs", "paragraphs"),
                ("Lines", "lines"),
            ]
            .into_iter()
            .map(|(label, key)| {
                let value = stats.get(key).map(|v| v.to_string()).unwrap_or_default();
                view! { <tr><th scope="row">{t(label)}</th><td>{value}</td></tr> }
            })
            .collect_view();
            view! {
                <table class="wc-dlg-table wc-dlg-stats"><tbody>{rows}</tbody></table>
                {close_only(f)}
            }
            .into_any()
        }
        Dialog::Zoom { .. } => zoom_body(f).into_any(),
        Dialog::Watermark { .. } => {
            let diagonal = lens!(bool Watermark.diagonal);
            view! {
                <div class="wc-dlg-grid">{text_field(f, "Text:", lens!(str Watermark.text))}</div>
                <div class="wc-dlg-row" role="radiogroup" aria-label=t("Custom Watermark")>
                    {radio(f, "wc-dlg-watermark", t("Diagonal"), diagonal, true)}
                    {radio(f, "wc-dlg-watermark", t("Horizontal"), diagonal, false)}
                </div>
                {ok_cancel(f, "OK")}
            }
            .into_any()
        }
        Dialog::NewStyle { .. } => view! {
            <div class="wc-dlg-grid">
                {text_field(f, "Name:", lens!(str NewStyle.name))}
                {text_field(f, "Style based on:", lens!(str NewStyle.based_on))}
            </div>
            <p class="wc-dlg-note">{t("The new style takes the formatting of the current paragraph.")}</p>
            {ok_cancel(f, "OK")}
        }
        .into_any(),
        Dialog::ModifyStyle { .. } => {
            let (before, after) = (next_id(), next_id());
            view! {
                <div class="wc-dlg-grid">
                    {text_field(f, "Name:", lens!(str ModifyStyle.name))}
                    {text_field(f, "Font:", lens!(str ModifyStyle.font))}
                    {number_field(f, "Size:", lens!(f32 ModifyStyle.size), Num { min: 1.0, max: 1638.0, step: 0.5, decimals: 0, unit: "pt" })}
                    {text_field(f, "Color:", lens!(str ModifyStyle.color))}
                    <label for=before.clone()>{t("Space before/after:")}</label>
                    <span class="wc-dlg-pair">
                        {number_input(f, before, lens!(f32 ModifyStyle.before), POINTS)}
                        <label class="wc-sr-only" for=after.clone()>{t("After:")}</label>
                        {number_input(f, after, lens!(f32 ModifyStyle.after), POINTS)}
                    </span>
                </div>
                <div class="wc-dlg-row">
                    {check(f, "Bold", lens!(bool ModifyStyle.bold))}
                    {check(f, "Italic", lens!(bool ModifyStyle.italic))}
                </div>
                {ok_cancel(f, "OK")}
            }
            .into_any()
        }
        Dialog::Commands { .. } => commands_body(f).into_any(),
        Dialog::About { .. } => about_body(f).into_any(),
    }
}

/// Paragraph ▸ Alignment. A value outside the four usual ones (`distribute`) stays selectable.
fn align_select(f: Form) -> impl IntoView {
    let lens = lens!(str Paragraph.align);
    let id = next_id();
    let current = f.d.with_untracked(|d| (lens.get)(d)).unwrap_or_default();
    let mut opts: Vec<(String, String)> = [("left", "Left"), ("center", "Center"), ("right", "Right"), ("justify", "Justify")]
        .into_iter()
        .map(|(v, l)| (v.to_string(), t(l)))
        .collect();
    if !current.is_empty() && !opts.iter().any(|(v, _)| *v == current) {
        opts.push((current.clone(), current));
    }
    let options = opts
        .into_iter()
        .map(|(value, label)| {
            let v = value.clone();
            view! { <option value=value prop:selected=move || f.get(lens).is_some_and(|a| a == v)>{label}</option> }
        })
        .collect_view();
    view! {
        <label for=id.clone()>{t("Alignment")}</label>
        <select id=id on:change=move |ev| f.set(lens, target_value(&ev))>{options}</select>
    }
}

/// Find and Replace: Find | Replace tabs, the fields, options, the last message and the buttons.
fn find_body(f: Form) -> impl IntoView {
    let mode = lens!(bool Find.replace_mode);
    let replace = Memo::new(move |_| f.get(mode).unwrap_or(false));
    let message = Memo::new(move |_| {
        f.d.with(|d| match d {
            Dialog::Find { message, .. } => message.clone(),
            _ => String::new(),
        })
    });
    let tab = move |label: &str, on: bool| {
        view! {
            <button type="button" role="tab" class="wc-dlg-tab" class:active=move || replace.get() == on
                aria-selected=move || (replace.get() == on).to_string() on:click=move |_| f.set(mode, on)>{t(label)}</button>
        }
    };
    view! {
        <div class="wc-dlg-tabs" role="tablist">{tab("Find", false)}{tab("Replace", true)}</div>
        <div class="wc-dlg-grid wc-dlg-wide">
            {text_field(f, "Find what:", lens!(str Find.query))}
            {move || replace.get().then(|| text_field(f, "Replace with:", lens!(str Find.replace)))}
        </div>
        <div class="wc-dlg-row">
            {check(f, "Match case", lens!(bool Find.match_case))}
            {check(f, "Whole words", lens!(bool Find.whole_word))}
            {check(f, "Wildcards (regex)", lens!(bool Find.regex))}
        </div>
        <p class="wc-dlg-message" role="status" aria-live="polite">{move || message.get()}</p>
        <div class="wc-dlg-buttons">
            {move || replace.get().then(|| view! {
                {button(f, "Replace All", DialogButton::ReplaceAll, false)}
                {button(f, "Replace", DialogButton::Replace, false)}
            })}
            {button(f, "Find Next", DialogButton::FindNext, true)}
            {button(f, "Close", DialogButton::Cancel, false)}
        </div>
    }
}

/// Zoom: preset radios, a 10–500 % box and slider, and the fit buttons.
fn zoom_body(f: Form) -> impl IntoView {
    let pct = lens!(f32 Zoom.percent);
    let num = Num { min: 10.0, max: 500.0, step: 1.0, decimals: 0, unit: "%" };
    let presets = [200.0, 100.0, 75.0].into_iter().map(|p| radio(f, "wc-dlg-zoom", format!("{p}%"), pct, p)).collect_view();
    let fit = |label: &str, value: &str| button(f, label, DialogButton::Fit(value.to_string()), false);
    view! {
        <div class="wc-dlg-row" role="radiogroup" aria-label=t("Zoom")>{presets}</div>
        <div class="wc-dlg-grid">{number_field(f, "Percent:", pct, num)}</div>
        <input type="range" class="wc-dlg-range" min=num.min max=num.max step="1" aria-label=t("Zoom")
            prop:value=move || f.get(pct).map(|v| show_num(v, 0)).unwrap_or_default()
            on:input=move |ev| {
                if let Some(v) = parse_num(&input_value(&ev), num) {
                    f.set(pct, v);
                    f.refresh();
                }
            }/>
        <div class="wc-dlg-row">
            {fit("Page width", "pageWidth")}
            {fit("Whole page", "onePage")}
            {fit("Many pages", "multiplePages")}
        </div>
        {ok_cancel(f, "OK")}
    }
}

/// Command search: a search box and up to 14 matching commands; Enter or a click runs one.
fn commands_body(f: Form) -> impl IntoView {
    let q = lens!(str Commands.query);
    let query = Memo::new(move |_| f.get(q).unwrap_or_default());
    let id = next_id();
    let hits = move || {
        let query = query.get();
        f.b.try_with_value(|b| {
            command_hits(&b.registry, &query, 14)
                .into_iter()
                .map(|c| {
                    let cmd = c.id.to_string();
                    let shortcut = b.shortcut(c.id);
                    view! {
                        <button type="button" class="wc-cmd-hit" role="option" on:click=move |_| f.press(DialogButton::Command(cmd.clone()))
                            on:keydown=move |ev| hit_keys(&ev)>
                            <span class="wc-cmd-label">{tl!(c.label).to_string()}</span>
                            <span class="wc-cmd-loc">{wordcraft_ui_egui::i18n::location(c.location)}</span>
                            <span class="wc-cmd-shortcut">{shortcut}</span>
                        </button>
                    }
                })
                .collect_view()
        })
    };
    view! {
        <label class="wc-sr-only" for=id.clone()>{t("Search Commands")}</label>
        <input type="search" class="wc-dlg-search" id=id data-autofocus="true" spellcheck="false" autocomplete="off"
            placeholder=t("Type a command, e.g. \"insert table\"")
            prop:value=move || f.shown(q).unwrap_or_default()
            on:input=move |ev| f.set(q, input_value(&ev))
            on:keydown=move |ev: web_sys::KeyboardEvent| {
                if ev.key() == "ArrowDown" {
                    ev.prevent_default();
                    focus_selector(".wc-dialog .wc-cmd-hit");
                }
            }/>
        <div class="wc-cmd-hits" role="listbox" aria-label=t("Search Commands")>{hits}</div>
    }
}

/// Arrow keys move between command hits; Up from the first goes back to the search box.
fn hit_keys(ev: &web_sys::KeyboardEvent) {
    let down = match ev.key().as_str() {
        "ArrowDown" => true,
        "ArrowUp" => false,
        _ => return,
    };
    ev.prevent_default();
    let Some(el) = ev.current_target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) else { return };
    let next = if down { el.next_element_sibling() } else { el.previous_element_sibling() };
    match next.and_then(|n| n.dyn_into::<web_sys::HtmlElement>().ok()) {
        Some(n) => {
            let _ = n.focus();
        }
        None if !down => focus_selector(".wc-dialog .wc-dlg-search"),
        None => {}
    }
}

/// About WordCraft: About / Contributors / Models tabs.
fn about_body(f: Form) -> impl IntoView {
    let tab = Memo::new(move |_| {
        f.d.with(|d| match d {
            Dialog::About { tab } => *tab,
            _ => 0,
        })
    });
    let tabs = ["About", "Contributors", "Models"]
        .into_iter()
        .zip(0u8..)
        .map(|(label, i)| {
            view! {
                <button type="button" role="tab" class="wc-dlg-tab" class:active=move || tab.get() == i
                    aria-selected=move || (tab.get() == i).to_string()
                    on:click=move |_| {
                        let _ = f.d.try_update(|d| {
                            if let Dialog::About { tab } = d {
                                *tab = i;
                            }
                        });
                    }>{t(label)}</button>
            }
        })
        .collect_view();
    view! {
        <div class="wc-dlg-tabs" role="tablist">{tabs}</div>
        <div class="wc-about-page">
            {move || match tab.get() {
                1 => contributors_view().into_any(),
                2 => models_view().into_any(),
                _ => about_view().into_any(),
            }}
        </div>
        {close_only(f)}
    }
}

/// About ▸ About: name, version, description and links.
fn about_view() -> impl IntoView {
    let build = option_env!("WORDCRAFT_BUILD_SHA").map(|s| s.get(..8).unwrap_or(s)).unwrap_or(tl!("development build"));
    let version = wordcraft_ui_egui::i18n::fmt(tl!("Version {version} ({build})"), &[("version", env!("CARGO_PKG_VERSION")), ("build", build)]);
    let link = |label: &str, href: &'static str| {
        view! { <a href=href target="_blank" rel="noopener">{t(label)}</a> }
    };
    view! {
        <h3 class="wc-about-name">{t("WordCraft")}</h3>
        <p>{version}</p>
        <p class="wc-about-text">{t("A free, open-source word processor written from scratch in Rust.\nPart of the Crafting Apps from the ArtCraft team.")}</p>
        <div class="wc-about-links">
            {link("getartcraft.com/apps/wordcraft", "https://getartcraft.com/apps/wordcraft")}
            {link("Join us on Discord: discord.gg/artcraft", "https://discord.gg/artcraft")}
            {link("Source code: github.com/storytold/wordcraft", "https://github.com/storytold/wordcraft")}
        </div>
        <p class="wc-dlg-note">{t("MIT OR Apache-2.0. Copyright (c) 2026 ArtCraft Team and the WordCraft contributors.")}</p>
    }
}

/// About ▸ Contributors: which name to show, the sort, and everyone as a grab bag of links.
fn contributors_view() -> impl IntoView {
    let names = RwSignal::new(NameMode::Username);
    let key = RwSignal::new(SortKey::FirstCommit);
    let ascending = RwSignal::new(SortKey::FirstCommit.default_ascending());
    let sort_id = next_id();
    let modes = NameMode::ALL
        .into_iter()
        .map(|m| {
            view! {
                <button type="button" class="wc-dlg-seg" class:active=move || names.get() == m aria-pressed=move || (names.get() == m).to_string()
                    on:click=move |_| names.set(m)>{t(m.label())}</button>
            }
        })
        .collect_view();
    let sorts = SortKey::ALL
        .into_iter()
        .enumerate()
        .map(|(i, k)| view! { <option value=i.to_string() prop:selected=move || key.get() == k>{t(k.label().0)}</option> })
        .collect_view();
    let list = move || {
        let list = credits::sorted(CONTRIBUTORS, names.get(), key.get(), ascending.get());
        if list.is_empty() {
            return view! { <p class="wc-dlg-note">{t("No contributor data was built into this copy.")}</p> }.into_any();
        }
        let mode = names.get();
        list.into_iter()
            .enumerate()
            .map(|(i, c)| {
                view! {
                    {(i > 0).then(|| view! { <span class="wc-credits-sep">"·"</span> })}
                    <a href=format!("https://github.com/{}", c.login) target="_blank" rel="noopener" title=c.summary()>{c.name(mode)}</a>
                }
            })
            .collect_view()
            .into_any()
    };
    view! {
        <div class="wc-credits-bar">
            <span>{t("Show")}</span>
            <span class="wc-dlg-segs" role="group" aria-label=t("Show")>{modes}</span>
            <label for=sort_id.clone()>{t("Sort")}</label>
            <select id=sort_id on:change=move |ev| {
                let picked = target_value(&ev).parse::<usize>().ok().and_then(|i| SortKey::ALL.get(i).copied());
                if let Some(k) = picked {
                    key.set(k);
                    ascending.set(k.default_ascending());
                }
            }>{sorts}</select>
            <button type="button" class="wc-dlg-btn wc-dlg-small" title=t("Reverse the order") aria-label=t("Reverse the order")
                on:click=move |_| ascending.update(|a| *a = !*a)>{move || if ascending.get() { "▲" } else { "▼" }}</button>
        </div>
        <p class="wc-dlg-note">{format!("{} contributors · {} commits", CONTRIBUTORS.len(), group(TOTAL_COMMITS))}</p>
        <div class="wc-credits-bag">{list}</div>
    }
}

/// About ▸ Models: AI models credited in Co-Authored-By trailers.
fn models_view() -> impl IntoView {
    if MODELS.is_empty() {
        return view! { <p class="wc-dlg-note">{t("No model credits were built into this copy.")}</p> }.into_any();
    }
    let assisted = MODELS.iter().map(|m| m.commits).max().unwrap_or(0).max(1);
    let total = TOTAL_COMMITS.max(assisted) as f64;
    let head =
        ["Company", "Model", "Version", "Commits", "% of all commits", "Lines +/−"].into_iter().map(|h| view! { <th>{t(h)}</th> }).collect_view();
    let rows = MODELS
        .iter()
        .map(|m| {
            view! {
                <tr>
                    <td>{m.company}</td>
                    <td>{m.model}</td>
                    <td>{m.version}</td>
                    <td class="num">{group(m.commits)}</td>
                    <td class="num">{format!("{:.0}%", 100.0 * m.commits as f64 / total)}</td>
                    <td class="num">{format!("+{} / −{}", group(m.lines_added), group(m.lines_deleted))}</td>
                </tr>
            }
        })
        .collect_view();
    view! {
        <div class="wc-dlg-scroll">
            <table class="wc-dlg-table wc-dlg-striped"><thead><tr>{head}</tr></thead><tbody>{rows}</tbody></table>
        </div>
    }
    .into_any()
}

/// `1234567` → `1,234,567`.
fn group(n: u64) -> String {
    let s = n.to_string();
    let len = s.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && len.saturating_sub(i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}
