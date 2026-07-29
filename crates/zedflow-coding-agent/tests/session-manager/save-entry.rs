use crate::session_manager::SessionInfo;
#[test]
fn persisted_session_records_file_while_memory_session_does_not() {
    let memory = SessionInfo::in_memory("/work", "id");
    let saved = SessionInfo::persisted("/work", "session.jsonl", "id");
    assert!(!memory.is_persisted());
    assert!(saved.is_persisted());
    assert_eq!(saved.session_file.as_deref(), Some("session.jsonl"));
}
