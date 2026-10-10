//! Side panes: Navigation (headings, pages, search results) left of the canvas; Styles and
//! Comments right of it (stacked when both are open). Same content and commands as the egui panes.

use leptos::prelude::*;
use serde_json::json;
use wordcraft_ui_egui::host::{CommentItem, NavResult, StyleItem};
use wordcraft_ui_egui::i18n::{fmt, style_name};
use wordcraft_ui_egui::{Action, tl};

use super::{input_value, keep_focus, t};
use crate::bridge::Bridge;

/// The empty-outline help text (same English key as the egui pane).
const NO_HEADINGS: &str = "Create an interactive outline of your document.\n\nIt's a great way to keep track of where you are or quickly move your content around.\n\nTo get started, go to the Home tab and apply Heading styles to the headings in your document.";

/// The pane left of the canvas: Navigation, while it's open.
pub fn left(b: Bridge) -> impl IntoView {
    let open = b.select(|s| s.nav.is_some());
    move || open.get().then(|| nav_pane(b.clone()))
}

/// The panes right of the canvas: Styles and/or Comments, stacked when both are open.
pub fn right(b: Bridge) -> impl IntoView {
    let open = b.select(|s| (s.styles_pane.is_some(), s.comments.is_some()));
    move || {
        let (styles, comments) = open.get();
        (styles || comments).then(|| {
            view! {
                <div class="wc-panes-right">
                    {styles.then(|| styles_pane(b.clone()))}
                    {comments.then(|| comments_pane(b.clone()))}
                </div>
            }
        })
    }
}

/// Pane title with a close button that runs `close_id` with `{"value": false}`.
fn header(b: &Bridge, title: &'static str, close_id: &'static str) -> impl IntoView + use<> {
    let b = b.clone();
    view! {
        <div class="wc-pane-header">
            <h2 class="wc-pane-title">{t(title)}</h2>
            <button class="wc-pane-close" title=t("Close") aria-label=t("Close") on:mousedown=keep_focus
                on:click=move |_| b.run(close_id, json!({"value": false}))>"✕"</button>
        </div>
    }
}

/// Navigation: search box, Headings/Pages/Results tabs and the active tab's list.
fn nav_pane(b: Bridge) -> impl IntoView {
    let tab = b.select(|s| s.nav.as_ref().map(|n| n.tab.clone()).unwrap_or_default());
    let b1 = b.clone();
    let tabs = [("headings", "Headings"), ("pages", "Pages"), ("results", "Results")]
        .into_iter()
        .map(|(key, label)| {
            let b = b.clone();
            // The app shows Headings for any other tab name, so Headings lights up for those too.
            let active = move || tab.with(|t| t == key || (key == "headings" && t != "pages" && t != "results"));
            view! {
                <button class="wc-pane-tab" role="tab" class:active=active aria-selected=move || active().to_string()
                    on:mousedown=keep_focus on:click=move |_| b.send(Action::NavTab(key.to_string()))>
                    {t(label)}
                </button>
            }
        })
        .collect_view();
    let b2 = b.clone();
    let body = move || match tab.get().as_str() {
        "pages" => nav_pages(b2.clone()).into_any(),
        "results" => nav_results(b2.clone()).into_any(),
        _ => nav_headings(b2.clone()).into_any(),
    };
    view! {
        <section class="wc-pane wc-nav-pane" role="complementary" aria-label=t("Navigation")>
            {header(&b, "Navigation", "view.navigationPane")}
            <input class="wc-pane-search" type="search" placeholder=t("Search document") aria-label=t("Search document")
                on:input=move |ev| b1.send(Action::NavSearch(input_value(&ev)))/>
            <div class="wc-pane-tabs" role="tablist">{tabs}</div>
            <div class="wc-pane-body">{body}</div>
        </section>
    }
}

/// Headings tab: the outline, indented by level, with the caret's heading highlighted.
fn nav_headings(b: Bridge) -> impl IntoView {
    let outline = b.select(|s| s.nav.as_ref().map(|n| (n.headings.clone(), n.current_heading)).unwrap_or_default());
    move || {
        let (headings, current) = outline.get();
        if headings.is_empty() {
            return view! { <p class="wc-pane-help">{t(NO_HEADINGS)}</p> }.into_any();
        }
        headings
            .into_iter()
            .enumerate()
            .map(|(i, h)| {
                let b = b.clone();
                let active = current == Some(i);
                let style = format!("padding-left:{}px", 6 + u32::from(h.level) * 14);
                let pos = h.pos;
                view! {
                    <button class="wc-pane-item wc-nav-heading" class:active=active class:top=h.level == 0 style=style
                        aria-current=active.then_some("location") on:mousedown=keep_focus
                        on:click=move |_| b.run("caret.set", json!({"pos": pos}))>
                        {h.text}
                    </button>
                }
            })
            .collect_view()
            .into_any()
    }
}

/// Pages tab: one entry per page; the current one is highlighted.
fn nav_pages(b: Bridge) -> impl IntoView {
    let pages = b.select(|s| s.nav.as_ref().map(|n| (n.pages.clone(), n.page_hint)).unwrap_or_default());
    move || {
        let (numbers, hint) = pages.get();
        numbers
            .into_iter()
            .enumerate()
            .map(|(i, number)| {
                let b = b.clone();
                let label = fmt(tl!("Page {page}"), &[("page", &number.to_string())]);
                view! {
                    <button class="wc-pane-item" class:active=hint == i aria-current=(hint == i).then_some("page") on:mousedown=keep_focus
                        on:click=move |_| b.run("edit.goto", json!({"page": i.saturating_add(1)}))>
                        {label}
                    </button>
                }
            })
            .collect_view()
    }
}

/// Results tab: the hit count, then each hit with some context and the match highlighted.
fn nav_results(b: Bridge) -> impl IntoView {
    let results = b.select(|s| s.nav.as_ref().map(|n| (n.results.clone(), n.result_count)).unwrap_or_default());
    move || {
        let (list, count) = results.get();
        let rows = list
            .into_iter()
            .map(|r: NavResult| {
                let b = b.clone();
                let (anchor, focus) = (r.anchor, r.focus);
                view! {
                    <button class="wc-pane-item wc-nav-result" on:mousedown=keep_focus
                        on:click=move |_| b.run("select.range", json!({"anchor": anchor, "focus": focus}))>
                        {r.pre}<mark>{r.hit}</mark>{r.post}
                    </button>
                }
            })
            .collect_view();
        view! {
            <p class="wc-pane-count" role="status">{format!("{count} results")}</p>
            {rows}
        }
    }
}

/// Styles: current style, New Style / Update to Match / Clear, and every style with its actions.
fn styles_pane(b: Bridge) -> impl IntoView {
    let pane = b.select(|s| s.styles_pane.clone().unwrap_or_default());
    // The style whose action row (Modify / Update / Delete) is showing.
    let actions_for = RwSignal::new(None::<String>);
    let (b1, b2, b3, b4) = (b.clone(), b.clone(), b.clone(), b.clone());
    let current = move || pane.with(|p| fmt(tl!("Current style: {style}"), &[("style", &p.current_name)]));
    let list = move || {
        let p = pane.get();
        p.styles.into_iter().map(|item| style_row(&b4, item, &p.current, actions_for)).collect_view()
    };
    view! {
        <section class="wc-pane wc-styles-pane" role="complementary" aria-label=t("Styles")>
            {header(&b, "Styles", "view.stylesPane")}
            <p class="wc-pane-note">{current}</p>
            <div class="wc-pane-buttons">
                <button class="wc-pane-button" on:mousedown=keep_focus
                    on:click=move |_| b1.send(Action::run("ui.dialog", json!({"name": "newStyle"})))>{t("New Style…")}</button>
                <button class="wc-pane-button" on:mousedown=keep_focus
                    on:click=move |_| b2.run("styles.updateToMatch", json!({}))>{t("Update to Match")}</button>
                <button class="wc-pane-button" on:mousedown=keep_focus
                    on:click=move |_| b3.run("format.clear", json!({}))>{t("Clear")}</button>
            </div>
            <div class="wc-pane-body" role="list">{list}</div>
        </section>
    }
}

/// One Styles pane row: click applies the style; "…" (or right-click) shows its actions.
fn style_row(b: &Bridge, item: StyleItem, current: &str, actions_for: RwSignal<Option<String>>) -> impl IntoView + use<> {
    let name = style_name(&item.name);
    let active = item.id == current;
    let id = item.id;
    let open = {
        let id = id.clone();
        move || actions_for.with(|a| a.as_deref() == Some(id.as_str()))
    };
    let open2 = open.clone();
    let toggle = {
        let id = id.clone();
        move || actions_for.update(|a| *a = if a.as_deref() == Some(id.as_str()) { None } else { Some(id.clone()) })
    };
    let toggle2 = toggle.clone();
    let (b1, b2, b3, b4) = (b.clone(), b.clone(), b.clone(), b.clone());
    let (id1, id2, id3, id4) = (id.clone(), id.clone(), id.clone(), id);
    let more_label = format!("{name} …");
    view! {
        <div class="wc-style-row" role="listitem" class:active=active>
            <div class="wc-style-line" on:contextmenu=move |ev| { ev.prevent_default(); toggle(); }>
                <button class="wc-pane-item wc-style-name" aria-pressed=active.to_string() on:mousedown=keep_focus
                    on:click=move |_| b1.run("para.style", json!({"style": id1.clone()}))>
                    <span class="wc-style-label">{name}</span>
                    <span class="wc-style-kind" aria-hidden="true">{if item.is_char { "a" } else { "¶" }}</span>
                </button>
                <button class="wc-style-more" title=more_label.clone() aria-label=more_label aria-haspopup="true"
                    aria-expanded=move || open().to_string() on:mousedown=keep_focus on:click=move |_| toggle2()>"…"</button>
            </div>
            {move || open2().then(|| {
                let (b2, b3, b4) = (b2.clone(), b3.clone(), b4.clone());
                let (id2, id3, id4) = (id2.clone(), id3.clone(), id4.clone());
                view! {
                    <div class="wc-style-actions" role="group">
                        <button class="wc-pane-button" on:mousedown=keep_focus on:click=move |_| {
                            actions_for.set(None);
                            b2.send(Action::ModifyStyle(id2.clone()));
                        }>{t("Modify…")}</button>
                        <button class="wc-pane-button" on:mousedown=keep_focus on:click=move |_| {
                            actions_for.set(None);
                            b3.run("styles.updateToMatch", json!({"style": id3.clone()}));
                        }>{t("Update to Match Selection")}</button>
                        <button class="wc-pane-button" on:mousedown=keep_focus on:click=move |_| {
                            actions_for.set(None);
                            b4.run("styles.delete", json!({"style": id4.clone()}));
                        }>{t("Delete")}</button>
                    </div>
                }
            })}
        </div>
    }
}

/// Comments: New comment, then one card per comment (replies indented).
fn comments_pane(b: Bridge) -> impl IntoView {
    let comments = b.select(|s| s.comments.clone().unwrap_or_default());
    let b1 = b.clone();
    let b2 = b.clone();
    let list = move || comments.get().into_iter().map(|c| comment_card(&b2, c)).collect_view();
    view! {
        <section class="wc-pane wc-comments-pane" role="complementary" aria-label=t("Comments")>
            {header(&b, "Comments", "view.commentsPane")}
            <div class="wc-pane-buttons">
                <button class="wc-pane-button" on:mousedown=keep_focus
                    on:click=move |_| b1.run("review.newComment", json!({"text": ""}))>{t("➕ New comment")}</button>
            </div>
            <div class="wc-pane-body" role="list">{list}</div>
        </section>
    }
}

/// One comment: author, date, Resolved; editable text (saved on blur when changed); actions.
fn comment_card(b: &Bridge, c: CommentItem) -> impl IntoView + use<> {
    let id = c.id;
    let original = c.text.clone();
    let (b1, b2, b3, b4) = (b.clone(), b.clone(), b.clone(), b.clone());
    let anchor = c.anchor;
    let has_anchor = anchor.is_some();
    let label = format!("{} — {}", c.author, c.date);
    // Same undoable command the balloons use; only sent when the text really changed.
    let save = move |ev: web_sys::FocusEvent| {
        let text = event_target_value(&ev);
        if text != original {
            b1.send(Action::run("review.editComment", json!({"id": id, "text": text})));
        }
    };
    view! {
        <div class="wc-comment" role="listitem" class:reply=c.reply class:resolved=c.resolved>
            <div class="wc-comment-head">
                <span class="wc-comment-author">{c.author}</span>
                <span class="wc-comment-date">{c.date}</span>
                {c.resolved.then(|| view! { <span class="wc-comment-resolved">{t("Resolved")}</span> })}
            </div>
            <textarea class="wc-comment-text" rows="2" placeholder=t("Add a comment") aria-label=label prop:value=c.text
                on:blur=save></textarea>
            <div class="wc-comment-actions">
                <button class="wc-pane-button" disabled=!has_anchor on:mousedown=keep_focus on:click=move |_| {
                    if let Some(a) = &anchor {
                        b2.run("caret.set", json!({"pos": a}));
                    }
                }>{t("Go to")}</button>
                <button class="wc-pane-button" on:mousedown=keep_focus
                    on:click=move |_| b3.send(Action::run("review.resolveComment", json!({"id": id})))>{t("Resolve")}</button>
                <button class="wc-pane-button" on:mousedown=keep_focus
                    on:click=move |_| b4.send(Action::run("review.deleteComment", json!({"id": id})))>{t("Delete")}</button>
            </div>
        </div>
    }
}
