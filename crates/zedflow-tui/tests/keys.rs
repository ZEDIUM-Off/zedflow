use zedflow_tui::parse_key;

#[test]
fn decodes_kitty_unicode_base_layout_and_shifted_keys() {
    assert_eq!(parse_key("\x1b[1089::99;5u"), Some("ctrl+c"));
    assert_eq!(parse_key("\x1b[69;2u"), Some("shift+e"));
    assert_eq!(parse_key("\x1b[1089:1057:99;6:2u"), Some("shift+ctrl+c"));
}

#[test]
fn keeps_raw_and_modified_kitty_special_keys_reachable() {
    assert_eq!(parse_key("\r"), Some("enter"));
    assert_eq!(parse_key("\t"), Some("tab"));
    assert_eq!(parse_key("\x7f"), Some("backspace"));
    assert_eq!(parse_key("\x1b[13;3u"), Some("alt+enter"));
    assert_eq!(parse_key("\x1b[9;5u"), Some("ctrl+tab"));
    assert_eq!(parse_key("\x1b[127;6u"), Some("shift+ctrl+backspace"));
}
