use std::sync::Mutex;

use zedflow_tui::parse_key;

static TERMINAL_ENV_LOCK: Mutex<()> = Mutex::new(());

fn with_terminal_env(vars: &[(&str, Option<&str>)], test: impl FnOnce()) {
    let saved: Vec<_> = vars
        .iter()
        .map(|(name, _)| (*name, std::env::var_os(name)))
        .collect();
    for (name, value) in vars {
        unsafe {
            if let Some(value) = value {
                std::env::set_var(name, value);
            } else {
                std::env::remove_var(name);
            }
        }
    }
    test();
    for (name, value) in saved {
        unsafe {
            if let Some(value) = value {
                std::env::set_var(name, value);
            } else {
                std::env::remove_var(name);
            }
        }
    }
}

#[test]
fn disambiguates_raw_backspace_for_local_windows_terminal_only() {
    let _terminal_env_lock = TERMINAL_ENV_LOCK.lock().unwrap();
    with_terminal_env(
        &[
            ("WT_SESSION", Some("test-session")),
            ("SSH_CONNECTION", None),
            ("SSH_CLIENT", None),
            ("SSH_TTY", None),
        ],
        || assert_eq!(parse_key("\x08"), Some("ctrl+backspace")),
    );
    with_terminal_env(
        &[
            ("WT_SESSION", Some("test-session")),
            ("SSH_CONNECTION", Some("127.0.0.1 22 22 127.0.0.1")),
            ("SSH_CLIENT", None),
            ("SSH_TTY", None),
        ],
        || assert_eq!(parse_key("\x08"), Some("backspace")),
    );
}

#[test]
fn decodes_kitty_unicode_base_layout_and_shifted_keys() {
    assert_eq!(parse_key("\x1b[1089::99;5u"), Some("ctrl+c"));
    assert_eq!(parse_key("\x1b[69;2u"), Some("shift+e"));
    assert_eq!(parse_key("\x1b[1089:1057:99;6:2u"), Some("shift+ctrl+c"));
}

#[test]
fn decodes_raw_ctrl_space() {
    assert_eq!(parse_key("\x00"), Some("ctrl+space"));
}

#[test]
fn decodes_raw_and_modified_backspace() {
    let _terminal_env_lock = TERMINAL_ENV_LOCK.lock().unwrap();
    assert_eq!(parse_key("\x08"), Some("backspace"));
    assert_eq!(parse_key("\x7f"), Some("backspace"));
    assert_eq!(parse_key("\x1b[127;6u"), Some("shift+ctrl+backspace"));
}

#[test]
fn decodes_kitty_keypad_and_functional_codes() {
    assert_eq!(parse_key("\x1b[57399u"), Some("0"));
    assert_eq!(parse_key("\x1b[57410;5u"), Some("ctrl+/"));
    assert_eq!(parse_key("\x1b[57414;3u"), Some("alt+enter"));
    assert_eq!(parse_key("\x1b[57417u"), Some("left"));
    assert_eq!(parse_key("\x1b[57426;2u"), Some("shift+delete"));
}

#[test]
fn formats_kitty_keys_for_escape_space_and_super() {
    assert_eq!(parse_key("\x1b[27u"), Some("escape"));
    assert_eq!(parse_key("\x1b[32;9u"), Some("super+space"));
    assert_eq!(parse_key("\x1b[27;13u"), Some("ctrl+super+escape"));
}

#[test]
fn keeps_modified_kitty_special_keys_reachable() {
    assert_eq!(parse_key("\r"), Some("enter"));
    assert_eq!(parse_key("\t"), Some("tab"));
    assert_eq!(parse_key("\x1b[13;3u"), Some("alt+enter"));
    assert_eq!(parse_key("\x1b[9;5u"), Some("ctrl+tab"));
}
