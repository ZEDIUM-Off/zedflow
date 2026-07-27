use zedflow_tui::parse_key;
#[test]
fn input_decodes_common_keys() {
    assert_eq!(parse_key("a"), Some("a"));
    assert_eq!(parse_key("\x1b[A"), Some("up"));
    assert_eq!(parse_key("\r"), Some("enter"));
}
