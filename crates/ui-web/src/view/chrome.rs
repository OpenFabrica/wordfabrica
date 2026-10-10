//! Title bar (File, AutoSave, Quick Access Toolbar, title, search, account), the tab strip, and
//! the status bar.

use leptos::prelude::*;
use serde_json::json;
use wordfabrica_ui_egui::{Action, tl};

use super::{icon_svg, icon_svg_in, input_value, keep_focus, menus, t};
use crate::bridge::{Bridge, MenuSource};
use crate::model::TABS;

/// One Quick Access Toolbar button.
fn qat<L: Fn() -> String + Send + Sync + 'static>(
    b: &Bridge,
    icon: &'static str,
    label: L,
    id: &'static str,
    enabled: Memo<bool>,
    dark: Memo<bool>,
) -> impl IntoView + use<L> {
    let b2 = b.clone();
    let b3 = b.clone();
    let label = std::sync::Arc::new(label);
    let l2 = label.clone();
    view! {
        <button class="wc-qat" title=move || b2.tooltip(&label(), id) aria-label=move || l2() disabled=move || !enabled.get()
            on:mousedown=keep_focus on:click=move |_| b3.run(id, json!({}))
            inner_html=move || icon_svg(icon, 16.0, dark.get(), enabled.get())>
        </button>
    }
}

pub fn title_bar(b: Bridge) -> impl IntoView {
    let dark = b.select(|s| s.dark);
    let can_undo = b.select(|s| s.can_undo);
    let can_redo = b.select(|s| s.can_redo);
    let undo_label = b.select(|s| s.undo_label.clone());
    let autosave = b.select(|s| (s.autosave, s.autosave_ready));
    let title = b.select(|s| if s.title_status.is_empty() { s.title.clone() } else { format!("{} — {}", s.title, tl!(&s.title_status)) });
    let author = b.select(|s| s.author.clone());
    let community = b.select(|s| s.show_community);
    let always = Memo::new(|_| true);
    let (b1, b2, b3, b4) = (b.clone(), b.clone(), b.clone(), b.clone());
    let initials = move || author.get().split_whitespace().filter_map(|w| w.chars().next()).take(2).collect::<String>();
    view! {
        <div class="wc-titlebar-left">
            <button class="wc-file-button" on:mousedown=keep_focus on:click=move |_| b1.send(Action::run("ui.tab", json!({"tab": "File"})))>
                <span class="wc-icon16" inner_html=move || icon_svg("menu", 16.0, dark.get(), true)></span>
                <span>{t("File")}</span>
            </button>
            <span class="wc-divider"></span>
            <span class="wc-autosave-label">{t("AutoSave")}</span>
            <button class="wc-switch" role="switch" class:on=move || autosave.get().0 aria-checked=move || autosave.get().0.to_string()
                aria-label=t("AutoSave") title=t("AutoSave saves every change to the file (needs a saved document)")
                on:mousedown=keep_focus on:click=move |_| b2.send(Action::ToggleAutosave)>
                <span class="wc-knob"></span>
            </button>
            {qat(&b, "save", || t("Save"), "file.save", always, dark)}
            {qat(&b, "undo", move || wordfabrica_ui_egui::i18n::prefixed("Undo", &undo_label.get()), "edit.undo", can_undo, dark)}
            {qat(&b, "redo", || t("Redo"), "edit.redo", can_redo, dark)}
        </div>
        <div class="wc-title" role="heading" attr:aria-level="1">{move || title.get()}</div>
        <div class="wc-titlebar-right">
            <button class="wc-search" on:mousedown=keep_focus on:click=move |_| b3.send(Action::run("ui.dialog", json!({"name": "commands"})))>
                <span class="wc-icon16" inner_html=move || icon_svg("search", 16.0, dark.get(), true)></span>
                <span>{t("Search commands and help")}</span>
            </button>
            {move || community.get().then(|| {
                let b = b4.clone();
                view! {
                    <button class="wc-community" title=t("Join the OpenFabrica community on GitHub") on:mousedown=keep_focus
                        on:click=move |_| b.send(Action::run("ui.community", json!({})))>
                        <span class="wc-icon14" inner_html=icon_svg_in("community", 14.0, egui::Color32::WHITE)></span>
                        <span>{t("Community")}</span>
                    </button>
                }
            })}
            <span class="wc-avatar" title=move || format!("{} — set your name in File › Options", author.get())>{initials}</span>
        </div>
    }
}

/// Ribbon tabs (with the table tabs while the caret is in a table), then one button per plugin,
/// Comments, the Editing/Reviewing mode and Share.
pub fn tab_strip(b: Bridge) -> impl IntoView {
    let tab = b.select(|s| s.tab.clone());
    let backstage = b.select(|s| s.backstage);
    let in_table = b.select(|s| s.in_table);
    let track = b.select(|s| s.status.track_changes);
    let read_only = b.select(|s| s.status.read_only);
    let dark = b.select(|s| s.dark);
    let b1 = b.clone();
    let tabs = move || {
        TABS.iter()
            .filter(|tb| !tb.contextual || in_table.get())
            .map(|tb| {
                let name = tb.name;
                let b = b1.clone();
                let b2 = b1.clone();
                let active = move || tab.get() == name && !backstage.get();
                view! {
                    <button class="wc-tab" role="tab" class:contextual=tb.contextual class:active=active aria-selected=move || active().to_string()
                        on:mousedown=keep_focus
                        on:click=move |_| b.send(Action::run("ui.tab", json!({"tab": name})))
                        on:dblclick=move |_| b2.send(Action::run("ui.collapseRibbon", json!({})))>
                        {t(name)}
                    </button>
                }
            })
            .collect_view()
    };
    let (b2, b3, b4) = (b.clone(), b.clone(), b.clone());
    view! {
        <div class="wc-tabs" role="tablist">{tabs}</div>
        <div class="wc-tabs-right">
            {super::plugins::chips(b.clone())}
            <button class="wc-chip" on:mousedown=keep_focus on:click=move |_| b2.run("view.commentsPane", json!({}))>
                <span class="wc-icon14" inner_html=move || icon_svg("newComment", 14.0, dark.get(), true)></span>
                {t("Comments")}
            </button>
            // Read-only (view link): a plain label instead of the Editing/Reviewing menu.
            <button class="wc-chip" class:read-only=read_only aria-haspopup="menu" disabled=read_only
                title=move || read_only.get().then(|| t("This document is open read-only"))
                on:mousedown=keep_focus on:click=move |ev| menus::toggle(&b3, "editMode", &ev, MenuSource::EditMode)>
                {move || if read_only.get() {
                    format!("🔒 {}", t("Read only"))
                } else {
                    format!("✎ {} ▾", if track.get() { t("Reviewing") } else { t("Editing") })
                }}
            </button>
            <button class="wc-share" title=t("Export a copy to share (PDF, Word document)") on:mousedown=keep_focus
                on:click=move |_| b4.send(Action::run("ui.backstage", json!({"value": true, "page": "export"})))>
                <span class="wc-icon14" inner_html=icon_svg_in("share", 14.0, egui::Color32::WHITE)></span>
                {t("Share")}
            </button>
        </div>
    }
}

/// Zoom slider position (0–1000) for a zoom percentage, logarithmic between 10% and 500%.
fn zoom_to_slider(pct: f32) -> f32 {
    ((pct.clamp(10.0, 500.0) / 10.0).ln() / 50f32.ln() * 1000.0).round()
}

fn slider_to_zoom(v: f32) -> f32 {
    (10.0 * 50f32.powf(v.clamp(0.0, 1000.0) / 1000.0)).round()
}

pub fn status_bar(b: Bridge) -> impl IntoView {
    let status = b.select(|s| s.status.clone());
    let dark = b.select(|s| s.dark);
    let toggles = b.select(|s| (s.toggles.get("webLayout").copied(), s.toggles.get("printLayout").copied(), s.toggles.get("readMode").copied()));
    // Show each status message for six seconds.
    let hidden_seq = RwSignal::new(0u64);
    Effect::new(move |_| {
        let seq = status.with(|s| s.message_seq);
        set_timeout(move || hidden_seq.set(seq), std::time::Duration::from_secs(6));
    });
    let (b1, b2, b3, b4, b5, b6) = (b.clone(), b.clone(), b.clone(), b.clone(), b.clone(), b.clone());
    let pages = move || {
        let s = status.get();
        wordfabrica_ui_egui::i18n::fmt(tl!("Page {page} of {pages}"), &[("page", &s.page.to_string()), ("pages", &s.pages.to_string())])
    };
    let words = move || {
        let s = status.get();
        match s.selected_words {
            Some(sel) => {
                wordfabrica_ui_egui::i18n::fmt(tl!("{selected} of {words} words"), &[("selected", &sel.to_string()), ("words", &s.words.to_string())])
            }
            None => wordfabrica_ui_egui::i18n::fmt(tl!("{words} words"), &[("words", &s.words.to_string())]),
        }
    };
    let language = move || {
        status.with(|s| match &s.doc_lang {
            Some(code) => wordfabrica_ui_egui::language::label(code).to_string(),
            None => t("Multiple languages"),
        })
    };
    let view_button =
        |icon: &'static str, id: &'static str, label: &'static str, on: fn(&(Option<bool>, Option<bool>, Option<bool>)) -> bool, b: Bridge| {
            view! {
                <button class="wc-status-icon" class:checked=move || on(&toggles.get()) aria-pressed=move || on(&toggles.get()).to_string()
                    title=t(label) aria-label=t(label) on:mousedown=keep_focus on:click=move |_| b.run(id, json!({}))
                    inner_html=move || icon_svg(icon, 16.0, dark.get(), true)>
                </button>
            }
        };
    view! {
        <div class="wc-status-left">
            <button class="wc-status-item" title=t("Go To (⌘⌥G)") on:mousedown=keep_focus on:click=move |_| b1.send(Action::run("ui.dialog", json!({"name": "goto"}))) >{pages}</button>
            <button class="wc-status-item" on:mousedown=keep_focus on:click=move |_| b2.send(Action::run("ui.dialog", json!({"name": "wordCount"})))>{words}</button>
            {move || status.with(|s| s.header_footer).then(|| view! { <span class="wc-status-item">{t("Editing header/footer")}</span> })}
            <button class="wc-status-item" aria-haspopup="menu" title=t("Document language") on:mousedown=keep_focus
                on:click=move |ev| menus::toggle(&b3, "statusLanguage", &ev, MenuSource::Language)>{language}</button>
            {move || status.with(|s| s.track_changes).then(|| view! { <span class="wc-status-item">{t("Track Changes: On")}</span> })}
            {move || status.with(|s| s.hosted.clone()).map(|h| {
                let text = if h.revision > 0 { format!("{} · v{}", tl!(&h.text), h.revision) } else { tl!(&h.text).to_string() };
                view! { <span class="wc-status-item wc-save-state" class:failed=h.failed role="status">{text}</span> }
            })}
            {move || {
                let (msg, seq) = status.with(|s| (s.message.clone(), s.message_seq));
                msg.filter(|_| seq != hidden_seq.get()).map(|m| view! { <span class="wc-status-message" role="status">{tl!(&m).to_string()}</span> })
            }}
        </div>
        <div class="wc-status-right">
            <button class="wc-status-item wc-focus" on:mousedown=keep_focus on:click=move |_| b4.run("view.focus", json!({}))>
                <span class="wc-icon16" inner_html=move || icon_svg("focus", 16.0, dark.get(), true)></span>
                {t("Focus")}
            </button>
            {view_button("readMode", "view.readMode", "Read Mode", |t| t.2.unwrap_or(false), b.clone())}
            {view_button("printLayout", "view.printLayout", "Print Layout", |t| t.1.unwrap_or(false), b.clone())}
            {view_button("webLayout", "view.webLayout", "Web Layout", |t| t.0.unwrap_or(false), b.clone())}
            <button class="wc-status-icon" title=t("Zoom Out") aria-label=t("Zoom Out") on:mousedown=keep_focus on:click=move |_| b5.run("view.zoomOut", json!({}))
                inner_html=move || icon_svg("minus", 14.0, dark.get(), true)></button>
            <input class="wc-zoom" type="range" min="0" max="1000" aria-label=t("Zoom")
                prop:value=move || zoom_to_slider(status.with(|s| s.zoom_percent)).to_string()
                on:change={
                    let b = b.clone();
                    move |ev| {
                        if let Ok(v) = input_value(&ev).parse::<f32>() {
                            b.run("view.zoom", json!({"value": slider_to_zoom(v)}));
                        }
                    }
                }/>
            <button class="wc-status-icon" title=t("Zoom In") aria-label=t("Zoom In") on:mousedown=keep_focus on:click=move |_| b6.run("view.zoomIn", json!({}))
                inner_html=move || icon_svg("plus", 14.0, dark.get(), true)></button>
            <button class="wc-status-item wc-zoom-pct" on:mousedown=keep_focus on:click={
                let b = b.clone();
                move |_| b.send(Action::run("ui.dialog", json!({"name": "zoom"})))
            }>{move || format!("{}%", status.with(|s| s.zoom_percent))}</button>
        </div>
    }
}
