//! Plugin chrome: a tab-strip button per plugin, and the open plugin's pane. Tag panes list
//! categorized snippets with a search box; clicking one inserts its text at the caret through
//! `text.insert`, the same undoable command typing uses.

use leptos::prelude::*;
use serde_json::json;
use wordcraft_ui_egui::i18n::fmt;

use super::{icon_svg, input_value, keep_focus, t};
use crate::bridge::Bridge;
use crate::plugins::{Plugin, PluginKind, Tag, filter_tags};

/// One button per plugin beside Comments; it opens or closes that plugin's pane.
pub fn chips(b: Bridge) -> impl IntoView {
    let dark = b.select(|s| s.dark);
    let plugins = b.plugins.clone();
    move || {
        plugins
            .get()
            .into_iter()
            .map(|p| {
                let (id, id2) = (p.id.clone(), p.id.clone());
                let pane = b.plugin_pane.clone();
                let pane2 = b.plugin_pane.clone();
                let open = move || pane.with(|o| o.as_deref() == Some(id.as_str()));
                let open2 = open.clone();
                view! {
                    <button class="wc-chip wc-plugin-chip" class:active=open aria-pressed=move || open2().to_string()
                        on:mousedown=keep_focus
                        on:click=move |_| pane2.update(|o| *o = if o.as_deref() == Some(id2.as_str()) { None } else { Some(id2.clone()) })>
                        <span class="wc-icon14" inner_html=move || icon_svg("quickParts", 14.0, dark.get(), true)></span>
                        {p.name}
                    </button>
                }
            })
            .collect_view()
    }
}

/// The open plugin's pane.
pub fn pane(b: Bridge, plugin: Plugin) -> impl IntoView {
    match plugin.kind {
        PluginKind::Tags { categories } => tags_pane(b, plugin.name, categories),
    }
}

/// Tags: search box, category filter ("All" first), then the matching tags under their category.
fn tags_pane(b: Bridge, name: String, categories: Vec<crate::plugins::TagCategory>) -> impl IntoView {
    let query = RwSignal::new(String::new());
    let category = RwSignal::new(None::<usize>);
    let categories = std::sync::Arc::new(categories);
    let pane = b.plugin_pane.clone();
    let label = name.clone();
    let cats = categories.clone();
    let filter = {
        let chip = move |index: Option<usize>, label: String| {
            let active = move || category.get() == index;
            view! {
                <button class="wc-tag-cat" role="tab" class:active=active aria-selected=move || active().to_string()
                    on:mousedown=keep_focus on:click=move |_| category.set(index)>{label}</button>
            }
        };
        let all = chip(None, t("All"));
        let rest = cats.iter().enumerate().map(|(i, c)| chip(Some(i), c.name.clone())).collect_view();
        view! { <div class="wc-tag-cats" role="tablist" aria-label=t("Categories")>{all}{rest}</div> }
    };
    let list = move || {
        let groups = filter_tags(&categories, category.get(), &query.get());
        if groups.is_empty() {
            return view! { <p class="wc-pane-help">{t("No matching tags")}</p> }.into_any();
        }
        groups
            .into_iter()
            .map(|(i, tags)| {
                let heading = categories.get(i).map(|c| c.name.clone()).unwrap_or_default();
                let rows = tags.into_iter().map(|tag| tag_button(&b, tag)).collect_view();
                view! {
                    <h3 class="wc-tag-group">{heading}</h3>
                    {rows}
                }
            })
            .collect_view()
            .into_any()
    };
    view! {
        <section class="wc-pane wc-tags-pane" role="complementary" aria-label=label>
            <div class="wc-pane-header">
                <h2 class="wc-pane-title">{name}</h2>
                <button class="wc-pane-close" title=t("Close") aria-label=t("Close") on:mousedown=keep_focus
                    on:click=move |_| pane.set(None)>"✕"</button>
            </div>
            <input class="wc-pane-search" type="search" placeholder=t("Search tags") aria-label=t("Search tags")
                prop:value=move || query.get() on:input=move |ev| query.set(input_value(&ev))/>
            {filter}
            <div class="wc-pane-body" role="list">{list}</div>
        </section>
    }
}

/// One tag: its label, and the text it inserts when that differs. Clicking inserts at the caret
/// (replacing the selection, like typing) and gives the keyboard back to the document.
fn tag_button(b: &Bridge, tag: &Tag) -> impl IntoView + use<> {
    let b = b.clone();
    let text = tag.insert_text().to_string();
    let shown = (text != tag.label).then(|| text.clone());
    let title = if tag.note.is_empty() { fmt(&t("Insert {text}"), &[("text", &text)]) } else { tag.note.clone() };
    view! {
        <button class="wc-pane-item wc-tag" role="listitem" title=title on:mousedown=keep_focus
            on:click=move |_| b.run("text.insert", json!({"text": text.clone()}))>
            <span class="wc-tag-label">{tag.label.clone()}</span>
            {shown.map(|s| view! { <span class="wc-tag-text">{s}</span> })}
        </button>
    }
}
