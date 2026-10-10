//! Browser preferences are local to this origin, independent of the server document.
use wordfabrica_ui_egui::{WordApp, i18n, tl};

const KEY: &str = "wordfabrica.interface-language";

pub struct Locale {
    last: String,
    failed: bool,
}

impl Locale {
    pub fn new(app: &mut WordApp) -> Self {
        let saved = web_sys::window().and_then(|w| w.local_storage().ok().flatten()).and_then(|s| s.get_item(KEY).ok().flatten());
        let browser = browser_language();
        let language = wordfabrica_web::locale::preferred(saved.as_deref(), &browser);
        app.ui.language = language.into();
        i18n::set_current(i18n::Lang::from_pref(language));
        set_document_language(language);
        Self { last: language.into(), failed: false }
    }

    /// Persist only a language change, never document content. Storage denial keeps switching usable.
    pub fn sync(&mut self, app: &mut WordApp) {
        if app.ui.language == i18n::AUTO {
            app.ui.language = wordfabrica_web::locale::preferred(None, &browser_language()).into();
        }
        i18n::set_current(i18n::Lang::from_pref(&app.ui.language));
        if self.last != app.ui.language {
            self.last = app.ui.language.clone();
            self.failed =
                web_sys::window().and_then(|w| w.local_storage().ok().flatten()).is_none_or(|storage| storage.set_item(KEY, &self.last).is_err());
            set_document_language(&self.last);
        }
    }

    pub fn notice(&self, ui: &mut egui::Ui) {
        if self.failed {
            egui::Panel::top("language_notice").show(ui, |ui| {
                ui.label(tl!("Language changed for this tab. Your browser could not save the preference."));
            });
        }
    }
}

fn browser_language() -> String {
    web_sys::window().and_then(|w| w.navigator().language()).unwrap_or_else(|| "en".into())
}

fn set_document_language(language: &str) {
    if let Some(document) = web_sys::window().and_then(|w| w.document())
        && let Some(root) = document.document_element()
    {
        let _ = root.set_attribute("lang", if language == "zh-hans" { "zh-Hans" } else { "en" });
    }
}
