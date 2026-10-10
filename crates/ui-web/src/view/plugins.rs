//! Plugin chrome: a tab-strip button per plugin, and the open plugin's pane. Tag panes list
//! categorized snippets with a search box; clicking one inserts its text at the caret through
//! `text.insert`, the same undoable command typing uses. Editable plugins add an Edit mode
//! whose changes the shell saves on the server for everyone.

use leptos::prelude::*;
use serde_json::json;
use wordcraft_ui_egui::i18n::fmt;

use super::{icon_svg, input_value, keep_focus, t};
use crate::bridge::Bridge;
use crate::plugins::{self as logic, Plugin, PluginKind, Tag, TagCategory, filter_tags};

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

/// The tag form's fields; `original` is the tag being edited (`None` = adding a new one).
#[derive(Clone, Copy)]
struct Form {
    category: RwSignal<String>,
    label: RwSignal<String>,
    text: RwSignal<String>,
    note: RwSignal<String>,
    original: RwSignal<Option<(usize, usize)>>,
    error: RwSignal<Option<String>>,
    /// A save from this form is in flight; the form clears when it succeeds.
    pending: RwSignal<bool>,
}

impl Form {
    fn new() -> Self {
        Form {
            category: RwSignal::new(String::new()),
            label: RwSignal::new(String::new()),
            text: RwSignal::new(String::new()),
            note: RwSignal::new(String::new()),
            original: RwSignal::new(None),
            error: RwSignal::new(None),
            pending: RwSignal::new(false),
        }
    }

    /// Empty the fields (the category stays, for adding several tags to one category).
    fn clear(&self) {
        self.label.set(String::new());
        self.text.set(String::new());
        self.note.set(String::new());
        self.original.set(None);
        self.error.set(None);
    }
}

/// Everything a pane's parts share.
#[derive(Clone)]
struct Pane {
    b: Bridge,
    id: String,
    /// The plugin as currently published (follows saves and reloads).
    data: Memo<Option<Plugin>>,
    editing: RwSignal<bool>,
    form: Form,
}

impl Pane {
    /// The plugin's current tags and revision.
    fn tags(&self) -> (Vec<TagCategory>, u64) {
        self.data.with_untracked(|p| p.as_ref().map(|p| (categories(p).to_vec(), p.revision)).unwrap_or_default())
    }

    /// Save `categories` (computed from the current tags) when the plugin accepts them.
    fn save(&self, revision: u64, categories: Vec<TagCategory>) -> Result<(), String> {
        let Some(mut plugin) = self.data.get_untracked() else { return Err(t("Plugin not found")) };
        plugin.kind = PluginKind::Tags { categories };
        if plugin.validate().is_err() {
            return Err(t("A name, text or note is too long or has line breaks"));
        }
        let PluginKind::Tags { categories } = plugin.kind;
        self.b.save_tags(&self.id, revision, categories);
        Ok(())
    }
}

fn categories(p: &Plugin) -> &[TagCategory] {
    match &p.kind {
        PluginKind::Tags { categories } => categories,
    }
}

/// The open plugin's pane.
pub fn pane(b: Bridge, id: String) -> impl IntoView {
    let data = {
        let (plugins, id) = (b.plugins.clone(), id.clone());
        Memo::new(move |_| plugins.with(|ps| ps.iter().find(|p| p.id == id).cloned()))
    };
    // Clear the form once its save went through.
    let form = Form::new();
    let (saving, error) = (b.plugin_saving.clone(), b.plugin_error.clone());
    Effect::new(move |_| {
        if !saving.get() && form.pending.get_untracked() {
            form.pending.set(false);
            if error.get_untracked().is_none() {
                form.clear();
            }
        }
    });
    tags_pane(Pane { b, id, data, editing: RwSignal::new(false), form })
}

/// Tags: search box, category filter ("All" first), then the matching tags under their category;
/// in Edit mode, the tag form and edit/delete buttons too.
fn tags_pane(p: Pane) -> impl IntoView {
    let query = RwSignal::new(String::new());
    // The category filter, by name, so it survives edits to other categories.
    let category = RwSignal::new(None::<String>);
    let data = p.data;
    let name = move || data.with(|d| d.as_ref().map(|d| d.name.clone()).unwrap_or_default());
    let editable = {
        let can = p.b.can_edit_plugins();
        move || can && data.with(|d| d.as_ref().is_some_and(|d| d.editable))
    };
    let pane_ref = p.b.plugin_pane.clone();
    let filter = move || {
        let names: Vec<String> = data.with(|d| d.as_ref().map(|d| categories(d).iter().map(|c| c.name.clone()).collect()).unwrap_or_default());
        let chip = move |value: Option<String>, label: String| {
            let v2 = value.clone();
            let active = move || category.with(|c| *c == value);
            view! {
                <button class="wc-tag-cat" role="tab" class:active=active.clone() aria-selected=move || active().to_string()
                    on:mousedown=keep_focus on:click=move |_| category.set(v2.clone())>{label}</button>
            }
        };
        let rest = names.into_iter().map(|n| chip(Some(n.clone()), n)).collect_view();
        view! { <div class="wc-tag-cats" role="tablist" aria-label=t("Categories")>{chip(None, t("All"))}{rest}</div> }
    };
    let list = {
        let p = p.clone();
        move || {
            let Some(plugin) = data.get() else { return ().into_any() };
            let cats = categories(&plugin);
            let index = category.with(|c| c.as_ref().and_then(|c| cats.iter().position(|x| &x.name == c)));
            let groups = filter_tags(cats, index, &query.get());
            if groups.is_empty() {
                return view! { <p class="wc-pane-help">{t("No matching tags")}</p> }.into_any();
            }
            let editing = p.editing.get();
            groups
                .into_iter()
                .map(|(ci, tags)| {
                    let cat = cats.get(ci).cloned().unwrap_or(TagCategory { name: String::new(), tags: vec![] });
                    let heading = category_heading(&p, ci, &cat, editing);
                    let rows = tags.into_iter().map(|(ti, tag)| tag_row(&p, ci, ti, &cat.name, tag, editing)).collect_view();
                    view! { {heading}{rows} }
                })
                .collect_view()
                .into_any()
        }
    };
    let edit_toggle = {
        let p = p.clone();
        move || {
            editable().then(|| {
                let p = p.clone();
                let label = move || if p.editing.get() { t("Done") } else { t("Edit") };
                view! {
                    <button class="wc-pane-button wc-tags-edit" aria-pressed=move || p.editing.get().to_string() on:mousedown=keep_focus
                        on:click=move |_| {
                            let on = !p.editing.get_untracked();
                            p.editing.set(on);
                            p.form.clear();
                            p.b.plugin_error.set(None);
                            // Start editing from the latest tags others may have saved.
                            if on {
                                p.b.reload_plugins();
                            }
                        }>{label}</button>
                }
            })
        }
    };
    let form = {
        let p = p.clone();
        move || (p.editing.get() && editable()).then(|| tag_form(p.clone(), category))
    };
    view! {
        <section class="wc-pane wc-tags-pane" role="complementary" aria-label=name>
            <div class="wc-pane-header">
                <h2 class="wc-pane-title">{name}</h2>
                {edit_toggle}
                <button class="wc-pane-close" title=t("Close") aria-label=t("Close") on:mousedown=keep_focus
                    on:click=move |_| pane_ref.set(None)>"✕"</button>
            </div>
            {form}
            <input class="wc-pane-search" type="search" placeholder=t("Search tags") aria-label=t("Search tags")
                prop:value=move || query.get() on:input=move |ev| query.set(input_value(&ev))/>
            {filter}
            <div class="wc-pane-body" role="list">{list}</div>
        </section>
    }
}

/// Edit mode's form: category (pick or type a new one), name, text to insert, note; Add or Save.
fn tag_form(p: Pane, filter: RwSignal<Option<String>>) -> impl IntoView {
    let f = p.form;
    // A new tag goes to the category being viewed, or the first one.
    if f.category.get_untracked().is_empty() {
        let first = p.data.with_untracked(|d| d.as_ref().and_then(|d| categories(d).first().map(|c| c.name.clone())));
        f.category.set(filter.get_untracked().or(first).unwrap_or_default());
    }
    let names =
        move || p.data.with(|d| d.as_ref().map(|d| categories(d).iter().map(|c| view! { <option value=c.name.clone()></option> }).collect_view()));
    let list_id = format!("wc-tag-cats-{}", p.id);
    let saving = p.b.plugin_saving.clone();
    let saving2 = saving.clone();
    let server_error = p.b.plugin_error.clone();
    let submit = {
        let p = p.clone();
        move || {
            let (label, cat) = (f.label.get_untracked().trim().to_string(), f.category.get_untracked().trim().to_string());
            if cat.is_empty() {
                return f.error.set(Some(t("Enter a category")));
            }
            if label.is_empty() {
                return f.error.set(Some(t("Enter a name")));
            }
            let tag = Tag { label, text: f.text.get_untracked().trim().to_string(), note: f.note.get_untracked().trim().to_string() };
            let (cats, revision) = p.tags();
            let next = logic::upsert_tag(&cats, f.original.get_untracked(), &cat, tag);
            match p.save(revision, next) {
                Ok(()) => {
                    f.error.set(None);
                    f.pending.set(true);
                }
                Err(e) => f.error.set(Some(e)),
            }
        }
    };
    let submit2 = submit.clone();
    let field = move |label: String, value: RwSignal<String>, placeholder: String, list: Option<String>| {
        let s = submit2.clone();
        view! {
            <label class="wc-tag-field">
                <span>{label}</span>
                <input type="text" prop:value=move || value.get() placeholder=placeholder list=list
                    on:input=move |ev| value.set(input_value(&ev))
                    on:keydown=move |ev: web_sys::KeyboardEvent| if ev.key() == "Enter" && !ev.is_composing() { ev.prevent_default(); s(); }/>
            </label>
        }
    };
    view! {
        <div class="wc-tag-form" role="group" aria-label=t("Edit tags")>
            {field(t("Category"), f.category, t("Pick or type a new category"), Some(list_id.clone()))}
            <datalist id=list_id>{names}</datalist>
            {field(t("Name"), f.label, String::new(), None)}
            {field(t("Text to insert"), f.text, t("Leave empty to insert the name"), None)}
            {field(t("Note"), f.note, t("Optional"), None)}
            <div class="wc-tag-form-buttons">
                <button class="wc-pane-button wc-primary" disabled=move || saving.get() on:mousedown=keep_focus on:click=move |_| submit()>
                    {move || if f.original.get().is_some() { t("Save") } else { t("Add") }}
                </button>
                {move || f.original.get().is_some().then(|| view! {
                    <button class="wc-pane-button" on:mousedown=keep_focus on:click=move |_| f.clear()>{t("Cancel")}</button>
                })}
                {move || saving2.get().then(|| view! { <span class="wc-tags-status" role="status">{t("Saving…")}</span> })}
            </div>
            {move || f.error.get().or(server_error.get()).map(|e| view! { <p class="wc-tags-error" role="alert">{e}</p> })}
        </div>
    }
}

/// A category title; in Edit mode with Rename and Delete.
fn category_heading(p: &Pane, ci: usize, cat: &TagCategory, editing: bool) -> impl IntoView + use<> {
    let name = cat.name.clone();
    if !editing {
        return view! { <h3 class="wc-tag-group">{name}</h3> }.into_any();
    }
    let (p1, p2) = (p.clone(), p.clone());
    let (n1, n2) = (name.clone(), name.clone());
    let count = cat.tags.len();
    let saving = p.b.plugin_saving.clone();
    let saving2 = saving.clone();
    view! {
        <h3 class="wc-tag-group wc-tag-group-edit">
            <span>{name}</span>
            <button class="wc-tag-action" title=t("Rename category") aria-label=t("Rename category") disabled=move || saving.get()
                on:mousedown=keep_focus on:click=move |_| {
                    let ask = fmt(&t("New name for category {name}"), &[("name", &n1)]);
                    let Some(new) = prompt(&ask, &n1).filter(|n| !n.trim().is_empty() && n.trim() != n1) else { return };
                    let (cats, revision) = p1.tags();
                    // Renaming can merge categories, which moves tags; drop a half-done tag edit.
                    if p1.form.original.get_untracked().is_some() {
                        p1.form.clear();
                    }
                    match p1.save(revision, logic::rename_category(&cats, ci, &new)) {
                        // New tags should keep going to this category under its new name.
                        Ok(()) if p1.form.category.get_untracked().trim() == n1 => p1.form.category.set(new.trim().to_string()),
                        Ok(()) => {}
                        Err(e) => p1.form.error.set(Some(e)),
                    }
                }>"✎"</button>
            <button class="wc-tag-action" title=t("Delete category") aria-label=t("Delete category") disabled=move || saving2.get()
                on:mousedown=keep_focus on:click=move |_| {
                    let ask = fmt(&t("Delete category {name} and its {count} tags?"), &[("name", &n2), ("count", &count.to_string())]);
                    if !confirm(&ask) {
                        return;
                    }
                    let (cats, revision) = p2.tags();
                    // Tag positions after this category move; a tag being edited can't be found again.
                    p2.form.clear();
                    let _ = p2.save(revision, logic::delete_category(&cats, ci));
                }>"✕"</button>
        </h3>
    }
    .into_any()
}

/// One tag: its label, and the text it inserts when that differs. Clicking inserts at the caret
/// (replacing the selection, like typing) and gives the keyboard back to the document. In Edit
/// mode, Edit loads the tag into the form and Delete removes it.
fn tag_row(p: &Pane, ci: usize, ti: usize, category: &str, tag: &Tag, editing: bool) -> impl IntoView + use<> {
    let b = p.b.clone();
    let text = tag.insert_text().to_string();
    let shown = (text != tag.label).then(|| text.clone());
    let title = if tag.note.is_empty() { fmt(&t("Insert {text}"), &[("text", &text)]) } else { tag.note.clone() };
    let insert = view! {
        <button class="wc-pane-item wc-tag" title=title on:mousedown=keep_focus
            on:click=move |_| b.run("text.insert", json!({"text": text.clone()}))>
            <span class="wc-tag-label">{tag.label.clone()}</span>
            {shown.map(|s| view! { <span class="wc-tag-text">{s}</span> })}
        </button>
    };
    let actions = editing.then(|| {
        let (p1, p2) = (p.clone(), p.clone());
        let (tag1, label) = (tag.clone(), tag.label.clone());
        let category = category.to_string();
        let saving = p.b.plugin_saving.clone();
        let saving2 = saving.clone();
        view! {
            <button class="wc-tag-action" title=t("Edit") aria-label=t("Edit") disabled=move || saving.get() on:mousedown=keep_focus
                on:click=move |_| {
                    let f = p1.form;
                    f.category.set(category.clone());
                    f.label.set(tag1.label.clone());
                    f.text.set(tag1.text.clone());
                    f.note.set(tag1.note.clone());
                    f.original.set(Some((ci, ti)));
                    f.error.set(None);
                }>"✎"</button>
            <button class="wc-tag-action" title=t("Delete") aria-label=t("Delete") disabled=move || saving2.get() on:mousedown=keep_focus
                on:click=move |_| {
                    if !confirm(&fmt(&t("Delete tag {name}?"), &[("name", &label)])) {
                        return;
                    }
                    let (cats, revision) = p2.tags();
                    // The form may be editing this tag or a later one in the category; start over.
                    p2.form.clear();
                    let _ = p2.save(revision, logic::delete_tag(&cats, ci, ti));
                }>"✕"</button>
        }
    });
    view! { <div class="wc-tag-row" role="listitem">{insert}{actions}</div> }
}

/// The browser's confirmation box (false when it can't be shown).
fn confirm(message: &str) -> bool {
    web_sys::window().and_then(|w| w.confirm_with_message(message).ok()).unwrap_or(false)
}

/// The browser's text prompt; `None` when cancelled.
fn prompt(message: &str, default: &str) -> Option<String> {
    web_sys::window().and_then(|w| w.prompt_with_message_and_default(message, default).ok()).flatten()
}
