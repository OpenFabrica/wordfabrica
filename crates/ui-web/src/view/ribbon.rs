//! The ribbon body: the active tab's groups and controls, drawn from [`crate::model`].

use leptos::prelude::*;
use serde_json::json;
use wasm_bindgen::JsCast as _;

use super::{icon_svg, input_checked, input_value, keep_focus, menus, t};
use crate::bridge::{Bridge, MenuSource, Preview};
use crate::model::{self, CELL_ALIGN, Cmd, Ctl, Group, Item, STYLE_SETS, Slot, Special, TABLE_LOOK, is_checked};

/// The ribbon for the active tab (nothing while it's collapsed).
pub fn ribbon(b: Bridge) -> impl IntoView {
    let tab = b.select(|s| s.tab.clone());
    let collapsed = b.select(|s| s.ribbon_collapsed);
    let in_table = b.select(|s| s.in_table);
    let dark = b.select(|s| s.dark);
    move || {
        if collapsed.get() {
            return ().into_any();
        }
        let name = tab.get();
        let mut tab = model::tab(&name);
        // A table tab stays selected after the caret leaves the table; show Home meanwhile.
        if tab.contextual && !in_table.get() {
            tab = model::tab("Home");
        }
        let groups = tab.groups.iter().map(|g| group_view(&b, g, dark)).collect_view();
        view! { <div class="wc-ribbon-body" role="toolbar" aria-label=t(tab.name)>{groups}</div> }.into_any()
    }
}

fn group_view(b: &Bridge, g: &'static Group, dark: Memo<bool>) -> impl IntoView + use<> {
    let ctls = g.ctls.iter().map(|c| ctl_view(b, c, dark)).collect_view();
    let launcher = g.launcher.map(|cmd| {
        let b = b.clone();
        let title = wordfabrica_ui_egui::i18n::fmt(&t("{group} settings"), &[("group", &t(g.title))]);
        view! {
            <button class="wc-launcher" title=title.clone() aria-label=title on:mousedown=keep_focus
                inner_html=move || icon_svg("launcher", 11.0, dark.get(), true)
                on:click=move |_| { let p = cmd.params(&b.state.get_untracked()); b.run(cmd.id, p); }>
            </button>
        }
    });
    view! {
        <div class="wc-group" role="group" aria-label=t(g.title)>
            <div class="wc-group-body">{ctls}</div>
            {launcher}
        </div>
    }
}

/// Whether `cmd` can run now (tracked).
fn enabled(b: &Bridge, id: &'static str) -> impl Fn() -> bool + Send + Sync + Clone + 'static + use<> {
    let st = b.state.clone();
    move || !st.with(|s| s.disabled.contains(id))
}

fn checked(b: &Bridge, check: Option<&'static str>) -> impl Fn() -> bool + Send + Sync + Clone + 'static + use<> {
    let st = b.state.clone();
    move || check.is_some_and(|k| st.with(|s| is_checked(s, k)))
}

/// Run a control's command with its current parameters.
fn click(b: &Bridge, cmd: Cmd) -> impl FnMut(web_sys::MouseEvent) + 'static + use<> {
    let b = b.clone();
    move |_| {
        let p = cmd.params(&b.state.get_untracked());
        b.run(cmd.id, p);
    }
}

fn tooltip(b: &Bridge, label: &'static str, id: &'static str) -> impl Fn() -> String + Send + Sync + 'static + use<> {
    let b = b.clone();
    move || {
        b.state.track();
        b.tooltip(&t(label).replace('\n', " "), id)
    }
}

fn ctl_view(b: &Bridge, ctl: &'static Ctl, dark: Memo<bool>) -> AnyView {
    match *ctl {
        Ctl::Big { icon, label, cmd, check } => {
            let on = enabled(b, cmd.id);
            let on2 = on.clone();
            let is_on = checked(b, check);
            let is_on2 = is_on.clone();
            view! {
                <button class="wc-big" class:checked=is_on aria-pressed=move || check.map(|_| is_on2().to_string())
                    disabled=move || !on() title=tooltip(b, label, cmd.id) on:mousedown=keep_focus on:click=click(b, cmd)>
                    <span class="wc-icon32" inner_html=move || icon_svg(icon, 32.0, dark.get(), on2())></span>
                    <span class="wc-label">{t(label)}</span>
                </button>
            }
            .into_any()
        }
        Ctl::Small { icon, label, tip, cmd, check } => {
            let on = enabled(b, cmd.id);
            let on2 = on.clone();
            let is_on = checked(b, check);
            let is_on2 = is_on.clone();
            view! {
                <button class="wc-small" class:checked=is_on aria-pressed=move || check.map(|_| is_on2().to_string())
                    disabled=move || !on() title=tooltip(b, tip, cmd.id) aria-label=t(tip) on:mousedown=keep_focus on:click=click(b, cmd)>
                    <span class="wc-icon16" inner_html=move || icon_svg(icon, 17.0, dark.get(), on2())></span>
                    {label.map(|l| view! { <span class="wc-label">{t(l)}</span> })}
                </button>
            }
            .into_any()
        }
        Ctl::Split { icon, tip, cmd, check, swatch, menu } => split_view(b, icon, tip, cmd, check, swatch, menu, dark),
        Ctl::Menu { icon, label, tip, big, menu } => menu_view(b, icon, label, tip, big, MenuSource::Items(menu), dark),
        Ctl::Rows(rows) => {
            let rows = rows
                .iter()
                .map(|row| {
                    let ctls = row.iter().map(|c| ctl_view(b, c, dark)).collect_view();
                    view! { <div class="wc-row">{ctls}</div> }
                })
                .collect_view();
            view! { <div class="wc-rows">{rows}</div> }.into_any()
        }
        Ctl::Special(s) => special_view(b, s, dark),
    }
}

#[allow(clippy::too_many_arguments)]
fn split_view(
    b: &Bridge,
    icon: &'static str,
    tip: &'static str,
    cmd: Cmd,
    check: Option<&'static str>,
    swatch: Option<Slot>,
    menu: &'static [Item],
    dark: Memo<bool>,
) -> AnyView {
    let on = enabled(b, cmd.id);
    let on2 = on.clone();
    let st = b.state.clone();
    let bar = swatch.map(|slot| {
        let style = move || {
            let hex = st.with(|s| s.last_colors.get(slot.key()).cloned().unwrap_or_default());
            match wordfabrica_doc::Rgb::parse(&hex) {
                Some(c) => format!("background:#{}", c.hex()),
                None => "background:transparent".to_string(),
            }
        };
        view! { <span class="wc-swatch-bar" style=style></span> }
    });
    let b2 = b.clone();
    let key = format!("split:{tip}");
    view! {
        <div class="wc-split" class:checked=checked(b, check)>
            <button class="wc-split-main" disabled=move || !on() title=tooltip(b, tip, cmd.id) aria-label=t(tip) on:mousedown=keep_focus on:click=click(b, cmd)>
                <span class="wc-icon16" inner_html=move || icon_svg(icon, 17.0, dark.get(), on2())></span>
                {bar}
            </button>
            <button class="wc-split-arrow" aria-haspopup="menu" aria-label=t(tip) on:mousedown=keep_focus
                on:click=move |ev| menus::toggle(&b2, &key, &ev, MenuSource::Items(menu))
                inner_html=move || icon_svg("dropdown", 10.0, dark.get(), true)>
            </button>
        </div>
    }
    .into_any()
}

fn menu_view(
    b: &Bridge,
    icon: &'static str,
    label: Option<&'static str>,
    tip: &'static str,
    big: bool,
    source: MenuSource,
    dark: Memo<bool>,
) -> AnyView {
    let b2 = b.clone();
    let key = format!("menu:{icon}:{tip}");
    let open = move |ev: web_sys::MouseEvent| menus::toggle(&b2, &key, &ev, source.clone());
    if big {
        view! {
            <button class="wc-big" aria-haspopup="menu" title=t(tip) on:mousedown=keep_focus on:click=open>
                <span class="wc-icon32" inner_html=move || icon_svg(icon, 32.0, dark.get(), true)></span>
                <span class="wc-label">{format!("{} ▾", t(label.unwrap_or("")))}</span>
            </button>
        }
        .into_any()
    } else {
        view! {
            <button class="wc-small wc-menu-button" aria-haspopup="menu" title=t(tip) aria-label=t(tip) on:mousedown=keep_focus on:click=open>
                <span class="wc-icon16" inner_html=move || icon_svg(icon, 17.0, dark.get(), true)></span>
                {label.map(|l| view! { <span class="wc-label">{t(l)}</span> })}
                <span class="wc-chevron" inner_html=move || icon_svg("dropdown", 9.0, dark.get(), true)></span>
            </button>
        }
        .into_any()
    }
}

/// Menus anchor to the whole combo field, not just its arrow.
fn open_combo_menu(b: &Bridge, key: &str, ev: &web_sys::MouseEvent, source: MenuSource) {
    if b.menu.with_untracked(|m| m.as_ref().is_some_and(|m| m.key == key)) {
        b.menu.set(None);
        return;
    }
    let field = ev.current_target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()).and_then(|e| e.parent_element());
    let (left, top, width) = match field {
        Some(f) => {
            let r = f.get_bounding_client_rect();
            (r.left(), r.bottom(), r.width() + 60.0)
        }
        None => super::anchor_of(ev),
    };
    b.menu.set(Some(crate::bridge::OpenMenu { key: key.to_string(), left, top, width, source }));
}

/// A text field with a dropdown list: Enter applies what was typed, Escape goes back to the
/// document, and leaving the field restores the current value.
fn combo(
    b: &Bridge,
    class: &'static str,
    label: &'static str,
    current: Memo<String>,
    source: MenuSource,
    apply: fn(&Bridge, &str),
    dark: Memo<bool>,
) -> AnyView {
    let b1 = b.clone();
    let b2 = b.clone();
    let key = format!("combo:{label}");
    // Like Word, entering the box selects its text so typing replaces it. The mouseup that
    // follows a focusing click would drop that selection, so it's skipped once.
    let focusing = std::rc::Rc::new(std::cell::Cell::new(false));
    let focusing2 = focusing.clone();
    view! {
        <div class=format!("wc-combo {class}")>
            <input class="wc-combo-input" type="text" aria-label=t(label) title=t(label) prop:value=move || current.get()
                on:focus=move |ev| {
                    if let Some(i) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) {
                        i.select();
                        focusing.set(true);
                    }
                }
                on:mouseup=move |ev: web_sys::MouseEvent| {
                    if focusing2.replace(false) {
                        ev.prevent_default();
                    }
                }
                on:keydown=move |ev: web_sys::KeyboardEvent| {
                    let input = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok());
                    match ev.key().as_str() {
                        "Enter" => {
                            ev.prevent_default();
                            if let Some(i) = input {
                                apply(&b1, i.value().trim());
                            }
                        }
                        "Escape" => {
                            ev.prevent_default();
                            if let Some(i) = input {
                                i.set_value(&current.get_untracked());
                            }
                            b1.send(wordfabrica_ui_egui::Action::FocusDocument);
                        }
                        _ => {}
                    }
                }
                on:blur=move |ev| {
                    if let Some(i) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) {
                        i.set_value(&current.get_untracked());
                    }
                }/>
            <button class="wc-combo-arrow" aria-haspopup="listbox" aria-label=t(label) on:mousedown=keep_focus
                on:click=move |ev| open_combo_menu(&b2, &key, &ev, source.clone())
                inner_html=move || icon_svg("dropdown", 11.0, dark.get(), true)>
            </button>
        </div>
    }
    .into_any()
}

fn apply_font(b: &Bridge, v: &str) {
    if !v.is_empty() {
        b.run("format.font", json!({"name": v}));
    }
}

fn apply_size(b: &Bridge, v: &str) {
    if let Ok(x) = v.parse::<f64>()
        && x.is_finite()
        && x > 0.0
    {
        b.run("format.size", json!({"size": x}));
    }
}

/// `12` rather than `12.0`.
fn size_text(s: Option<f64>) -> String {
    match s {
        Some(s) if s.fract() == 0.0 => format!("{s:.0}"),
        Some(s) => format!("{s}"),
        None => String::new(),
    }
}

fn special_view(b: &Bridge, s: Special, dark: Memo<bool>) -> AnyView {
    match s {
        Special::FontCombo => {
            let font = b.select(|s| s.format.font.clone().unwrap_or_default());
            combo(b, "wc-font-combo", "Font", font, MenuSource::Fonts, apply_font, dark)
        }
        Special::SizeCombo => {
            let size = b.select(|s| size_text(s.format.size));
            combo(b, "wc-size-combo", "Font Size", size, MenuSource::Sizes, apply_size, dark)
        }
        Special::StyleGallery => style_gallery(b, dark),
        Special::StyleSetGallery => {
            let tiles = STYLE_SETS
                .iter()
                .map(|name| {
                    let b2 = b.clone();
                    let b3 = b.clone();
                    let key = format!("set:{name}");
                    let img = move || b2.image(&key, || Preview::StyleSet(name.to_string())).filter(|u| !u.is_empty());
                    view! {
                        <button class="wc-set-tile" title=format!("Style set: {name}") on:mousedown=keep_focus
                            on:click=move |_| b3.run("design.styleSet", json!({"name": name}))>
                            {move || img().map(|u| view! { <img src=u alt=""/> })}
                        </button>
                    }
                })
                .collect_view();
            view! { <div class="wc-gallery wc-set-gallery">{tiles}</div> }.into_any()
        }
        Special::TableStyles => {
            let b = b.clone();
            let st = b.state.clone();
            view! {
                <div class="wc-gallery wc-table-gallery">
                    {move || {
                        let (styles, skey) = st.with(|s| (s.table.as_ref().map(|t| t.styles.clone()).unwrap_or_default(), s.styles_key));
                        styles.into_iter().map(|item| {
                            let b2 = b.clone();
                            let b3 = b.clone();
                            let key = format!("tstyle:{}:{skey}", item.id);
                            let id = item.id.clone();
                            let id2 = item.id.clone();
                            let img = move || b2.image(&key, || Preview::TableStyle(id.clone())).filter(|u| !u.is_empty());
                            view! {
                                <button class="wc-table-tile" title=item.name.clone() on:mousedown=keep_focus
                                    on:click=move |_| b3.run("table.style", json!({"style": id2.clone()}))>
                                    {move || img().map(|u| view! { <img src=u alt=""/> })}
                                </button>
                            }
                        }).collect_view()
                    }}
                </div>
            }
            .into_any()
        }
        Special::TableLook => {
            let checks = TABLE_LOOK
                .iter()
                .map(|&(label, key)| {
                    let st = b.state.clone();
                    let b = b.clone();
                    let value = move || {
                        st.with(|s| {
                            s.table.as_ref().is_some_and(|t| match key {
                                "headerRow" => t.header_row,
                                "firstColumn" => t.first_column,
                                "totalRow" => t.total_row,
                                "lastColumn" => t.last_column,
                                "bandedRows" => t.banded_rows,
                                _ => t.banded_columns,
                            })
                        })
                    };
                    view! {
                        <label class="wc-check-label">
                            <input type="checkbox" prop:checked=value on:change=move |ev| b.run("table.look", json!({key: input_checked(&ev)}))/>
                            {t(label)}
                        </label>
                    }
                })
                .collect_view();
            view! { <div class="wc-look-grid">{checks}</div> }.into_any()
        }
        Special::ParaFields => para_fields(b),
        Special::ShowChecks => {
            let checks = [
                ("Ruler", "view.ruler", "ruler"),
                ("Gridlines", "view.gridlines", "gridlines"),
                ("Navigation Pane", "view.navigationPane", "navPane"),
            ]
            .into_iter()
            .map(|(label, id, toggle)| {
                let st = b.state.clone();
                let b = b.clone();
                view! {
                    <label class="wc-check-label">
                        <input type="checkbox" prop:checked=move || st.with(|s| is_checked(s, toggle))
                            on:change=move |ev| b.run(id, json!({"value": input_checked(&ev)}))/>
                        {t(label)}
                    </label>
                }
            })
            .collect_view();
            view! { <div class="wc-rows wc-checks">{checks}</div> }.into_any()
        }
        Special::CellAlign => {
            let cells = CELL_ALIGN
                .iter()
                .flat_map(|row| row.iter())
                .map(|&v| {
                    let on = enabled(b, "table.cellAlign");
                    let on2 = on.clone();
                    let b = b.clone();
                    view! {
                        <button class="wc-small" title=v aria-label=v disabled=move || !on() on:mousedown=keep_focus
                            on:click=move |_| b.run("table.cellAlign", json!({"value": v}))>
                            <span class="wc-icon16" inner_html=move || icon_svg("cellAlign", 17.0, dark.get(), on2())></span>
                        </button>
                    }
                })
                .collect_view();
            view! { <div class="wc-cell-align">{cells}</div> }.into_any()
        }
        Special::MarkupMenu => {
            let markup = b.select(|s| is_checked(s, "showMarkup"));
            let b = b.clone();
            (move || {
                let label = if markup.get() { "All Markup" } else { "No Markup" };
                menu_view(&b, "markup", Some(label), "Display for Review", false, MenuSource::Items(model::MARKUP_MENU), dark)
            })
            .into_any()
        }
    }
}

/// Home ▸ Styles: five tiles at a time, each the style's name drawn in that style.
fn style_gallery(b: &Bridge, dark: Memo<bool>) -> AnyView {
    const VISIBLE: usize = 5;
    let start = RwSignal::new(0usize);
    let gallery = b.select(|s| (s.gallery.clone(), s.styles_key, s.format.style.clone(), s.lang.clone(), s.dark));
    let b1 = b.clone();
    let tiles = move || {
        let (items, skey, current, lang, is_dark) = gallery.get();
        let from = start.get().min(items.len().saturating_sub(1));
        items
            .into_iter()
            .skip(from)
            .take(VISIBLE)
            .map(|item| {
                let label = wordfabrica_ui_egui::i18n::style_name(&item.name);
                let key = format!("style:{}:{lang}:{is_dark}:{skey}", item.id);
                let b2 = b1.clone();
                let b3 = b1.clone();
                let (id, id2, is_char, l2) = (item.id.clone(), item.id.clone(), item.is_char, label.clone());
                let img = move || b2.image(&key, || Preview::Style { id: id.clone(), label: l2.clone(), is_char }).filter(|u| !u.is_empty());
                let active = item.id == current;
                let (title, aria) = (label.clone(), label.clone());
                view! {
                    <button class="wc-style-tile" class:active=active title=title aria-label=aria aria-pressed=active.to_string()
                        on:mousedown=keep_focus on:click=move |_| b3.run("para.style", json!({"style": id2.clone()}))>
                        {move || match img() {
                            Some(u) => view! { <img src=u alt=""/> }.into_any(),
                            None => view! { <span>{label.clone()}</span> }.into_any(),
                        }}
                    </button>
                }
            })
            .collect_view()
    };
    let count = b.select(|s| s.gallery.len());
    let b4 = b.clone();
    view! {
        <div class="wc-gallery wc-style-gallery">
            <div class="wc-gallery-tiles">{tiles}</div>
            <div class="wc-gallery-arrows">
                <button title=t("Previous") aria-label=t("Previous") on:mousedown=keep_focus
                    on:click=move |_| start.update(|s| *s = s.saturating_sub(VISIBLE))
                    inner_html=move || icon_svg("chevronUp", 12.0, dark.get(), true)></button>
                <button title=t("Next") aria-label=t("Next") on:mousedown=keep_focus
                    on:click=move |_| start.update(|s| *s = (*s + VISIBLE).min(count.get_untracked().saturating_sub(1)))
                    inner_html=move || icon_svg("chevronDown", 12.0, dark.get(), true)></button>
                <button title=t("Styles Pane") aria-label=t("Styles Pane") on:mousedown=keep_focus
                    on:click=move |_| b4.run("view.stylesPane", json!({"value": true}))
                    inner_html=move || icon_svg("more", 12.0, dark.get(), true)></button>
            </div>
        </div>
    }
    .into_any()
}

/// Layout ▸ Paragraph: indents (inches) and spacing (points), applied when a field changes.
fn para_fields(b: &Bridge) -> AnyView {
    type Get = fn(&wordfabrica_ui_egui::host::ParaState) -> f32;
    let field = |label: &'static str, get: Get, id: &'static str, key: &'static str, inches: bool| {
        let value = b.select(move |s| get(&s.para));
        let b = b.clone();
        let (step, min, max, unit) = if inches { (0.1, -11.0, 22.0, "\"") } else { (1.0, 0.0, 1584.0, " pt") };
        view! {
            <label class="wc-field">
                <span class="wc-field-label">{t(label)}</span>
                <input type="number" step=step min=min max=max aria-label=t(label)
                    prop:value=move || if inches { format!("{:.2}", value.get()) } else { format!("{:.0}", value.get()) }
                    on:change=move |ev| {
                        if let Ok(v) = input_value(&ev).trim().parse::<f32>()
                            && v.is_finite()
                        {
                            let v = v.clamp(min as f32, max as f32);
                            let v = if inches { v * 72.0 } else { v };
                            b.run(id, json!({key: v}));
                        }
                    }/>
                <span class="wc-field-unit">{unit}</span>
            </label>
        }
    };
    view! {
        <div class="wc-rows wc-para-fields">
            <div class="wc-row">{field("Indent Left:", |p| p.indent_left, "para.indents", "left", true)}</div>
            <div class="wc-row">{field("Indent Right:", |p| p.indent_right, "para.indents", "right", true)}</div>
        </div>
        <div class="wc-rows wc-para-fields">
            <div class="wc-row">{field("Before:", |p| p.space_before, "para.spacing", "before", false)}</div>
            <div class="wc-row">{field("After:", |p| p.space_after, "para.spacing", "after", false)}</div>
        </div>
    }
    .into_any()
}
