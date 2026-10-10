//! Shared status-bar and Review menu for document language, separate from UI localization.
use crate::WordApp;
use serde_json::json;

/// Display name of a document language code (translated).
pub fn label(code: &str) -> &str {
    match code {
        "zh-CN" | "zh-Hans" => tl!("Chinese (China)"),
        "en-US" => tl!("English (United States)"),
        _ => code,
    }
}

pub fn current_label(app: &WordApp) -> String {
    let state = wordfabrica_engine::cmd::format::state(&app.session);
    state.get("lang").and_then(serde_json::Value::as_str).map(label).unwrap_or(tl!("Multiple languages")).to_string()
}

pub fn menu(app: &mut WordApp, ui: &mut egui::Ui) {
    ui.label(tl!("Document language"));
    ui.label(tl!("Apply to the entire document"));
    ui.separator();
    let current = app.session.doc.styles.default_chr.lang.as_deref().unwrap_or("en-US").to_string();
    for code in ["zh-CN", "en-US"] {
        if ui.selectable_label(current == code, label(code)).clicked() {
            let _ = app.run("review.language", json!({"lang":code, "scope":"document"}));
            ui.close();
        }
    }
}
