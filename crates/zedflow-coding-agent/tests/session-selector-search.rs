use std::time::{Duration, UNIX_EPOCH};

use zedflow_coding_agent::session_selector_search::{
    NameFilter, SessionInfo, SortMode, filter_and_sort_sessions,
};

fn session(id: &str, name: Option<&str>, text: &str, modified: u64) -> SessionInfo {
    SessionInfo {
        id: id.into(),
        name: name.map(str::to_owned),
        all_messages_text: text.into(),
        cwd: "/tmp".into(),
        modified: UNIX_EPOCH + Duration::from_secs(modified),
    }
}

#[test]
fn selector_search_handles_phrases_regex_invalid_patterns_and_name_filtering() {
    let sessions = vec![
        session("a", Some("Release"), "node\n cve", 1),
        session("b", None, "node something else", 2),
        session("c", Some("   "), "node cve", 3),
    ];

    assert_eq!(
        filter_and_sort_sessions(&sessions, "\"node cve\"", SortMode::Recent, NameFilter::All)
            .iter()
            .map(|session| session.id.as_str())
            .collect::<Vec<_>>(),
        ["a", "c"]
    );
    assert_eq!(
        filter_and_sort_sessions(&sessions, "re:^A", SortMode::Recent, NameFilter::All).len(),
        1
    );
    assert!(
        filter_and_sort_sessions(&sessions, "re:[", SortMode::Recent, NameFilter::All).is_empty()
    );
    assert_eq!(
        filter_and_sort_sessions(&sessions, "node", SortMode::Recent, NameFilter::Named).len(),
        1
    );
}
