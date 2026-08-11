use std::{fs, sync::Arc};

use zedflow_agent::harness::{
    env::nodejs::NodeExecutionEnv,
    session::{JsonlSessionStorage, JsonlSessionStorageCreateOptions, Session},
};
use zedflow_coding_agent::session_manager::{
    LabelEntry, SessionInfoEntry, SessionTreeEntry, SessionTreeEntryBase, build_session_tree,
    delete_session, list_session_infos, load_session_info, set_session_name,
};

#[tokio::test]
async fn jsonl_session_appends_entries_and_reopens_them() {
    let file = std::env::temp_dir().join(format!("zedflow-session-{}.jsonl", std::process::id()));
    let cwd = std::env::temp_dir().to_string_lossy().into_owned();
    let storage = JsonlSessionStorage::create(
        Arc::new(NodeExecutionEnv::with_cwd(cwd.clone())),
        file.to_string_lossy().into_owned(),
        JsonlSessionStorageCreateOptions {
            cwd: cwd.clone(),
            session_id: "file-session".into(),
            parent_session_path: None,
        },
    )
    .await
    .unwrap();
    Session::new(storage)
        .append_custom_entry("data", None)
        .await
        .unwrap();

    let reopened = JsonlSessionStorage::open(
        Arc::new(NodeExecutionEnv::with_cwd(cwd)),
        file.to_string_lossy().into_owned(),
    )
    .await
    .unwrap();
    assert_eq!(Session::new(reopened).get_entries().await.len(), 1);
    fs::remove_file(file).unwrap();
}

#[test]
fn selector_metadata_can_be_listed_renamed_and_deleted() {
    let dir = std::env::temp_dir().join(format!("zedflow-session-list-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("session.jsonl");
    fs::write(
        &file,
        concat!(
            r#"{"type":"session","version":3,"id":"id","timestamp":"2025-01-01T00:00:00Z","cwd":"/work"}"#,
            "\n",
            r#"{"type":"message","id":"one","parentId":null,"timestamp":"2025-01-01T00:00:01Z","message":{"role":"user","content":"hello"}}"#,
            "\n"
        ),
    )
    .unwrap();

    let info = load_session_info(&file).unwrap();
    assert_eq!(
        (info.session_id.as_str(), info.first_message.as_str()),
        ("id", "hello")
    );
    let mut progress = Vec::new();
    assert_eq!(
        list_session_infos(
            &dir,
            Some(std::path::Path::new("/work")),
            |loaded, total| progress.push((loaded, total))
        )
        .unwrap()
        .len(),
        1
    );
    assert_eq!(progress, [(1, 1)]);

    set_session_name(&file, " renamed\n session ").unwrap();
    assert_eq!(
        load_session_info(&file).unwrap().name.as_deref(),
        Some("renamed  session")
    );
    delete_session(&file).unwrap();
    assert!(!file.exists());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn selector_tree_resolves_labels_and_children() {
    let base = |id: &str, parent_id: Option<&str>, timestamp: &str| SessionTreeEntryBase {
        id: id.into(),
        parent_id: parent_id.map(str::to_owned),
        timestamp: timestamp.into(),
    };
    let entries = vec![
        SessionTreeEntry::SessionInfo(SessionInfoEntry {
            base: base("root", None, "1"),
            name: None,
        }),
        SessionTreeEntry::SessionInfo(SessionInfoEntry {
            base: base("child", Some("root"), "2"),
            name: None,
        }),
        SessionTreeEntry::Label(LabelEntry {
            base: base("label", Some("child"), "3"),
            target_id: "child".into(),
            label: Some("checkpoint".into()),
        }),
    ];

    let tree = build_session_tree(&entries);
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].children[0].label.as_deref(), Some("checkpoint"));
}
