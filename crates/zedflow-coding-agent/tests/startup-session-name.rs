use zedflow_coding_agent::session_manager::SessionInfo;
#[test]
fn session_name_state_is_independent_of_persistence() {
    let session = SessionInfo::persisted("/work", "named.jsonl", "name-id");
    assert_eq!(session.session_id, "name-id");
    assert!(session.is_persisted());
}
