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

#[tokio::test]
async fn resource_reads_capture_exact_provenance_and_share_resident_content() {
    use std::sync::Arc;
    use zf_core::types::{DataType, TypeRegistry};
    use zf_runtime::resources::ResourceReads;
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("one.txt"), "captured α").unwrap();
    let child_cwd = temp.path().join("child");
    std::fs::create_dir(&child_cwd).unwrap();
    std::fs::write(child_cwd.join("one.txt"), "captured α").unwrap();
    let db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let content = ContentStore::new(db).await.unwrap();
    let cancel = tokio_util::sync::CancellationToken::new();
    let host = ResourceReads::new(Some(content.clone()), cancel.clone());
    let readers = host.native_readers(temp.path().into()).unwrap();
    let input = json!({"path":"one.txt"});
    let first = host
        .read(
            &readers,
            "file.text",
            &input,
            &DataType::Text,
            &TypeRegistry::new(),
        )
        .await
        .unwrap()
        .unwrap();
    let child_host = host.clone();
    let child_readers = child_host.native_readers(child_cwd.clone()).unwrap();
    let second = child_host
        .read(
            &child_readers,
            "file.text",
            &input,
            &DataType::Text,
            &TypeRegistry::new(),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&first.value, &second.value));
    assert_eq!(
        second.provenance["source"]["path"],
        json!(child_cwd.join("one.txt"))
    );
    assert_eq!(*first.value, json!("captured α"));
    assert_eq!(first.provenance["kind"], "reader");
    assert_eq!(first.provenance["reader"]["id"], "file.text");
    assert_eq!(first.provenance["input"], serde_json::Value::Null);
    assert_eq!(
        first.provenance["source"]["path"],
        json!(temp.path().join("one.txt"))
    );
    assert!(first.provenance["source"].get("source").is_none());
    assert_eq!(
        content
            .resolve(first.provenance["inputRef"].as_str().unwrap())
            .await
            .unwrap(),
        input
    );
    assert_eq!(
        content
            .resolve(first.provenance["contentRef"].as_str().unwrap())
            .await
            .unwrap(),
        json!("captured α")
    );
    std::fs::write(temp.path().join("one.txt"), "changed").unwrap();
    assert_eq!(*first.value, json!("captured α"));
    assert!(
        host.read(
            &readers,
            "file.text",
            &json!({"path":"absent"}),
            &DataType::Text,
            &TypeRegistry::new()
        )
        .await
        .unwrap()
        .is_none()
    );
    assert!(
        host.read(
            &readers,
            "file.text",
            &input,
            &DataType::Number,
            &TypeRegistry::new()
        )
        .await
        .is_err()
    );
    cancel.cancel();
    assert!(
        host.read(
            &readers,
            "file.text",
            &input,
            &DataType::Text,
            &TypeRegistry::new()
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn native_readers_enforce_json_types_sqlite_identity_and_exact_content_decoding() {
    use sqlx::Connection;
    use zf_core::types::{DataType, TypeRegistry};
    use zf_runtime::resources::ResourceReads;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("records.db");
    let mut db = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TABLE records(id TEXT, document TEXT)")
        .execute(&mut db)
        .await
        .unwrap();
    for (id, value) in [
        ("one", "42"),
        ("text", "\"42\""),
        ("duplicate", "1"),
        ("duplicate", "2"),
    ] {
        sqlx::query("INSERT INTO records VALUES (?, ?)")
            .bind(id)
            .bind(value)
            .execute(&mut db)
            .await
            .unwrap();
    }
    db.close().await.unwrap();
    let original = std::fs::read(&path).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let content = ContentStore::new(pool).await.unwrap();
    let host = ResourceReads::new(
        Some(content.clone()),
        tokio_util::sync::CancellationToken::new(),
    );
    let readers = host.native_readers(temp.path().into()).unwrap();
    let ty = TypeRegistry::new();
    let result = host
        .read(
            &readers,
            "sqlite.json",
            &json!({"path":"records.db","table":"records","id":"one"}),
            &DataType::Number,
            &ty,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*result.value, json!(42));
    for id in ["text", "duplicate"] {
        assert!(
            host.read(
                &readers,
                "sqlite.json",
                &json!({"path":"records.db","table":"records","id":id}),
                &DataType::Number,
                &ty
            )
            .await
            .is_err()
        );
    }
    assert!(
        host.read(
            &readers,
            "sqlite.json",
            &json!({"path":"records.db","table":"records; DROP TABLE records","id":"one"}),
            &DataType::Number,
            &ty
        )
        .await
        .is_err()
    );
    assert!(
        host.read(
            &readers,
            "sqlite.json",
            &json!({"path":"missing.db","table":"records","id":"one"}),
            &DataType::Number,
            &ty
        )
        .await
        .unwrap()
        .is_none()
    );
    assert!(!temp.path().join("missing.db").exists());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    std::fs::write(temp.path().join("data.json"), "42").unwrap();
    assert_eq!(
        *host
            .read(
                &readers,
                "file.json",
                &json!({"path":"data.json"}),
                &DataType::Number,
                &ty
            )
            .await
            .unwrap()
            .unwrap()
            .value,
        json!(42)
    );
    let raw = content
        .intern(&json!([{"index":0,"stream":"stdout","data":"b2s="}]))
        .await
        .unwrap();
    let decoded = host
        .read(
            &readers,
            "content.text",
            &json!({"contentRef":raw}),
            &DataType::Text,
            &ty,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*decoded.value, json!("ok"));
    let number = content.intern(&json!(42)).await.unwrap();
    assert_eq!(
        *host
            .read(
                &readers,
                "content.json",
                &json!({"contentRef":number}),
                &DataType::Number,
                &ty
            )
            .await
            .unwrap()
            .unwrap()
            .value,
        json!(42)
    );
    let bad = content
        .intern(&json!([{"index":1,"stream":"stdout","data":"b2s="}]))
        .await
        .unwrap();
    assert!(
        host.read(
            &readers,
            "content.text",
            &json!({"contentRef":bad}),
            &DataType::Text,
            &ty
        )
        .await
        .is_err()
    );
    let oversized = std::fs::File::create(temp.path().join("large.txt")).unwrap();
    oversized.set_len(16 * 1024 * 1024 + 1).unwrap();
    assert!(
        host.read(
            &readers,
            "file.text",
            &json!({"path":"large.txt"}),
            &DataType::Text,
            &ty
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn injected_reader_is_cancelled_mid_read_and_trial_provenance_remains_inline() {
    use std::sync::Arc;
    use zf_context::resource_readers::{
        ReadResource, ReaderContract, ReaderOutput, ReaderRegistry, ResourceReader,
    };
    use zf_core::types::{DataType, TypeRegistry};
    use zf_runtime::resources::ResourceReads;
    struct WaitingReader(Arc<tokio::sync::Notify>);
    impl ResourceReader for WaitingReader {
        fn contract(&self) -> ReaderContract {
            ReaderContract {
                id: "waiting".into(),
                version: "1".into(),
                input: DataType::Text,
                output: ReaderOutput::Fixed {
                    data_type: DataType::Text,
                },
            }
        }
        fn read<'a>(
            &'a self,
            _: &'a serde_json::Value,
        ) -> futures::future::BoxFuture<'a, anyhow::Result<Option<ReadResource>>> {
            Box::pin(async move {
                self.0.notify_one();
                std::future::pending().await
            })
        }
    }
    let started = Arc::new(tokio::sync::Notify::new());
    let mut registry = ReaderRegistry::new();
    registry
        .register(Arc::new(WaitingReader(started.clone())))
        .unwrap();
    let cancel = tokio_util::sync::CancellationToken::new();
    let host = ResourceReads::new(None, cancel.clone());
    let task = tokio::spawn(async move {
        host.read(
            &registry,
            "waiting",
            &json!("input"),
            &DataType::Text,
            &TypeRegistry::new(),
        )
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), started.notified())
        .await
        .unwrap();
    cancel.cancel();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("trial.txt"), "trial").unwrap();
    let host = ResourceReads::new(None, tokio_util::sync::CancellationToken::new());
    let registry = host.native_readers(temp.path().into()).unwrap();
    let input = json!({"path":"trial.txt"});
    let trial = host
        .read(
            &registry,
            "file.text",
            &input,
            &DataType::Text,
            &TypeRegistry::new(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(trial.provenance["input"], input);
    assert_eq!(trial.provenance["inputRef"], serde_json::Value::Null);
    assert_eq!(trial.provenance["contentRef"], serde_json::Value::Null);
}

#[test]
fn reader_dependency_diagnostics_are_metadata_only_and_keep_nested_paths() {
    use zf_runtime::resources::dependency_diagnostics;
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("invalid.db"), "not a database").unwrap();
    let child = json!({"id":"child","name":"child","edges":[],"nodes":[{"id":"prepare","position":{"x":0,"y":0},"data":{"label":"Prepare","kind":"context","config":{"contextBindings":{
        "missing":{"kind":"reader","reader":"file.text","input":{"kind":"literal","value":{"path":"missing.txt"}}},
        "database":{"kind":"reader","reader":"sqlite.json","input":{"kind":"literal","value":{"path":"invalid.db","table":"x","id":"one"}}},
        "dynamic":{"kind":"reader","reader":"file.text","input":{"kind":"state","field":"path"}}
    }}}}]});
    let doc = serde_json::from_value(json!({"id":"root","name":"root","edges":[],"nodes":[{"id":"child","position":{"x":0,"y":0},"data":{"label":"Child","kind":"subgraph","config":{"composition":child}}}]})).unwrap();
    let diagnostics = dependency_diagnostics(&doc, temp.path());
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "reader_dependency");
    assert_eq!(diagnostics[0].path, "child/prepare/bindings/missing");
    assert!(
        diagnostics[0]
            .message
            .contains("required only if this resource is selected")
    );
    assert_eq!(
        std::fs::read_to_string(temp.path().join("invalid.db")).unwrap(),
        "not a database"
    );
}

#[tokio::test]
async fn run_services_scope_native_reads_and_share_capture_store_and_cancellation() {
    use std::sync::Arc;
    use zf_core::types::{DataType, TypeRegistry};
    use zf_runtime::{runtime::RunServices, workspace_context::ContextSnapshot};
    let temp = tempfile::tempdir().unwrap();
    let child_dir = temp.path().join("docs");
    std::fs::create_dir(&child_dir).unwrap();
    std::fs::write(temp.path().join("value.txt"), "same content").unwrap();
    std::fs::write(child_dir.join("value.txt"), "same content").unwrap();
    let services = RunServices::new(
        "scope-test".into(),
        temp.path().into(),
        temp.path().join("data"),
        ContextSnapshot::default(),
        json!({}),
        vec![],
    )
    .unwrap();
    services.set_context_sources(vec![], None);
    // Child services created before storage attachment must see the same store.
    let child = services.for_working_directory(Some("docs")).unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let content = ContentStore::new(pool.clone()).await.unwrap();
    services.set_content_store(content.clone());
    let input = json!({"path":"value.txt"});
    let types = TypeRegistry::new();
    let parent_readers = services.resource_readers().unwrap();
    let child_readers = child.resource_readers().unwrap();
    let parent_value = services
        .resource_reads()
        .read(
            &parent_readers,
            "file.text",
            &input,
            &DataType::Text,
            &types,
        )
        .await
        .unwrap()
        .unwrap();
    let child_value = child
        .resource_reads()
        .read(&child_readers, "file.text", &input, &DataType::Text, &types)
        .await
        .unwrap()
        .unwrap();
    assert!(Arc::ptr_eq(&parent_value.value, &child_value.value));
    assert_eq!(
        child_value.provenance["source"]["path"],
        json!(child_dir.join("value.txt"))
    );
    assert_eq!(
        parent_value.provenance["source"]["path"],
        json!(temp.path().join("value.txt"))
    );
    let reference = child_value.provenance["contentRef"].as_str().unwrap();
    assert_eq!(
        content.resolve(reference).await.unwrap(),
        json!("same content")
    );
    // The common cancellation token applies to acquisitions in every instance.
    services.cancel.cancel();
    assert!(
        child
            .resource_reads()
            .read(&child_readers, "file.text", &input, &DataType::Text, &types)
            .await
            .is_err()
    );
    pool.close().await;
}
