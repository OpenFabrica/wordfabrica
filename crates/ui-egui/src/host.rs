//! Hosted chrome: a front end that draws the window chrome itself (the web build's HTML ribbon,
//! panes, dialogs and File screen) reads one [`ChromeState`] snapshot per frame and sends
//! [`Action`]s back. The document, selection, undo history and open dialog all stay in
//! [`WordApp`], so the HTML chrome and the egui document canvas can never disagree.
//!
//! The snapshot is plain data (no egui types) and compares with `==`, so a host only redraws when
//! something it shows changed.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use wordcraft_doc::{Pos, StoryRef};

use crate::WordApp;
use crate::dialogs::{Dialog, DialogButton};

/// Which parts of the chrome the host draws itself; egui skips them. Desktop uses
/// [`HostedChrome::NONE`] (egui draws everything).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostedChrome {
    pub title_bar: bool,
    pub ribbon: bool,
    pub status_bar: bool,
    pub panes: bool,
    pub dialogs: bool,
    pub backstage: bool,
}

impl HostedChrome {
    /// egui draws the whole window.
    pub const NONE: HostedChrome =
        HostedChrome { title_bar: false, ribbon: false, status_bar: false, panes: false, dialogs: false, backstage: false };
    /// The host draws all chrome; egui draws only the document canvas.
    pub const ALL: HostedChrome = HostedChrome { title_bar: true, ribbon: true, status_bar: true, panes: true, dialogs: true, backstage: true };
}

/// Character formatting at the selection (`format.state`); `None` means mixed.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatState {
    pub font: Option<String>,
    pub size: Option<f64>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub subscript: bool,
    pub superscript: bool,
    pub color: Option<String>,
    pub highlight: Option<String>,
    pub lang: Option<String>,
    /// Paragraph style id and display name at the caret.
    pub style: String,
    pub style_name: String,
}

impl FormatState {
    /// Read the JSON `format.state` returns.
    pub fn from_value(v: &Value) -> FormatState {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let b = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
        FormatState {
            font: s("font"),
            size: v.get("size").and_then(Value::as_f64),
            bold: b("bold"),
            italic: b("italic"),
            underline: b("underline"),
            strike: b("strike"),
            subscript: b("subscript"),
            superscript: b("superscript"),
            color: s("color"),
            highlight: s("highlight"),
            lang: s("lang"),
            style: s("style").unwrap_or_else(|| "Normal".into()),
            style_name: s("styleName").unwrap_or_default(),
        }
    }
}

/// Paragraph numbers the Layout tab edits in place.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParaState {
    /// `left`, `center`, `right` or `justify`.
    pub align: String,
    /// Inches.
    pub indent_left: f32,
    pub indent_right: f32,
    /// Points.
    pub space_before: f32,
    pub space_after: f32,
}

/// Server save state of the hosted web editor.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct HostedStatus {
    /// English source text; the host translates it.
    pub text: String,
    pub revision: u64,
    pub failed: bool,
}

/// What the status bar shows.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusState {
    pub page: usize,
    pub pages: usize,
    pub words: usize,
    /// Words in the selection when it isn't collapsed.
    pub selected_words: Option<usize>,
    pub header_footer: bool,
    /// The selection's document language (`None`: several).
    pub doc_lang: Option<String>,
    /// The document-wide default language the language menu marks as current.
    pub default_lang: String,
    pub track_changes: bool,
    /// The host opened the document read-only (e.g. a view link): the edit-mode button says so.
    pub read_only: bool,
    pub hosted: Option<HostedStatus>,
    /// The latest status message (English source or already formatted) and a counter that
    /// changes with every message, so the host can show each one for a few seconds.
    pub message: Option<String>,
    pub message_seq: u64,
    pub zoom_percent: f32,
}

/// One style in the Home Styles gallery or the Styles pane.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleItem {
    pub id: String,
    pub name: String,
    pub is_char: bool,
}

/// Navigation pane contents (only the active tab is filled).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavState {
    /// `headings`, `pages` or `results`.
    pub tab: String,
    pub headings: Vec<NavHeading>,
    pub current_heading: Option<usize>,
    /// Printed page numbers.
    pub pages: Vec<u32>,
    pub page_hint: usize,
    pub results: Vec<NavResult>,
    pub result_count: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NavHeading {
    pub pos: Pos,
    pub level: u8,
    pub text: String,
}

/// A search hit with some context on each side.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NavResult {
    pub pre: String,
    pub hit: String,
    pub post: String,
    pub anchor: Pos,
    pub focus: Pos,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StylesPaneState {
    pub current: String,
    pub current_name: String,
    pub styles: Vec<StyleItem>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CommentItem {
    pub id: u64,
    pub author: String,
    pub date: String,
    pub resolved: bool,
    pub reply: bool,
    pub text: String,
    pub anchor: Option<Value>,
}

/// Table Design ▸ Table Style Options.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableState {
    pub header_row: bool,
    pub first_column: bool,
    pub total_row: bool,
    pub last_column: bool,
    pub banded_rows: bool,
    pub banded_columns: bool,
    /// Table styles (id, name) for the gallery.
    pub styles: Vec<StyleItem>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ServerDocument {
    pub id: String,
    pub title: String,
    pub revision: u64,
}

/// Hosted web: the server document list on File ▸ Open.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerList {
    pub documents: Vec<ServerDocument>,
    pub listing: bool,
    pub list_error: Option<String>,
    pub busy: bool,
}

/// Document properties edited on File ▸ Info.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Properties {
    pub title: String,
    pub subject: String,
    pub author: String,
    pub keywords: String,
    pub category: String,
}

/// File screen contents (only while it's open).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackstageState {
    pub page: String,
    pub recent: Vec<String>,
    pub server: Option<ServerList>,
    pub properties: Properties,
    /// `file.info` (statistics and path).
    pub info: Value,
    /// Interface language preference (`auto` or a code).
    pub language: String,
    pub autosave: bool,
}

/// Everything the window chrome shows, in one comparable snapshot.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromeState {
    /// Interface language code in effect (strings are translated with [`crate::i18n::t`]).
    pub lang: String,
    pub dark: bool,
    /// Document name and the browser/window title.
    pub title: String,
    pub window_title: String,
    /// `Edited`, `Saved`, `Not saved` (English source), or empty when storage is hosted.
    pub title_status: String,
    pub dirty: bool,
    pub hosted_storage: bool,
    /// AutoSave switch: whether it shows on, and whether the document can autosave at all.
    pub autosave: bool,
    pub autosave_ready: bool,
    pub author: String,
    pub show_discord: bool,
    pub tab: String,
    pub backstage: bool,
    pub ribbon_collapsed: bool,
    pub in_table: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_label: String,
    pub format: FormatState,
    pub para: ParaState,
    /// Named on/off states for toggle buttons (see [`WordApp::chrome_state`]).
    pub toggles: BTreeMap<String, bool>,
    /// Of the command ids the host asked about, those that are disabled right now.
    pub disabled: BTreeSet<String>,
    pub status: StatusState,
    /// The last colours picked on split buttons (`highlight`, `fontColor`, `shading`).
    pub last_colors: BTreeMap<String, String>,
    /// The document theme's colours (hex), for colour grids.
    pub theme_colors: Vec<String>,
    /// Home ▸ Styles gallery and a key that changes whenever its previews must re-render.
    pub gallery: Vec<StyleItem>,
    pub styles_key: u64,
    pub table: Option<TableState>,
    pub dialog: Option<Dialog>,
    /// The document's bookmark names, only while the Bookmark dialog is open (it lists them).
    pub bookmarks: Vec<String>,
    pub nav: Option<NavState>,
    pub styles_pane: Option<StylesPaneState>,
    pub comments: Option<Vec<CommentItem>>,
    pub file: Option<BackstageState>,
}

/// Split buttons remember the colour picked last.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorSlot {
    Highlight,
    FontColor,
    Shading,
}

/// File screen actions that need the host's services (pickers, downloads, server storage).
#[derive(Clone, Debug, PartialEq)]
pub enum FileAction {
    /// Save a copy with this extension (`pdf`, `docx`, … or `png`).
    Export(String),
    SaveAs,
    /// Hosted: open a server document (refused while there are unsaved changes).
    OpenServer(String),
    RefreshServer,
    Properties(Properties),
}

/// What the hosted chrome asks the app to do. Everything ends in [`WordApp::run`] or a small
/// piece of UI state, so the CLI, MCP and shortcuts reach the same behaviour.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Run a command by id (`ui.*` or engine).
    Run {
        id: String,
        params: Value,
    },
    /// Remember a colour for a split button's main half.
    LastColor {
        slot: ColorSlot,
        hex: String,
    },
    /// A dialog button: `dialog` carries the form as the user left it.
    Dialog {
        dialog: Dialog,
        button: DialogButton,
    },
    /// Open Modify Style for a style id.
    ModifyStyle(String),
    /// Navigation pane search (empty clears the results) and tab.
    NavSearch(String),
    NavTab(String),
    File(FileAction),
    /// File ▸ Options.
    SetAuthor(String),
    SetAutosave(bool),
    SetShowDiscord(bool),
    /// Title bar AutoSave switch: toggles, or asks for a file name first.
    ToggleAutosave,
    /// Put the selection on the system clipboard (after `edit.copy` / `edit.cut`).
    CopySelection,
    /// Give keyboard focus back to the document.
    FocusDocument,
}

impl Action {
    /// Shorthand for [`Action::Run`].
    pub fn run(id: &str, params: Value) -> Action {
        Action::Run { id: id.to_string(), params }
    }
}

impl WordApp {
    /// The chrome snapshot. `ids` are the command ids the host shows; their enabled state is
    /// reported in [`ChromeState::disabled`]. Toggle names: `painter`, `marks`, `trackChanges`,
    /// `showComments`, `showMarkup`, `ruler`, `gridlines`, `navPane`, `stylesPane`,
    /// `commentsPane`, `onePage`, `multiplePages`, `pageWidth`, `hyphenation`, `webLayout`,
    /// `printLayout`, `readMode`.
    pub fn chrome_state(&mut self, ids: &[&str]) -> ChromeState {
        let lang = crate::i18n::Lang::from_pref(&self.ui.language);
        let fmt_value = wordcraft_engine::cmd::format::state(&self.session);
        let format = FormatState::from_value(&fmt_value);
        let para = self
            .session
            .doc
            .para_at(&self.session.sel.focus)
            .map(|p| self.session.doc.styles.resolve_para(&p.props))
            .map(|rp| ParaState {
                align: format!("{:?}", rp.align).to_lowercase(),
                indent_left: rp.indent_left / 72.0,
                indent_right: rp.indent_right / 72.0,
                space_before: rp.space_before,
                space_after: rp.space_after,
            })
            .unwrap_or_default();
        let v = &self.session.view;
        let mode = v.mode;
        let mut toggles = BTreeMap::new();
        for (k, on) in [
            ("painter", self.session.painter.is_some()),
            ("marks", v.marks),
            ("trackChanges", self.session.doc.settings.track_changes),
            ("showComments", v.show_comments),
            ("showMarkup", v.show_markup),
            ("ruler", v.ruler),
            ("gridlines", v.gridlines),
            ("navPane", v.nav_pane),
            ("stylesPane", v.styles_pane),
            ("commentsPane", v.comments_pane),
            ("onePage", v.fit == "onePage"),
            ("multiplePages", v.multi_page),
            ("pageWidth", v.fit == "pageWidth"),
            ("hyphenation", self.session.doc.settings.auto_hyphenation),
            ("webLayout", mode == wordcraft_layout::ViewMode::Web),
            ("printLayout", mode == wordcraft_layout::ViewMode::Print && !v.read_mode),
            ("readMode", v.read_mode),
        ] {
            toggles.insert(k.to_string(), on);
        }
        let disabled = ids.iter().filter(|id| !crate::widgets::enabled(self, id)).map(|id| id.to_string()).collect();
        let hosted_storage = self.services.request_save.is_some();
        let title_status = if hosted_storage {
            ""
        } else if self.session.dirty {
            "Edited"
        } else if self.session.path.is_some() {
            "Saved"
        } else {
            "Not saved"
        };
        let in_table = self.session.sel.focus.path.cell().is_some();
        let gallery = self
            .session
            .doc
            .styles
            .gallery()
            .iter()
            .map(|s| StyleItem { id: s.id.clone(), name: s.name.clone(), is_char: s.kind == wordcraft_doc::StyleKind::Character })
            .collect();
        let rev = self.session.rev();
        let styles_key = self.previews.styles_key(&self.session.doc, rev);
        let last_colors =
            [("highlight", &self.canvas.last_highlight), ("fontColor", &self.canvas.last_font_color), ("shading", &self.canvas.last_shading)]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect();
        ChromeState {
            lang: lang.code().to_string(),
            dark: self.ui.dark || self.session.view.dark_mode,
            title: self.title_stem(),
            window_title: self.window_title(),
            title_status: title_status.to_string(),
            dirty: self.session.dirty,
            hosted_storage,
            autosave: (self.session.path.is_some() || hosted_storage) && self.autosave,
            autosave_ready: self.session.path.is_some() || hosted_storage,
            author: self.session.author.clone(),
            show_discord: self.ui.show_discord,
            tab: self.ui.tab.clone(),
            backstage: self.ui.backstage,
            ribbon_collapsed: self.ui.ribbon_collapsed,
            in_table,
            can_undo: self.session.can_undo(),
            can_redo: self.session.can_redo(),
            undo_label: self.session.undo_label().unwrap_or("").to_string(),
            format,
            para,
            toggles,
            disabled,
            status: self.status_state(),
            last_colors,
            theme_colors: self.session.doc.settings.theme_colors.iter().map(|c| c.hex()).collect(),
            gallery,
            styles_key,
            table: if in_table { self.table_state() } else { None },
            dialog: self.dialog.clone(),
            bookmarks: if matches!(self.dialog, Some(Dialog::Bookmark { .. })) {
                self.session.doc.bookmarks().into_iter().map(|(n, _)| n).collect()
            } else {
                Vec::new()
            },
            nav: self.session.view.nav_pane.then(|| self.nav_state()),
            styles_pane: self.session.view.styles_pane.then(|| self.styles_pane_state(&fmt_value)),
            comments: self.session.view.comments_pane.then(|| self.comment_items()),
            file: self.ui.backstage.then(|| self.backstage_state()),
        }
    }

    fn status_state(&mut self) -> StatusState {
        let (page, pages) = self.session.page_status();
        let words = self.cached_word_count();
        let selected_words = (!self.session.sel.is_collapsed()).then(|| wordcraft_doc::count_words(&self.session.selected_text()));
        let doc_lang = wordcraft_engine::cmd::format::state(&self.session).get("lang").and_then(Value::as_str).map(str::to_string);
        let hosted = self.hosted.as_ref().map(|h| HostedStatus { text: h.status.to_string(), revision: h.revision, failed: h.failed });
        StatusState {
            page,
            pages,
            words,
            selected_words,
            header_footer: self.session.sel.focus.story != StoryRef::Body,
            doc_lang,
            default_lang: self.session.doc.styles.default_chr.lang.clone().unwrap_or_else(|| "en-US".into()),
            track_changes: self.session.doc.settings.track_changes,
            read_only: self.session.read_only,
            hosted,
            message: self.status_msg.as_ref().map(|(m, _)| m.clone()),
            message_seq: self.status_seq,
            zoom_percent: (self.canvas.scale / crate::canvas::PX_PER_PT * 100.0).round(),
        }
    }

    fn table_state(&self) -> Option<TableState> {
        let (tp, _, _) = self.session.sel.focus.path.cell()?;
        let look = self.session.doc.table(self.session.sel.focus.story, &tp)?.props.look;
        let styles = self
            .session
            .doc
            .styles
            .styles
            .iter()
            .filter(|s| s.kind == wordcraft_doc::StyleKind::Table && !s.hidden)
            .map(|s| StyleItem { id: s.id.clone(), name: s.name.clone(), is_char: false })
            .collect();
        Some(TableState {
            header_row: look.header_row,
            first_column: look.first_column,
            total_row: look.total_row,
            last_column: look.last_column,
            banded_rows: look.banded_rows,
            banded_columns: look.banded_columns,
            styles,
        })
    }

    fn nav_state(&mut self) -> NavState {
        let tab = self.ui.nav_tab.clone();
        let mut st = NavState { tab: tab.clone(), page_hint: self.session.page_hint, ..Default::default() };
        match tab.as_str() {
            "pages" => {
                st.pages = self.session.layout().pages.iter().map(|p| p.number).collect();
            }
            "results" => {
                st.result_count = self.session.find.results.len();
                for (a, b) in self.session.find.results.iter().take(500) {
                    let Some(p) = self.session.doc.para_at(a) else { continue };
                    let from = p.clamp(a.off.saturating_sub(30));
                    let to = p.clamp(b.off.saturating_add(40).min(p.len()));
                    st.results.push(NavResult {
                        pre: p.text.get(from..a.off).unwrap_or("").to_string(),
                        hit: p.text.get(a.off..b.off).unwrap_or("").to_string(),
                        post: p.text.get(b.off..to).unwrap_or("").to_string(),
                        anchor: a.clone(),
                        focus: b.clone(),
                    });
                }
            }
            _ => {
                let caret = self.session.sel.focus.clone();
                for path in self.session.doc.para_paths(StoryRef::Body) {
                    let Some(p) = self.session.doc.para(StoryRef::Body, &path) else { continue };
                    let rp = self.session.doc.styles.resolve_para(&p.props);
                    let Some(level) = rp.outline_level else { continue };
                    let text = p.plain_text().trim().to_string();
                    if text.is_empty() || rp.style.starts_with("TOC") {
                        continue;
                    }
                    if path <= caret.path {
                        st.current_heading = Some(st.headings.len());
                    }
                    st.headings.push(NavHeading { pos: Pos { story: StoryRef::Body, path, off: 0 }, level, text });
                }
            }
        }
        st
    }

    fn styles_pane_state(&self, fmt_value: &Value) -> StylesPaneState {
        let mut styles: Vec<StyleItem> = self
            .session
            .doc
            .styles
            .styles
            .iter()
            .filter(|s| !s.hidden && s.kind != wordcraft_doc::StyleKind::Table)
            .map(|s| StyleItem { id: s.id.clone(), name: s.name.clone(), is_char: s.kind == wordcraft_doc::StyleKind::Character })
            .collect();
        styles.sort_by_key(|s| s.name.to_lowercase());
        StylesPaneState {
            current: fmt_value.get("style").and_then(Value::as_str).unwrap_or("Normal").to_string(),
            current_name: fmt_value.get("styleName").and_then(Value::as_str).unwrap_or("").to_string(),
            styles,
        }
    }

    fn comment_items(&mut self) -> Vec<CommentItem> {
        let list = self.session.run("review.comments", &json!({})).unwrap_or_default();
        let mut out = Vec::new();
        for c in list.as_array().cloned().unwrap_or_default() {
            let id = c.get("id").and_then(Value::as_u64).unwrap_or(0);
            let text = u32::try_from(id)
                .ok()
                .and_then(|i| self.session.doc.comments.get(&i))
                .map(|x| self.session.doc.plain_text(StoryRef::Part(x.part)))
                .unwrap_or_default();
            out.push(CommentItem {
                id,
                author: c.get("author").and_then(Value::as_str).unwrap_or("").to_string(),
                date: c.get("date").and_then(Value::as_str).unwrap_or("").get(..10).unwrap_or("").to_string(),
                resolved: c.get("resolved").and_then(Value::as_bool).unwrap_or(false),
                reply: c.get("parent").is_some_and(|p| !p.is_null()),
                text,
                anchor: c.get("anchor").filter(|a| !a.is_null()).cloned(),
            });
        }
        out
    }

    fn backstage_state(&mut self) -> BackstageState {
        let core = &self.session.doc.core;
        let properties = Properties {
            title: core.title.clone(),
            subject: core.subject.clone(),
            author: core.creator.clone(),
            keywords: core.keywords.clone(),
            category: core.category.clone(),
        };
        let dirty = self.session.dirty;
        let server = self.hosted.as_ref().map(|h| ServerList {
            documents: h.documents.iter().map(|d| ServerDocument { id: d.id.clone(), title: d.title.clone(), revision: d.revision }).collect(),
            listing: h.listing,
            list_error: h.list_error.clone(),
            busy: h.busy || dirty,
        });
        BackstageState {
            page: self.ui.backstage_page.clone(),
            recent: self.ui.recent.clone(),
            server,
            properties,
            info: if self.ui.backstage_page == "info" { self.session.run("file.info", &json!({})).unwrap_or_default() } else { Value::Null },
            language: self.ui.language.clone(),
            autosave: self.autosave,
        }
    }

    /// Carry out an action from the hosted chrome.
    pub fn apply(&mut self, action: Action) {
        match action {
            Action::Run { id, params } => {
                let _ = self.run(&id, params);
            }
            Action::LastColor { slot, hex } => match slot {
                ColorSlot::Highlight => self.canvas.last_highlight = hex,
                ColorSlot::FontColor => self.canvas.last_font_color = hex,
                ColorSlot::Shading => self.canvas.last_shading = hex,
            },
            Action::Dialog { mut dialog, button } => {
                // Like `dialogs::show`: the dialog is out of `self.dialog` while its button runs, so
                // a command that opens another dialog (Search Commands ▸ Word Count) keeps it open.
                self.dialog = None;
                let close = crate::dialogs::press(self, &mut dialog, &button);
                if !close {
                    self.dialog = Some(dialog);
                } else if self.dialog.is_none() {
                    self.canvas.want_focus = true;
                }
            }
            Action::ModifyStyle(id) => self.dialog = Dialog::modify_style(self, &id),
            Action::NavSearch(q) => {
                // Like the egui pane: search without moving the caret or scrolling.
                if q.is_empty() {
                    self.session.find.results.clear();
                } else {
                    let _ = self.session.run("edit.find", &json!({"text": q}));
                }
                self.ui.nav_tab = "results".into();
            }
            Action::NavTab(tab) => self.ui.nav_tab = tab,
            Action::File(f) => self.file_action(f),
            Action::SetAuthor(name) => self.session.author = name,
            Action::SetAutosave(on) => self.autosave = on,
            Action::SetShowDiscord(on) => self.ui.show_discord = on,
            Action::ToggleAutosave => {
                if self.session.path.is_some() || self.services.request_save.is_some() {
                    self.toggle_autosave();
                } else {
                    self.save_as_dialog();
                }
            }
            Action::CopySelection => self.copy_out = Some(self.session.clipboard_text.clone()),
            Action::FocusDocument => self.canvas.want_focus = true,
        }
    }

    fn file_action(&mut self, f: FileAction) {
        match f {
            FileAction::Export(ext) => self.export_copy(&ext),
            FileAction::SaveAs => self.save_as_dialog(),
            FileAction::OpenServer(id) => {
                let dirty = self.session.dirty;
                if let Some(h) = &mut self.hosted
                    && !dirty
                    && !h.busy
                {
                    h.open = Some(id);
                    self.ui.backstage = false;
                }
            }
            FileAction::RefreshServer => {
                if let Some(h) = &mut self.hosted {
                    h.refresh = true;
                }
            }
            FileAction::Properties(p) => {
                let _ = self.run(
                    "file.properties",
                    json!({"title": p.title, "subject": p.subject, "author": p.author, "keywords": p.keywords, "category": p.category}),
                );
            }
        }
    }

    /// File ▸ Export: save a copy in another format (a download on the web).
    pub fn export_copy(&mut self, ext: &str) {
        let name = format!("{}.{ext}", self.title_stem());
        let picked = self.services.pick_save.as_ref().and_then(|f| f(&name));
        if let Some(path) = picked {
            let r = if ext == "png" { self.run("file.exportPng", json!({"path": path})) } else { self.run("file.saveAs", json!({"path": path})) };
            if r.is_ok() {
                self.status(crate::i18n::fmt(tl!("Exported {path}"), &[("path", &path)]));
            }
        }
    }

    /// Every command for a command palette: (id, label, location, shortcut), hidden ones left out.
    pub fn command_list(&self) -> Vec<CommandInfo> {
        self.session
            .registry
            .all()
            .iter()
            .filter(|c| !crate::ribbon::hidden_in_ui(c.id))
            .map(|c| CommandInfo {
                id: c.id.to_string(),
                label: c.label.to_string(),
                location: c.location.to_string(),
                shortcut: c.shortcut.to_string(),
            })
            .collect()
    }
}

/// A registry entry for hosted command search and tooltips.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CommandInfo {
    pub id: String,
    pub label: String,
    pub location: String,
    /// Engine notation (`Mod+B`, alternatives separated by ` / `).
    pub shortcut: String,
}

/// Shortcut text for display (`⌘B` on macOS, `Ctrl+B` elsewhere), from engine notation.
pub fn shortcut_display(shortcut: &str, mac: bool) -> String {
    let sc = shortcut.split(" / ").next().unwrap_or("");
    if sc.is_empty() {
        return String::new();
    }
    if mac { sc.replace("Mod+", "⌘").replace("Shift+", "⇧").replace("Alt+", "⌥").replace("Ctrl+", "⌃") } else { sc.replace("Mod+", "Ctrl+") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Services;
    use wordcraft_engine::Session;

    fn app() -> WordApp {
        WordApp::new(Session::new(wordcraft_doc::Document::new()), Services::default())
    }

    #[test]
    fn snapshot_follows_bold_and_undo() {
        let mut a = app();
        a.run("text.insert", json!({"text": "Hello"})).unwrap();
        a.run("select.all", json!({})).unwrap();
        let before = a.chrome_state(&["format.bold", "edit.undo"]);
        assert!(!before.format.bold);
        a.apply(Action::run("format.bold", json!({})));
        let bold = a.chrome_state(&["format.bold", "edit.undo"]);
        assert!(bold.format.bold);
        assert!(bold.can_undo);
        assert_ne!(before, bold);
        a.apply(Action::run("edit.undo", json!({})));
        let undone = a.chrome_state(&["format.bold", "edit.undo"]);
        assert!(!undone.format.bold);
        // Nothing changed between two snapshots: they compare equal, so hosts skip the redraw.
        assert_eq!(undone, a.chrome_state(&["format.bold", "edit.undo"]));
    }

    #[test]
    fn snapshot_reports_disabled_ids_and_survives_unknown_ones() {
        let mut a = app();
        let s = a.chrome_state(&["table.insertRowAbove", "no.such.command", "ui.dialog"]);
        assert!(s.disabled.contains("table.insertRowAbove"), "{:?}", s.disabled);
        assert!(s.disabled.contains("no.such.command"));
        assert!(!s.disabled.contains("ui.dialog"));
        a.apply(Action::run("no.such.command", json!({"junk": [1, 2]})));
    }

    #[test]
    fn dialog_actions_run_their_command_and_close() {
        let mut a = app();
        a.apply(Action::run("ui.dialog", json!({"name": "insertTable"})));
        let Some(Dialog::InsertTable { .. }) = a.chrome_state(&[]).dialog else { panic!("dialog not open") };
        a.apply(Action::Dialog { dialog: Dialog::InsertTable { rows: 3, cols: 4 }, button: DialogButton::Ok });
        assert!(a.dialog.is_none());
        assert!(a.canvas.want_focus);
        assert!(a.session.sel.focus.path.cell().is_some(), "caret should be in the new table");
        assert!(a.chrome_state(&[]).in_table);
    }

    #[test]
    fn a_dialog_button_can_open_another_dialog() {
        let mut a = app();
        a.apply(Action::run("ui.dialog", json!({"name": "commands"})));
        let Some(dialog) = a.dialog.clone() else { panic!("commands dialog should open") };
        a.apply(Action::Dialog { dialog, button: DialogButton::Command("ui.dialog".into()) });
        assert!(a.dialog.is_none(), "ui.dialog without a name opens nothing and the search closes");
        let Some(dialog) = Dialog::open("commands", &mut a) else { panic!("commands dialog") };
        a.apply(Action::Dialog { dialog, button: DialogButton::Command("edit.goto".into()) });
        assert!(matches!(a.chrome_state(&[]).dialog, Some(Dialog::Goto { .. })), "{:?}", a.dialog);
    }

    #[test]
    fn panes_and_file_screen_fill_only_while_open() {
        let mut a = app();
        let s = a.chrome_state(&[]);
        assert!(s.nav.is_none() && s.styles_pane.is_none() && s.comments.is_none() && s.file.is_none());
        a.apply(Action::run("view.navigationPane", json!({"value": true})));
        a.apply(Action::run("view.stylesPane", json!({"value": true})));
        a.apply(Action::run("ui.backstage", json!({"value": true, "page": "info"})));
        let s = a.chrome_state(&[]);
        assert!(s.nav.is_some());
        assert!(s.styles_pane.as_ref().is_some_and(|p| !p.styles.is_empty()));
        assert_eq!(s.file.as_ref().map(|f| f.page.as_str()), Some("info"));
    }

    #[test]
    fn nav_search_fills_results() {
        let mut a = app();
        a.run("text.insert", json!({"text": "alpha beta alpha"})).unwrap();
        a.apply(Action::run("view.navigationPane", json!({"value": true})));
        a.apply(Action::NavSearch("alpha".into()));
        let nav = a.chrome_state(&[]).nav.unwrap_or_default();
        assert_eq!(nav.tab, "results");
        assert_eq!(nav.result_count, 2);
        assert_eq!(nav.results.first().map(|r| r.hit.as_str()), Some("alpha"));
        a.apply(Action::NavSearch(String::new()));
        assert_eq!(a.chrome_state(&[]).nav.unwrap_or_default().result_count, 0);
    }

    #[test]
    fn shortcut_display_per_platform() {
        assert_eq!(shortcut_display("Mod+Shift+B / Ctrl+B", true), "⌘⇧B");
        assert_eq!(shortcut_display("Mod+B", false), "Ctrl+B");
        assert_eq!(shortcut_display("", false), "");
    }
}
