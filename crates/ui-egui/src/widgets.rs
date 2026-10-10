//! Ribbon widgets: large and small buttons, split buttons, groups, combo fields, colour grids.

use egui::{Align2, Color32, CornerRadius, Rect, Response, Sense, Stroke, Ui, pos2, vec2};
use serde_json::Value;

use crate::theme::{Tokens, regular};
use crate::{WordApp, icons};

/// Height of the ribbon content area. Groups carry no title row (their name is the launcher's
/// tooltip), so this is the whole ribbon body.
pub const CONTENT_H: f32 = 66.0;
/// Height of a small ribbon button row, and the largest gap between centred rows.
pub const ROW_H: f32 = 22.0;
pub const ROW_GAP: f32 = 8.0;

/// Shortcut text for a command (`⌘B` on macOS, `Ctrl+B` elsewhere).
pub fn shortcut_text(app: &WordApp, id: &str) -> String {
    let Some(spec) = app.session.registry.get(id) else { return String::new() };
    let sc = spec.shortcut.split(" / ").next().unwrap_or("");
    if sc.is_empty() {
        return String::new();
    }
    if cfg!(target_os = "macos") {
        sc.replace("Mod+", "⌘").replace("Shift+", "⇧").replace("Alt+", "⌥").replace("Ctrl+", "⌃")
    } else {
        sc.replace("Mod+", "Ctrl+")
    }
}

/// `label` is already in the interface language.
fn tooltip(app: &WordApp, resp: Response, label: &str, id: &str) -> Response {
    let sc = shortcut_text(app, id);
    let desc = app.session.registry.get(id).map(|s| crate::i18n::location(s.location)).unwrap_or_default();
    let enabled = app.session.registry.get(id).map(|s| (s.enabled)(&app.session).is_none()).unwrap_or(true);
    resp.on_hover_ui(|ui| {
        ui.set_max_width(260.0);
        if sc.is_empty() {
            ui.label(egui::RichText::new(label).font(crate::theme::semibold(12.0)));
        } else {
            ui.label(egui::RichText::new(format!("{label} ({sc})")).font(crate::theme::semibold(12.0)));
        }
        if !desc.is_empty() {
            ui.label(egui::RichText::new(desc).small().weak());
        }
        if !enabled {
            ui.label(egui::RichText::new(tl!("Not available right now")).small().weak());
        }
    })
}

/// Whether a command exists and is enabled.
pub fn enabled(app: &WordApp, id: &str) -> bool {
    match app.session.registry.get(id) {
        Some(s) => (s.enabled)(&app.session).is_none(),
        None => id.starts_with("ui."),
    }
}

fn bg(ui: &Ui, r: Rect, resp: &Response, checked: bool, t: &Tokens) {
    let fill = if resp.is_pointer_button_down_on() {
        t.pressed
    } else if checked {
        t.checked
    } else if resp.hovered() {
        t.hover
    } else {
        return;
    };
    ui.painter().rect_filled(r, CornerRadius::same(4), fill);
    if checked {
        ui.painter().rect_stroke(r, CornerRadius::same(4), Stroke::new(1.0, t.accent.linear_multiply(0.5)), egui::StrokeKind::Inside);
    }
}

/// A large ribbon button: 32 px icon over a (possibly two-line) label.
pub fn big(ui: &mut Ui, app: &mut WordApp, icon: &str, label: &str, id: &str, params: Value, menu: bool) -> Response {
    big_impl(ui, app, icon, label, id, params, menu, false)
}

/// A large toggle button (e.g. Format Painter): like [`big`], drawn checked while `checked`.
pub fn big_checked(ui: &mut Ui, app: &mut WordApp, icon: &str, label: &str, id: &str, params: Value, checked: bool) -> Response {
    big_impl(ui, app, icon, label, id, params, false, checked)
}

/// Shared body of [`big`] and [`big_checked`].
#[allow(clippy::too_many_arguments)]
fn big_impl(ui: &mut Ui, app: &mut WordApp, icon: &str, label: &str, id: &str, params: Value, menu: bool, checked: bool) -> Response {
    let label = tl!(label);
    let t = Tokens::get(ui.ctx());
    let galley_w =
        label.split('\n').map(|l| ui.ctx().fonts_mut(|f| f.layout_no_wrap(l.to_string(), regular(11.5), t.text).size().x)).fold(0.0, f32::max);
    let w = (galley_w + 12.0).max(44.0);
    let (r, resp) = ui.allocate_exact_size(vec2(w, CONTENT_H), Sense::click());
    let on = enabled(app, id);
    bg(ui, r, &resp, checked, &t);
    let ic = Rect::from_center_size(pos2(r.center().x, r.min.y + 20.0), vec2(32.0, 32.0));
    let (c, a) = if on { (t.icon, t.accent) } else { (t.text_disabled, t.text_disabled) };
    icons::paint(ui.painter(), ic, icon, c, a);
    let mut y = r.min.y + 43.0;
    let lines: Vec<&str> = label.split('\n').collect();
    for (i, l) in lines.iter().enumerate() {
        let txt = if menu && i + 1 == lines.len() { format!("{l} ▾") } else { (*l).to_string() };
        ui.painter().text(pos2(r.center().x, y), Align2::CENTER_TOP, txt, regular(11.5), if on { t.text } else { t.text_disabled });
        y += 13.0;
    }
    let resp = tooltip(app, resp, &label.replace('\n', " "), id);
    if resp.clicked() && on && !menu {
        let _ = app.run(id, params);
    }
    resp
}

/// A small button: 16 px icon with an optional label; `checked` draws the toggled state.
pub fn small(ui: &mut Ui, app: &mut WordApp, icon: &str, label: Option<&str>, tip: &str, id: &str, params: Value, checked: bool) -> Response {
    let (label, tip) = (label.map(|l| tl!(l)), tl!(tip));
    let t = Tokens::get(ui.ctx());
    let text_w = label.map(|l| ui.ctx().fonts_mut(|f| f.layout_no_wrap(l.to_string(), regular(11.5), t.text).size().x) + 6.0).unwrap_or(0.0);
    let (r, resp) = ui.allocate_exact_size(vec2(24.0 + text_w, 22.0), Sense::click());
    let on = enabled(app, id);
    bg(ui, r, &resp, checked, &t);
    let ic = Rect::from_center_size(pos2(r.min.x + 12.0, r.center().y), vec2(17.0, 17.0));
    let (c, a) = if on { (t.icon, t.accent) } else { (t.text_disabled, t.text_disabled) };
    icons::paint(ui.painter(), ic, icon, c, a);
    if let Some(l) = label {
        ui.painter().text(pos2(r.min.x + 24.0, r.center().y), Align2::LEFT_CENTER, l, regular(11.5), if on { t.text } else { t.text_disabled });
    }
    let resp = tooltip(app, resp, tip, id);
    if resp.clicked() && on {
        let _ = app.run(id, params);
    }
    resp
}

/// A split button: the left part runs `id`, the arrow opens a menu filled by `menu`.
pub fn split(
    ui: &mut Ui,
    app: &mut WordApp,
    icon: &str,
    tip: &str,
    id: &str,
    params: Value,
    checked: bool,
    swatch: Option<Color32>,
    menu: impl FnOnce(&mut Ui, &mut WordApp),
) {
    let tip = tl!(tip);
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(24.0, 22.0), Sense::click());
    let (ar, aresp) = ui.allocate_exact_size(vec2(12.0, 22.0), Sense::click());
    bg(ui, r, &resp, checked, &t);
    bg(ui, ar, &aresp, false, &t);
    let ic = Rect::from_center_size(r.center(), vec2(17.0, 17.0));
    icons::paint(ui.painter(), ic, icon, t.icon, t.accent);
    if let Some(sw) = swatch {
        ui.painter().rect_filled(Rect::from_min_max(pos2(r.min.x + 4.0, r.max.y - 5.0), pos2(r.max.x - 4.0, r.max.y - 2.0)), 0.0, sw);
    }
    icons::paint(ui.painter(), Rect::from_center_size(ar.center(), vec2(10.0, 10.0)), "dropdown", t.icon, t.accent);
    let resp = tooltip(app, resp, tip, id);
    if resp.clicked() {
        let _ = app.run(id, params);
    }
    egui::Popup::menu(&aresp).show(|ui| {
        ui.set_min_width(180.0);
        menu(ui, app);
    });
}

/// A button that only opens a menu (icon + label + ▾).
pub fn menu_button(
    ui: &mut Ui,
    app: &mut WordApp,
    icon: &str,
    label: Option<&str>,
    tip: &str,
    big_btn: bool,
    menu: impl FnOnce(&mut Ui, &mut WordApp),
) {
    let (label, tip) = (label.map(|l| tl!(l)), tl!(tip));
    let t = Tokens::get(ui.ctx());
    let resp = if big_btn {
        let galley_w = label
            .unwrap_or("")
            .split('\n')
            .map(|l| ui.ctx().fonts_mut(|f| f.layout_no_wrap(l.to_string(), regular(11.5), t.text).size().x))
            .fold(0.0, f32::max);
        let w = (galley_w + 22.0).max(44.0);
        let (r, resp) = ui.allocate_exact_size(vec2(w, CONTENT_H), Sense::click());
        bg(ui, r, &resp, false, &t);
        icons::paint(ui.painter(), Rect::from_center_size(pos2(r.center().x, r.min.y + 20.0), vec2(32.0, 32.0)), icon, t.icon, t.accent);
        let lines: Vec<&str> = label.unwrap_or("").split('\n').collect();
        let mut y = r.min.y + 43.0;
        for (i, l) in lines.iter().enumerate() {
            let txt = if i + 1 == lines.len() { format!("{l} ▾") } else { (*l).to_string() };
            ui.painter().text(pos2(r.center().x, y), Align2::CENTER_TOP, txt, regular(11.5), t.text);
            y += 13.0;
        }
        resp
    } else {
        let text_w = label.map(|l| ui.ctx().fonts_mut(|f| f.layout_no_wrap(l.to_string(), regular(11.5), t.text).size().x) + 6.0).unwrap_or(0.0);
        let (r, resp) = ui.allocate_exact_size(vec2(24.0 + text_w + 10.0, 22.0), Sense::click());
        bg(ui, r, &resp, false, &t);
        icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 12.0, r.center().y), vec2(17.0, 17.0)), icon, t.icon, t.accent);
        if let Some(l) = label {
            ui.painter().text(pos2(r.min.x + 24.0, r.center().y), Align2::LEFT_CENTER, l, regular(11.5), t.text);
        }
        icons::paint(ui.painter(), Rect::from_center_size(pos2(r.max.x - 6.0, r.center().y), vec2(9.0, 9.0)), "dropdown", t.icon, t.accent);
        resp
    };
    let resp = resp.on_hover_text(tip);
    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(200.0);
        menu(ui, app);
    });
}

/// A ribbon group: content, an optional dialog launcher in its bottom-right corner and a divider
/// on the right. Like WPS there is no title row; the title names the launcher in its tooltip.
pub fn group(ui: &mut Ui, title: &str, launcher: Option<&str>, app: &mut WordApp, add: impl FnOnce(&mut Ui, &mut WordApp)) {
    // The English title keys the launcher's id; the tooltip is translated.
    let id_title = title;
    let title = tl!(title);
    let t = Tokens::get(ui.ctx());
    let start = ui.cursor().min;
    ui.vertical(|ui| {
        ui.allocate_ui_with_layout(vec2(ui.available_width(), CONTENT_H), egui::Layout::left_to_right(egui::Align::Min), |ui| {
            ui.spacing_mut().item_spacing = vec2(1.0, 1.0);
            add(ui, app);
        });
    });
    if let Some(cmd) = launcher {
        // The launcher sits in its own narrow column so it never covers the group's controls.
        ui.add_space(2.0);
        let x = ui.cursor().min.x;
        ui.add_space(11.0);
        let lr = Rect::from_min_size(pos2(x, start.y + CONTENT_H - 11.0), vec2(11.0, 11.0));
        let resp = ui.interact(lr, ui.id().with(("launch", id_title)), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(lr.expand(2.0), 2.0, t.hover);
        }
        icons::paint(ui.painter(), lr, "launcher", t.text_dim, t.accent);
        if resp.on_hover_text(crate::i18n::fmt(tl!("{group} settings"), &[("group", title)])).clicked() {
            let _ = app.run(cmd, serde_json::json!({}));
        }
    }
    ui.add_space(4.0);
    let x = ui.cursor().min.x;
    ui.painter().line_segment([pos2(x, start.y + 8.0), pos2(x, start.y + CONTENT_H - 8.0)], Stroke::new(1.0, t.border));
    ui.add_space(5.0);
}

/// Stack `rows` small-control rows vertically centred in the ribbon, spaced by up to `ROW_GAP`
/// (three rows fill the height with no gap).
pub fn centered_rows(ui: &mut Ui, rows: usize, add: impl FnOnce(&mut Ui)) {
    let n = rows.max(1) as f32;
    let gap = if n > 1.0 { ((CONTENT_H - n * ROW_H) / (n - 1.0)).clamp(0.0, ROW_GAP) } else { 0.0 };
    let top = ((CONTENT_H - n * ROW_H - (n - 1.0) * gap) / 2.0).max(0.0);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(1.0, gap);
        ui.add_space(top);
        add(ui);
    });
}

/// A row of small buttons (used inside a vertical stack).
pub fn row(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(1.0, 0.0);
        add(ui);
    });
}

/// Word's standard colour grid (theme colours + tints, standard colours). Returns a picked hex.
pub fn color_grid(ui: &mut Ui, theme: &[wordcraft_doc::Rgb]) -> Option<String> {
    let mut picked = None;
    ui.label(egui::RichText::new(tl!("Theme Colors")).small().weak());
    let base: Vec<wordcraft_doc::Rgb> = theme.iter().take(10).copied().collect();
    let tint = |c: wordcraft_doc::Rgb, k: f32| {
        let f = |v: u8| if k >= 0.0 { (v as f32 + (255.0 - v as f32) * k) as u8 } else { (v as f32 * (1.0 + k)) as u8 };
        wordcraft_doc::Rgb(f(c.0), f(c.1), f(c.2))
    };
    let order = [1usize, 0, 3, 2, 4, 5, 6, 7, 8, 9];
    for k in [0.0f32, 0.8, 0.6, 0.4, -0.25, -0.5] {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(3.0, if k == 0.0 { 6.0 } else { 0.0 });
            for i in order {
                let Some(c) = base.get(i).copied() else { continue };
                let c = tint(c, k);
                if swatch(ui, c) {
                    picked = Some(c.hex());
                }
            }
        });
    }
    ui.add_space(4.0);
    ui.label(egui::RichText::new(tl!("Standard Colors")).small().weak());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(3.0, 0.0);
        for hex in ["C00000", "FF0000", "FFC000", "FFFF00", "92D050", "00B050", "00B0F0", "0070C0", "002060", "7030A0"] {
            if let Some(c) = wordcraft_doc::Rgb::parse(hex)
                && swatch(ui, c)
            {
                picked = Some(hex.to_string());
            }
        }
    });
    picked
}

fn swatch(ui: &mut Ui, c: wordcraft_doc::Rgb) -> bool {
    let (r, resp) = ui.allocate_exact_size(vec2(15.0, 15.0), Sense::click());
    ui.painter().rect_filled(r, 0.0, Color32::from_rgb(c.0, c.1, c.2));
    let t = Tokens::get(ui.ctx());
    ui.painter().rect_stroke(
        r,
        0.0,
        Stroke::new(if resp.hovered() { 1.5 } else { 0.5 }, if resp.hovered() { t.accent } else { t.border_strong }),
        egui::StrokeKind::Inside,
    );
    resp.on_hover_text(format!("#{}", c.hex())).clicked()
}

/// A ribbon-style text field with a dropdown list (one rounded frame, chevron inside on the
/// right, as in WPS). Returns a chosen/typed value.
pub fn combo(
    ui: &mut Ui,
    id: &str,
    width: f32,
    current: &str,
    items: &[String],
    preview: Option<&dyn Fn(&mut Ui, &str) -> Response>,
) -> Option<String> {
    let mut out = None;
    let edit_id = ui.id().with(("combo_text", id));
    let mut text = ui.data_mut(|d| d.get_temp::<String>(edit_id)).unwrap_or_else(|| current.to_string());
    let t = Tokens::get(ui.ctx());
    let (r, _) = ui.allocate_exact_size(vec2(width, 24.0), Sense::hover());
    // The frame is painted after the text field so its colour can follow hover and focus.
    let frame = ui.painter().add(egui::Shape::Noop);
    let ar = Rect::from_min_max(pos2(r.max.x - 20.0, r.min.y), r.max);
    let aresp = ui.interact(ar, ui.id().with(("combo_arrow", id)), Sense::click());
    let row_h = ui.ctx().fonts_mut(|f| f.row_height(&regular(12.0)));
    let text_rect = Rect::from_min_max(pos2(r.min.x + 6.0, r.center().y - row_h / 2.0), pos2(ar.min.x - 2.0, r.center().y + row_h / 2.0));
    let te =
        egui::TextEdit::singleline(&mut text).frame(egui::Frame::NONE).desired_width(text_rect.width()).font(regular(12.0)).margin(vec2(0.0, 0.0));
    // A child ui keeps the text field from moving the row's cursor back inside the frame.
    let resp = ui.new_child(egui::UiBuilder::new().max_rect(text_rect).layout(egui::Layout::left_to_right(egui::Align::Center))).add(te);
    if resp.has_focus() {
        ui.data_mut(|d| d.insert_temp(edit_id, text.clone()));
    } else {
        ui.data_mut(|d| d.remove::<String>(edit_id));
    }
    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        out = Some(text.clone());
    }
    let border = if resp.has_focus() {
        t.accent
    } else if resp.hovered() || aresp.hovered() {
        t.border_strong
    } else {
        t.input_border
    };
    ui.painter().set(frame, egui::epaint::RectShape::new(r, CornerRadius::same(4), t.input, Stroke::new(1.0, border), egui::StrokeKind::Inside));
    if aresp.hovered() {
        ui.painter().rect_filled(ar.shrink(2.0), CornerRadius::same(3), t.hover);
    }
    icons::paint(ui.painter(), Rect::from_center_size(ar.center(), vec2(11.0, 11.0)), "dropdown", t.icon, t.accent);
    egui::Popup::menu(&aresp).show(|ui| {
        egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
            ui.set_min_width(width + 60.0);
            for it in items {
                let clicked = match preview {
                    Some(p) => p(ui, it).clicked(),
                    None => ui.selectable_label(it == current, it).clicked(),
                };
                if clicked {
                    out = Some(it.clone());
                    ui.close();
                }
            }
        });
    });
    out
}
