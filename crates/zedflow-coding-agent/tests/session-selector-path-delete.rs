use std::time::SystemTime;
use zedflow_coding_agent::session_selector_search::{
    SessionInfo, match_session, parse_search_query,
};
#[test]
fn selector_search_matches_name_and_rejects_other_terms() {
    let session = SessionInfo {
        id: "id".into(),
        name: Some("release work".into()),
        all_messages_text: "hello".into(),
        cwd: "/tmp".into(),
        modified: SystemTime::now(),
    };
    assert!(match_session(&session, &parse_search_query("release")).matches);
    assert!(!match_session(&session, &parse_search_query("absent")).matches);
}
