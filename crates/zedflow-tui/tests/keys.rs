use zedflow_tui::parse_key;

#[test]
fn decodes_kitty_unicode_base_layout_and_shifted_keys() {
    assert_eq!(parse_key("\x1b[1089::99;5u"), Some("ctrl+c"));
    assert_eq!(parse_key("\x1b[69;2u"), Some("shift+e"));
    assert_eq!(parse_key("\x1b[1089:1057:99;6:2u"), Some("shift+ctrl+c"));
}
