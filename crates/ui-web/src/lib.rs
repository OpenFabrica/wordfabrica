//! WordCraft's web chrome: the title bar, ribbon, status bar, side panes, dialogs and File
//! screen as HTML/CSS (written in Rust with Leptos), around the egui canvas that still draws and
//! edits the document.
//!
//! One rule keeps the two halves consistent: the chrome owns no document state. It reads a
//! [`wordcraft_ui_egui::ChromeState`] snapshot that the app publishes after every frame and sends
//! [`wordcraft_ui_egui::Action`]s back; the app applies them before the next frame. Menu and
//! hover state are the only things the HTML side keeps.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod model;
pub mod plugins;

#[cfg(target_arch = "wasm32")]
mod bridge;
#[cfg(target_arch = "wasm32")]
mod view;

#[cfg(target_arch = "wasm32")]
pub use bridge::Bridge;

/// The registry's name for a keyboard shortcut (`Mod+Shift+B`, `F7`…) from a DOM `KeyboardEvent`
/// (`key`, modifier flags). `mac` maps ⌘ to `Mod`; elsewhere Ctrl is `Mod`. Keys the registry
/// never binds return `None`.
pub fn shortcut_name(key: &str, ctrl: bool, meta: bool, alt: bool, shift: bool, mac: bool) -> Option<String> {
    let name = match key {
        "ArrowLeft" => "Left".to_string(),
        "ArrowRight" => "Right".to_string(),
        "ArrowUp" => "Up".to_string(),
        "ArrowDown" => "Down".to_string(),
        "Home" | "End" | "PageUp" | "PageDown" | "Enter" | "Tab" | "Backspace" | "Delete" | "Escape" => key.to_string(),
        " " => "Space".to_string(),
        "=" | "+" => "=".to_string(),
        "-" | "_" => "-".to_string(),
        "." | "," | "[" | "]" => key.to_string(),
        k if k.len() >= 2 && k.starts_with('F') && k.chars().skip(1).all(|c| c.is_ascii_digit()) => k.to_string(),
        k => {
            let mut chars = k.chars();
            let (Some(c), None) = (chars.next(), chars.next()) else { return None };
            if !c.is_ascii_alphanumeric() {
                return None;
            }
            c.to_ascii_uppercase().to_string()
        }
    };
    let command = if mac { meta } else { ctrl };
    let mut s = String::new();
    if command {
        s.push_str("Mod+");
    }
    if ctrl && mac {
        s.push_str("Ctrl+");
    }
    if alt {
        s.push_str("Alt+");
    }
    if shift {
        s.push_str("Shift+");
    }
    s.push_str(&name);
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_names_match_the_registry() {
        let reg = wordcraft_engine::cmd::registry();
        let bold = shortcut_name("b", true, false, false, false, false);
        assert_eq!(bold.as_deref(), Some("Mod+B"));
        assert_eq!(reg.by_shortcut("Mod+B").map(|s| s.id), Some("format.bold"));
        assert_eq!(shortcut_name("b", false, true, false, false, true).as_deref(), Some("Mod+B"));
        assert_eq!(shortcut_name("Z", true, false, false, true, false).as_deref(), Some("Mod+Shift+Z"));
        assert_eq!(shortcut_name("F7", false, false, false, false, false).as_deref(), Some("F7"));
        assert_eq!(shortcut_name("ArrowLeft", false, false, false, true, false).as_deref(), Some("Shift+Left"));
        assert_eq!(shortcut_name("Dead", false, false, false, false, false), None);
        assert_eq!(shortcut_name("é", true, false, false, false, false), None);
        assert_eq!(shortcut_name("", true, false, false, false, false), None);
    }
}
