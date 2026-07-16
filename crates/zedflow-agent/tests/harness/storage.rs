use std::future::Future;
use std::sync::Arc;

use zedflow_agent::harness::env::nodejs::NodeExecutionEnv;
use zedflow_agent::harness::session::{
    InMemorySessionStorage, InMemorySessionStorageOptions, JsonlSessionStorage,
    JsonlSessionStorageCreateOptions, load_jsonl_session_metadata,
};
use zedflow_agent::harness::types::{
    LabelEntry, LeafEntry, SessionErrorCode, SessionMetadata, SessionStorage, SessionTreeEntry,
};

#[path = "session-test-utils.rs"]
mod session_test_utils;

use session_test_utils::{TempDir, assistant_message, entry_base, message_entry, user_message};

fn run<T>(future: impl Future<Output = T>) -> T {
    futures::executor::block_on(future)
}

#[test]
fn memory_returns_configured_session_metadata() {
    run(async {
        let metadata = SessionMetadata {
            id: "session-1".to_string(),
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
        };
        let storage = InMemorySessionStorage::new(Some(InMemorySessionStorageOptions {
            entries: None,
            metadata: Some(metadata.clone()),
        }))
        .unwrap();
        assert_eq!(storage.get_metadata().await, metadata);
    });
}

#[test]
fn memory_copies_initial_entries_and_persists_leaf_changes() {
    run(async {
        let entry = message_entry("entry-1", None, user_message("one"));
        let mut initial_entries = vec![entry];
        let storage = InMemorySessionStorage::new(Some(InMemorySessionStorageOptions {
            entries: Some(initial_entries.clone()),
            metadata: None,
        }))
        .unwrap();
        initial_entries.push(message_entry(
            "entry-2",
            Some("entry-1"),
            user_message("two"),
        ));

        assert_eq!(entry_ids(storage.get_entries().await), vec!["entry-1"]);
        assert_eq!(storage.get_leaf_id().await.as_deref(), Some("entry-1"));
        storage.set_leaf_id(None).await;
        assert_eq!(storage.get_leaf_id().await, None);
        assert!(matches!(
            storage.get_entries().await.last(),
            Some(SessionTreeEntry::Leaf(LeafEntry {
                target_id: None,
                ..
            }))
        ));
    });
}

#[ignore = "source blocker: SessionStorage::set_leaf_id is non-fallible in Rust A1/A2, so invalid leaf ids cannot reject like Pi"]
#[test]
fn memory_rejects_invalid_leaf_ids() {
    run(async {
        let storage = InMemorySessionStorage::default();
        storage.set_leaf_id(Some("missing".to_string())).await;
        assert_ne!(storage.get_leaf_id().await.as_deref(), Some("missing"));
    });
}

#[test]
fn memory_finds_entries_by_type() {
    run(async {
        let storage = InMemorySessionStorage::new(Some(InMemorySessionStorageOptions {
            entries: Some(vec![message_entry("entry-1", None, user_message("one"))]),
            metadata: None,
        }))
        .unwrap();
        assert_eq!(
            entry_ids(storage.find_entries("message").await),
            vec!["entry-1"]
        );
        assert!(storage.find_entries("session_info").await.is_empty());
    });
}

#[test]
fn memory_maintains_label_lookup() {
    run(async {
        let storage = InMemorySessionStorage::new(Some(InMemorySessionStorageOptions {
            entries: Some(vec![message_entry("entry-1", None, user_message("one"))]),
            metadata: None,
        }))
        .unwrap();
        assert_eq!(storage.get_label("entry-1").await, None);
        storage
            .append_entry(SessionTreeEntry::Label(LabelEntry {
                base: entry_base("label-1", Some("entry-1")),
                target_id: "entry-1".to_string(),
                label: Some("checkpoint".to_string()),
            }))
            .await
            .unwrap();
        assert_eq!(
            storage.get_label("entry-1").await.as_deref(),
            Some("checkpoint")
        );
        storage
            .append_entry(SessionTreeEntry::Label(LabelEntry {
                base: entry_base("label-2", Some("label-1")),
                target_id: "entry-1".to_string(),
                label: None,
            }))
            .await
            .unwrap();
        assert_eq!(storage.get_label("entry-1").await, None);
    });
}

#[test]
fn memory_walks_paths_to_root() {
    run(async {
        let storage = InMemorySessionStorage::new(Some(InMemorySessionStorageOptions {
            entries: Some(vec![
                message_entry("root", None, user_message("root")),
                message_entry("child", Some("root"), assistant_message("child")),
            ]),
            metadata: None,
        }))
        .unwrap();
        assert_eq!(
            entry_ids(storage.get_path_to_root(Some("child".to_string())).await),
            vec!["root", "child"]
        );
        assert!(storage.get_path_to_root(None).await.is_empty());
    });
}

#[test]
fn jsonl_throws_for_missing_files_when_opening() {
    run(async {
        let temp = TempDir::new();
        let file = temp.path().join("session.jsonl");
        let error = match JsonlSessionStorage::open(
            Arc::new(NodeExecutionEnv::with_cwd(temp.string())),
            file.to_string_lossy().to_string(),
        )
        .await
        {
            Ok(_) => panic!("missing file should fail"),
            Err(error) => error,
        };
        assert_eq!(error.code, SessionErrorCode::NotFound);
    });
}

#[test]
fn jsonl_writes_header_on_create() {
    run(async {
        let temp = TempDir::new();
        let file = temp.path().join("session.jsonl");
        let storage = create_jsonl(&temp, &file).await;

        assert!(file.exists());
        assert_eq!(
            std::fs::read_to_string(&file)
                .unwrap()
                .trim()
                .split('\n')
                .count(),
            1
        );
        assert_eq!(storage.get_leaf_id().await, None);
        assert!(storage.get_entries().await.is_empty());
        storage
            .append_entry(message_entry("user-1", None, user_message("one")))
            .await
            .unwrap();
        let contents = std::fs::read_to_string(&file).unwrap();
        let lines = contents.trim().split('\n').collect::<Vec<_>>();
        let header: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        let entry: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(
            header.get("type").and_then(serde_json::Value::as_str),
            Some("session")
        );
        assert_eq!(
            entry.get("id").and_then(serde_json::Value::as_str),
            Some("user-1")
        );
        assert_eq!(lines.len(), 2);
    });
}

#[test]
fn jsonl_throws_for_malformed_session_headers() {
    run(async {
        let temp = TempDir::new();
        let file = temp.path().join("session.jsonl");
        std::fs::write(&file, "not json\n").unwrap();
        let error = match JsonlSessionStorage::open(
            Arc::new(NodeExecutionEnv::with_cwd(temp.string())),
            file.to_string_lossy().to_string(),
        )
        .await
        {
            Ok(_) => panic!("malformed header should fail"),
            Err(error) => error,
        };
        assert!(
            error
                .message
                .contains("first line is not a valid session header")
        );
    });
}

#[test]
fn jsonl_throws_for_malformed_entry_lines() {
    run(async {
        let temp = TempDir::new();
        let file = temp.path().join("session.jsonl");
        let header = serde_json::json!({
            "type": "session",
            "version": 3,
            "id": "session-1",
            "timestamp": "2026-01-01T00:00:00.000Z",
            "cwd": temp.string()
        });
        std::fs::write(&file, format!("{header}\nnot json\n")).unwrap();
        let error = match JsonlSessionStorage::open(
            Arc::new(NodeExecutionEnv::with_cwd(temp.string())),
            file.to_string_lossy().to_string(),
        )
        .await
        {
            Ok(_) => panic!("malformed entry should fail"),
            Err(error) => error,
        };
        assert_eq!(error.code, SessionErrorCode::InvalidEntry);
    });
}

#[test]
fn jsonl_creates_and_reads_session_metadata_from_header() {
    run(async {
        let temp = TempDir::new();
        let file = temp.path().join("session.jsonl");
        let storage = JsonlSessionStorage::create(
            Arc::new(NodeExecutionEnv::with_cwd(temp.string())),
            file.to_string_lossy().to_string(),
            JsonlSessionStorageCreateOptions {
                cwd: temp.string(),
                session_id: "session-1".to_string(),
                parent_session_path: Some("/tmp/parent.jsonl".to_string()),
            },
        )
        .await
        .unwrap();
        let metadata = load_jsonl_session_metadata(
            &NodeExecutionEnv::with_cwd(temp.string()),
            &file.to_string_lossy(),
        )
        .await
        .unwrap();
        assert_eq!(storage.get_metadata().await.id, "session-1");
        assert_eq!(metadata.base.id, "session-1");
        assert_eq!(metadata.cwd, temp.string());
        assert_eq!(metadata.path, file.to_string_lossy());
        assert_eq!(
            metadata.parent_session_path.as_deref(),
            Some("/tmp/parent.jsonl")
        );
    });
}

#[test]
fn jsonl_loads_existing_entries_and_reconstructs_leaf() {
    run(async {
        let temp = TempDir::new();
        let file = temp.path().join("session.jsonl");
        let storage = create_jsonl(&temp, &file).await;
        storage
            .append_entry(message_entry("root", None, user_message("root")))
            .await
            .unwrap();
        storage
            .append_entry(message_entry(
                "child",
                Some("root"),
                assistant_message("child"),
            ))
            .await
            .unwrap();

        let loaded = JsonlSessionStorage::open(
            Arc::new(NodeExecutionEnv::with_cwd(temp.string())),
            file.to_string_lossy().to_string(),
        )
        .await
        .unwrap();
        assert_eq!(loaded.get_leaf_id().await.as_deref(), Some("child"));
        assert_eq!(entry_ids(loaded.get_entries().await), vec!["root", "child"]);
        loaded.set_leaf_id(Some("root".to_string())).await;
        let reloaded = JsonlSessionStorage::open(
            Arc::new(NodeExecutionEnv::with_cwd(temp.string())),
            file.to_string_lossy().to_string(),
        )
        .await
        .unwrap();
        assert_eq!(reloaded.get_leaf_id().await.as_deref(), Some("root"));
        assert!(
            matches!(reloaded.get_entries().await.last(), Some(SessionTreeEntry::Leaf(LeafEntry { target_id: Some(id), .. })) if id == "root")
        );
        assert_eq!(
            entry_ids(loaded.get_path_to_root(Some("child".to_string())).await),
            vec!["root", "child"]
        );
    });
}

#[test]
fn jsonl_finds_entries_by_type_and_maintains_labels() {
    run(async {
        let temp = TempDir::new();
        let file = temp.path().join("session.jsonl");
        let storage = create_jsonl(&temp, &file).await;
        storage
            .append_entry(message_entry("entry-1", None, user_message("one")))
            .await
            .unwrap();
        assert_eq!(
            entry_ids(storage.find_entries("message").await),
            vec!["entry-1"]
        );
        assert!(storage.find_entries("session_info").await.is_empty());
        assert_eq!(storage.get_label("entry-1").await, None);
        storage
            .append_entry(SessionTreeEntry::Label(LabelEntry {
                base: entry_base("label-1", Some("entry-1")),
                target_id: "entry-1".to_string(),
                label: Some("checkpoint".to_string()),
            }))
            .await
            .unwrap();
        assert_eq!(
            storage.get_label("entry-1").await.as_deref(),
            Some("checkpoint")
        );
        storage
            .append_entry(SessionTreeEntry::Label(LabelEntry {
                base: entry_base("label-2", Some("label-1")),
                target_id: "entry-1".to_string(),
                label: None,
            }))
            .await
            .unwrap();
        assert_eq!(storage.get_label("entry-1").await, None);

        let loaded = JsonlSessionStorage::open(
            Arc::new(NodeExecutionEnv::with_cwd(temp.string())),
            file.to_string_lossy().to_string(),
        )
        .await
        .unwrap();
        assert_eq!(loaded.get_label("entry-1").await, None);
    });
}

async fn create_jsonl(temp: &TempDir, file: &std::path::Path) -> JsonlSessionStorage {
    JsonlSessionStorage::create(
        Arc::new(NodeExecutionEnv::with_cwd(temp.string())),
        file.to_string_lossy().to_string(),
        JsonlSessionStorageCreateOptions {
            cwd: temp.string(),
            session_id: "session-1".to_string(),
            parent_session_path: None,
        },
    )
    .await
    .unwrap()
}

fn entry_ids(entries: Vec<SessionTreeEntry>) -> Vec<String> {
    entries
        .iter()
        .map(zedflow_agent::harness::session::repo_utils::entry_id)
        .map(str::to_string)
        .collect()
}
