use adk_graph::{
    checkpoint::Checkpointer,
    state::{Checkpoint, State},
};
use serde_json::json;
use zf_runtime::stored_checkpointer::StoredCheckpointer;
use zf_storage::{content_store::ContentStore, contracts::CheckpointStore};

#[tokio::test]
async fn adk_checkpoint_roundtrip_retains_nested_resume_state_and_notifies_after_commit() {
    let temp = tempfile::tempdir().unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(2)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(temp.path().join("storage.db"))
                .create_if_missing(true)
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                .synchronous(sqlx::sqlite::SqliteSynchronous::Full),
        )
        .await
        .unwrap();
    let content = ContentStore::new(pool.clone()).await.unwrap();
    let checkpoints = CheckpointStore::new(content.clone()).await.unwrap();
    let cp = StoredCheckpointer::new(checkpoints.clone());
    let mut notices = cp.subscribe();
    let mut state = State::new();
    state.insert("history".into(), json!([{"role":"user","text":"é🙂"}]));
    state.insert("__zedflow:consumedMessages".into(), json!(["message-1"]));
    let mut checkpoint = Checkpoint::new("run/child@3", state, 4, vec!["inbox".into()]);
    checkpoint.cleared_interrupt = Some("inbox".into());
    checkpoint.attempts.insert("tool".into(), 2);
    checkpoint
        .child_ledger
        .insert("nested".into(), json!({"effect":"already done"}));
    checkpoint
        .metadata
        .insert("extension".into(), json!({"extra":[1,true,null]}));
    let expected = serde_json::to_value(&checkpoint).unwrap();
    cp.save(&checkpoint).await.unwrap();
    let header = notices.recv().await.unwrap();
    assert_eq!(header.consumed_messages, vec!["message-1"]);
    // A separate store/connection can hydrate the receipt as soon as it is visible.
    assert_eq!(checkpoints.hydrate(&header).await.unwrap(), expected);
    assert_eq!(
        serde_json::to_value(cp.load(&checkpoint.thread_id).await.unwrap().unwrap()).unwrap(),
        expected
    );
    assert_eq!(
        serde_json::to_value(cp.list(&checkpoint.thread_id).await.unwrap()).unwrap(),
        json!([expected])
    );
    let mut altered = checkpoint.clone();
    altered.state.insert("new".into(), json!("input"));
    assert!(cp.save(&altered).await.is_err());
    assert!(matches!(
        notices.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
    assert_eq!(
        serde_json::to_value(
            cp.load_by_id(&checkpoint.checkpoint_id)
                .await
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        expected
    );
    cp.delete(&checkpoint.thread_id).await.unwrap();
    assert!(cp.load(&checkpoint.thread_id).await.unwrap().is_none());
    assert_eq!(
        content.resolve(&header.checkpoint_ref).await.unwrap(),
        expected,
        "deleting a checkpoint index does not garbage-collect immutable content"
    );
    pool.close().await;
}

#[tokio::test]
async fn historical_sqlite_reader_matches_the_actual_adk_serialized_checkpoint() {
    use zf_storage::session_archive::checkpoints;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("checkpoints.db");
    let writer = adk_graph::checkpoint::SqliteCheckpointer::new(&format!(
        "sqlite://{}?mode=rwc",
        path.display()
    ))
    .await
    .unwrap();
    let plain = Checkpoint::new("run", State::new(), 1, vec!["inbox".into()]);
    let mut nested = Checkpoint::new("run/child", State::new(), 2, vec!["tool".into()]);
    nested.cleared_interrupt = Some("tool".into());
    nested.attempts.insert("tool".into(), 2);
    nested
        .child_ledger
        .insert("nested".into(), json!({"effect":"already done"}));
    writer.save(&plain).await.unwrap();
    writer.save(&nested).await.unwrap();
    let restored = checkpoints(
        &path,
        "run",
        &zf_runtime::archive_validation::AdkCheckpointCodec,
    )
    .await
    .unwrap();
    assert_eq!(
        restored,
        vec![
            serde_json::to_value(plain).unwrap(),
            serde_json::to_value(nested).unwrap()
        ]
    );
}
