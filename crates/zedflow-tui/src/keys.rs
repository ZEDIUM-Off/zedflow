//! Minimal terminal key decoding shared by TUI consumers.

use std::sync::atomic::{AtomicBool, Ordering};

static KITTY_PROTOCOL_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn set_kitty_protocol_active(active: bool) {
    KITTY_PROTOCOL_ACTIVE.store(active, Ordering::Relaxed);
}
pub fn is_kitty_protocol_active() -> bool {
    KITTY_PROTOCOL_ACTIVE.load(Ordering::Relaxed)
}

/// Decode the legacy escape sequences used by terminals for navigation keys.
pub fn parse_key(data: &str) -> Option<&'static str> {
    // Legacy sequences, plus Kitty CSI-u and xterm modifier forms.
    if let Some(key) = parse_kitty_key(data) {
        return Some(Box::leak(key.into_boxed_str()));
    }
    if data.len() == 1 {
        let byte = data.as_bytes()[0];
        if (1..=26).contains(&byte) {
            return Some(Box::leak(
                format!("ctrl+{}", (b'a' + byte - 1) as char).into_boxed_str(),
            ));
        }
    }
    if let Some(rest) = data.strip_prefix('\x1b') {
        if rest.len() == 1 {
            let byte = rest.as_bytes()[0];
            if (b'a'..=b'z').contains(&byte) || (b'0'..=b'9').contains(&byte) {
                return Some(Box::leak(format!("alt+{}", byte as char).into_boxed_str()));
            }
            if (1..=26).contains(&byte) {
                return Some(Box::leak(
                    format!("ctrl+alt+{}", (b'a' + byte - 1) as char).into_boxed_str(),
                ));
            }
        }
    }
    // Pi treats a raw printable character as its own key identifier.
    if data.chars().count() == 1 {
        let character = data.chars().next().unwrap();
        if character > ' ' && character != '\x7f' {
            return Some(Box::leak(data.to_owned().into_boxed_str()));
        }
    }
    let key = match data {
        "\x1b" => "escape",
        "\r" | "\n" => "enter",
        "\t" => "tab",
        " " => "space",
        "\x7f" => "backspace",
        "\x1b[A" | "\x1bOA" => "up",
        "\x1b[B" | "\x1bOB" => "down",
        "\x1b[C" | "\x1bOC" => "right",
        "\x1b[D" | "\x1bOD" => "left",
        "\x1b[H" | "\x1bOH" | "\x1b[1~" | "\x1b[7~" => "home",
        "\x1b[F" | "\x1bOF" | "\x1b[4~" | "\x1b[8~" => "end",
        "\x1b[E" | "\x1bOE" => "clear",
        "\x1b[2~" => "insert",
        "\x1b[3~" => "delete",
        "\x1b[5~" | "\x1b[[5~" => "pageUp",
        "\x1b[6~" | "\x1b[[6~" => "pageDown",
        "\x1bOP" | "\x1b[11~" | "\x1b[[A" => "f1",
        "\x1bOQ" | "\x1b[12~" | "\x1b[[B" => "f2",
        "\x1bOR" | "\x1b[13~" | "\x1b[[C" => "f3",
        "\x1bOS" | "\x1b[14~" | "\x1b[[D" => "f4",
        "\x1b[15~" => "f5",
        "\x1b[17~" => "f6",
        "\x1b[18~" => "f7",
        "\x1b[19~" => "f8",
        "\x1b[20~" => "f9",
        "\x1b[21~" => "f10",
        "\x1b[23~" => "f11",
        "\x1b[24~" => "f12",
        "\x1b[a" => "shift+up",
        "\x1b[b" => "shift+down",
        "\x1b[c" => "shift+right",
        "\x1b[d" => "shift+left",
        "\x1bOa" => "ctrl+up",
        "\x1bOb" => "ctrl+down",
        "\x1bOc" => "ctrl+right",
        "\x1bOd" => "ctrl+left",
        "\x1b[2$" => "shift+insert",
        "\x1b[3$" => "shift+delete",
        "\x1b[2^" => "ctrl+insert",
        "\x1b[3^" => "ctrl+delete",
        "\x1b[1;2A" => "shift+up",
        "\x1b[1;2B" => "shift+down",
        "\x1b[1;2C" => "shift+right",
        "\x1b[1;2D" => "shift+left",
        "\x1b[1;5A" => "ctrl+up",
        "\x1b[1;5B" => "ctrl+down",
        "\x1b[1;5C" => "ctrl+right",
        "\x1b[1;5D" => "ctrl+left",
        "\x1b[13;2u" => "shift+enter",
        "\x1b[13;5u" => "ctrl+enter",
        "\x1b[9;3u" => "alt+tab",
        // Unsupported input, including unsupported one-character input.
        _ => return None,
    };
    Some(key)
}

fn parse_kitty_key(data: &str) -> Option<String> {
    let body = data.strip_prefix("\x1b[")?.strip_suffix('u')?;
    let (key_codes, modifier) = body.split_once(';').map_or((body, "1"), |v| v);
    let mut key_codes = key_codes.split(':');
    let code: u32 = key_codes.next()?.parse().ok()?;
    let _shifted = key_codes.next();
    let base_layout_code = key_codes
        .next()
        .filter(|code| !code.is_empty())
        .and_then(|code| code.parse().ok());
    let modifier = modifier
        .split(':')
        .next()?
        .parse::<u32>()
        .ok()?
        .saturating_sub(1);
    let code = if modifier & 1 != 0 && (65..=90).contains(&code) {
        code + 32
    } else {
        code
    };
    let code = if (32..=126).contains(&code) {
        code
    } else {
        base_layout_code?
    };
    let key = match code {
        13 => "enter".to_owned(),
        9 => "tab".to_owned(),
        127 => "backspace".to_owned(),
        32..=126 => char::from_u32(code)?.to_string(),
        _ => return None,
    };
    let mut prefix = String::new();
    if modifier & 1 != 0 {
        prefix.push_str("shift+");
    }
    if modifier & 2 != 0 {
        prefix.push_str("alt+");
    }
    if modifier & 4 != 0 {
        prefix.push_str("ctrl+");
    }
    Some(format!("{prefix}{key}"))
}

pub fn is_key_release(data: &str) -> bool {
    !data.contains("\x1b[200~")
        && [":3u", ":3~", ":3A", ":3B", ":3C", ":3D", ":3H", ":3F"]
            .iter()
            .any(|s| data.contains(s))
}

pub fn is_key_repeat(data: &str) -> bool {
    !data.contains("\x1b[200~")
        && [":2u", ":2~", ":2A", ":2B", ":2C", ":2D", ":2H", ":2F"]
            .iter()
            .any(|s| data.contains(s))
}

pub fn matches_key(data: &str, key: &str) -> bool {
    parse_key(data) == Some(key)
}

#[cfg(test)]
mod tests {
    use super::parse_key;

    #[test]
    fn parses_raw_printable_input() {
        assert_eq!(parse_key("a"), Some("a"));
        assert_eq!(parse_key("é"), Some("é"));
        assert_eq!(parse_key(" "), Some("space"));
        assert_eq!(parse_key("\x01"), Some("ctrl+a"));
    }
}
