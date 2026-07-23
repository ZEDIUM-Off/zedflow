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
    Some(match data {
        "\u{1b}" => "escape",
        "\r" => "enter",
        "\n" => "enter",
        "\t" => "tab",
        " " => "space",
        "\u{7f}" => "backspace",
        "\u{1b}[A" => "up",
        "\u{1b}[B" => "down",
        "\u{1b}[C" => "right",
        "\u{1b}[D" => "left",
        "\u{1b}[H" => "home",
        "\u{1b}[F" => "end",
        "\u{1b}[2~" => "insert",
        "\u{1b}[3~" => "delete",
        "\u{1b}[5~" => "pageUp",
        "\u{1b}[6~" => "pageDown",
        "\u{1b}OP" => "f1",
        "\u{1b}OQ" => "f2",
        "\u{1b}OR" => "f3",
        "\u{1b}OS" => "f4",
        _ => return None,
    })
}

pub fn matches_key(data: &str, key: &str) -> bool {
    parse_key(data) == Some(key)
}
