//! File tab (Backstage): Home/New, Open, Info, Save As, Print, Export, Options, About.

use egui::{Align2, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;

use crate::theme::{APP_COLOR, Tokens, medium, regular, semibold};
use crate::{WordApp, icons};

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

pub fn show(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::left("backstage_nav")
        .exact_size(200.0)
        .frame(egui::Frame::NONE.fill(APP_COLOR).inner_margin(egui::Margin { left: 0, right: 0, top: 12, bottom: 12 }))
        .show(ui, |ui| {
            let (r, resp) = ui.allocate_exact_size(vec2(200.0, 40.0), Sense::click());
            icons::paint(
                ui.painter(),
                Rect::from_center_size(pos2(r.min.x + 26.0, r.center().y), vec2(18.0, 18.0)),
                "chevronLeft",
                egui::Color32::WHITE,
                egui::Color32::WHITE,
            );
            if resp.clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                app.ui.backstage = false;
                app.canvas.want_focus = true;
            }
            ui.add_space(6.0);
            for (id, label) in PAGES {
                let (r, resp) = ui.allocate_exact_size(vec2(200.0, 38.0), Sense::click());
                let active = app.ui.backstage_page == id;
                if active {
                    ui.painter().rect_filled(r, 0.0, egui::Color32::from_white_alpha(46));
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 0.0, egui::Color32::from_white_alpha(26));
                }
                ui.painter().text(
                    pos2(r.min.x + 22.0, r.center().y),
                    Align2::LEFT_CENTER,
                    tl!(label),
                    if active { semibold(14.0) } else { medium(14.0) },
                    egui::Color32::WHITE,
                );
                if resp.clicked() {
                    match id {
                        "save" => {
                            let _ = app.run("file.save", json!({}));
                        }
                        "saveAs" => app.save_as_dialog(),
                        "open" => {
                            app.ui.backstage_page = id.into();
                        }
                        _ => app.ui.backstage_page = id.into(),
                    }
                }
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                let (r, resp) = ui.allocate_exact_size(vec2(200.0, 32.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(r, 0.0, egui::Color32::from_white_alpha(26));
                }
                ui.painter().text(pos2(r.min.x + 22.0, r.center().y), Align2::LEFT_CENTER, tl!("About"), regular(13.0), egui::Color32::WHITE);
                if resp.clicked() {
                    app.dialog = crate::dialogs::Dialog::open("about", app);
                }
            });
        });
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.ribbon).inner_margin(egui::Margin::symmetric(40, 30))).show(ui, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| match app.ui.backstage_page.as_str() {
            "new" | "home" => new_page(app, ui),
            "open" => open_page(app, ui),
            "info" => info_page(app, ui),
            "print" | "export" => export_page(app, ui),
            "options" => options_page(app, ui),
            _ => new_page(app, ui),
        });
    });
}

fn heading(ui: &mut Ui, s: &str) {
    ui.label(egui::RichText::new(tl!(s)).font(semibold(26.0)));
    ui.add_space(16.0);
}

fn template_tile(ui: &mut Ui, app: &mut WordApp, label: &str, template: &str) {
    let t = Tokens::get(ui.ctx());
    ui.vertical(|ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(150.0, 194.0), Sense::click());
        ui.painter().rect(
            r,
            2.0,
            egui::Color32::WHITE,
            Stroke::new(if resp.hovered() { 2.0 } else { 1.0 }, if resp.hovered() { t.accent } else { t.border_strong }),
            egui::StrokeKind::Inside,
        );
        let ppp = ui.ctx().pixels_per_point();
        let key = format!("tpl:{template}:{ppp}");
        let tex = ui.ctx().data(|d| d.get_temp::<egui::TextureHandle>(egui::Id::new(&key))).or_else(|| {
            let img = crate::previews::template_image(template, 150.0 * ppp)?;
            let h = ui.ctx().load_texture(&key, img, egui::TextureOptions::LINEAR);
            ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(&key), h.clone()));
            Some(h)
        });
        if let Some(h) = tex {
            ui.painter().image(h.id(), r.shrink(1.0), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
        ui.label(egui::RichText::new(tl!(label)).font(regular(12.5)));
        if resp.clicked() {
            let _ = app.run("file.new", json!({"template": template}));
            app.ui.backstage = false;
            app.canvas.want_focus = true;
        }
    });
}

fn new_page(app: &mut WordApp, ui: &mut Ui) {
    let hour = wordfabrica_engine::cmd::now_iso().get(11..13).and_then(|h| h.parse::<u32>().ok()).unwrap_or(9);
    heading(
        ui,
        if hour < 12 {
            "Good morning"
        } else if hour < 18 {
            "Good afternoon"
        } else {
            "Good evening"
        },
    );
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(18.0, 0.0);
        template_tile(ui, app, "Blank document", "blank");
        template_tile(ui, app, "Studio handbook (sample)", "sample");
        template_tile(ui, app, "Letter", "letter");
        template_tile(ui, app, "Résumé", "resume");
        template_tile(ui, app, "Report", "report");
    });
    ui.add_space(28.0);
    open_list(app, ui);
}

/// Documents to reopen: server documents when hosted (browsers can't reopen local paths), else Recent.
fn open_list(app: &mut WordApp, ui: &mut Ui) {
    if app.hosted.is_some() {
        server_list(app, ui);
        return;
    }
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(tl!("Recent")).font(semibold(16.0)));
    ui.add_space(6.0);
    if app.ui.recent.is_empty() {
        ui.label(egui::RichText::new(tl!("Documents you open will show up here.")).color(t.text_dim));
    }
    for p in app.ui.recent.clone() {
        let name = std::path::Path::new(&p).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(p.clone());
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width().min(700.0), 40.0), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r, 4.0, t.hover);
        }
        icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 18.0, r.center().y), vec2(22.0, 22.0)), "printLayout", t.icon, t.accent);
        ui.painter().text(pos2(r.min.x + 40.0, r.min.y + 13.0), Align2::LEFT_CENTER, &name, medium(13.0), t.text);
        ui.painter().text(pos2(r.min.x + 40.0, r.min.y + 29.0), Align2::LEFT_CENTER, &p, regular(11.0), t.text_dim);
        if resp.clicked() {
            let _ = app.run("file.open", json!({"path": p}));
            app.ui.backstage = false;
            app.canvas.want_focus = true;
        }
    }
}

/// Hosted web: the server document list. Clicks only leave a request; the host loads the document.
fn server_list(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let dirty = app.session.dirty;
    let Some(hosted) = &mut app.hosted else { return };
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(tl!("Server documents")).font(semibold(16.0)));
        if ui.add_enabled(!hosted.listing, egui::Button::new(tl!("Refresh"))).clicked() {
            hosted.refresh = true;
        }
    });
    ui.add_space(6.0);
    if hosted.listing {
        ui.label(egui::RichText::new(tl!("Loading…")).color(t.text_dim));
    }
    if let Some(error) = &hosted.list_error {
        ui.label(egui::RichText::new(crate::i18n::t(error)).color(t.red));
    }
    if dirty {
        ui.label(egui::RichText::new(tl!("Save your current changes before opening another document.")).color(t.text_dim));
    }
    if hosted.documents.is_empty() && !hosted.listing && hosted.list_error.is_none() {
        ui.label(egui::RichText::new(tl!("No saved documents yet.")).color(t.text_dim));
    }
    let enabled = !dirty && !hosted.busy;
    let mut picked = None;
    for item in &hosted.documents {
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width().min(700.0), 40.0), if enabled { Sense::click() } else { Sense::hover() });
        if enabled && resp.hovered() {
            ui.painter().rect_filled(r, 4.0, t.hover);
        }
        let text = if enabled { t.text } else { t.text_dim };
        icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 18.0, r.center().y), vec2(22.0, 22.0)), "printLayout", t.icon, t.accent);
        ui.painter().text(pos2(r.min.x + 40.0, r.min.y + 13.0), Align2::LEFT_CENTER, &item.title, medium(13.0), text);
        ui.painter().text(pos2(r.min.x + 40.0, r.min.y + 29.0), Align2::LEFT_CENTER, format!("v{}", item.revision), regular(11.0), t.text_dim);
        if resp.clicked() {
            picked = Some(item.id.clone());
        }
    }
    if let Some(id) = picked {
        hosted.open = Some(id);
        app.ui.backstage = false;
    }
}

fn open_page(app: &mut WordApp, ui: &mut Ui) {
    heading(ui, "Open");
    if ui.button(egui::RichText::new(tl!("📂  Browse…")).font(medium(14.0))).clicked() {
        let _ = app.run("ui.openFileDialog", json!({}));
    }
    ui.add_space(18.0);
    open_list(app, ui);
}

fn info_page(app: &mut WordApp, ui: &mut Ui) {
    heading(ui, "Info");
    let info = app.session.run("file.info", &json!({})).unwrap_or_default();
    ui.columns(2, |cols| {
        let ui = &mut cols[0];
        ui.label(egui::RichText::new(tl!("Properties")).font(semibold(15.0)));
        let mut props = app.session.doc.core.clone();
        let mut changed = false;
        egui::Grid::new("props").num_columns(2).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
            for (l, v) in [("Title", &mut props.title), ("Subject", &mut props.subject), ("Author", &mut props.creator), ("Keywords", &mut props.keywords), ("Category", &mut props.category)] {
                ui.label(tl!(l));
                changed |= ui.text_edit_singleline(v).lost_focus();
                ui.end_row();
            }
        });
        if changed {
            let _ = app.run("file.properties", json!({"title": props.title, "subject": props.subject, "author": props.creator, "keywords": props.keywords, "category": props.category}));
        }
        let ui = &mut cols[1];
        ui.label(egui::RichText::new(tl!("Statistics")).font(semibold(15.0)));
        for (l, k) in [("Pages", "pages"), ("Words", "words"), ("Paragraphs", "paragraphs"), ("Sections", "sections"), ("Comments", "comments")] {
            ui.label(format!("{}: {}", tl!(l), info.get(k).map(|v| v.to_string()).unwrap_or_default()));
        }
        let path = info.get("path").and_then(|v| v.as_str()).unwrap_or(tl!("Not saved yet"));
        ui.label(crate::i18n::fmt(tl!("Location: {path}"), &[("path", path)]));
    });
}

fn export_page(app: &mut WordApp, ui: &mut Ui) {
    heading(ui, if app.ui.backstage_page == "print" { "Print" } else { "Export" });
    ui.label(tl!("Save a copy in another format. Printing goes through a PDF you can print from any viewer."));
    ui.add_space(12.0);
    for (label, ext) in [
        ("PDF document (*.pdf)", "pdf"),
        ("Word document (*.docx)", "docx"),
        ("OpenDocument Text (*.odt)", "odt"),
        ("Rich Text Format (*.rtf)", "rtf"),
        ("Web page (*.html)", "html"),
        ("Markdown (*.md)", "md"),
        ("Plain text (*.txt)", "txt"),
        ("Page image (*.png)", "png"),
    ] {
        if ui.add(egui::Button::new(egui::RichText::new(tl!(label)).font(medium(13.5))).min_size(vec2(320.0, 34.0))).clicked() {
            app.export_copy(ext);
        }
        ui.add_space(4.0);
    }
}

fn options_page(app: &mut WordApp, ui: &mut Ui) {
    heading(ui, "Options");
    ui.label(egui::RichText::new(tl!("General")).font(semibold(15.0)));
    language_picker(app, ui);
    ui.horizontal(|ui| {
        ui.label(tl!("User name:"));
        let mut n = app.session.author.clone();
        if ui.text_edit_singleline(&mut n).changed() {
            app.session.author = n;
        }
    });
    let mut dark = app.ui.dark;
    if ui.checkbox(&mut dark, tl!("Dark mode")).changed() {
        let _ = app.run("ui.dark", json!({"value": dark}));
    }
    ui.checkbox(&mut app.autosave, tl!("AutoSave documents that have been saved"));
    ui.add_space(10.0);
    ui.label(egui::RichText::new(tl!("Display")).font(semibold(15.0)));
    let mut marks = app.session.view.marks;
    if ui.checkbox(&mut marks, tl!("Show all formatting marks")).changed() {
        let _ = app.run("view.marks", json!({"value": marks}));
    }
    let mut ruler = app.session.view.ruler;
    if ui.checkbox(&mut ruler, tl!("Show rulers")).changed() {
        let _ = app.run("view.ruler", json!({"value": ruler}));
    }
    ui.add_space(10.0);
    ui.label(egui::RichText::new(tl!("Agents")).font(semibold(15.0)));
    ui.label(tl!("Every command is available to scripts and AI agents: run `wordfabrica-cli mcp` for an MCP server, or start the app with `--control <port>` for the JSON control channel."));
}

/// File ▸ Options ▸ Interface language: follow the system (the default) or pick one (#8).
fn language_picker(app: &mut WordApp, ui: &mut Ui) {
    use crate::i18n::{AUTO, Lang};
    let system = crate::i18n::system_lang();
    let auto_label = crate::i18n::fmt(tl!("Automatic ({language})"), &[("language", system.name())]);
    let current = if app.ui.language == AUTO { auto_label.clone() } else { Lang::from_pref(&app.ui.language).name().to_string() };
    ui.horizontal(|ui| {
        ui.label(tl!("Interface language:"));
        egui::ComboBox::from_id_salt("interface_language").selected_text(current).width(220.0).show_ui(ui, |ui| {
            if !cfg!(target_arch = "wasm32") && ui.selectable_label(app.ui.language == AUTO, auto_label.as_str()).clicked() {
                let _ = app.run("ui.language", json!({"value": AUTO}));
            }
            for lang in Lang::all() {
                if ui.selectable_label(app.ui.language == lang.code(), lang.name()).clicked() {
                    let _ = app.run("ui.language", json!({"value": lang.code()}));
                }
            }
        });
    });
    let help = if cfg!(target_arch = "wasm32") {
        tl!("Menus and commands change; your documents don't.")
    } else {
        tl!("Automatic follows your system's language. Menus and commands change; your documents don't.")
    };
    ui.label(egui::RichText::new(help).small().weak());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Hosted, HostedDocument};
    use wordfabrica_doc::Document;
    use wordfabrica_engine::Session;

    /// A hosted app on File ▸ Open with one server document listed.
    fn hosted_app(dirty: bool) -> WordApp {
        crate::i18n::set_current(crate::i18n::Lang::EN);
        let mut app = WordApp::new(Session::new(Document::new()), Default::default());
        app.ui.language = "en".into();
        app.ui.backstage = true;
        app.ui.backstage_page = "open".into();
        app.ui.recent = vec!["/tmp/local.docx".into()];
        app.session.dirty = dirty;
        app.hosted = Some(Hosted {
            documents: vec![HostedDocument { id: "doc-1".into(), title: "Quarterly notes".into(), revision: 4 }],
            ..Default::default()
        });
        app
    }

    /// Text shapes the last frame painted, with their positions.
    fn texts(h: &egui_kittest::Harness<'_, WordApp>) -> Vec<(String, egui::Pos2)> {
        h.output()
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some((text.galley.job.text.clone(), text.pos)),
                _ => None,
            })
            .collect()
    }

    /// Click the first painted text equal to `label`.
    fn click_text(h: &mut egui_kittest::Harness<'_, WordApp>, label: &str) {
        let at = texts(h).into_iter().find(|(t, _)| t == label).map(|(_, p)| p + vec2(4.0, 4.0));
        let Some(at) = at else { panic!("{label:?} not painted") };
        let input = h.input_mut();
        input.events.push(egui::Event::PointerMoved(at));
        for pressed in [true, false] {
            input.events.push(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() });
        }
        h.run_steps(2);
    }

    #[test]
    fn hosted_open_lists_server_documents_and_requests_the_pick() {
        let mut h = egui_kittest::Harness::builder().with_size(vec2(1200.0, 800.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                app.logic(ui.ctx());
                app.ui(ui);
            },
            hosted_app(false),
        );
        h.run_steps(6);
        let painted: Vec<_> = texts(&h).into_iter().map(|(t, _)| t).collect();
        assert!(painted.iter().any(|t| t == "Server documents"), "{painted:?}");
        assert!(painted.iter().any(|t| t == "v4"), "{painted:?}");
        // Browsers can't reopen local paths, so Recent stays hidden when hosted.
        assert!(!painted.iter().any(|t| t == "/tmp/local.docx"), "{painted:?}");
        click_text(&mut h, "Quarterly notes");
        assert_eq!(h.state().hosted.as_ref().and_then(|x| x.open.as_deref()), Some("doc-1"));
        assert!(!h.state().ui.backstage);
    }

    #[test]
    fn hosted_open_keeps_unsaved_changes_from_being_replaced() {
        let mut h = egui_kittest::Harness::builder().with_size(vec2(1200.0, 800.0)).build_ui_state(
            |ui, app: &mut WordApp| {
                app.logic(ui.ctx());
                app.ui(ui);
            },
            hosted_app(true),
        );
        h.run_steps(6);
        click_text(&mut h, "Quarterly notes");
        assert_eq!(h.state().hosted.as_ref().and_then(|x| x.open.clone()), None);
        assert!(h.state().ui.backstage);
    }
}
