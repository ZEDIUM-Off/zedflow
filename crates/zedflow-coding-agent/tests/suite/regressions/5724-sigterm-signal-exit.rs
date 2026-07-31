use zedflow_coding_agent::session_manager::SessionInfo;

#[test]
fn persisted_session_metadata_retains_its_identity() {
    let session = SessionInfo::persisted("/work", "/work/session.jsonl", "session-id");
    assert!(session.is_persisted());
    assert_eq!(session.session_id, "session-id");
}
