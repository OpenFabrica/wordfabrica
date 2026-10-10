//! Only Chinese and English are exposed by the browser application.

/// A saved explicit choice wins; invalid preferences fall back to the browser, then English.
pub fn preferred(saved: Option<&str>, browser: &str) -> &'static str {
    match saved {
        Some("zh-hans") => "zh-hans",
        Some("en") => "en",
        _ if browser.split(['-', '_']).next().is_some_and(|tag| tag.eq_ignore_ascii_case("zh")) => "zh-hans",
        _ => "en",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_choice_survives_browser_language_and_invalid_storage() {
        assert_eq!(preferred(Some("en"), "zh-CN"), "en");
        assert_eq!(preferred(Some("zh-hans"), "en-US"), "zh-hans");
        assert_eq!(preferred(None, "zh-SG"), "zh-hans");
        assert_eq!(preferred(None, "zh-TW"), "zh-hans");
        assert_eq!(preferred(Some("invalid"), "en-GB"), "en");
        assert_eq!(preferred(None, "fr-FR"), "en");
    }
}
