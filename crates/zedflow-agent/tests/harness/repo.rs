use std::future::Future;
use std::sync::Arc;

use zedflow_agent::harness::env::nodejs::NodeExecutionEnv;
use zedflow_agent::harness::session::{InMemorySessionRepo, JsonlSessionRepo};
use zedflow_agent::harness::types::{
    JsonlSessionCreateOptions, JsonlSessionListOptions, JsonlSessionMetadata, SessionCreateOptions,
    SessionForkOptions, SessionRepo,
};

#[path = "session-test-utils.rs"]
mod session_test_utils;

use session_test_utils::{TempDir, assistant_message, user_message};

fn run<T>(future: impl Future<Output = T>) -> T {
    futures::executor::block_on(future)
}

#[test]
fn memory_opens_deletes_and_forks_by_metadata() {
    run(async {
        let repo = InMemorySessionRepo::new();
        let session = repo
            .create(SessionCreateOptions {
                id: Some("session-1".to_string()),
            })
            .await
            .unwrap();
        let metadata = session.get_metadata().await;
        let user1 = session.append_message(user_message("one")).await.unwrap();
        let assistant1 = session
            .append_message(assistant_message("two"))
            .await
            .unwrap();
        let user2 = session.append_message(user_message("three")).await.unwrap();

        assert_eq!(
            repo.open(metadata.clone())
                .await
                .unwrap()
                .get_metadata()
                .await,
            metadata
        );
        assert_eq!(
            repo.list()
                .await
                .unwrap()
                .iter()
                .map(|info| info.id.as_str())
                .collect::<Vec<_>>(),
            vec!["session-1"]
        );
        let fork = repo
            .fork(
                metadata.clone(),
                SessionForkOptions {
                    entry_id: Some(user2.clone()),
                    position: None,
                    id: Some("session-2".to_string()),
                },
            )
            .await
            .unwrap();
        assert_eq!(
            entry_ids(fork.get_entries().await),
            vec![user1.clone(), assistant1.clone()]
        );
        let full_fork = repo
            .fork(
                metadata.clone(),
                SessionForkOptions {
                    entry_id: None,
                    position: None,
                    id: Some("session-3".to_string()),
                },
            )
            .await
            .unwrap();
        assert_eq!(
            entry_ids(full_fork.get_entries().await),
            vec![user1, assistant1, user2]
        );
        repo.delete(metadata.clone()).await.unwrap();
        let error = match repo.open(metadata).await {
            Ok(_) => panic!("deleted session should not open"),
            Err(error) => error,
        };
        assert_eq!(error.message, "Session not found: session-1");
    });
}

#[test]
fn jsonl_stores_sessions_below_encoded_cwd_directories_and_lists_by_cwd() {
    run(async {
        let temp = TempDir::new();
        let env = Arc::new(NodeExecutionEnv::with_cwd(temp.string()));
        let repo = JsonlSessionRepo::new(env, temp.string());
        let cwd = "/tmp/my-project";
        let other_cwd = "/tmp/other-project";
        repo.create(JsonlSessionCreateOptions {
            cwd: cwd.to_string(),
            id: Some("019de8c2-de29-73e9-ae0c-e134db34c447".to_string()),
            parent_session_path: None,
        })
        .await
        .unwrap();
        repo.create(JsonlSessionCreateOptions {
            cwd: other_cwd.to_string(),
            id: Some("other-session".to_string()),
            parent_session_path: None,
        })
        .await
        .unwrap();
        let metadata = only_jsonl(&repo, Some(cwd)).await;
        let other_metadata = only_jsonl(&repo, Some(other_cwd)).await;

        assert!(metadata.path.contains("--tmp-my-project--"));
        assert!(other_metadata.path.contains("--tmp-other-project--"));
        assert!(std::path::Path::new(&metadata.path).exists());
        assert_eq!(
            repo.list(JsonlSessionListOptions {
                cwd: Some(cwd.to_string())
            })
            .await
            .unwrap()
            .iter()
            .map(|session_metadata| session_metadata.base.id.as_str())
            .collect::<Vec<_>>(),
            vec![metadata.base.id.as_str()]
        );
        let mut listed = repo
            .list(JsonlSessionListOptions { cwd: None })
            .await
            .unwrap()
            .into_iter()
            .map(|session_metadata| session_metadata.base.id)
            .collect::<Vec<_>>();
        listed.sort();
        let mut expected = vec![metadata.base.id, other_metadata.base.id];
        expected.sort();
        assert_eq!(listed, expected);
    });
}

#[test]
fn jsonl_opens_deletes_and_forks_by_metadata() {
    run(async {
        let temp = TempDir::new();
        let env = Arc::new(NodeExecutionEnv::with_cwd(temp.string()));
        let repo = JsonlSessionRepo::new(env, temp.string());
        let source = repo
            .create(JsonlSessionCreateOptions {
                cwd: "/tmp/source".to_string(),
                id: Some("source-session".to_string()),
                parent_session_path: None,
            })
            .await
            .unwrap();
        let user1 = source.append_message(user_message("one")).await.unwrap();
        let assistant1 = source
            .append_message(assistant_message("two"))
            .await
            .unwrap();
        let user2 = source.append_message(user_message("three")).await.unwrap();
        let source_metadata = only_jsonl(&repo, Some("/tmp/source")).await;

        assert_eq!(
            repo.open(source_metadata.clone())
                .await
                .unwrap()
                .get_metadata()
                .await
                .id,
            source_metadata.base.id
        );
        let fork = repo
            .fork(
                source_metadata.clone(),
                JsonlSessionCreateOptions {
                    cwd: "/tmp/target".to_string(),
                    id: Some("fork-session".to_string()),
                    parent_session_path: None,
                },
                SessionForkOptions {
                    entry_id: Some(user2.clone()),
                    position: None,
                    id: None,
                },
            )
            .await
            .unwrap();
        let fork_metadata = jsonl_by_id(&repo, "fork-session").await;
        assert_eq!(fork_metadata.cwd, "/tmp/target");
        assert_eq!(
            fork_metadata.parent_session_path.as_deref(),
            Some(source_metadata.path.as_str())
        );
        assert_eq!(
            entry_ids(fork.get_entries().await),
            vec![user1.clone(), assistant1.clone()]
        );

        let full_fork = repo
            .fork(
                source_metadata.clone(),
                JsonlSessionCreateOptions {
                    cwd: "/tmp/target".to_string(),
                    id: Some("full-fork-session".to_string()),
                    parent_session_path: None,
                },
                SessionForkOptions {
                    entry_id: None,
                    position: None,
                    id: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(
            entry_ids(full_fork.get_entries().await),
            vec![user1, assistant1, user2]
        );
        repo.delete(source_metadata.clone()).await.unwrap();
        assert!(!std::path::Path::new(&source_metadata.path).exists());
        let error = match repo.open(source_metadata).await {
            Ok(_) => panic!("deleted session should not open"),
            Err(error) => error,
        };
        assert!(error.message.contains("Session not found"));
    });
}

async fn only_jsonl(repo: &JsonlSessionRepo, cwd: Option<&str>) -> JsonlSessionMetadata {
    let sessions = repo
        .list(JsonlSessionListOptions {
            cwd: cwd.map(str::to_string),
        })
        .await
        .unwrap();
    assert_eq!(sessions.len(), 1);
    sessions.into_iter().next().unwrap()
}

async fn jsonl_by_id(repo: &JsonlSessionRepo, id: &str) -> JsonlSessionMetadata {
    repo.list(JsonlSessionListOptions { cwd: None })
        .await
        .unwrap()
        .into_iter()
        .find(|metadata| metadata.base.id == id)
        .expect("session id should be listed")
}

fn entry_ids(entries: Vec<zedflow_agent::harness::types::SessionTreeEntry>) -> Vec<String> {
    entries
        .iter()
        .map(zedflow_agent::harness::session::repo_utils::entry_id)
        .map(str::to_string)
        .collect()
}
