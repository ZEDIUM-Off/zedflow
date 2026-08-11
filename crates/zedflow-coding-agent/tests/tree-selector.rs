#[test]
fn tree_picker_delete_requires_the_delete_shortcut() {
    use zedflow_coding_agent::session_selector::{SessionSelectorKey, should_confirm_delete};
    assert!(should_confirm_delete(SessionSelectorKey::CtrlD, "search"));
    assert!(!should_confirm_delete(SessionSelectorKey::Other, ""));
}
