use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use zf_storage::content_store::ContentStore;

#[tokio::test]
async fn immutable_content_survives_reopen_and_exports_without_losing_checkpoint_fields() {
    let directory = tempfile::tempdir().unwrap();
    let options = SqliteConnectOptions::new()
        .filename(directory.path().join("storage.db"))
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Full);
    let pool = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(options.clone())
        .await
        .unwrap();
    let store = ContentStore::new(pool.clone()).await.unwrap();
    let value = json!({"history":[{"role":"user","parts":[{"text":"é🙂\n"}]}],
        "interrupts":[{"id":"wait-one","value":{"answer":null}}],"attempts":{"child":2},
        "subgraphResults":{"child":{"completed":false,"state":{"extra":[1,true,null]}}},
        "reference":"sha256:literal-is-not-a-pointer"});
    let reference = store.intern(&value).await.unwrap();
    assert_eq!(store.intern(&value).await.unwrap(), reference);
    let blobs = store
        .export_blobs(std::slice::from_ref(&reference))
        .await
        .unwrap();
    drop(store);
    pool.close().await;
    let reopened = SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(options)
        .await
        .unwrap();
    let store = ContentStore::new(reopened.clone()).await.unwrap();
    assert_eq!(store.resolve(&reference).await.unwrap(), value);
    let target_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let target = ContentStore::new(target_pool).await.unwrap();
    target.import_blobs(&blobs).await.unwrap();
    assert_eq!(target.resolve(&reference).await.unwrap(), value);
    reopened.close().await;
}

#[tokio::test]
async fn registry_shares_content_only_through_explicit_aliases_and_retains_frozen_revisions() {
    use std::sync::Arc;
    use zf_core::identity::{Permission, Scope};
    use zf_storage::data::{DataError, DataRegistry};
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let content = ContentStore::new(pool.clone()).await.unwrap();
    let registry = DataRegistry::new(pool.clone(), content.clone(), "run-a")
        .await
        .unwrap();
    let other = DataRegistry::new(pool, content, "run-b").await.unwrap();
    let writer = Scope::Flow("writer".into());
    let reader = Scope::Bridge("reader".into());
    let initial = registry
        .create(&writer, "result", &json!({"text":"original"}))
        .await
        .unwrap();
    assert!(matches!(
        registry.snapshot(&reader, "result").await,
        Err(DataError::NotFound)
    ));
    registry
        .grant(&writer, "result", &reader, "result", Permission::Read)
        .await
        .unwrap();
    let shared = registry.snapshot(&reader, "result").await.unwrap();
    assert!(Arc::ptr_eq(&initial.value, &shared.value));
    assert!(matches!(
        other.snapshot(&writer, "result").await,
        Err(DataError::NotFound)
    ));
    assert!(matches!(
        registry
            .publish(
                &reader,
                "result",
                &initial.revision,
                &json!({"text":"denied"})
            )
            .await,
        Err(DataError::PermissionDenied)
    ));
    let updated = registry
        .publish(
            &writer,
            "result",
            &initial.revision,
            &json!({"text":"updated"}),
        )
        .await
        .unwrap();
    assert_eq!(
        *registry.snapshot(&reader, "result").await.unwrap().value,
        json!({"text":"updated"})
    );
    assert_eq!(
        *registry
            .revision(&reader, "result", &initial.revision)
            .await
            .unwrap()
            .value,
        json!({"text":"original"})
    );
    assert_ne!(initial.revision, updated.revision);
    let repeated = registry
        .publish(&writer, "result", &updated.revision, &updated.value)
        .await
        .unwrap();
    assert_eq!(updated.content_ref, repeated.content_ref);
    assert_ne!(updated.revision, repeated.revision);
}

#[tokio::test]
async fn checkpoint_publication_is_atomic_immutable_and_keeps_the_full_resume_document() {
    use zf_storage::contracts::CheckpointStore;
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let content = ContentStore::new(pool).await.unwrap();
    let checkpoints = CheckpointStore::new(content.clone()).await.unwrap();
    let value = json!({"checkpoint_id":"cp-1","thread_id":"run/child@3","step":4,
        "pending_nodes":["ask"],"created_at":"2026-09-16T12:00:00Z",
        "state":{"history":[{"role":"user","text":"hello"}],"__zedflow:consumedMessages":["m1"]},
        "cleared_interrupt":"ask","attempts":{"tool":2},"child_ledger":{"nested":{"result":"effect done"}},
        "metadata":{"extension":{"null":null}}});
    let header = checkpoints.save(&value).await.unwrap();
    assert_eq!(header.consumed_messages, vec!["m1"]);
    assert_eq!(
        content.resolve(&header.state_ref).await.unwrap(),
        value["state"]
    );
    assert_eq!(
        checkpoints.load("run/child@3").await.unwrap().unwrap(),
        value
    );
    assert_eq!(
        checkpoints.load_by_id("cp-1").await.unwrap().unwrap(),
        value
    );
    assert_eq!(checkpoints.list_run_headers("run").await.unwrap().len(), 1);
    assert!(checkpoints.list_run_headers("ru").await.unwrap().is_empty());
    assert_eq!(checkpoints.save(&value).await.unwrap(), header);
    let mut changed = value.clone();
    changed["attempts"]["tool"] = json!(3);
    assert!(checkpoints.save(&changed).await.is_err());
    assert_eq!(
        checkpoints.load_by_id("cp-1").await.unwrap().unwrap(),
        value
    );
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let target = ContentStore::new(pool).await.unwrap();
    target
        .import_blobs(
            &content
                .export_blobs(std::slice::from_ref(&header.checkpoint_ref))
                .await
                .unwrap(),
        )
        .await
        .unwrap();
    let imported = CheckpointStore::new(target).await.unwrap();
    let mut altered = header.clone();
    altered.consumed_messages = vec!["unconsumed".into()];
    assert!(imported.install_headers(&[altered]).await.is_err());
    assert!(imported.load_by_id("cp-1").await.unwrap().is_none());
    imported.install_headers(&[header]).await.unwrap();
    assert_eq!(imported.load_by_id("cp-1").await.unwrap().unwrap(), value);
}
