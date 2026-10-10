//! File screen (Backstage): a full-window overlay with the accent-coloured page list on the left
//! (Home, New, Open, Info, Save, Save As, Print, Export, Options) and the page on the right. Same
//! content and commands as the egui File screen.

use leptos::prelude::*;
use serde_json::json;
use wordfabrica_ui_egui::host::{FileAction, Properties};
use wordfabrica_ui_egui::i18n::{Lang, fmt};
use wordfabrica_ui_egui::{Action, tl};

use super::{icon_svg, icon_svg_in, input_checked, input_value, t};
use crate::bridge::{Bridge, Preview};

/// Page ids and labels, in nav order.
const PAGES: [(&str, &str); 9] = [
    ("home", "Home"),
    ("new", "New"),
    ("open", "Open"),
    ("info", "Info"),
    ("save", "Save"),
    ("saveAs", "Save As"),
    ("print", "Print"),
    ("export", "Export"),
    ("options", "Options"),
];

/// New-document templates: label and `file.new` template name.
const TEMPLATES: [(&str, &str); 5] =
    [("Blank document", "blank"), ("Studio handbook (sample)", "sample"), ("Letter", "letter"), ("Résumé", "resume"), ("Report", "report")];

/// Export formats: label and extension.
const FORMATS: [(&str, &str); 8] = [
    ("PDF document (*.pdf)", "pdf"),
    ("Word document (*.docx)", "docx"),
    ("OpenDocument Text (*.odt)", "odt"),
    ("Rich Text Format (*.rtf)", "rtf"),
    ("Web page (*.html)", "html"),
    ("Markdown (*.md)", "md"),
    ("Plain text (*.txt)", "txt"),
    ("Page image (*.png)", "png"),
];

/// The File screen, while it's open.
pub fn file_screen(b: Bridge) -> impl IntoView {
    let open = b.select(|s| s.backstage && s.file.is_some());
    move || open.get().then(|| screen(b.clone()))
}

/// Leave the File screen and give the keyboard back to the document.
fn close(b: &Bridge) {
    b.send(Action::run("ui.backstage", json!({"value": false})));
    b.send(Action::FocusDocument);
}

/// The overlay: nav column plus the current page. Handles its own Escape (the page key handler
/// leaves keys inside `.wc-file` alone).
fn screen(b: Bridge) -> impl IntoView {
    let page = b.select(|s| s.file.as_ref().map(|f| f.page.clone()).unwrap_or_default());
    // Move focus into the screen so Escape and Tab work right away: onto the current page's item
    // (`querySelector` takes document order, so the Back fallback is a separate try).
    Effect::new(|_| {
        let doc = web_sys::window().and_then(|w| w.document());
        let current = doc.and_then(|d| d.query_selector(".wc-file-nav [aria-current=page]").ok().flatten());
        if current.is_some() {
            super::focus_selector(".wc-file-nav [aria-current=page]");
        } else {
            super::focus_selector(".wc-file-back");
        }
    });
    let nav = PAGES
        .iter()
        .map(|&(id, label)| {
            let b = b.clone();
            let active = move || page.with(|p| p == id);
            view! {
                <button class="wc-file-nav-item" class:active=active aria-current=move || active().then_some("page")
                    on:click=move |_| match id {
                        "save" => b.send(Action::run("file.save", json!({}))),
                        "saveAs" => b.send(Action::File(FileAction::SaveAs)),
                        _ => b.send(Action::run("ui.backstage", json!({"value": true, "page": id}))),
                    }>
                    {t(label)}
                </button>
            }
        })
        .collect_view();
    let (b1, b2, b3, b4, b5) = (b.clone(), b.clone(), b.clone(), b.clone(), b.clone());
    let content = move || match page.get().as_str() {
        "open" => open_page(b5.clone()).into_any(),
        "info" => info_page(b5.clone()).into_any(),
        "print" => export_page(b5.clone(), "Print").into_any(),
        "export" => export_page(b5.clone(), "Export").into_any(),
        "options" => options_page(b5.clone()).into_any(),
        _ => new_page(b5.clone()).into_any(),
    };
    view! {
        <div class="wc-file" role="dialog" aria-modal="true" aria-label=t("File")
            on:keydown=move |ev: web_sys::KeyboardEvent| {
                if ev.key() == "Escape" {
                    ev.prevent_default();
                    close(&b1);
                }
            }>
            <nav class="wc-file-nav" aria-label=t("File")>
                <button class="wc-file-back" title=t("Back") aria-label=t("Back") on:click=move |_| close(&b2)
                    inner_html=icon_svg_in("chevronLeft", 18.0, egui::Color32::WHITE)></button>
                {nav}
                <div class="wc-file-nav-spacer"></div>
                <button class="wc-file-nav-minor" on:click=move |_| b3.send(Action::run("ui.dialog", json!({"name": "about"})))>
                    {t("About")}
                </button>
                <button class="wc-file-nav-minor" on:click=move |_| b4.send(Action::run("ui.community", json!({})))>
                    {t("Community")}
                </button>
            </nav>
            <div class="wc-file-content">{content}</div>
        </div>
    }
}

/// Home / New: greeting, template tiles, then the documents to reopen.
fn new_page(b: Bridge) -> impl IntoView {
    let hour = wordfabrica_engine::cmd::now_iso().get(11..13).and_then(|h| h.parse::<u32>().ok()).unwrap_or(9);
    let greeting = if hour < 12 {
        "Good morning"
    } else if hour < 18 {
        "Good afternoon"
    } else {
        "Good evening"
    };
    let tiles = TEMPLATES
        .iter()
        .map(|&(label, name)| {
            let (b1, b2) = (b.clone(), b.clone());
            let key = format!("tpl:{name}");
            // `None` while the thumbnail renders; an empty URL means it failed (show the frame only).
            let img = move || b1.image(&key, || Preview::Template(name.to_string())).filter(|u| !u.is_empty());
            view! {
                <button class="wc-template" on:click=move |_| {
                    b2.send(Action::run("file.new", json!({"template": name})));
                    close(&b2);
                }>
                    <span class="wc-template-thumb">{move || img().map(|u| view! { <img src=u alt=""/> })}</span>
                    <span class="wc-template-label">{t(label)}</span>
                </button>
            }
        })
        .collect_view();
    view! {
        <h1 class="wc-file-heading">{t(greeting)}</h1>
        <div class="wc-templates">{tiles}</div>
        {open_list(b.clone())}
    }
}

/// Open: Browse…, then the documents to reopen.
fn open_page(b: Bridge) -> impl IntoView {
    let b1 = b.clone();
    view! {
        <h1 class="wc-file-heading">{t("Open")}</h1>
        <button class="wc-fs-button wc-browse" on:click=move |_| b1.send(Action::run("ui.openFileDialog", json!({})))>
            {t("📂  Browse…")}
        </button>
        {open_list(b)}
    }
}

/// Documents to reopen: server documents when hosted (browsers can't reopen local paths), else Recent.
fn open_list(b: Bridge) -> impl IntoView {
    let hosted = b.select(|s| s.file.as_ref().is_some_and(|f| f.server.is_some()));
    move || if hosted.get() { server_list(b.clone()).into_any() } else { recent_list(b.clone()).into_any() }
}

/// One document row: icon, name and a dim second line.
fn doc_row(dark: bool, name: String, detail: String, disabled: bool, on_click: impl Fn() + 'static) -> impl IntoView {
    view! {
        <button class="wc-file-doc" disabled=disabled on:click=move |_| on_click()>
            <span class="wc-file-doc-icon" inner_html=icon_svg("printLayout", 22.0, dark, true)></span>
            <span class="wc-file-doc-text">
                <span class="wc-file-doc-name">{name}</span>
                <span class="wc-file-doc-detail">{detail}</span>
            </span>
        </button>
    }
}

/// Recently opened files; clicking one opens it and closes the File screen.
fn recent_list(b: Bridge) -> impl IntoView {
    let recent = b.select(|s| (s.file.as_ref().map(|f| f.recent.clone()).unwrap_or_default(), s.dark));
    let rows = move || {
        let (paths, dark) = recent.get();
        if paths.is_empty() {
            return view! { <p class="wc-file-dim">{t("Documents you open will show up here.")}</p> }.into_any();
        }
        paths
            .into_iter()
            .map(|p| {
                let b = b.clone();
                let name = std::path::Path::new(&p).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.clone());
                let path = p.clone();
                doc_row(dark, name, p, false, move || {
                    b.send(Action::run("file.open", json!({"path": path})));
                    close(&b);
                })
            })
            .collect_view()
            .into_any()
    };
    view! {
        <section class="wc-file-list">
            <h2 class="wc-file-subheading">{t("Recent")}</h2>
            {rows}
        </section>
    }
}

/// Hosted web: the server document list. A click only leaves a request; the host loads it.
fn server_list(b: Bridge) -> impl IntoView {
    let server = b.select(|s| s.file.as_ref().and_then(|f| f.server.clone()).unwrap_or_default());
    let dirty = b.select(|s| s.dirty);
    let dark = b.select(|s| s.dark);
    let b1 = b.clone();
    let rows = move || {
        let list = server.get();
        let dark = dark.get();
        list.documents
            .into_iter()
            .map(|d| {
                let b = b.clone();
                let id = d.id;
                doc_row(dark, d.title, format!("v{}", d.revision), list.busy, move || {
                    b.send(Action::File(FileAction::OpenServer(id.clone())));
                    b.send(Action::FocusDocument);
                })
            })
            .collect_view()
    };
    view! {
        <section class="wc-file-list">
            <div class="wc-file-list-head">
                <h2 class="wc-file-subheading">{t("Server documents")}</h2>
                <button class="wc-fs-button" disabled=move || server.with(|s| s.listing)
                    on:click=move |_| b1.send(Action::File(FileAction::RefreshServer))>{t("Refresh")}</button>
            </div>
            {move || server.with(|s| s.listing).then(|| view! { <p class="wc-file-dim" role="status">{t("Loading…")}</p> })}
            {move || server.with(|s| s.list_error.clone()).map(|e| {
                view! { <p class="wc-file-error" role="alert">{wordfabrica_ui_egui::i18n::t(&e).to_string()}</p> }
            })}
            {move || dirty.get().then(|| {
                view! { <p class="wc-file-dim">{t("Save your current changes before opening another document.")}</p> }
            })}
            {move || server.with(|s| s.documents.is_empty() && !s.listing && s.list_error.is_none()).then(|| {
                view! { <p class="wc-file-dim">{t("No saved documents yet.")}</p> }
            })}
            {rows}
        </section>
    }
}

/// Picks one field out of [`Properties`].
type PropField = fn(&mut Properties) -> &mut String;

/// Info: editable properties (applied when a field changes), statistics and location.
fn info_page(b: Bridge) -> impl IntoView {
    let properties = b.select(|s| s.file.as_ref().map(|f| f.properties.clone()).unwrap_or_default());
    let info = b.select(|s| s.file.as_ref().map(|f| f.info.clone()).unwrap_or_default());
    let fields: [(&str, &str, PropField); 5] = [
        ("Title", "wc-prop-title", |p| &mut p.title),
        ("Subject", "wc-prop-subject", |p| &mut p.subject),
        ("Author", "wc-prop-author", |p| &mut p.author),
        ("Keywords", "wc-prop-keywords", |p| &mut p.keywords),
        ("Category", "wc-prop-category", |p| &mut p.category),
    ];
    let rows = fields
        .into_iter()
        .map(|(label, id, field)| {
            let b = b.clone();
            let value = move || {
                let mut p = properties.get();
                field(&mut p).clone()
            };
            view! {
                <label class="wc-file-field-label" for=id>{t(label)}</label>
                <input class="wc-file-input" id=id type="text" prop:value=value on:change=move |ev| {
                    // Send the whole set with this one field replaced.
                    let mut p = properties.get_untracked();
                    *field(&mut p) = input_value(&ev);
                    b.send(Action::File(FileAction::Properties(p)));
                }/>
            }
        })
        .collect_view();
    let stats = [("Pages", "pages"), ("Words", "words"), ("Paragraphs", "paragraphs"), ("Sections", "sections"), ("Comments", "comments")]
        .into_iter()
        .map(|(label, key)| {
            let value = move || info.with(|v| v.get(key).map(|x| x.to_string()).unwrap_or_default());
            view! { <p class="wc-file-stat">{format!("{}: ", t(label))}{value}</p> }
        })
        .collect_view();
    let location = move || {
        info.with(|v| {
            let path = v.get("path").and_then(|p| p.as_str()).unwrap_or(tl!("Not saved yet"));
            fmt(tl!("Location: {path}"), &[("path", path)])
        })
    };
    view! {
        <h1 class="wc-file-heading">{t("Info")}</h1>
        <div class="wc-file-columns">
            <section>
                <h2 class="wc-file-subheading">{t("Properties")}</h2>
                <div class="wc-file-form">{rows}</div>
            </section>
            <section>
                <h2 class="wc-file-subheading">{t("Statistics")}</h2>
                {stats}
                <p class="wc-file-stat wc-file-location">{location}</p>
            </section>
        </div>
    }
}

/// Print / Export: save a copy in another format.
fn export_page(b: Bridge, title: &'static str) -> impl IntoView {
    let buttons = FORMATS
        .iter()
        .map(|&(label, ext)| {
            let b = b.clone();
            view! {
                <button class="wc-fs-button wc-export" on:click=move |_| b.send(Action::File(FileAction::Export(ext.to_string())))>
                    {t(label)}
                </button>
            }
        })
        .collect_view();
    view! {
        <h1 class="wc-file-heading">{t(title)}</h1>
        <p class="wc-file-text">{t("Save a copy in another format. Printing goes through a PDF you can print from any viewer.")}</p>
        <div class="wc-exports">{buttons}</div>
    }
}

/// A labelled checkbox that sends `action(checked)` when toggled.
fn check(label: &'static str, checked: Memo<bool>, b: &Bridge, action: fn(bool) -> Action) -> impl IntoView + use<> {
    let b = b.clone();
    view! {
        <label class="wc-file-check">
            <input type="checkbox" prop:checked=move || checked.get() on:change=move |ev| b.send(action(input_checked(&ev)))/>
            <span>{t(label)}</span>
        </label>
    }
}

/// Options: interface language, user name, appearance, AutoSave, display and agents.
fn options_page(b: Bridge) -> impl IntoView {
    let language = b.select(|s| s.file.as_ref().map(|f| f.language.clone()).unwrap_or_default());
    let author = b.select(|s| s.author.clone());
    let dark = b.select(|s| s.dark);
    let autosave = b.select(|s| s.file.as_ref().is_some_and(|f| f.autosave));
    let community = b.select(|s| s.show_community);
    let marks = b.select(|s| s.toggles.get("marks").copied().unwrap_or(false));
    let ruler = b.select(|s| s.toggles.get("ruler").copied().unwrap_or(false));
    // No "Automatic" entry: on the web the language is always an explicit pick (like egui's wasm build).
    let languages = Lang::all()
        .map(|l| {
            let code = l.code();
            view! { <option value=code prop:selected=move || language.with(|p| Lang::from_pref(p).code() == code)>{l.name()}</option> }
        })
        .collect_view();
    let (b1, b2) = (b.clone(), b.clone());
    view! {
        <h1 class="wc-file-heading">{t("Options")}</h1>
        <h2 class="wc-file-subheading">{t("General")}</h2>
        <div class="wc-file-form">
            <label class="wc-file-field-label" for="wc-opt-language">{t("Interface language:")}</label>
            <select class="wc-file-input" id="wc-opt-language"
                on:change=move |ev| b1.send(Action::run("ui.language", json!({"value": event_target_value(&ev)})))>
                {languages}
            </select>
            <span></span>
            <p class="wc-file-hint">{t("Menus and commands change; your documents don't.")}</p>
            <label class="wc-file-field-label" for="wc-opt-author">{t("User name:")}</label>
            <input class="wc-file-input" id="wc-opt-author" type="text" prop:value=move || author.get()
                on:change=move |ev| b2.send(Action::SetAuthor(input_value(&ev)))/>
        </div>
        {check("Dark mode", dark, &b, |on| Action::run("ui.dark", json!({"value": on})))}
        {check("AutoSave documents that have been saved", autosave, &b, Action::SetAutosave)}
        {check("Show the community button in the title bar", community, &b, Action::SetShowCommunity)}
        <h2 class="wc-file-subheading">{t("Display")}</h2>
        {check("Show all formatting marks", marks, &b, |on| Action::run("view.marks", json!({"value": on})))}
        {check("Show rulers", ruler, &b, |on| Action::run("view.ruler", json!({"value": on})))}
        <h2 class="wc-file-subheading">{t("Agents")}</h2>
        <p class="wc-file-text">
            {t("Every command is available to scripts and AI agents: run `wordfabrica-cli mcp` for an MCP server, or start the app with `--control <port>` for the JSON control channel.")}
        </p>
    }
}
