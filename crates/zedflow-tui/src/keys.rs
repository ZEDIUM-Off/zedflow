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
    Some(match data {
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
        _ => return None,
    })
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
