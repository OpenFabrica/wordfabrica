//! Versioned snapshots shared by the browser shell and the storage service.
//! Revision zero creates a document; subsequent writes must match the stored revision.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use serde::{Deserialize, Serialize};
use wordcraft_doc::Document;

/// Maximum encoded request/snapshot size (including embedded pictures).
pub const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Metadata {
    pub id: String,
    pub title: String,
    pub revision: u64,
    /// Unix milliseconds, assigned by the server.
    pub updated_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub metadata: Metadata,
    pub document: Document,
    /// Opened through a view-only link: the editor refuses changes and never saves.
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Serialize, Deserialize)]
pub struct WriteRequest {
    pub revision: u64,
    pub title: String,
    pub document: Document,
    /// An explicit save (Ctrl+S / Save), not an autosave. For documents opened by a third
    /// party, the server then exports DOCX to file storage and calls the third party back.
    #[serde(default)]
    pub commit: bool,
}

/// IDs are opaque, client-generated UUIDs. They are never filesystem paths.
pub fn valid_id(id: &str) -> bool {
    id.len() == 36 && id.bytes().enumerate().all(|(i, c)| if [8, 13, 18, 23].contains(&i) { c == b'-' } else { c.is_ascii_hexdigit() })
}

/// Bounded, eager font delivery keeps startup predictable without a separate font service.
pub const MAX_FONT_BYTES: usize = 40 * 1024 * 1024;
pub const MAX_FONT_TOTAL: usize = 128 * 1024 * 1024;
pub const MAX_FONT_FILES: usize = 64;

/// One immutable server font file. Its opaque ID changes when the server restarts;
/// a stale catalog fails with 404 rather than resolving to a different font.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FontFile {
    pub id: String,
    pub families: Vec<String>,
    pub bytes: usize,
}

/// Administrator-provided plugin files are small, bounded data. They carry no code, so a plugin
/// can only add chrome built into the app (for now: a tag pane that inserts text).
pub const MAX_PLUGIN_BYTES: usize = 1024 * 1024;
pub const MAX_PLUGINS: usize = 32;
pub const MAX_PLUGIN_TAGS: usize = 5000;
/// Longest tag text, in characters.
pub const MAX_TAG_TEXT: usize = 2000;

/// One plugin: an ID, a display name, and what it adds to the web chrome.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    /// Users may change the tags in the pane; the server stores their version (shared by all).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub editable: bool,
    /// Version of the users' tags, set by the server: 0 while the file's tags are in use.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub revision: u64,
    #[serde(flatten)]
    pub kind: PluginKind,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

/// `PUT /api/plugins/{id}/tags`: replace an editable plugin's tags. `revision` is the version the
/// edit started from; the server refuses the write when someone saved a newer one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TagsWrite {
    pub revision: u64,
    pub categories: Vec<TagCategory>,
}

/// What a plugin adds. The `kind` field in the JSON picks the variant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PluginKind {
    /// Template tags: a side pane of categorized text snippets; clicking one inserts it at the caret.
    Tags { categories: Vec<TagCategory> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TagCategory {
    pub name: String,
    pub tags: Vec<Tag>,
}

/// A template tag: `label` is shown in the pane, `text` is inserted (the label when empty).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tag {
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// Optional help shown as the tag's tooltip.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

impl Tag {
    /// The text inserted into the document.
    pub fn insert_text(&self) -> &str {
        if self.text.is_empty() { &self.label } else { &self.text }
    }
}

impl Plugin {
    /// Check a plugin read from a file or the network: IDs, sizes and characters are bounded, so
    /// the chrome never shows or inserts hostile content. Returns the first problem found.
    pub fn validate(&self) -> Result<(), String> {
        let id_ok = !self.id.is_empty()
            && self.id.len() <= 64
            && self.id.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'-' | b'_' | b'.'));
        if !id_ok {
            return Err(format!("plugin id {:?} must be 1–64 of a-z 0-9 - _ .", self.id));
        }
        short_text(&self.name, 100).map_err(|e| format!("{}: name {e}", self.id))?;
        match &self.kind {
            PluginKind::Tags { categories } => {
                // Users may delete every category, so an empty list is valid.
                if categories.len() > 500 {
                    return Err(format!("{}: more than 500 categories", self.id));
                }
                let mut count = 0usize;
                for category in categories {
                    short_text(&category.name, 100).map_err(|e| format!("{}: category name {e}", self.id))?;
                    count = count.saturating_add(category.tags.len());
                    if count > MAX_PLUGIN_TAGS {
                        return Err(format!("{}: more than {MAX_PLUGIN_TAGS} tags", self.id));
                    }
                    for tag in &category.tags {
                        short_text(&tag.label, 200).map_err(|e| format!("{}: tag label {e}", self.id))?;
                        // Tabs are fine in inserted text; other control characters (line breaks
                        // included) would turn one click into structural edits.
                        if tag.text.chars().count() > MAX_TAG_TEXT || tag.text.chars().any(|c| c.is_control() && c != '\t') {
                            return Err(format!("{}: tag {:?} text is too long or has control characters", self.id, tag.label));
                        }
                        if tag.note.chars().count() > 500 || tag.note.chars().any(|c| c.is_control() && c != '\n') {
                            return Err(format!("{}: tag {:?} note is too long or has control characters", self.id, tag.label));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// A one-line display string: not blank, at most `max` characters, no control characters.
fn short_text(s: &str, max: usize) -> Result<(), String> {
    if s.trim().is_empty() || s.chars().count() > max || s.chars().any(char::is_control) {
        return Err(format!("{s:?} must be 1–{max} characters on one line"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(json: &str) -> Plugin {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn tag_plugins_parse_and_validate() {
        let p = tags(
            r#"{"id":"contract-tags","name":"模版标签","kind":"tags","categories":[
                {"name":"客户","tags":[{"label":"客户名称","text":"{{客户名称}}","note":"甲方全称"},{"label":"{{日期}}"}]}]}"#,
        );
        assert_eq!(p.validate(), Ok(()));
        let PluginKind::Tags { categories } = &p.kind;
        assert_eq!(categories[0].tags[0].insert_text(), "{{客户名称}}");
        assert_eq!(categories[0].tags[1].insert_text(), "{{日期}}", "empty text inserts the label");
        // Round trip keeps the shape administrators write.
        assert_eq!(tags(&serde_json::to_string(&p).unwrap()), p);
    }

    #[test]
    fn hostile_plugins_are_rejected() {
        let ok = r#"{"id":"a","name":"A","kind":"tags","categories":[{"name":"C","tags":[{"label":"L","text":"T"}]}]}"#;
        assert_eq!(tags(ok).validate(), Ok(()));
        for bad in [
            ok.replace(r#""id":"a""#, r#""id":"../etc""#),
            ok.replace(r#""id":"a""#, r#""id":"""#),
            ok.replace(r#""name":"A""#, r#""name":" ""#),
            ok.replace(r#""name":"C""#, r#""name":"C\nD""#),
            ok.replace(r#""text":"T""#, r#""text":"line\nbreak""#),
            ok.replace(r#""text":"T""#, &format!(r#""text":"{}""#, "x".repeat(MAX_TAG_TEXT + 1))),
            ok.replace(r#"[{"name":"C","tags":[{"label":"L","text":"T"}]}]"#, &format!("[{}]", vec![r#"{"name":"C","tags":[]}"#; 501].join(","))),
        ] {
            assert!(tags(&bad).validate().is_err(), "{bad}");
        }
        assert!(serde_json::from_str::<Plugin>(&ok.replace(r#""kind":"tags""#, r#""kind":"script""#)).is_err(), "unknown kinds don't parse");
        let empty = ok.replace(r#"[{"name":"C","tags":[{"label":"L","text":"T"}]}]"#, "[]");
        assert_eq!(tags(&empty).validate(), Ok(()), "users may delete every category");
        assert!(!tags(ok).editable && tags(ok).revision == 0, "files are read-only unless they say otherwise");
    }
}
