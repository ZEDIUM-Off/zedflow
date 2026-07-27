//! Terminal key decoding compatible with Pi's legacy, Kitty CSI-u, and xterm modes.

use std::sync::atomic::{AtomicBool, Ordering};

static KITTY_PROTOCOL_ACTIVE: AtomicBool = AtomicBool::new(false);
const SHIFT: u32 = 1;
const ALT: u32 = 2;
const CTRL: u32 = 4;
const SUPER: u32 = 8;
const LOCKS: u32 = 64 | 128;

pub fn set_kitty_protocol_active(active: bool) {
    KITTY_PROTOCOL_ACTIVE.store(active, Ordering::Relaxed);
}
pub fn is_kitty_protocol_active() -> bool {
    KITTY_PROTOCOL_ACTIVE.load(Ordering::Relaxed)
}

#[derive(Debug)]
struct ParsedSequence {
    code: i32,
    shifted: Option<u32>,
    base: Option<u32>,
    modifiers: u32,
}

fn parse_u32(value: Option<&str>, default: u32) -> Option<u32> {
    match value {
        Some("") | None => Some(default),
        Some(value) => value.parse().ok(),
    }
}

fn parse_sequence(data: &str) -> Option<ParsedSequence> {
    let body = data.strip_prefix("\x1b[")?;
    if let Some(body) = body.strip_suffix('u') {
        let (keys, modifier_event) = body.split_once(';').unwrap_or((body, "1"));
        let mut key_parts = keys.split(':');
        let code = key_parts.next()?.parse::<u32>().ok()?;
        let shifted = key_parts
            .next()
            .filter(|v| !v.is_empty())
            .and_then(|v| v.parse().ok());
        let base = key_parts
            .next()
            .filter(|v| !v.is_empty())
            .and_then(|v| v.parse().ok());
        let modifiers = parse_u32(modifier_event.split(':').next(), 1)?.checked_sub(1)?;
        return Some(ParsedSequence {
            code: normalize_functional(code),
            shifted,
            base,
            modifiers,
        });
    }
    if let Some(body) = body.strip_suffix('~') {
        let parts: Vec<_> = body.split([';', ':']).collect();
        if parts.first() == Some(&"27") && parts.len() >= 3 {
            return Some(ParsedSequence {
                code: parts[2].parse::<u32>().ok()? as i32,
                shifted: None,
                base: None,
                modifiers: parts[1].parse::<u32>().ok()?.checked_sub(1)?,
            });
        }
        let code = match parts.first()?.parse::<u32>().ok()? {
            2 => -11,
            3 => -10,
            5 => -12,
            6 => -13,
            7 => -14,
            8 => -15,
            _ => return None,
        };
        return Some(ParsedSequence {
            code,
            shifted: None,
            base: None,
            modifiers: parse_u32(parts.get(1).copied(), 1)?.checked_sub(1)?,
        });
    }
    let suffix = body.chars().last()?;
    if matches!(suffix, 'A' | 'B' | 'C' | 'D' | 'H' | 'F') {
        let params = body[..body.len() - 1].strip_prefix("1;")?;
        return Some(ParsedSequence {
            code: match suffix {
                'A' => -1,
                'B' => -2,
                'C' => -3,
                'D' => -4,
                'H' => -14,
                'F' => -15,
                _ => unreachable!(),
            },
            shifted: None,
            base: None,
            modifiers: parse_u32(params.split(':').next(), 1)?.checked_sub(1)?,
        });
    }
    None
}

fn normalize_functional(code: u32) -> i32 {
    match code {
        57399..=57408 => (code - 57399 + 48) as i32,
        57409 => 46,
        57410 => 47,
        57411 => 42,
        57412 => 45,
        57413 => 43,
        57414 => 13,
        57415 => 61,
        57416 => 44,
        57417 => -4,
        57418 => -3,
        57419 => -1,
        57420 => -2,
        57421 => -12,
        57422 => -13,
        57423 => -14,
        57424 => -15,
        57425 => -11,
        57426 => -10,
        _ => code as i32,
    }
}

fn supported_symbol(c: char) -> bool {
    "`-=[]\\;',./!@#$%^&*()_+|~{}:<>?".contains(c)
}

fn format_sequence(mut sequence: ParsedSequence) -> Option<String> {
    if sequence.modifiers & !(SHIFT | ALT | CTRL | SUPER | LOCKS) != 0 {
        return None;
    }
    let modifiers = sequence.modifiers & !LOCKS;
    if modifiers & SHIFT != 0 && (65..=90).contains(&sequence.code) {
        sequence.code += 32;
    }
    let recognized = matches!(sequence.code, 9 | 13 | 27 | 32 | 127 | -15..=-10 | -4..=-1)
        || u32::try_from(sequence.code)
            .ok()
            .and_then(char::from_u32)
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || supported_symbol(c));
    if !recognized {
        sequence.code = sequence.base? as i32;
    }
    let key = match sequence.code {
        27 => "escape".into(),
        9 => "tab".into(),
        13 => "enter".into(),
        32 => "space".into(),
        127 => "backspace".into(),
        -1 => "up".into(),
        -2 => "down".into(),
        -3 => "right".into(),
        -4 => "left".into(),
        -10 => "delete".into(),
        -11 => "insert".into(),
        -12 => "pageUp".into(),
        -13 => "pageDown".into(),
        -14 => "home".into(),
        -15 => "end".into(),
        code => char::from_u32(code.try_into().ok()?)?.to_string(),
    };
    let mut prefix = String::new();
    if modifiers & SHIFT != 0 {
        prefix.push_str("shift+");
    }
    if modifiers & CTRL != 0 {
        prefix.push_str("ctrl+");
    }
    if modifiers & ALT != 0 {
        prefix.push_str("alt+");
    }
    if modifiers & SUPER != 0 {
        prefix.push_str("super+");
    }
    Some(prefix + &key)
}

fn parse_key_owned(data: &str) -> Option<String> {
    if let Some(sequence) = parse_sequence(data) {
        return format_sequence(sequence);
    }
    if is_kitty_protocol_active() {
        if data == "\x1b\r" || data == "\n" {
            return Some("shift+enter".into());
        }
    }
    let key = match data {
        "\x1b" => "escape",
        "\x1c" => "ctrl+\\",
        "\x1d" => "ctrl+]",
        "\x1f" => "ctrl+-",
        "\x1b\x1b" => "ctrl+alt+[",
        "\x1b\x1c" => "ctrl+alt+\\",
        "\x1b\x1d" => "ctrl+alt+]",
        "\x1b\x1f" => "ctrl+alt+-",
        "\t" => "tab",
        "\r" | "\x1bOM" => "enter",
        "\n" if !is_kitty_protocol_active() => "enter",
        "\x00" => "ctrl+space",
        " " => "space",
        "\x7f" => "backspace",
        "\x08" => {
            if is_local_windows_terminal() {
                "ctrl+backspace"
            } else {
                "backspace"
            }
        }
        "\x1b[Z" => "shift+tab",
        "\x1b\r" if !is_kitty_protocol_active() => "alt+enter",
        "\x1b " if !is_kitty_protocol_active() => "alt+space",
        "\x1b\x7f" | "\x1b\x08" => "alt+backspace",
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
        "\x1b[15~" | "\x1b[[E" => "f5",
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
        "\x1bb" | "\x1bB" if !is_kitty_protocol_active() => "alt+left",
        "\x1bf" | "\x1bF" if !is_kitty_protocol_active() => "alt+right",
        "\x1bp" => "alt+up",
        "\x1bn" => "alt+down",
        _ => {
            let bytes = data.as_bytes();
            if bytes.len() == 1 && (1..=26).contains(&bytes[0]) {
                return Some(format!("ctrl+{}", (b'a' + bytes[0] - 1) as char));
            }
            if let Some(rest) = data.strip_prefix('\x1b') {
                let b = rest.as_bytes();
                if b.len() == 1 && (1..=26).contains(&b[0]) {
                    return Some(format!("ctrl+alt+{}", (b'a' + b[0] - 1) as char));
                }
                if !is_kitty_protocol_active()
                    && b.len() == 1
                    && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
                {
                    return Some(format!("alt+{}", b[0] as char));
                }
            }
            if data.chars().count() == 1 && data.chars().next().is_some_and(|c| !c.is_control()) {
                return Some(data.into());
            }
            return None;
        }
    };
    Some(key.into())
}

pub fn parse_key(data: &str) -> Option<&'static str> {
    parse_key_owned(data).map(|key| Box::leak(key.into_boxed_str()) as &'static str)
}

fn normalized_id(id: &str) -> Option<(u32, String)> {
    let mut modifiers = 0;
    let mut key = None;
    for part in id.split('+') {
        match part.to_ascii_lowercase().as_str() {
            "shift" => modifiers |= SHIFT,
            "ctrl" => modifiers |= CTRL,
            "alt" => modifiers |= ALT,
            "super" => modifiers |= SUPER,
            value => {
                key = Some(
                    match value {
                        "esc" => "escape",
                        "return" => "enter",
                        other => other,
                    }
                    .to_owned(),
                )
            }
        }
    }
    Some((modifiers, key?))
}

pub fn matches_key(data: &str, key: &str) -> bool {
    let Some(expected) = normalized_id(key) else {
        return false;
    };
    if expected.0 == CTRL && expected.1.len() == 1 {
        let c = expected.1.as_bytes()[0];
        let raw = match c {
            b'a'..=b'z' => c & 0x1f,
            b'[' => 27,
            b'\\' => 28,
            b']' => 29,
            b'-' | b'_' => 31,
            _ => 255,
        };
        if data.as_bytes() == [raw] {
            return true;
        }
    }
    if parse_key(data).and_then(|id| normalized_id(&id)).as_ref() == Some(&expected) {
        return true;
    }
    if expected.0 == SHIFT && expected.1.len() == 1 {
        return data == expected.1.to_ascii_uppercase();
    }
    false
}

pub fn decode_kitty_printable(data: &str) -> Option<String> {
    let sequence = parse_sequence(data)?;
    let modifiers = sequence.modifiers & !LOCKS;
    if modifiers & !SHIFT != 0 || sequence.code < 32 {
        return None;
    }
    let code = if modifiers & SHIFT != 0 {
        sequence.shifted.unwrap_or(sequence.code as u32)
    } else {
        sequence.code as u32
    };
    char::from_u32(code).map(|c| c.to_string())
}

pub fn decode_printable_key(data: &str) -> Option<String> {
    decode_kitty_printable(data)
}

fn is_local_windows_terminal() -> bool {
    std::env::var("WT_SESSION").is_ok_and(|value| !value.is_empty())
        && ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"]
            .iter()
            .all(|name| std::env::var(name).map_or(true, |value| value.is_empty()))
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
