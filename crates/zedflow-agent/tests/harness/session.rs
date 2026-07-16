use std::future::Future;
use std::sync::Arc;

use serde_json::json;
use zedflow_agent::harness::env::nodejs::NodeExecutionEnv;
use zedflow_agent::harness::session::{
    InMemorySessionStorage, JsonlSessionStorage, JsonlSessionStorageCreateOptions, Session,
};
use zedflow_agent::harness::types::{BranchSummaryDraft, CustomMessageContent};

#[path = "session-test-utils.rs"]
mod session_test_utils;

use session_test_utils::{TempDir, assistant_message, message_role, message_roles, user_message};

fn run<T>(future: impl Future<Output = T>) -> T {
    futures::executor::block_on(future)
}

#[test]
fn in_memory_appends_messages_and_builds_context_in_order() {
    run(async {
        let session = Session::new(InMemorySessionStorage::default());
        session.append_message(user_message("one")).await.unwrap();
        session
            .append_message(assistant_message("two"))
            .await
            .unwrap();

        let context = session.build_context().await;
        assert_eq!(message_roles(&context.messages), vec!["user", "assistant"]);
    });
}

#[test]
fn jsonl_appends_messages_and_builds_context_in_order() {
    run(async {
        let temp = TempDir::new();
        let file = temp.path().join("session.jsonl");
        let storage = jsonl_storage(&temp, &file).await;
        let session = Session::new(storage);
        session.append_message(user_message("one")).await.unwrap();
        session
            .append_message(assistant_message("two"))
            .await
            .unwrap();

        let context = session.build_context().await;
        assert_eq!(message_roles(&context.messages), vec!["user", "assistant"]);
    });
}

#[test]
fn tracks_model_and_thinking_level_changes() {
    run(async {
        let session = Session::new(InMemorySessionStorage::default());
        session.append_message(user_message("one")).await.unwrap();
        session
            .append_model_change("openai", "gpt-4.1")
            .await
            .unwrap();
        session.append_thinking_level_change("high").await.unwrap();

        let context = session.build_context().await;
        assert_eq!(context.thinking_level, "high");
        let model = context.model.unwrap();
        assert_eq!(model.provider, "openai");
        assert_eq!(model.model_id, "gpt-4.1");
    });
}

#[test]
fn supports_branching_by_moving_leaf_and_appending_new_branch() {
    run(async {
        let session = Session::new(InMemorySessionStorage::default());
        let user1 = session.append_message(user_message("one")).await.unwrap();
        let assistant1 = session
            .append_message(assistant_message("two"))
            .await
            .unwrap();
        session.append_message(user_message("three")).await.unwrap();
        session.move_to(Some(user1.clone()), None).await.unwrap();
        session
            .append_message(assistant_message("branched"))
            .await
            .unwrap();

        let branch_ids = session
            .get_branch(None)
            .await
            .iter()
            .map(zedflow_agent::harness::session::repo_utils::entry_id)
            .map(str::to_string)
            .collect::<Vec<_>>();
        assert!(branch_ids.contains(&user1));
        assert!(!branch_ids.contains(&assistant1));
        assert_eq!(
            message_roles(&session.build_context().await.messages),
            vec!["user", "assistant"]
        );
    });
}

#[test]
fn supports_moving_leaf_to_root() {
    run(async {
        let session = Session::new(InMemorySessionStorage::default());
        session.append_message(user_message("one")).await.unwrap();
        session.move_to(None, None).await.unwrap();

        assert_eq!(session.get_leaf_id().await, None);
        assert!(session.build_context().await.messages.is_empty());
    });
}

#[test]
fn reconstructs_compaction_summaries_in_context() {
    run(async {
        let session = Session::new(InMemorySessionStorage::default());
        session.append_message(user_message("one")).await.unwrap();
        session
            .append_message(assistant_message("two"))
            .await
            .unwrap();
        let user2 = session.append_message(user_message("three")).await.unwrap();
        session
            .append_message(assistant_message("four"))
            .await
            .unwrap();
        session
            .append_compaction("summary", user2, 1234, None, None)
            .await
            .unwrap();
        session.append_message(user_message("five")).await.unwrap();

        let context = session.build_context().await;
        assert_eq!(
            message_role(&context.messages[0]),
            Some("compactionSummary")
        );
        assert_eq!(context.messages.len(), 4);
    });
}

#[test]
fn supports_moving_with_branch_summary_entries_in_context() {
    run(async {
        let session = Session::new(InMemorySessionStorage::default());
        let user1 = session.append_message(user_message("one")).await.unwrap();
        let summary_id = session
            .move_to(
                Some(user1.clone()),
                Some(BranchSummaryDraft {
                    summary: "summary text".to_string(),
                    details: None,
                    from_hook: None,
                }),
            )
            .await
            .unwrap()
            .unwrap();

        let summary_entry = session.get_entry(&summary_id).await.unwrap();
        assert!(
            matches!(summary_entry, zedflow_agent::harness::types::SessionTreeEntry::BranchSummary(entry) if entry.base.parent_id == Some(user1.clone()) && entry.from_id == user1)
        );
        let context = session.build_context().await;
        assert_eq!(message_role(&context.messages[1]), Some("branchSummary"));
    });
}

#[test]
fn supports_custom_message_entries_in_context() {
    run(async {
        let session = Session::new(InMemorySessionStorage::default());
        session.append_message(user_message("one")).await.unwrap();
        session
            .append_custom_message_entry(
                "custom",
                CustomMessageContent::Text("hello".to_string()),
                true,
                Some(json!({ "ok": true })),
            )
            .await
            .unwrap();

        let context = session.build_context().await;
        assert_eq!(message_role(&context.messages[1]), Some("custom"));
    });
}

#[test]
fn normalizes_session_names() {
    run(async {
        let session = Session::new(InMemorySessionStorage::default());
        session
            .append_session_name(" hello\nworld\r\nagain ")
            .await
            .unwrap();
        assert_eq!(
            session.get_session_name().await.as_deref(),
            Some("hello world again")
        );
    });
}

#[test]
fn supports_labels_and_session_info_entries_without_affecting_context() {
    run(async {
        let session = Session::new(InMemorySessionStorage::default());
        let user1 = session.append_message(user_message("one")).await.unwrap();
        session
            .append_label(user1.clone(), Some("checkpoint".to_string()))
            .await
            .unwrap();
        session.append_session_name("name").await.unwrap();

        let entries = session.get_entries().await;
        assert!(entries.iter().any(|entry| matches!(
            entry,
            zedflow_agent::harness::types::SessionTreeEntry::Label(_)
        )));
        assert!(entries.iter().any(|entry| matches!(
            entry,
            zedflow_agent::harness::types::SessionTreeEntry::SessionInfo(_)
        )));
        assert_eq!(
            session.get_label(&user1).await.as_deref(),
            Some("checkpoint")
        );
        assert_eq!(session.get_session_name().await.as_deref(), Some("name"));
        assert_eq!(session.build_context().await.messages.len(), 1);
    });
}

#[test]
fn rejects_labels_for_missing_entries() {
    run(async {
        let session = Session::new(InMemorySessionStorage::default());
        let error = session
            .append_label("missing", Some("checkpoint".to_string()))
            .await
            .unwrap_err();
        assert_eq!(error.message, "Entry missing not found");
    });
}

#[test]
fn persists_leaf_changes_and_appended_entries_via_jsonl_storage() {
    run(async {
        let temp = TempDir::new();
        let file = temp.path().join("session.jsonl");
        let storage = jsonl_storage(&temp, &file).await;
        let session = Session::new(storage);
        let user1 = session.append_message(user_message("one")).await.unwrap();
        session
            .append_message(assistant_message("two"))
            .await
            .unwrap();
        session
            .append_label(user1.clone(), Some("checkpoint".to_string()))
            .await
            .unwrap();
        session.append_session_name("name").await.unwrap();
        session.move_to(Some(user1.clone()), None).await.unwrap();
        session
            .append_message(assistant_message("branched"))
            .await
            .unwrap();

        let session2 = Session::new(
            JsonlSessionStorage::open(
                Arc::new(NodeExecutionEnv::with_cwd(temp.string())),
                file.to_string_lossy().to_string(),
            )
            .await
            .unwrap(),
        );
        assert_eq!(
            message_roles(&session2.build_context().await.messages),
            vec!["user", "assistant"]
        );
        assert_eq!(
            session2.get_label(&user1).await.as_deref(),
            Some("checkpoint")
        );
        assert_eq!(session2.get_session_name().await.as_deref(), Some("name"));

        let lines = std::fs::read_to_string(&file).unwrap();
        let lines = lines.trim().split('\n').collect::<Vec<_>>();
        assert!(lines.len() > 1);
        let header: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(
            header.get("type").and_then(serde_json::Value::as_str),
            Some("session")
        );
        assert_eq!(
            header.get("version").and_then(serde_json::Value::as_u64),
            Some(3)
        );
        let entries = lines[1..]
            .iter()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert!(
            entries
                .iter()
                .any(|entry| entry.get("type").and_then(serde_json::Value::as_str) == Some("leaf"))
        );
        for entry in entries {
            assert_ne!(
                entry.get("type").and_then(serde_json::Value::as_str),
                Some("entry")
            );
            assert!(
                entry
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .is_some()
            );
        }
    });
}

async fn jsonl_storage(temp: &TempDir, file: &std::path::Path) -> JsonlSessionStorage {
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
