use zedflow_tui::matches_key;
#[test]
fn keybindings_match_decoded_input() {
    assert!(matches_key("\x1b[B", "down"));
    assert!(!matches_key("x", "down"));
}
