use zedflow_tui::parse_key;

#[test]
fn decodes_kitty_unicode_base_layout_and_shifted_keys() {
    assert_eq!(parse_key("\x1b[1089::99;5u"), Some("ctrl+c"));
    assert_eq!(parse_key("\x1b[69;2u"), Some("shift+e"));
    assert_eq!(parse_key("\x1b[1089:1057:99;6:2u"), Some("shift+ctrl+c"));
}

#[test]
fn decodes_raw_and_modified_backspace() {
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
fn keeps_modified_kitty_special_keys_reachable() {
    assert_eq!(parse_key("\r"), Some("enter"));
    assert_eq!(parse_key("\t"), Some("tab"));
    assert_eq!(parse_key("\x1b[13;3u"), Some("alt+enter"));
    assert_eq!(parse_key("\x1b[9;5u"), Some("ctrl+tab"));
}
