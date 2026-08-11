//! Keybinding hint formatting shared by interactive chrome.

use zedflow_tui::get_keybindings;

#[must_use]
pub fn format_key_text(key: &str, capitalize: bool) -> String {
    format_key_text_for_platform(key, capitalize, cfg!(target_os = "macos"))
}

#[must_use]
pub fn format_key_text_for_platform(key: &str, capitalize: bool, macos: bool) -> String {
    key.split('/')
        .map(|key| {
            key.split('+')
                .map(|part| {
                    let part = if macos && part.eq_ignore_ascii_case("alt") {
                        "option"
                    } else {
                        part
                    };
                    if capitalize {
                        let mut chars = part.chars();
                        chars
                            .next()
                            .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                            .unwrap_or_default()
                    } else {
                        part.into()
                    }
                })
                .collect::<Vec<_>>()
                .join("+")
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[must_use]
pub fn key_text(keybinding: &str) -> String {
    let keys = get_keybindings().lock().unwrap().get_keys(keybinding);
    format_key_text(&keys.join("/"), false)
}

#[must_use]
pub fn key_display_text(keybinding: &str) -> String {
    let keys = get_keybindings().lock().unwrap().get_keys(keybinding);
    format_key_text(&keys.join("/"), true)
}

#[must_use]
pub fn key_hint(keybinding: &str, description: &str) -> String {
    format!("{} {description}", key_text(keybinding))
}
#[must_use]
pub fn raw_key_hint(key: &str, description: &str) -> String {
    format!("{} {description}", format_key_text(key, false))
}
