//! Dropdown menus: one at a time, positioned under the control that opened it, closed by a click
//! outside, Escape, or picking an entry.

use leptos::prelude::*;
use serde_json::json;
use wordfabrica_ui_egui::Action;
use wordfabrica_ui_egui::host::ColorSlot;

use super::{keep_focus, t};
use crate::bridge::{Bridge, MenuSource, OpenMenu, Preview};
use crate::model::{Item, STANDARD_COLORS, Slot, is_checked, theme_color_rows};

/// Open (or, when it's already open, close) the menu `key` under the clicked control.
pub fn toggle(b: &Bridge, key: &str, ev: &web_sys::MouseEvent, source: MenuSource) {
    if b.menu.with_untracked(|m| m.as_ref().is_some_and(|m| m.key == key)) {
        b.menu.set(None);
        return;
    }
    let (left, top, width) = super::anchor_of(ev);
    b.menu.set(Some(OpenMenu { key: key.to_string(), left, top, width, source }));
}

/// Close the menu and run a command with focus going back to the document.
fn pick(b: &Bridge, id: &str, params: serde_json::Value) {
    b.menu.set(None);
    b.run(id, params);
}

fn slot_action(slot: Slot, hex: &str) -> Action {
    let slot = match slot {
        Slot::Highlight => ColorSlot::Highlight,
        Slot::FontColor => ColorSlot::FontColor,
        Slot::Shading => ColorSlot::Shading,
    };
    Action::LastColor { slot, hex: hex.to_string() }
}

/// The open menu, if any, with a transparent backdrop that closes it.
pub fn menu_host(b: Bridge) -> impl IntoView {
    let menu = b.menu.clone();
    move || {
        let m = menu.get()?;
        let b = b.clone();
        let close = b.menu.clone();
        let style = format!("left:{}px;top:{}px;min-width:{}px", m.left.max(0.0), m.top, m.width.max(160.0));
        let body = match m.source {
            MenuSource::Items(items) => items_view(b.clone(), items).into_any(),
            MenuSource::Fonts => fonts_view(b.clone()).into_any(),
            MenuSource::Sizes => sizes_view(b.clone()).into_any(),
            MenuSource::Language => language_view(b.clone()).into_any(),
            MenuSource::EditMode => edit_mode_view(b.clone()).into_any(),
        };
        Some(view! {
            <div class="wc-menu-backdrop" on:mousedown=move |ev| { ev.prevent_default(); close.set(None); }></div>
            <div class="wc-menu" role="menu" style=style on:mousedown=keep_focus>{body}</div>
        })
    }
}

/// A plain menu entry: label, shortcut, check mark, greyed when disabled.
fn entry(b: &Bridge, label: String, id: &'static str, params: serde_json::Value, checked: bool) -> impl IntoView + use<> {
    let disabled = b.disabled(id) && !id.starts_with("ui.");
    let sc = b.shortcut(id);
    let b = b.clone();
    view! {
        <button class="wc-menu-item" role="menuitem" disabled=disabled on:click=move |_| pick(&b, id, params.clone())>
            <span class="wc-check">{if checked { "✓" } else { "" }}</span>
            <span class="wc-menu-label">{label}</span>
            <span class="wc-menu-shortcut">{sc}</span>
        </button>
    }
}

fn items_view(b: Bridge, items: &'static [Item]) -> impl IntoView {
    let st = b.state.get_untracked();
    items
        .iter()
        .map(|item| -> AnyView {
            match *item {
                Item::Cmd(label, cmd) => entry(&b, t(label), cmd.id, cmd.params(&st), false).into_any(),
                Item::Check(label, cmd, key) => entry(&b, t(label), cmd.id, cmd.params(&st), is_checked(&st, key)).into_any(),
                Item::Sep => view! { <hr class="wc-menu-sep"/> }.into_any(),
                Item::Header(h) => view! { <div class="wc-menu-header">{t(h)}</div> }.into_any(),
                Item::Sub(h, sub) => view! {
                    <div class="wc-menu-header">{t(h)}</div>
                    <div class="wc-menu-sub">{items_view(b.clone(), sub)}</div>
                }
                .into_any(),
                Item::Colors { id, slot } => colors_view(b.clone(), id, slot).into_any(),
                Item::Highlights => highlights_view(b.clone()).into_any(),
                Item::Glyphs { id, key, chars } => {
                    let b = b.clone();
                    let cells = chars
                        .iter()
                        .map(|ch| {
                            let b = b.clone();
                            let ch = *ch;
                            view! { <button class="wc-glyph" on:click=move |_| pick(&b, id, json!({key: ch}))>{ch}</button> }
                        })
                        .collect_view();
                    view! { <div class="wc-glyphs">{cells}</div> }.into_any()
                }
                Item::TableGrid => table_grid(b.clone()).into_any(),
                Item::Language => language_view(b.clone()).into_any(),
                Item::Themes => wordfabrica_engine::cmd::design::THEMES
                    .iter()
                    .map(|(name, ..)| entry(&b, t(name), "design.theme", json!({"name": name}), false))
                    .collect_view()
                    .into_any(),
                Item::ThemeColors => wordfabrica_engine::cmd::design::THEMES
                    .iter()
                    .map(|(name, ..)| entry(&b, t(name), "design.themeColors", json!({"name": name}), false))
                    .collect_view()
                    .into_any(),
                Item::ThemeFonts => wordfabrica_engine::cmd::design::THEMES
                    .iter()
                    .map(|(name, h, body, _)| {
                        entry(&b, format!("{}: {h} / {body}", t(name)), "design.themeFonts", json!({"heading": h, "body": body}), false)
                    })
                    .collect_view()
                    .into_any(),
                Item::PaperSizes => wordfabrica_geom::PAPER_SIZES
                    .iter()
                    .map(|(n, w, h)| entry(&b, format!("{n}   {:.2}\" × {:.2}\"", w / 72.0, h / 72.0), "layout.size", json!({"name": n}), false))
                    .collect_view()
                    .into_any(),
                Item::ListLevels => (0..5u64)
                    .map(|lv| entry(&b, t(&format!("Change to Level {}", lv + 1)), "para.listLevel", json!({"level": lv}), false))
                    .collect_view()
                    .into_any(),
            }
        })
        .collect_view()
}

/// Theme colours with tints, then the standard colours.
fn colors_view(b: Bridge, id: &'static str, slot: Option<Slot>) -> impl IntoView {
    let theme = b.state.with_untracked(|s| s.theme_colors.clone());
    let swatch = move |hex: String| {
        let b = b.clone();
        let style = format!("background:#{hex}");
        let title = format!("#{hex}");
        view! {
            <button class="wc-swatch" style=style title=title on:click=move |_| {
                if let Some(slot) = slot {
                    b.send(slot_action(slot, &hex));
                }
                pick(&b, id, json!({"color": hex.clone()}));
            }></button>
        }
    };
    let rows = theme_color_rows(&theme)
        .into_iter()
        .enumerate()
        .map(|(i, row)| {
            let class = if i == 0 { "wc-swatch-row wc-swatch-base" } else { "wc-swatch-row" };
            view! { <div class=class>{row.into_iter().map(swatch.clone()).collect_view()}</div> }
        })
        .collect_view();
    let standard = STANDARD_COLORS.iter().map(|h| swatch.clone()(h.to_string())).collect_view();
    view! {
        <div class="wc-colors">
            <div class="wc-menu-header">{t("Theme Colors")}</div>
            {rows}
            <div class="wc-menu-header">{t("Standard Colors")}</div>
            <div class="wc-swatch-row">{standard}</div>
        </div>
    }
}

/// The highlighter colours.
fn highlights_view(b: Bridge) -> impl IntoView {
    let cells = wordfabrica_doc::props::Highlight::ALL
        .iter()
        .skip(1)
        .map(|h| {
            let b = b.clone();
            let c = h.rgb().unwrap_or(wordfabrica_doc::Rgb::WHITE);
            let style = format!("background:#{}", c.hex());
            let name = h.name().to_string();
            let ooxml = h.ooxml().to_string();
            view! {
                <button class="wc-swatch wc-swatch-big" style=style title=name on:click=move |_| {
                    b.send(slot_action(Slot::Highlight, &ooxml));
                    pick(&b, "format.highlight", json!({"color": ooxml.clone()}));
                }></button>
            }
        })
        .collect_view();
    view! { <div class="wc-highlights">{cells}</div> }
}

/// Insert ▸ Table: hover over the grid to size the table, click to insert it.
fn table_grid(b: Bridge) -> impl IntoView {
    let hover = RwSignal::new((0usize, 0usize));
    let label = move || {
        let (r, c) = hover.get();
        if r > 0 { format!("{c}x{r} {}", t("Table")) } else { t("Insert Table") }
    };
    let cells = (1..=8usize)
        .flat_map(|r| (1..=10usize).map(move |c| (r, c)))
        .map(|(r, c)| {
            let b = b.clone();
            view! {
                <button
                    class="wc-grid-cell"
                    class:on=move || { let (hr, hc) = hover.get(); r <= hr && c <= hc }
                    on:mouseenter=move |_| hover.set((r, c))
                    on:click=move |_| pick(&b, "insert.table", json!({"rows": r, "cols": c}))
                ></button>
            }
        })
        .collect_view();
    view! {
        <div class="wc-menu-header">{label}</div>
        <div class="wc-table-grid" on:mouseleave=move |_| hover.set((0, 0))>{cells}</div>
    }
}

/// Document language for the whole document (status bar and Review ▸ Language).
fn language_view(b: Bridge) -> impl IntoView {
    let current = b.state.with_untracked(|s| s.status.default_lang.clone());
    let opts = ["zh-CN", "en-US"]
        .into_iter()
        .map(|code| {
            entry(
                &b,
                wordfabrica_ui_egui::language::label(code).to_string(),
                "review.language",
                json!({"lang": code, "scope": "document"}),
                current == code,
            )
        })
        .collect_view();
    view! {
        <div class="wc-menu-header">{t("Document language")}</div>
        <div class="wc-menu-note">{t("Apply to the entire document")}</div>
        <hr class="wc-menu-sep"/>
        {opts}
    }
}

/// Editing (change the document) or Reviewing (track changes).
fn edit_mode_view(b: Bridge) -> impl IntoView {
    let track = b.state.with_untracked(|s| s.status.track_changes);
    view! {
        {entry(&b, t("Editing — edit the document directly"), "review.trackChanges", json!({"value": false}), !track)}
        {entry(&b, t("Reviewing — edits become suggestions"), "review.trackChanges", json!({"value": true}), track)}
    }
}

/// The font list, each name previewed in its own face once rendered.
fn fonts_view(b: Bridge) -> impl IntoView {
    let current = b.state.with_untracked(|s| s.format.font.clone().unwrap_or_default());
    let fonts = b.fonts.clone();
    let rows = fonts
        .iter()
        .map(|name| {
            let b2 = b.clone();
            let name = name.clone();
            let key = format!("font:{name}");
            let n2 = name.clone();
            let n3 = name.clone();
            let img = move || b2.image(&key, || Preview::Font(n2.clone()));
            let b3 = b.clone();
            let active = current == name;
            let title = name.clone();
            view! {
                <button class="wc-menu-item wc-font-item" class:active=active title=title on:click=move |_| pick(&b3, "format.font", json!({"name": n3.clone()}))>
                    {move || match img() {
                        Some(url) if !url.is_empty() => view! { <img class="wc-font-preview" src=url alt=name.clone()/> }.into_any(),
                        _ => view! { <span class="wc-menu-label">{name.clone()}</span> }.into_any(),
                    }}
                </button>
            }
        })
        .collect_view();
    view! { <div class="wc-menu-scroll">{rows}</div> }
}

/// Font sizes.
fn sizes_view(b: Bridge) -> impl IntoView {
    let current = b.state.with_untracked(|s| s.format.size);
    let rows = wordfabrica_engine::cmd::format::SIZES
        .iter()
        .map(|&s| {
            let b = b.clone();
            let label = if s.fract() == 0.0 { format!("{s:.0}") } else { s.to_string() };
            let active = current.is_some_and(|c| (c - f64::from(s)).abs() < 0.01);
            view! {
                <button class="wc-menu-item" class:active=active on:click=move |_| pick(&b, "format.size", json!({"size": s}))>
                    <span class="wc-check">{if active { "✓" } else { "" }}</span>
                    <span class="wc-menu-label">{label}</span>
                </button>
            }
        })
        .collect_view();
    view! { <div class="wc-menu-scroll">{rows}</div> }
}
