use zedflow_coding_agent::session_selector::{SessionSelectorKey, should_confirm_delete};

#[test]
fn delete_shortcuts_distinguish_search_editing_from_deletion() {
    assert!(!should_confirm_delete(
        SessionSelectorKey::CtrlBackspace,
        "query"
    ));
    assert!(should_confirm_delete(SessionSelectorKey::CtrlD, "query"));
    assert!(should_confirm_delete(SessionSelectorKey::CtrlBackspace, ""));
    assert!(!should_confirm_delete(SessionSelectorKey::Other, ""));
}
