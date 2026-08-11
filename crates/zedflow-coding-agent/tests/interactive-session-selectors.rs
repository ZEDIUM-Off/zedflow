use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use zedflow_coding_agent::{
    session_manager::load_session_info,
    session_selector::{SessionSelectorAction, SessionSelectorState},
    session_selector_search::{NameFilter, SessionInfo, SortMode, filter_and_sort_sessions},
    tree_selector::{FilterMode, TreeEntryKind, TreeItem, TreeSelectorAction, TreeSelectorState},
    trust_manager::ProjectTrustStoreEntry,
    trust_selector::{TrustSelectorState, format_saved_decision},
    user_message_selector::{UserMessageItem, UserMessageSelectorAction, UserMessageSelectorState},
};

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("zedflow-p4-t3-{}-{name}", std::process::id()))
}

fn write_session(path: &PathBuf, id: &str) {
    fs::write(path, format!(
        "{{\"type\":\"session\",\"id\":\"{id}\",\"timestamp\":\"2026-01-01T00:00:00Z\",\"cwd\":\"/project\"}}\n{{\"type\":\"message\",\"id\":\"m1\",\"parentId\":null,\"timestamp\":\"2026-01-01T00:00:01Z\",\"message\":{{\"role\":\"user\",\"content\":\"hello\",\"timestamp\":1}}}}\n"
    )).unwrap();
}

fn search_session(id: &str, name: Option<&str>, text: &str, modified: u64) -> SessionInfo {
    SessionInfo {
        id: id.into(),
        name: name.map(str::to_owned),
        all_messages_text: text.into(),
        cwd: "/project".into(),
        modified: UNIX_EPOCH + Duration::from_secs(modified),
    }
}

#[test]
fn session_search_sort_and_safe_file_actions_match_pi() {
    let sessions = vec![
        search_session("late", Some("Named"), "xxxx node\n cve", 3),
        search_session("early", None, "node cve", 1),
    ];
    assert_eq!(
        filter_and_sort_sessions(
            &sessions,
            "\"node cve\"",
            SortMode::Relevance,
            NameFilter::All
        )
        .iter()
        .map(|session| session.id.as_str())
        .collect::<Vec<_>>(),
        ["early", "late"]
    );
    assert_eq!(
        filter_and_sort_sessions(&sessions, "node", SortMode::Recent, NameFilter::Named).len(),
        1
    );
    assert!(
        filter_and_sort_sessions(&sessions, "re:[", SortMode::Recent, NameFilter::All).is_empty()
    );

    let rename_path = temp_path("rename.jsonl");
    let delete_path = temp_path("delete.jsonl");
    write_session(&rename_path, "rename");
    write_session(&delete_path, "delete");
    let mut rename = load_session_info(&rename_path).unwrap();
    let mut delete = load_session_info(&delete_path).unwrap();
    rename.modified = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(2));
    delete.modified = Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1));
    let mut state = SessionSelectorState::new(vec![rename, delete], None);

    state.rename_selected(" Renamed ").unwrap();
    assert_eq!(
        load_session_info(&rename_path).unwrap().name.as_deref(),
        Some("Renamed")
    );
    state.move_down();
    assert_eq!(
        state.request_delete(),
        SessionSelectorAction::ConfirmDelete(delete_path.clone())
    );
    state.cancel_delete();
    assert!(delete_path.exists());
    assert_eq!(
        state.request_delete(),
        SessionSelectorAction::ConfirmDelete(delete_path.clone())
    );
    assert_eq!(
        state.confirm_delete().unwrap(),
        SessionSelectorAction::Deleted(delete_path.clone())
    );
    assert!(!delete_path.exists());
    fs::remove_file(rename_path).unwrap();
}

fn item(
    id: &str,
    parent: Option<&str>,
    text: &str,
    kind: TreeEntryKind,
    children: Vec<TreeItem>,
) -> TreeItem {
    TreeItem {
        id: id.into(),
        parent_id: parent.map(str::to_owned),
        text: text.into(),
        label: None,
        kind,
        children,
    }
}

#[test]
fn tree_navigation_filters_searches_selects_and_cancels() {
    let roots = vec![item(
        "root",
        None,
        "start",
        TreeEntryKind::UserMessage,
        vec![
            item(
                "tool",
                Some("root"),
                "read file",
                TreeEntryKind::ToolResult,
                vec![],
            ),
            item(
                "leaf",
                Some("root"),
                "final answer",
                TreeEntryKind::AssistantMessage,
                vec![],
            ),
        ],
    )];
    let mut tree = TreeSelectorState::new(roots, Some("leaf".into()), None);
    assert_eq!(tree.select(), TreeSelectorAction::Select("leaf".into()));
    tree.set_filter_mode(FilterMode::NoTools);
    assert_eq!(
        tree.visible_items()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["root", "leaf"]
    );
    tree.set_query("start");
    assert_eq!(tree.select(), TreeSelectorAction::Select("root".into()));
    assert_eq!(tree.cancel_search_or_picker(), TreeSelectorAction::None);
    assert_eq!(tree.cancel_search_or_picker(), TreeSelectorAction::Cancel);
}

#[test]
fn trust_and_user_message_choices_preserve_selection_semantics() {
    let cwd = temp_path("trust-parent").join("project");
    fs::create_dir_all(&cwd).unwrap();
    let saved = ProjectTrustStoreEntry {
        path: cwd.clone(),
        decision: true,
    };
    let mut trust = TrustSelectorState::new(&cwd, Some(saved.clone())).unwrap();
    assert!(trust.is_saved_option(0));
    assert!(trust.select().unwrap().trusted);
    trust.move_down();
    assert!(trust.select().unwrap().trusted);
    assert_eq!(
        format_saved_decision(Some(&cwd), Some(&saved)),
        format!("trusted ({})", cwd.display())
    );

    let mut messages = UserMessageSelectorState::new(
        vec![
            UserMessageItem {
                id: "one".into(),
                text: "first\nmessage".into(),
                timestamp: None,
            },
            UserMessageItem {
                id: "two".into(),
                text: "second".into(),
                timestamp: None,
            },
        ],
        None,
    );
    assert_eq!(
        messages.select(),
        UserMessageSelectorAction::Select("two".into())
    );
    messages.move_down();
    assert_eq!(
        messages.select(),
        UserMessageSelectorAction::Select("one".into())
    );
    assert_eq!(
        UserMessageSelectorState::normalized_text(&messages.messages()[0]),
        "first message"
    );
    assert_eq!(messages.cancel(), UserMessageSelectorAction::Cancel);

    fs::remove_dir_all(cwd.parent().unwrap()).unwrap();
}
