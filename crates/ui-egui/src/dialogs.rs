//! Dialogs: Font, Paragraph, Find & Replace, Go To, Insert Table, Page Setup, Link, Bookmark,
//! Word Count, Zoom, Watermark, New/Modify Style, Command search, About. Every dialog ends by
//! running a command, so agents get the same result without the dialog.

use egui::{Sense, Ui, vec2};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::WordApp;
use crate::theme::{Tokens, semibold};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "dialog", rename_all = "camelCase")]
pub enum Dialog {
    Font {
        font: String,
        size: String,
        bold: bool,
        italic: bool,
        underline: bool,
        strike: bool,
        sup: bool,
        sub: bool,
        small_caps: bool,
        caps: bool,
        hidden: bool,
        color: String,
        spacing: f32,
    },
    Paragraph {
        align: String,
        left: f32,
        right: f32,
        first: f32,
        before: f32,
        after: f32,
        line: f32,
        keep_next: bool,
        keep_lines: bool,
        page_break: bool,
        widow: bool,
    },
    Find {
        query: String,
        replace: String,
        match_case: bool,
        whole_word: bool,
        regex: bool,
        replace_mode: bool,
        message: String,
    },
    Goto {
        page: String,
    },
    InsertTable {
        rows: u32,
        cols: u32,
    },
    PageSetup {
        top: f32,
        bottom: f32,
        left: f32,
        right: f32,
        landscape: bool,
    },
    Link {
        url: String,
        text: String,
    },
    Bookmark {
        name: String,
    },
    WordCount {
        stats: Value,
    },
    Zoom {
        percent: f32,
    },
    Watermark {
        text: String,
        diagonal: bool,
    },
    NewStyle {
        name: String,
        based_on: String,
    },
    ModifyStyle {
        id: String,
        name: String,
        font: String,
        size: f32,
        bold: bool,
        italic: bool,
        color: String,
        before: f32,
        after: f32,
    },
    Commands {
        query: String,
    },
    /// Tabs: 0 About, 1 Contributors, 2 Models.
    About {
        tab: u8,
    },
}

impl Dialog {
    pub fn name(&self) -> &'static str {
        match self {
            Dialog::Font { .. } => "font",
            Dialog::Paragraph { .. } => "paragraph",
            Dialog::Find { replace_mode: false, .. } => "find",
            Dialog::Find { .. } => "replace",
            Dialog::Goto { .. } => "goto",
            Dialog::InsertTable { .. } => "insertTable",
            Dialog::PageSetup { .. } => "pageSetup",
            Dialog::Link { .. } => "link",
            Dialog::Bookmark { .. } => "bookmark",
            Dialog::WordCount { .. } => "wordCount",
            Dialog::Zoom { .. } => "zoom",
            Dialog::Watermark { .. } => "watermark",
            Dialog::NewStyle { .. } => "newStyle",
            Dialog::ModifyStyle { .. } => "modifyStyle",
            Dialog::Commands { .. } => "commands",
            Dialog::About { .. } => "about",
        }
    }

    pub fn open(name: &str, app: &mut WordApp) -> Option<Dialog> {
        let st = app.session.run("format.state", &json!({})).unwrap_or_default();
        let s = |k: &str| st.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let b = |k: &str| st.get(k).and_then(Value::as_bool).unwrap_or(false);
        Some(match name {
            "font" => Dialog::Font {
                font: s("font"),
                size: st.get("size").and_then(Value::as_f64).map(|v| v.to_string()).unwrap_or_default(),
                bold: b("bold"),
                italic: b("italic"),
                underline: b("underline"),
                strike: b("strike"),
                sup: b("superscript"),
                sub: b("subscript"),
                small_caps: false,
                caps: false,
                hidden: false,
                color: s("color"),
                spacing: 0.0,
            },
            "paragraph" => {
                let rp = app.session.doc.para_at(&app.session.sel.focus).map(|p| app.session.doc.styles.resolve_para(&p.props));
                let rp = rp?;
                let line = match rp.line_spacing {
                    wordfabrica_doc::props::LineSpacing::Multiple(m) => m,
                    _ => 1.0,
                };
                Dialog::Paragraph {
                    align: format!("{:?}", rp.align).to_lowercase(),
                    left: rp.indent_left / 72.0,
                    right: rp.indent_right / 72.0,
                    first: rp.indent_first / 72.0,
                    before: rp.space_before,
                    after: rp.space_after,
                    line,
                    keep_next: rp.keep_next,
                    keep_lines: rp.keep_lines,
                    page_break: rp.page_break_before,
                    widow: rp.widow_control,
                }
            }
            "find" | "replace" => Dialog::Find {
                query: if app.session.sel.is_collapsed() { app.session.find.query.clone() } else { app.session.selected_text() },
                replace: app.session.find.replace.clone(),
                match_case: app.session.find.match_case,
                whole_word: app.session.find.whole_word,
                regex: app.session.find.regex,
                replace_mode: name == "replace",
                message: String::new(),
            },
            "goto" => Dialog::Goto { page: String::new() },
            "insertTable" => Dialog::InsertTable { rows: 2, cols: 5 },
            "pageSetup" => {
                let sp = wordfabrica_engine::cmd::page::sect(&app.session);
                Dialog::PageSetup {
                    top: sp.margin_top / 72.0,
                    bottom: sp.margin_bottom / 72.0,
                    left: sp.margin_left / 72.0,
                    right: sp.margin_right / 72.0,
                    landscape: sp.landscape,
                }
            }
            "link" => Dialog::Link { url: "https://".into(), text: app.session.selected_text() },
            "bookmark" => Dialog::Bookmark { name: String::new() },
            "wordCount" => Dialog::WordCount { stats: app.session.run("review.wordCount", &json!({})).unwrap_or_default() },
            "zoom" => Dialog::Zoom { percent: (app.session.view.zoom * 100.0).round() },
            "watermark" => Dialog::Watermark { text: "CONFIDENTIAL".into(), diagonal: true },
            "newStyle" => Dialog::NewStyle { name: "Style1".into(), based_on: "Normal".into() },
            "commands" => Dialog::Commands { query: String::new() },
            "about" => Dialog::About { tab: 0 },
            "contributors" => Dialog::About { tab: 1 },
            "models" => Dialog::About { tab: 2 },
            _ => return None,
        })
    }

    pub fn modify_style(app: &WordApp, id: &str) -> Option<Dialog> {
        let st = app.session.doc.styles.get(id)?;
        let rc = app.session.doc.styles.resolve_char(
            if st.kind == wordfabrica_doc::StyleKind::Paragraph { Some(id) } else { None },
            &wordfabrica_doc::CharProps {
                style: if st.kind == wordfabrica_doc::StyleKind::Character { Some(id.into()) } else { None },
                ..Default::default()
            },
        );
        let rp = app.session.doc.styles.resolve_para(&wordfabrica_doc::ParaProps { style: Some(id.into()), ..Default::default() });
        Some(Dialog::ModifyStyle {
            id: id.into(),
            name: st.name.clone(),
            font: rc.font,
            size: rc.size,
            bold: rc.bold,
            italic: rc.italic,
            color: match rc.color {
                wordfabrica_doc::TextColor::Rgb(c) => c.hex(),
                wordfabrica_doc::TextColor::Auto => "auto".into(),
            },
            before: rp.space_before,
            after: rp.space_after,
        })
    }
}

/// A dialog button. Both the egui dialogs and hosted (HTML) dialogs go through [`press`], so a
/// dialog runs the same commands whichever front end shows it.
#[derive(Clone, Debug, PartialEq)]
pub enum DialogButton {
    Ok,
    Cancel,
    /// Find and Replace.
    FindNext,
    Replace,
    ReplaceAll,
    /// Zoom: fit `pageWidth`, `onePage` or `multiplePages`.
    Fit(String),
    /// Bookmark: jump to a bookmark by name.
    GotoBookmark(String),
    /// Command search: run this command id.
    Command(String),
}

/// Carry out `button` on dialog `d` (as the user left the form). Returns true when the dialog
/// should close. Messages (Find's match count…) are written back into `d`.
pub fn press(app: &mut WordApp, d: &mut Dialog, button: &DialogButton) -> bool {
    use DialogButton as B;
    if *button == B::Cancel {
        return true;
    }
    match d {
        Dialog::Font { font, size, bold, italic, underline, strike, sup, sub, small_caps, caps, hidden, color, spacing } => {
            if *button != B::Ok {
                return false;
            }
            let mut props = json!({
                "bold": *bold, "italic": *italic, "strike": *strike,
                "underline": if *underline { "single" } else { "none" },
                "vertAlign": if *sup { "superscript" } else if *sub { "subscript" } else { "baseline" },
                "smallCaps": *small_caps, "caps": *caps, "hidden": *hidden, "spacing": *spacing,
            });
            if !font.trim().is_empty() {
                props["font"] = json!(font.trim());
            }
            if let Ok(s) = size.trim().parse::<f64>() {
                props["size"] = json!(s);
            }
            if let Some(c) = wordfabrica_doc::Rgb::parse(color) {
                props["color"] = json!({"Rgb": [c.0, c.1, c.2]});
            }
            let _ = app.run("format.set", json!({"props": props}));
            true
        }
        Dialog::Paragraph { align, left, right, first, before, after, line, keep_next, keep_lines, page_break, widow } => {
            if *button != B::Ok {
                return false;
            }
            let _ = app.run("para.align", json!({"value": align}));
            let _ = app.run(
                "para.set",
                json!({"props": {
                    "indentLeft": *left * 72.0, "indentRight": *right * 72.0, "indentFirst": *first * 72.0,
                    "spaceBefore": *before, "spaceAfter": *after, "lineSpacing": {"rule": "multiple", "value": *line},
                    "keepNext": *keep_next, "keepLines": *keep_lines, "pageBreakBefore": *page_break, "widowControl": *widow,
                }}),
            );
            true
        }
        Dialog::Find { query, replace, match_case, whole_word, regex, message, .. } => {
            let opts = json!({"text": query, "with": replace, "matchCase": *match_case, "wholeWord": *whole_word, "regex": *regex});
            match button {
                B::ReplaceAll => match app.run("edit.replaceAll", opts) {
                    Ok(r) => *message = format!("All done. We made {} replacements.", r["replaced"]),
                    Err(e) => *message = e,
                },
                B::Replace => {
                    let _ = app.run("edit.replace", opts);
                }
                B::FindNext | B::Ok => {
                    let _ = app.session.run("edit.find", &opts);
                    match app.run("edit.findNext", json!({})) {
                        Ok(r) => *message = format!("Match {} of {}", r["index"].as_u64().unwrap_or(0) + 1, r["count"]),
                        Err(e) => *message = e,
                    }
                }
                _ => {}
            }
            false
        }
        Dialog::Goto { page } => {
            if *button != B::Ok {
                return false;
            }
            if let Ok(n) = page.trim().parse::<u64>() {
                let _ = app.run("edit.goto", json!({"page": n}));
            }
            true
        }
        Dialog::InsertTable { rows, cols } => {
            if *button != B::Ok {
                return false;
            }
            let _ = app.run("insert.table", json!({"rows": *rows, "cols": *cols}));
            true
        }
        Dialog::PageSetup { top, bottom, left, right, landscape } => {
            if *button != B::Ok {
                return false;
            }
            let _ = app.run("layout.orientation", json!({"value": if *landscape { "landscape" } else { "portrait" }}));
            let _ = app.run("layout.margins", json!({"top": *top * 72.0, "bottom": *bottom * 72.0, "left": *left * 72.0, "right": *right * 72.0}));
            true
        }
        Dialog::Link { url, text } => {
            if *button != B::Ok {
                return false;
            }
            let _ = app.run("insert.link", json!({"url": url, "text": text}));
            true
        }
        Dialog::Bookmark { name } => match button {
            B::GotoBookmark(n) => {
                let _ = app.run("edit.goto", json!({"bookmark": n}));
                false
            }
            B::Ok => {
                let _ = app.run("insert.bookmark", json!({"name": name}));
                true
            }
            _ => false,
        },
        Dialog::WordCount { .. } | Dialog::About { .. } => *button == B::Ok,
        Dialog::Zoom { percent } => match button {
            B::Fit(f) => {
                let _ = app.run("view.zoom", json!({"value": f}));
                true
            }
            B::Ok => {
                let _ = app.run("view.zoom", json!({"value": *percent}));
                true
            }
            _ => false,
        },
        Dialog::Watermark { text, diagonal } => {
            if *button != B::Ok {
                return false;
            }
            let _ = app.run("design.watermark", json!({"text": text, "diagonal": *diagonal}));
            true
        }
        Dialog::NewStyle { name, based_on } => {
            if *button != B::Ok {
                return false;
            }
            let _ = app.run("styles.create", json!({"name": name, "basedOn": based_on}));
            true
        }
        Dialog::ModifyStyle { id, name, font, size, bold, italic, color, before, after } => {
            if *button != B::Ok {
                return false;
            }
            let mut chr = json!({"font": font, "size": *size, "bold": *bold, "italic": *italic});
            if let Some(c) = wordfabrica_doc::Rgb::parse(color) {
                chr["color"] = json!({"Rgb": [c.0, c.1, c.2]});
            }
            let _ = app.run("styles.modify", json!({"style": id, "name": name, "chr": chr, "para": {"spaceBefore": *before, "spaceAfter": *after}}));
            true
        }
        Dialog::Commands { .. } => match button {
            B::Command(id) => {
                let _ = app.run(id, json!({}));
                true
            }
            _ => false,
        },
    }
}

/// Commands matching a search query (label, translated label, id or ribbon location), as the
/// command search dialog lists them.
pub fn command_hits<'a>(reg: &'a wordfabrica_engine::Registry, query: &str, max: usize) -> Vec<&'a wordfabrica_engine::CommandSpec> {
    let q = query.to_lowercase();
    let mut hits: Vec<&wordfabrica_engine::CommandSpec> = reg
        .all()
        .iter()
        .filter(|c| {
            !q.is_empty()
                && !crate::ribbon::hidden_in_ui(c.id)
                && (c.label.to_lowercase().contains(&q)
                    || tl!(c.label).to_lowercase().contains(&q)
                    || c.id.to_lowercase().contains(&q)
                    || c.location.to_lowercase().contains(&q)
                    || crate::i18n::location(c.location).to_lowercase().contains(&q))
        })
        .collect();
    hits.truncate(max);
    hits
}

/// The window title of a dialog (English source text).
pub fn title(d: &Dialog) -> &'static str {
    match d {
        Dialog::Font { .. } => "Font",
        Dialog::Paragraph { .. } => "Paragraph",
        Dialog::Find { replace_mode: false, .. } => "Find",
        Dialog::Find { .. } => "Find and Replace",
        Dialog::Goto { .. } => "Go To",
        Dialog::InsertTable { .. } => "Insert Table",
        Dialog::PageSetup { .. } => "Page Setup",
        Dialog::Link { .. } => "Insert Hyperlink",
        Dialog::Bookmark { .. } => "Bookmark",
        Dialog::WordCount { .. } => "Word Count",
        Dialog::Zoom { .. } => "Zoom",
        Dialog::Watermark { .. } => "Custom Watermark",
        Dialog::NewStyle { .. } => "Create New Style",
        Dialog::ModifyStyle { .. } => "Modify Style",
        Dialog::Commands { .. } => "Search Commands",
        Dialog::About { .. } => "About WordFabrica",
    }
}

/// Grid picker for Insert › Table (hover to size, click to insert).
pub fn table_grid_picker(ui: &mut Ui, app: &mut WordApp) {
    let t = Tokens::get(ui.ctx());
    let id = egui::Id::new("table_picker_hover");
    let hover: (usize, usize) = ui.data(|d| d.get_temp(id)).unwrap_or((0, 0));
    ui.label(if hover.0 > 0 { format!("{}x{} Table", hover.1, hover.0) } else { "Insert Table".to_string() });
    let mut new_hover = (0, 0);
    let mut clicked = None;
    egui::Grid::new("tgp").spacing(vec2(2.0, 2.0)).show(ui, |ui| {
        for r in 1..=8 {
            for c in 1..=10 {
                let (rect, resp) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
                let on = r <= hover.0 && c <= hover.1;
                ui.painter().rect(
                    rect,
                    1.0,
                    if on { t.checked } else { t.input },
                    egui::Stroke::new(1.0, if on { t.accent } else { t.border_strong }),
                    egui::StrokeKind::Inside,
                );
                if resp.hovered() {
                    new_hover = (r, c);
                }
                if resp.clicked() {
                    clicked = Some((r, c));
                }
            }
            ui.end_row();
        }
    });
    ui.data_mut(|d| d.insert_temp(id, new_hover));
    if let Some((r, c)) = clicked {
        let _ = app.run("insert.table", json!({"rows": r, "cols": c}));
        ui.close();
    }
}

pub fn show(app: &mut WordApp, ctx: &egui::Context) {
    let Some(mut d) = app.dialog.take() else { return };
    let mut open = true;
    let mut close = false;
    let mut pressed = None;
    let title = title(&d);
    egui::Window::new(tl!(title))
        .id(egui::Id::new(("dialog", title)))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, -40.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.set_min_width(340.0);
            pressed = body(app, ui, &mut d);
        });
    if let Some(b) = pressed {
        close = press(app, &mut d, &b);
    }
    if open && !close {
        app.dialog = Some(d);
    } else {
        app.canvas.want_focus = true;
    }
}

fn buttons(ui: &mut Ui, ok: &str) -> (bool, bool) {
    let mut r = (false, false);
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(tl!("Cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                r.1 = true;
            }
            let okb = ui.add(egui::Button::new(egui::RichText::new(ok).color(egui::Color32::WHITE)).fill(crate::theme::APP_COLOR));
            if okb.clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                r.0 = true;
            }
        });
    });
    r
}

/// The OK/Cancel pair as a pressed button.
fn ok_cancel((ok, cancel): (bool, bool)) -> Option<DialogButton> {
    if ok {
        Some(DialogButton::Ok)
    } else if cancel {
        Some(DialogButton::Cancel)
    } else {
        None
    }
}

/// Draw the dialog's form; returns the button pressed this frame, if any.
fn body(app: &mut WordApp, ui: &mut Ui, d: &mut Dialog) -> Option<DialogButton> {
    match d {
        Dialog::Font { font, size, bold, italic, underline, strike, sup, sub, small_caps, caps, hidden, color, spacing } => {
            egui::Grid::new("fontdlg").num_columns(2).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                ui.label(tl!("Font:"));
                ui.text_edit_singleline(font);
                ui.end_row();
                ui.label(tl!("Size:"));
                ui.text_edit_singleline(size);
                ui.end_row();
                ui.label(tl!("Font color:"));
                ui.text_edit_singleline(color);
                ui.end_row();
                ui.label(tl!("Spacing (pt):"));
                ui.add(egui::DragValue::new(spacing).speed(0.1).range(-20.0..=20.0));
                ui.end_row();
            });
            ui.separator();
            ui.label(egui::RichText::new(tl!("Effects")).font(semibold(12.5)));
            egui::Grid::new("fx").num_columns(2).show(ui, |ui| {
                ui.checkbox(bold, tl!("Bold"));
                ui.checkbox(italic, tl!("Italic"));
                ui.end_row();
                ui.checkbox(underline, tl!("Underline"));
                ui.checkbox(strike, tl!("Strikethrough"));
                ui.end_row();
                ui.checkbox(sup, tl!("Superscript"));
                ui.checkbox(sub, tl!("Subscript"));
                ui.end_row();
                ui.checkbox(small_caps, tl!("Small caps"));
                ui.checkbox(caps, tl!("All caps"));
                ui.end_row();
                ui.checkbox(hidden, tl!("Hidden"));
                ui.end_row();
            });
            ok_cancel(buttons(ui, tl!("OK")))
        }
        Dialog::Paragraph { align, left, right, first, before, after, line, keep_next, keep_lines, page_break, widow } => {
            ui.label(egui::RichText::new(tl!("General")).font(semibold(12.5)));
            egui::ComboBox::from_label(tl!("Alignment")).selected_text(align.clone()).show_ui(ui, |ui| {
                for a in ["left", "center", "right", "justify"] {
                    ui.selectable_value(align, a.to_string(), a);
                }
            });
            ui.label(egui::RichText::new(tl!("Indentation")).font(semibold(12.5)));
            egui::Grid::new("ind").num_columns(4).show(ui, |ui| {
                ui.label(tl!("Left:"));
                ui.add(egui::DragValue::new(left).speed(0.05).suffix("\"").max_decimals(2));
                ui.label(tl!("Right:"));
                ui.add(egui::DragValue::new(right).speed(0.05).suffix("\"").max_decimals(2));
                ui.end_row();
                ui.label(tl!("First line:"));
                ui.add(egui::DragValue::new(first).speed(0.05).suffix("\"").max_decimals(2));
                ui.end_row();
            });
            ui.label(egui::RichText::new(tl!("Spacing")).font(semibold(12.5)));
            egui::Grid::new("sp").num_columns(4).show(ui, |ui| {
                ui.label(tl!("Before:"));
                ui.add(egui::DragValue::new(before).speed(1.0).range(0.0..=1584.0).suffix(" pt"));
                ui.label(tl!("Line spacing:"));
                ui.add(egui::DragValue::new(line).speed(0.05).range(0.5..=5.0));
                ui.end_row();
                ui.label(tl!("After:"));
                ui.add(egui::DragValue::new(after).speed(1.0).range(0.0..=1584.0).suffix(" pt"));
                ui.end_row();
            });
            ui.label(egui::RichText::new(tl!("Line and Page Breaks")).font(semibold(12.5)));
            ui.checkbox(widow, tl!("Widow/Orphan control"));
            ui.checkbox(keep_next, tl!("Keep with next"));
            ui.checkbox(keep_lines, tl!("Keep lines together"));
            ui.checkbox(page_break, tl!("Page break before"));
            ok_cancel(buttons(ui, tl!("OK")))
        }
        Dialog::Find { query, replace, match_case, whole_word, regex, replace_mode, message } => {
            ui.horizontal(|ui| {
                if ui.selectable_label(!*replace_mode, tl!("Find")).clicked() {
                    *replace_mode = false;
                }
                if ui.selectable_label(*replace_mode, tl!("Replace")).clicked() {
                    *replace_mode = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label(tl!("Find what:"));
                ui.add(egui::TextEdit::singleline(query).desired_width(240.0));
            });
            if *replace_mode {
                ui.horizontal(|ui| {
                    ui.label(tl!("Replace with:"));
                    ui.add(egui::TextEdit::singleline(replace).desired_width(228.0));
                });
            }
            ui.horizontal(|ui| {
                ui.checkbox(match_case, tl!("Match case"));
                ui.checkbox(whole_word, tl!("Whole words"));
                ui.checkbox(regex, tl!("Wildcards (regex)"));
            });
            if !message.is_empty() {
                ui.label(egui::RichText::new(message.as_str()).weak());
            }
            let mut pressed = None;
            ui.horizontal(|ui| {
                if *replace_mode {
                    if ui.button(tl!("Replace All")).clicked() {
                        pressed = Some(DialogButton::ReplaceAll);
                    }
                    if ui.button(tl!("Replace")).clicked() {
                        pressed = Some(DialogButton::Replace);
                    }
                }
                if ui.button(tl!("Find Next")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    pressed = Some(DialogButton::FindNext);
                }
                if ui.button(tl!("Close")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    pressed = Some(DialogButton::Cancel);
                }
            });
            pressed
        }
        Dialog::Goto { page } => {
            ui.horizontal(|ui| {
                ui.label(tl!("Enter page number:"));
                ui.text_edit_singleline(page);
            });
            ok_cancel(buttons(ui, tl!("Go To")))
        }
        Dialog::InsertTable { rows, cols } => {
            egui::Grid::new("it").num_columns(2).show(ui, |ui| {
                ui.label(tl!("Number of columns:"));
                ui.add(egui::DragValue::new(cols).range(1..=63));
                ui.end_row();
                ui.label(tl!("Number of rows:"));
                ui.add(egui::DragValue::new(rows).range(1..=1000));
                ui.end_row();
            });
            ok_cancel(buttons(ui, tl!("OK")))
        }
        Dialog::PageSetup { top, bottom, left, right, landscape } => {
            ui.label(egui::RichText::new(tl!("Margins")).font(semibold(12.5)));
            egui::Grid::new("ps").num_columns(4).show(ui, |ui| {
                ui.label(tl!("Top:"));
                ui.add(egui::DragValue::new(top).speed(0.05).suffix("\"").max_decimals(2));
                ui.label(tl!("Bottom:"));
                ui.add(egui::DragValue::new(bottom).speed(0.05).suffix("\"").max_decimals(2));
                ui.end_row();
                ui.label(tl!("Left:"));
                ui.add(egui::DragValue::new(left).speed(0.05).suffix("\"").max_decimals(2));
                ui.label(tl!("Right:"));
                ui.add(egui::DragValue::new(right).speed(0.05).suffix("\"").max_decimals(2));
                ui.end_row();
            });
            ui.label(egui::RichText::new(tl!("Orientation")).font(semibold(12.5)));
            ui.horizontal(|ui| {
                ui.radio_value(landscape, false, tl!("Portrait"));
                ui.radio_value(landscape, true, tl!("Landscape"));
            });
            ok_cancel(buttons(ui, tl!("OK")))
        }
        Dialog::Link { url, text } => {
            egui::Grid::new("lnk").num_columns(2).show(ui, |ui| {
                ui.label(tl!("Text to display:"));
                ui.text_edit_singleline(text);
                ui.end_row();
                ui.label(tl!("Address:"));
                ui.text_edit_singleline(url);
                ui.end_row();
            });
            ok_cancel(buttons(ui, tl!("OK")))
        }
        Dialog::Bookmark { name } => {
            ui.label(tl!("Bookmark name:"));
            ui.text_edit_singleline(name);
            let marks = app.session.doc.bookmarks();
            let mut pressed = None;
            for (n, _) in &marks {
                if ui.selectable_label(false, n).clicked() {
                    pressed = Some(DialogButton::GotoBookmark(n.clone()));
                }
            }
            ok_cancel(buttons(ui, tl!("Add"))).or(pressed)
        }
        Dialog::WordCount { stats } => {
            egui::Grid::new("wc").num_columns(2).spacing(vec2(30.0, 6.0)).show(ui, |ui| {
                for (l, k) in [
                    ("Pages", "pages"),
                    ("Words", "words"),
                    ("Characters (no spaces)", "characters"),
                    ("Characters (with spaces)", "charactersWithSpaces"),
                    ("Paragraphs", "paragraphs"),
                    ("Lines", "lines"),
                ] {
                    ui.label(tl!(l));
                    ui.label(stats.get(k).map(|v| v.to_string()).unwrap_or_default());
                    ui.end_row();
                }
            });
            ok_cancel(buttons(ui, tl!("Close")))
        }
        Dialog::Zoom { percent } => {
            ui.horizontal(|ui| {
                for p in [200.0, 100.0, 75.0] {
                    ui.radio_value(percent, p, format!("{p}%"));
                }
            });
            ui.add(egui::Slider::new(percent, 10.0..=500.0).suffix("%"));
            let mut fit = None;
            ui.horizontal(|ui| {
                if ui.button(tl!("Page width")).clicked() {
                    fit = Some("pageWidth");
                }
                if ui.button(tl!("Whole page")).clicked() {
                    fit = Some("onePage");
                }
                if ui.button(tl!("Many pages")).clicked() {
                    fit = Some("multiplePages");
                }
            });
            if let Some(f) = fit {
                return Some(DialogButton::Fit(f.to_string()));
            }
            ok_cancel(buttons(ui, tl!("OK")))
        }
        Dialog::Watermark { text, diagonal } => {
            ui.horizontal(|ui| {
                ui.label(tl!("Text:"));
                ui.text_edit_singleline(text);
            });
            ui.horizontal(|ui| {
                ui.radio_value(diagonal, true, tl!("Diagonal"));
                ui.radio_value(diagonal, false, tl!("Horizontal"));
            });
            ok_cancel(buttons(ui, tl!("OK")))
        }
        Dialog::NewStyle { name, based_on } => {
            egui::Grid::new("ns").num_columns(2).show(ui, |ui| {
                ui.label(tl!("Name:"));
                ui.text_edit_singleline(name);
                ui.end_row();
                ui.label(tl!("Style based on:"));
                ui.text_edit_singleline(based_on);
                ui.end_row();
            });
            ui.label(egui::RichText::new(tl!("The new style takes the formatting of the current paragraph.")).small().weak());
            ok_cancel(buttons(ui, tl!("OK")))
        }
        Dialog::ModifyStyle { name, font, size, bold, italic, color, before, after, .. } => {
            egui::Grid::new("ms").num_columns(2).show(ui, |ui| {
                ui.label(tl!("Name:"));
                ui.text_edit_singleline(name);
                ui.end_row();
                ui.label(tl!("Font:"));
                ui.text_edit_singleline(font);
                ui.end_row();
                ui.label(tl!("Size:"));
                ui.add(egui::DragValue::new(size).range(1.0..=1638.0).speed(0.5));
                ui.end_row();
                ui.label(tl!("Color:"));
                ui.text_edit_singleline(color);
                ui.end_row();
                ui.label(tl!("Space before/after:"));
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(before).range(0.0..=1584.0));
                    ui.add(egui::DragValue::new(after).range(0.0..=1584.0));
                });
                ui.end_row();
            });
            ui.horizontal(|ui| {
                ui.checkbox(bold, tl!("Bold"));
                ui.checkbox(italic, tl!("Italic"));
            });
            ok_cancel(buttons(ui, tl!("OK")))
        }
        Dialog::Commands { query } => {
            let r = ui.add(egui::TextEdit::singleline(query).hint_text(tl!("Type a command, e.g. \"insert table\"")).desired_width(380.0));
            r.request_focus();
            let reg = app.session.registry.clone();
            let hits = command_hits(&reg, query, 14);
            let mut pressed = None;
            for c in hits {
                let sc = crate::widgets::shortcut_text(app, c.id);
                let label = format!("{}   —   {}", tl!(c.label), crate::i18n::location(c.location));
                if ui.add(egui::Button::new(label).shortcut_text(sc).min_size(vec2(380.0, 0.0))).clicked() {
                    pressed = Some(DialogButton::Command(c.id.to_string()));
                }
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                pressed = Some(DialogButton::Cancel);
            }
            pressed
        }
        Dialog::About { tab } => {
            ui.set_width(660.0);
            ui.horizontal(|ui| {
                for (i, l) in ["About", "Contributors", "Models"].into_iter().enumerate() {
                    let i = i as u8;
                    if ui.selectable_label(*tab == i, tl!(l)).clicked() {
                        *tab = i;
                    }
                }
            });
            ui.separator();
            match *tab {
                1 => crate::credits::contributors_ui(ui),
                2 => crate::credits::models_ui(ui),
                _ => {
                    ui.label(egui::RichText::new(tl!("WordFabrica")).font(semibold(22.0)));
                    let build = option_env!("WORDFABRICA_BUILD_SHA").map(|s| s.get(..8).unwrap_or(s)).unwrap_or(tl!("development build"));
                    ui.label(crate::i18n::fmt(tl!("Version {version} ({build})"), &[("version", env!("CARGO_PKG_VERSION")), ("build", build)]));
                    ui.label(tl!(
                        "A free, open-source word processor written from scratch in Rust.\nPart of OpenFabrica: open source tools for makers, builders, and creatives."
                    ));
                    ui.add_space(6.0);
                    ui.hyperlink_to(tl!("openfabrica.dev"), "https://openfabrica.dev");
                    ui.hyperlink_to(tl!("Join the community: GitHub Discussions"), "https://github.com/OpenFabrica/wordfabrica/discussions");
                    ui.hyperlink_to(tl!("Source code: github.com/OpenFabrica/wordfabrica"), "https://github.com/OpenFabrica/wordfabrica");
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(tl!("MIT OR Apache-2.0. Copyright (c) 2026 the OpenFabrica contributors. Based on WordCraft by the ArtCraft Team and the WordCraft contributors."))
                            .small()
                            .weak(),
                    );
                }
            }
            ok_cancel(buttons(ui, tl!("Close")))
        }
    }
}
