use zedflow_tui::{is_key_release, is_key_repeat};
#[test]
fn key_tester_classifies_key_events() {
    assert!(is_key_release("\x1b[13:3u"));
    assert!(is_key_repeat("\x1b[13:2u"));
}
