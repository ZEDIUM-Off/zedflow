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

#[tokio::test]
async fn session_projection_keeps_exact_details_out_of_wire_bootstrap_and_idle_heartbeats() {
    use std::sync::Arc;
    use zf_storage::{contracts::RuntimeInspection, session_store, session_sync::SessionSync};
    struct Inspector;
    impl RuntimeInspection for Inspector {
        fn summary(&self, value: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
            Ok(value["summary"].clone())
        }
        fn interactive(&self, value: &serde_json::Value) -> anyhow::Result<bool> {
            Ok(value["interactive"].as_bool().unwrap())
        }
    }
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query("CREATE TABLE runs(id TEXT PRIMARY KEY,document TEXT NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE events(seq INTEGER PRIMARY KEY,run TEXT NOT NULL,document TEXT NOT NULL)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let content = ContentStore::new(pool.clone()).await.unwrap();
    session_store::initialize(&pool).await.unwrap();
    let run = json!({"id":"one","workspaceId":"workspace-one","status":"waiting", "state":{"secret":"full state"},
        "flowSource":"exact Rust source", "activities":[{"occurrenceId":"visit-1","input":{"large":"data"},"output":{"nested":[1,null]}}],
        "runtimeGraph":{"interactive":true,"summary":{"entry":{"instance":"root","port":"main"}}},
        "messages":[{"id":"message-1","text":"hello"}],"timeline":[{"id":"message-1","seq":1,"kind":"message","text":"hello"}]});
    session_store::save(&pool, "one", &run).await.unwrap();
    let projection = session_store::load_projection(&pool, "one").await.unwrap();
    assert!(projection["state"].is_null());
    let restored = session_store::load(&pool, "one").await.unwrap();
    assert_eq!(restored["state"], run["state"]);
    assert_eq!(restored["activities"], run["activities"]);
    assert_eq!(restored["flowSource"], run["flowSource"]);
    let sync = SessionSync::new(pool.clone(), content, Arc::new(Inspector));
    let wire = sync.wire_run(projection.clone()).await.unwrap();
    assert_eq!(wire["state"], json!({}));
    assert_eq!(wire["messages"], json!([]));
    assert_eq!(wire["runtimeGraphSummary"], run["runtimeGraph"]["summary"]);
    assert!(wire["flowSource"].is_null());
    sync.committed("one", projection, 7).await;
    assert_eq!(
        sync.payload("one", Some(7)).await.unwrap(),
        json!({"type":"heartbeat","runId":"one","workspaceId":"workspace-one","revision":7,"cursor":7})
    );
    assert_eq!(
        session_store::backfill_interactive(&pool, &Inspector)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        session_store::load_projection(&pool, "one").await.unwrap()["interactive"],
        true
    );
}

#[tokio::test]
async fn source_catalog_isolated_by_workspace_preserves_accepted_ancestry_and_rejects_external_edits()
 {
    use zf_storage::context_store::SourceStore;
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let a = SourceStore::new(first.path().into(), &["context"]).unwrap();
    let b = SourceStore::new(second.path().into(), &["context"]).unwrap();
    assert!(a.list().await.unwrap().is_empty());
    let one = a
        .save("strategy", "// exact source v1\n", None)
        .await
        .unwrap();
    assert!(b.list().await.unwrap().is_empty());
    assert!(b.read("strategy").await.is_err());
    let two = a
        .save("strategy", "// exact source v2\n", Some(&one.hash))
        .await
        .unwrap();
    assert_eq!(
        a.resolve("strategy", Some(&one.hash)).await.unwrap().hash,
        two.hash
    );
    assert!(
        a.save("strategy", "// stale write", Some(&one.hash))
            .await
            .is_err()
    );
    assert_eq!(
        a.preflight("strategy", Some(&one.hash), &two.hash)
            .await
            .unwrap()
            .source
            .as_deref(),
        Some("// exact source v2\n")
    );
    tokio::fs::write(&two.path, "// edited outside accepted writer\n")
        .await
        .unwrap();
    assert!(a.resolve("strategy", Some(&two.hash)).await.is_err());
    assert!(
        a.preflight("strategy", Some(&two.hash), &two.hash)
            .await
            .is_err()
    );
    assert_eq!(
        a.read("strategy").await.unwrap().source.as_deref(),
        Some("// edited outside accepted writer\n")
    );
    assert!(SourceStore::new(first.path().into(), &["../escape"]).is_err());
}

#[tokio::test]
async fn flow_catalog_uses_injected_validation_and_cannot_read_another_workspaces_local_flow() {
    use std::sync::Arc;
    use zf_flows::schema::Composition;
    use zf_storage::{flow_store::FlowStore, workspaces};
    let home = tempfile::tempdir().unwrap();
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    workspaces::initialize(&pool).await.unwrap();
    let a = workspaces::open(&pool, first.path()).await.unwrap();
    let b = workspaces::open(&pool, second.path()).await.unwrap();
    let validator = Arc::new(|doc: &Composition| {
        anyhow::ensure!(doc.name != "refused", "rejected by fixture contract");
        Ok(())
    });
    let files = FlowStore::new(home.path().into(), validator);
    let node = |id: &str, kind: &str, config: serde_json::Value| json!({"id":id,"position":{"x":0,"y":0},"data":{"kind":kind,"label":id,"config":config}});
    let doc:Composition=serde_json::from_value(json!({"id":"flow","name":"allowed", "nodes":[node("start","start",json!({})),node("write","transform",json!({"field":"output","value":"done"})),node("end","end",json!({}))],"edges":[{"id":"a","source":"start","target":"write"},{"id":"b","source":"write","target":"end"}]})).unwrap();
    let saved = files
        .store(&a, doc.clone(), "workspace", None, None)
        .await
        .unwrap();
    assert!(saved.composition.is_some());
    assert_eq!(files.list(&a).await.unwrap().len(), 1);
    assert!(files.list(&b).await.unwrap().is_empty());
    assert!(files.get(&b, &saved.key).await.is_err());
    let mut refused = doc;
    refused.name = "refused".into();
    assert!(
        files
            .store(
                &a,
                refused,
                "workspace",
                Some(&saved.key),
                Some(&saved.hash)
            )
            .await
            .is_err()
    );
    assert_eq!(files.get(&a, &saved.key).await.unwrap().hash, saved.hash);
}

#[tokio::test]
async fn known_resource_types_are_discoverable_without_creating_any_workspace_source() {
    use std::sync::Arc;
    use zf_core::types::DataType;
    use zf_flows::schema::Composition;
    use zf_storage::{flow_store::FlowStore, source_catalog, workspaces::Workspace};
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let workspace = Workspace {
        id: "workspace".into(),
        name: "empty".into(),
        path: root.path().into(),
        open: true,
    };
    let store = FlowStore::new(
        home.path().into(),
        Arc::new(|_: &Composition| anyhow::bail!("unexpected flow in empty fixture")),
    );
    let catalog = source_catalog::collect(&store, &workspace).await.unwrap();
    let route = catalog
        .entries
        .iter()
        .find(|entry| entry.id == "builtin:route-result")
        .unwrap();
    assert_eq!(route.types["RouteResult"], DataType::Text);
    assert!(
        catalog
            .entries
            .iter()
            .any(|entry| entry.id == "reader:file.text")
    );
    assert!(!root.path().join(".zedflow").exists());
}

#[tokio::test]
async fn bridge_catalog_roundtrips_definitions_and_refuses_stale_overwrite() {
    use zf_flows::composition::BridgeDefinition;
    use zf_storage::bridge_store::BridgeStore;
    let directory = tempfile::tempdir().unwrap();
    let store = BridgeStore::new(directory.path().into()).unwrap();
    let bridge = BridgeDefinition::default();
    let first = store.save("example", &bridge, None).await.unwrap();
    let mut revised = bridge;
    revised.requires.insert("dependency".into());
    let second = store
        .save("example", &revised, Some(&first.hash))
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(store.read("example").await.unwrap().bridge.unwrap()).unwrap(),
        serde_json::to_value(&revised).unwrap()
    );
    assert!(
        store
            .save("example", &BridgeDefinition::default(), Some(&first.hash))
            .await
            .is_err()
    );
    assert_eq!(store.read("example").await.unwrap().hash, second.hash);
}

#[tokio::test]
async fn registry_archive_preserves_alias_rights_and_publication_identity_across_import() {
    use zf_core::identity::{Permission, Scope};
    use zf_storage::{
        data::{DataError, DataRegistry},
        data_archive,
    };
    let source_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let source = ContentStore::new(source_pool.clone()).await.unwrap();
    let registry = DataRegistry::new(source_pool.clone(), source.clone(), "original")
        .await
        .unwrap();
    let scope = Scope::Flow("writer".into());
    let reader = Scope::Bridge("observer".into());
    let first = registry
        .publish_unique(
            &scope,
            "result",
            None,
            &json!({"text":"first"}),
            "publication-one",
        )
        .await
        .unwrap();
    registry
        .grant(&scope, "result", &reader, "result", Permission::Read)
        .await
        .unwrap();
    registry
        .publish(&scope, "result", &first.revision, &json!({"text":"second"}))
        .await
        .unwrap();
    let archive = data_archive::capture(&source_pool, "original")
        .await
        .unwrap();
    archive.validate_contents(&source).await.unwrap();
    let target_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let target = ContentStore::new(target_pool.clone()).await.unwrap();
    let restored = DataRegistry::new(target_pool.clone(), target.clone(), "imported")
        .await
        .unwrap();
    target
        .import_blobs(&source.export_blobs(&archive.roots()).await.unwrap())
        .await
        .unwrap();
    let mut tx = target_pool.begin().await.unwrap();
    archive.install(&mut tx, "imported").await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        *restored.snapshot(&reader, "result").await.unwrap().value,
        json!({"text":"second"})
    );
    assert_eq!(
        restored
            .publish_unique(
                &scope,
                "result",
                None,
                &json!({"text":"first"}),
                "publication-one"
            )
            .await
            .unwrap()
            .revision,
        first.revision
    );
    assert_eq!(
        *restored.snapshot(&scope, "result").await.unwrap().value,
        json!({"text":"second"})
    );
    assert!(matches!(
        restored
            .publish(&reader, "result", &first.revision, &json!({}))
            .await,
        Err(DataError::PermissionDenied)
    ));
    let mut tx = target_pool.begin().await.unwrap();
    assert!(archive.install(&mut tx, "imported").await.is_err());
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn legacy_checkpoints_preserve_resume_fields_and_apply_runtime_validation() {
    use zf_storage::{contracts::CheckpointCodec, session_archive::checkpoints};
    struct Validator;
    impl CheckpointCodec for Validator {
        fn decode_legacy_checkpoint(
            &self,
            value: serde_json::Value,
        ) -> anyhow::Result<serde_json::Value> {
            self.validate_checkpoint(&value)?;
            Ok(value)
        }
        fn validate_checkpoint(&self, value: &serde_json::Value) -> anyhow::Result<()> {
            anyhow::ensure!(value["step"].as_u64().is_some(), "invalid fixture step");
            Ok(())
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.db");
    assert!(
        checkpoints(&path, "run", &Validator)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(!path.exists());
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::query("CREATE TABLE graph_checkpoints(id TEXT,thread_id TEXT,step INTEGER,created_at TEXT,state TEXT,pending_nodes TEXT,metadata TEXT,attempts TEXT,child_ledger TEXT,cleared_interrupt TEXT)")
        .execute(&pool).await.unwrap();
    for (id, thread) in [("a", "run"), ("b", "run/child"), ("c", "run-other")] {
        sqlx::query(
            "INSERT INTO graph_checkpoints VALUES(?,?,4,'2026-09-15T12:00:00Z',?,?,?,?,?,?)",
        )
        .bind(id)
        .bind(thread)
        .bind(r#"{"input":"é🙂","__zedflow:consumedMessages":["message-1"]}"#)
        .bind(r#"["inbox"]"#)
        .bind(r#"{"extra":[1,true,null]}"#)
        .bind(r#"{"inbox":2}"#)
        .bind(r#"{"child":{"effect":"done"}}"#)
        .bind("inbox")
        .execute(&pool)
        .await
        .unwrap();
    }
    let result = checkpoints(&path, "run", &Validator).await.unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(
        result[1],
        json!({"checkpoint_id":"b","thread_id":"run/child","step":4,
        "created_at":"2026-09-15T12:00:00Z","state":{"input":"é🙂","__zedflow:consumedMessages":["message-1"]},
        "pending_nodes":["inbox"],"metadata":{"extra":[1,true,null]},"attempts":{"inbox":2},
        "child_ledger":{"child":{"effect":"done"}},"cleared_interrupt":"inbox"})
    );
    sqlx::query("UPDATE graph_checkpoints SET step=-1 WHERE id='b'")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        checkpoints(&path, "run", &Validator)
            .await
            .unwrap_err()
            .to_string()
            .contains("invalid fixture step")
    );
    pool.close().await;
}

#[tokio::test]
async fn offline_maintenance_keeps_new_sessions_and_exact_content_with_recoverable_backup() {
    use zf_storage::{contracts::CheckpointCodec, migration, session_store};
    struct Validator;
    impl CheckpointCodec for Validator {
        fn decode_legacy_checkpoint(
            &self,
            value: serde_json::Value,
        ) -> anyhow::Result<serde_json::Value> {
            self.validate_checkpoint(&value)?;
            Ok(value)
        }
        fn validate_checkpoint(&self, value: &serde_json::Value) -> anyhow::Result<()> {
            anyhow::ensure!(value["state"].is_object(), "invalid fixture state");
            Ok(())
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");
    tokio::fs::create_dir(&data).await.unwrap();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(data.join("zedflow.db"))
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::query("CREATE TABLE runs(id TEXT PRIMARY KEY,document TEXT NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE events(seq INTEGER PRIMARY KEY AUTOINCREMENT,run TEXT NOT NULL,document TEXT NOT NULL)").execute(&pool).await.unwrap();
    let keep = uuid::Uuid::new_v4().to_string();
    let old = uuid::Uuid::new_v4().to_string();
    let new = uuid::Uuid::new_v4().to_string();
    for (id, date) in [(&keep, 100), (&old, 50), (&new, 300)] {
        let value = json!({"id":id,"workspaceId":"w","status":"waiting","createdAt":date,
            "state":{"history":[{"text":"é🙂"}],"pending":{"waitId":"w1"}},
            "flowSource":"fn original() {}","messages":[{"id":"m1","text":"é🙂"}]});
        sqlx::query("INSERT INTO runs VALUES(?,?)")
            .bind(id)
            .bind(value.to_string())
            .execute(&pool)
            .await
            .unwrap();
        let output = data.join("runs").join(id).join("tool-output");
        tokio::fs::create_dir_all(&output).await.unwrap();
        tokio::fs::write(output.join("full.bin"), [0, 255, 128, 65])
            .await
            .unwrap();
    }
    let expected = session_store::load(&pool, &keep).await.unwrap();
    pool.close().await;
    let held = migration::lock(&data).unwrap();
    assert!(
        migration::maintain(&data, &keep, 200, &Validator)
            .await
            .is_err()
    );
    drop(held);
    let report = migration::maintain(&data, &keep, 200, &Validator)
        .await
        .unwrap();
    assert_eq!(report.removed_session_ids, vec![old.clone()]);
    assert!(report.retained_session_ids.contains(&new));
    assert!(
        report
            .backup
            .join("runs")
            .join(&old)
            .join("tool-output/full.bin")
            .exists()
    );
    assert!(!data.join("runs").join(&old).exists());
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(data.join("zedflow.db")))
        .await
        .unwrap();
    let actual = session_store::load(&pool, &keep).await.unwrap();
    assert_eq!(actual["state"], expected["state"]);
    assert_eq!(actual["flowSource"], expected["flowSource"]);
    assert_eq!(actual["messages"], expected["messages"]);
    let store = ContentStore::from_pool(pool.clone());
    assert_eq!(
        store
            .record(&keep, "tool-output", "legacy:full.bin")
            .await
            .unwrap()
            .unwrap()["content"]["chunks"],
        json!(["AP+AQQ=="])
    );
    assert!(session_store::load(&pool, &new).await.is_ok());
    pool.close().await;
    migration::recover(&data).await.unwrap();
}

#[tokio::test]
async fn portable_session_archive_preserves_content_and_import_is_idempotent_without_effects() {
    use zf_storage::{
        contracts::{ArchiveRuntime, ArchiveSnapshot, CheckpointCodec, DependencyInspection},
        session_archive, session_store,
        workspaces::Workspace,
    };
    struct FixtureRuntime;
    impl CheckpointCodec for FixtureRuntime {
        fn decode_legacy_checkpoint(
            &self,
            value: serde_json::Value,
        ) -> anyhow::Result<serde_json::Value> {
            self.validate_checkpoint(&value)?;
            Ok(value)
        }
        fn validate_checkpoint(&self, value: &serde_json::Value) -> anyhow::Result<()> {
            anyhow::ensure!(value["state"].is_object(), "invalid fixture checkpoint");
            Ok(())
        }
    }
    impl ArchiveRuntime for FixtureRuntime {
        fn definition_diagnostics(&self, run: &serde_json::Value) -> Vec<String> {
            if run["flowSource"] == "fn fixture() {}" {
                vec![]
            } else {
                vec!["unknown fixture source".into()]
            }
        }
        fn dependencies(&self, _: &serde_json::Value) -> DependencyInspection {
            // This fixture has no external resources or readers.
            DependencyInspection {
                blocked: vec![],
                resources: vec![],
            }
        }
        fn resumable_internal_receipt<'a>(
            &'a self,
            _: &'a ContentStore,
            _: ArchiveSnapshot<'a>,
            _: &'a serde_json::Value,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<bool>> + Send + 'a>>
        {
            // No fixture tool protocol is permitted to resume an uncertain effect.
            Box::pin(async { Ok(false) })
        }
    }
    async fn database(path: &std::path::Path) -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::query("CREATE TABLE runs(id TEXT PRIMARY KEY,document TEXT NOT NULL)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE events(seq INTEGER PRIMARY KEY AUTOINCREMENT,run TEXT NOT NULL,document TEXT NOT NULL)").execute(&pool).await.unwrap();
        session_store::initialize(&pool).await.unwrap();
        ContentStore::new(pool.clone()).await.unwrap();
        pool
    }
    let dir = tempfile::tempdir().unwrap();
    let from = dir.path().join("from");
    let to = dir.path().join("to");
    tokio::fs::create_dir(&from).await.unwrap();
    tokio::fs::create_dir(&to).await.unwrap();
    let workspace = Workspace {
        id: "source-workspace".into(),
        name: "source".into(),
        path: from.clone(),
        open: true,
    };
    let target = Workspace {
        id: "target-workspace".into(),
        name: "target".into(),
        path: to.clone(),
        open: true,
    };
    let db = database(&from.join("data.db")).await;
    let imported = database(&to.join("data.db")).await;
    let id = uuid::Uuid::new_v4().to_string();
    let run = json!({"id":id,"workspaceId":workspace.id,"workspacePath":from,"status":"completed",
        "state":{"history":[{"text":"é🙂"}],"literal":"do not rewrite /source/command"},
        "context":{"cwd":from,"instructions":[],"skills":[]},
        "composition":{"id":"fixture","name":"fixture","nodes":[],"edges":[]},"flowSource":"fn fixture() {}",
        "messages":[{"id":"m1","text":"é🙂"}]});
    session_store::save(&db, &id, &run).await.unwrap();
    let checkpoints =
        zf_storage::contracts::CheckpointStore::new(ContentStore::from_pool(db.clone()))
            .await
            .unwrap();
    let checkpoint_id = uuid::Uuid::new_v4().to_string();
    let checkpoint = json!({"checkpoint_id":checkpoint_id,"thread_id":id,"state":{"history":[{"text":"é🙂"}]},
        "step":2,"pending_nodes":[],"created_at":"2026-09-15T12:00:00Z","attempts":{"model":2},
        "child_ledger":{"child":{"effect":"already done"}},"cleared_interrupt":"inbox", "metadata":{"unknown":[1,true,null]},
        "future_extension":{"keep":"exact"}});
    checkpoints.save(&checkpoint).await.unwrap();
    let result = session_archive::export_sessions(
        &db,
        &from,
        &workspace,
        std::slice::from_ref(&id),
        &FixtureRuntime,
    )
    .await
    .unwrap();
    let archive = &result.exports[0].path;
    let first = session_archive::import_sessions(&imported, &to, &target, archive, &FixtureRuntime)
        .await
        .unwrap();
    assert_eq!(first.imported, 1);
    let restored_checkpoints =
        zf_storage::contracts::CheckpointStore::new(ContentStore::from_pool(imported.clone()))
            .await
            .unwrap();
    assert_eq!(
        restored_checkpoints
            .load_by_id(&checkpoint_id)
            .await
            .unwrap(),
        Some(checkpoint.clone())
    );
    assert_eq!(first.runs[0]["state"], run["state"]);
    assert_eq!(first.runs[0]["messages"], run["messages"]);
    assert_eq!(first.runs[0]["flowSource"], run["flowSource"]);
    assert_eq!(first.runs[0]["workspaceId"], "target-workspace");
    assert_eq!(first.runs[0]["import"]["resumeBlocked"], json!([]));
    let again = session_archive::import_sessions(&imported, &to, &target, archive, &FixtureRuntime)
        .await
        .unwrap();
    assert_eq!((again.imported, again.unchanged), (0, 1));
    let bad = Workspace {
        id: "other".into(),
        ..workspace.clone()
    };
    assert!(
        session_archive::export_sessions(
            &db,
            &from,
            &bad,
            std::slice::from_ref(&id),
            &FixtureRuntime
        )
        .await
        .is_err()
    );
    let mut changed = run.clone();
    changed["messages"][0]["text"] = json!("different");
    session_store::save(&db, &id, &changed).await.unwrap();
    let updated = session_archive::export_sessions(
        &db,
        &from,
        &workspace,
        std::slice::from_ref(&id),
        &FixtureRuntime,
    )
    .await
    .unwrap();
    assert!(
        session_archive::import_sessions(
            &imported,
            &to,
            &target,
            &updated.exports[0].path,
            &FixtureRuntime
        )
        .await
        .is_err()
    );
    assert_eq!(
        session_store::load(&imported, &id).await.unwrap()["messages"],
        run["messages"]
    );

    // A v1 archive contains complete checkpoints in JSONL, not CAS references.
    // Construct that historical format independently of the current exporter.
    async fn legacy_archive(
        path: &std::path::Path,
        run: serde_json::Value,
        checkpoint: &serde_json::Value,
    ) {
        use sha2::{Digest, Sha256};
        let digest = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
        tokio::fs::create_dir(path).await.unwrap();
        let header = json!({"type":"zedflow-session","version":1,"zedflowVersion":env!("CARGO_PKG_VERSION"),
            "adkVersion":"2.2.0","run":run,"runtimeRoot":"/old/runtime","contextFiles":[],"resumeBlocked":[]});
        let bytes = format!(
            "{}\n{}\n",
            header,
            json!({"type":"checkpoint","checkpoint":checkpoint})
        )
        .into_bytes();
        let files = vec![session_archive::InventoryFile {
            path: "session.jsonl".into(),
            sha256: digest(&bytes),
            bytes: bytes.len() as u64,
        }];
        let manifest = session_archive::Manifest {
            format: "zedflow-session".into(),
            version: 1,
            session_id: run["id"].as_str().unwrap().into(),
            archive_hash: digest(&serde_json::to_vec(&files).unwrap()),
            files,
        };
        tokio::fs::write(path.join("session.jsonl"), bytes)
            .await
            .unwrap();
        tokio::fs::write(
            path.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .await
        .unwrap();
    }
    let legacy_id = uuid::Uuid::new_v4().to_string();
    let mut legacy_run = run.clone();
    legacy_run["id"] = json!(legacy_id);
    let mut legacy_checkpoint = checkpoint.clone();
    legacy_checkpoint["thread_id"] = json!(legacy_id);
    legacy_checkpoint["checkpoint_id"] = json!(uuid::Uuid::new_v4().to_string());
    let legacy_path = dir.path().join("legacy");
    legacy_archive(&legacy_path, legacy_run, &legacy_checkpoint).await;
    let legacy =
        session_archive::import_sessions(&imported, &to, &target, &legacy_path, &FixtureRuntime)
            .await
            .unwrap();
    assert_eq!(legacy.imported, 1);
    assert_eq!(
        restored_checkpoints
            .load_by_id(legacy_checkpoint["checkpoint_id"].as_str().unwrap())
            .await
            .unwrap(),
        Some(legacy_checkpoint)
    );
    assert_eq!(legacy.runs[0]["state"], run["state"]);
    let rejected_id = uuid::Uuid::new_v4().to_string();
    let mut bad_run = run.clone();
    bad_run["id"] = json!(rejected_id);
    let mut bad_checkpoint = checkpoint.clone();
    bad_checkpoint["thread_id"] = json!(rejected_id);
    bad_checkpoint["checkpoint_id"] = json!(uuid::Uuid::new_v4().to_string());
    bad_checkpoint["state"] = json!([]);
    let rejected_path = dir.path().join("bad-legacy");
    legacy_archive(&rejected_path, bad_run, &bad_checkpoint).await;
    let error =
        session_archive::import_sessions(&imported, &to, &target, &rejected_path, &FixtureRuntime)
            .await
            .unwrap_err();
    assert!(error.to_string().contains("invalid fixture checkpoint"));
    assert!(session_store::load(&imported, &rejected_id).await.is_err());
    assert!(!to.join("runs").join(&rejected_id).exists());
    db.close().await;
    imported.close().await;
}

#[tokio::test]
async fn live_definition_snapshots_are_workspace_scoped_and_do_not_hydrate_conversation() {
    use zf_storage::{live_files, session_store};
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query("CREATE TABLE runs(id TEXT PRIMARY KEY,document TEXT NOT NULL)")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE events(seq INTEGER PRIMARY KEY,run TEXT,document TEXT)")
        .execute(&db)
        .await
        .unwrap();
    session_store::initialize(&db).await.unwrap();
    let store = ContentStore::new(db.clone()).await.unwrap();
    let definition =
        json!({"key":"flow","hash":"frozen","source":"fn exact() {}","composition":{"id":"flow"}});
    let reference = store.intern(&definition).await.unwrap();
    let graph = json!({"flows":{"root":{"source":"fn initial() {}"}},"definitions":{"contextSelections":{"root/model":{"strategy":"a"}}}});
    let graph_ref = store.intern(&graph).await.unwrap();
    for (id, workspace, status) in [
        ("wanted", "a", "waiting"),
        ("foreign", "b", "waiting"),
        ("done", "a", "completed"),
    ] {
        let run = json!({"id":id,"workspaceId":workspace,"status":status,"runtimeGraphRef":graph_ref,
            "composition":{"id":"flow"},"flowSource":"fn initial() {}","flowRef":{"key":"flow"},
            "stateRef":"sha256:missing-state-not-to-be-loaded","messages":[{"contentRef":"sha256:missing-message"}]});
        sqlx::query("INSERT INTO runs VALUES(?,?)")
            .bind(id)
            .bind(run.to_string())
            .execute(&db)
            .await
            .unwrap();
        store
            .put_record(
                id,
                "revision-heads",
                "instance-key",
                &json!({"instance":"root","definitionRef":reference}),
            )
            .await
            .unwrap();
    }
    let current = json!({"flows":{"root":{"source":"fn current() {}"}}});
    let current_ref = store.intern(&current).await.unwrap();
    store
        .put_record(
            "wanted",
            "runtime-graph-heads",
            "current",
            &json!({"graphRef":current_ref}),
        )
        .await
        .unwrap();
    let snapshots = live_files::snapshots(&db, "a").await.unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].run_id, "wanted");
    assert_eq!(snapshots[0].runtime_graph, graph);
    assert_eq!(snapshots[0].flow_source, json!("fn initial() {}"));
    assert_eq!(
        snapshots[0].revision_heads["instance-key"].head["instance"],
        "root"
    );
    assert_eq!(
        snapshots[0].revision_heads["instance-key"].definition,
        definition
    );
    assert_eq!(
        snapshots[0].runtime_graph_head.as_ref().unwrap().definition,
        current
    );
    assert!(
        live_files::snapshots(&db, "absent")
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn window_edits_are_atomic_and_unique_publications_do_not_reapply_after_head_advances() {
    use zf_context::window::{PreparedWindow, WindowPatch};
    use zf_core::identity::{Permission, Scope};
    use zf_storage::data::{DataError, DataRegistry, WindowRegistry, WindowStoreError};
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let content = ContentStore::new(db.clone()).await.unwrap();
    let data = DataRegistry::new(db, content, "run").await.unwrap();
    let windows = WindowRegistry::new(data.clone());
    let scope = Scope::Flow("author".into());
    let read_scope = Scope::Flow("reader".into());
    let window:PreparedWindow=serde_json::from_value(json!({"strategyId":"strategy","strategyRevision":"frozen",
        "items":[{"kind":"fragment","id":"one","role":"data","format":"text","value":"first","sources":[]},
                 {"kind":"fragment","id":"two","role":"data","format":"text","value":"second","sources":[]}],
        "sourceRevisions":{},"capabilities":[]})).unwrap();
    let first = windows.create(&scope, "context", &window).await.unwrap();
    let bad = [
        WindowPatch::Remove { id: "one".into() },
        WindowPatch::Remove {
            id: "missing".into(),
        },
    ];
    assert!(
        windows
            .patch(&scope, "context", &first.revision, &bad)
            .await
            .is_err()
    );
    assert_eq!(
        windows.read(&scope, "context").await.unwrap().revision,
        first.revision
    );
    data.grant(&scope, "context", &read_scope, "context", Permission::Read)
        .await
        .unwrap();
    assert!(matches!(
        windows
            .patch(
                &read_scope,
                "context",
                &first.revision,
                &[WindowPatch::Remove { id: "one".into() }]
            )
            .await,
        Err(WindowStoreError::Data(DataError::PermissionDenied))
    ));
    let second = windows
        .patch_unique(
            &scope,
            "context",
            &first.revision,
            &[WindowPatch::Remove { id: "one".into() }],
            "patch-one",
        )
        .await
        .unwrap();
    let third = windows
        .patch(
            &scope,
            "context",
            &second.revision,
            &[WindowPatch::Remove { id: "two".into() }],
        )
        .await
        .unwrap();
    assert!(matches!(
        windows
            .patch(
                &scope,
                "context",
                &first.revision,
                &[WindowPatch::Remove { id: "one".into() }]
            )
            .await,
        Err(WindowStoreError::Data(DataError::Conflict { .. }))
    ));
    let replay = windows
        .patch_unique(
            &scope,
            "context",
            &first.revision,
            &[WindowPatch::Remove { id: "one".into() }],
            "patch-one",
        )
        .await
        .unwrap();
    assert_eq!(replay.revision, second.revision);
    assert_eq!(
        windows.read(&scope, "context").await.unwrap().revision,
        third.revision
    );
    assert_eq!(
        windows
            .revision(&scope, "context", &first.revision)
            .await
            .unwrap()
            .value,
        first.value
    );
}

#[tokio::test]
async fn examples_follow_complete_type_identity_across_sources() {
    use zf_core::types::{DataType, TypeRegistry};
    use zf_storage::source_catalog::examples;
    let workspace = tempfile::tempdir().unwrap();
    let kind = DataType::Named {
        name: "RouteResult".into(),
    };
    let types = TypeRegistry::from([("RouteResult".into(), DataType::Text)]);
    let saved = examples::save(
        workspace.path().into(),
        kind.clone(),
        types.clone(),
        "Documentation terminée".into(),
        json!("docs checked"),
    )
    .await
    .unwrap();
    let all = examples::list(workspace.path().into(), kind.clone(), types.clone())
        .await
        .unwrap();
    assert_eq!(all.len(), 2);
    assert!(
        all.iter()
            .any(|example| example.id == saved.id && example.value == json!("docs checked"))
    );
    let changed = TypeRegistry::from([("RouteResult".into(), DataType::Number)]);
    let different = examples::list(workspace.path().into(), kind.clone(), changed)
        .await
        .unwrap();
    assert_eq!(different.len(), 1);
    assert_eq!(different[0].id, "builtin");
    assert!(
        examples::save(
            workspace.path().into(),
            kind,
            types,
            "Invalid".into(),
            json!(42)
        )
        .await
        .is_err()
    );
    let catalog = examples::catalog(workspace.path().into()).await.unwrap();
    assert_eq!(catalog.len(), 1);
    assert!(catalog[0].diagnostics.is_empty());
}

#[tokio::test]
async fn definition_packages_preserve_examples_and_validate_bridges_before_installing() {
    use zf_context::context_package::{ArtifactKind, ArtifactSelection, SourceArtifact};
    use zf_core::types::{DataType, TypeRegistry};
    use zf_flows::{bridge_source, composition::BridgeDefinition};
    use zf_storage::{
        context_store::{self, packages},
        source_catalog::examples,
    };
    let source = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let saved = examples::save(
        source.path().into(),
        DataType::Text,
        TypeRegistry::new(),
        "Shared".into(),
        json!("exact payload"),
    )
    .await
    .unwrap();
    let selection = vec![ArtifactSelection {
        kind: ArtifactKind::Example,
        key: saved.id.clone(),
    }];
    let mut package = packages::export_selection(source.path().into(), &selection)
        .await
        .unwrap();
    assert_eq!(package.version, 2);
    let bridge =
        bridge_source::generate(&BridgeDefinition::new().import("docs", "working-system")).unwrap();
    package.artifacts.push(SourceArtifact {
        kind: ArtifactKind::Bridge,
        key: "docs".into(),
        hash: context_store::hash(bridge.as_bytes()),
        source: bridge,
    });
    let imported = packages::import_package(target.path().into(), &package)
        .await
        .unwrap();
    assert_eq!(imported.files.len(), 2);
    assert!(
        imported
            .prerequisites
            .iter()
            .any(|p| p.kind == "flow" && p.key == "working-system")
    );
    assert!(
        packages::import_package(target.path().into(), &package)
            .await
            .is_err()
    );
    let roundtrip = packages::export_selection(target.path().into(), &selection)
        .await
        .unwrap();
    assert_eq!(roundtrip.artifacts[0], package.artifacts[0]);
    let valid_bridge = package.artifacts[1].clone();
    let untouched = tempfile::tempdir().unwrap();
    package.artifacts[1].source = "fn bridge() { panic!(\"must never run\"); }".into();
    package.artifacts[1].hash = context_store::hash(package.artifacts[1].source.as_bytes());
    assert!(
        packages::import_package(untouched.path().into(), &package)
            .await
            .is_err()
    );
    assert!(!untouched.path().join(".zedflow").exists());
    package.artifacts[1] = valid_bridge;
    package.artifacts[0].hash = "wrong".into();
    assert!(
        packages::import_package(untouched.path().into(), &package)
            .await
            .is_err()
    );
}

async fn window_selection_fixture() -> (tempfile::TempDir, ContentStore) {
    let directory = tempfile::tempdir().unwrap();
    let options = SqliteConnectOptions::new()
        .filename(directory.path().join("selections.db"))
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Full);
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .unwrap();
    (directory, ContentStore::new(pool).await.unwrap())
}

fn window_selection_command(
    id: &str,
    path: &str,
) -> zf_context::window_preparation::WindowSelectionCommand {
    zf_context::window_preparation::WindowSelectionCommand {
        id: id.into(),
        node_path: path.into(),
        alias: "prepared".into(),
        revision: zf_core::identity::Revision::from("selected-revision"),
        program_hash: "frozen-program".into(),
    }
}

#[tokio::test]
async fn window_selection_identity_is_idempotent_and_unique_across_nodes_in_a_run() {
    use zf_storage::data::queue_window_selection;
    let (_directory, store) = window_selection_fixture().await;
    let command = window_selection_command("command-one", "root/agent");
    let expected = json!({"id":"command-one","nodePath":"root/agent","alias":"prepared",
        "revision":"selected-revision","programHash":"frozen-program"});
    assert_eq!(
        queue_window_selection(&store, "run", &command)
            .await
            .unwrap(),
        expected
    );
    assert_eq!(
        queue_window_selection(&store, "run", &command)
            .await
            .unwrap(),
        expected
    );
    for changed in [
        zf_context::window_preparation::WindowSelectionCommand {
            revision: "other-revision".into(),
            ..command.clone()
        },
        window_selection_command("command-one", "root/other-agent"),
    ] {
        let error = queue_window_selection(&store, "run", &changed)
            .await
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("identity reused with different arguments")
        );
    }
    let next = window_selection_command("command-two", "root/agent");
    assert!(
        queue_window_selection(&store, "run", &next)
            .await
            .unwrap_err()
            .to_string()
            .contains("unconsumed window selection")
    );
    assert!(
        queue_window_selection(&store, "other-run", &next)
            .await
            .is_ok()
    );
    assert!(
        queue_window_selection(
            &store,
            "run",
            &window_selection_command("command-three", "root/other-agent")
        )
        .await
        .is_ok()
    );
}

#[tokio::test]
async fn window_selection_claim_captures_absence_and_replays_across_reopen() {
    use zf_storage::data::{claim_window_selection, queue_window_selection};
    let (directory, store) = window_selection_fixture().await;
    let command = window_selection_command("command-one", "root/agent");
    assert!(
        claim_window_selection(&store, "run", "root/agent", "empty")
            .await
            .unwrap()
            .is_none()
    );
    queue_window_selection(&store, "run", &command)
        .await
        .unwrap();
    assert!(
        claim_window_selection(&store, "run", "root/agent", "empty")
            .await
            .unwrap()
            .is_none()
    );
    let captured = claim_window_selection(&store, "run", "root/agent", "first")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(captured.id, command.id);
    assert_eq!(captured.revision, command.revision);
    queue_window_selection(&store, "run", &command)
        .await
        .unwrap();
    let next = window_selection_command("command-two", "root/agent");
    queue_window_selection(&store, "run", &next).await.unwrap();
    store.pool().close().await;
    drop(store);
    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(SqliteConnectOptions::new().filename(directory.path().join("selections.db")))
        .await
        .unwrap();
    let reopened = ContentStore::new(pool).await.unwrap();
    assert!(
        claim_window_selection(&reopened, "run", "root/agent", "empty")
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        claim_window_selection(&reopened, "run", "root/agent", "first")
            .await
            .unwrap()
            .unwrap()
            .id,
        command.id
    );
    assert_eq!(
        claim_window_selection(&reopened, "run", "root/agent", "second")
            .await
            .unwrap()
            .unwrap()
            .id,
        next.id
    );
    assert!(
        claim_window_selection(&reopened, "run", "root/agent", "third")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn window_selection_concurrent_claims_consume_once_and_replay_their_capture() {
    use zf_storage::data::{claim_window_selection, queue_window_selection};
    let (_directory, store) = window_selection_fixture().await;
    let command = window_selection_command("command-one", "root/agent");
    queue_window_selection(&store, "run", &command)
        .await
        .unwrap();
    let (left, right) = tokio::join!(
        claim_window_selection(&store, "run", "root/agent", "left"),
        claim_window_selection(&store, "run", "root/agent", "right"),
    );
    let (left, right) = (left.unwrap(), right.unwrap());
    assert_ne!(left.is_some(), right.is_some());
    for (occurrence, original) in [("left", left), ("right", right)] {
        let retry = claim_window_selection(&store, "run", "root/agent", occurrence)
            .await
            .unwrap();
        assert_eq!(retry.map(|value| value.id), original.map(|value| value.id));
    }
    let next = window_selection_command("command-two", "root/agent");
    queue_window_selection(&store, "run", &next).await.unwrap();
    let (left, right) = tokio::join!(
        claim_window_selection(&store, "run", "root/agent", "shared"),
        claim_window_selection(&store, "run", "root/agent", "shared"),
    );
    assert_eq!(left.unwrap().unwrap().id, next.id);
    assert_eq!(right.unwrap().unwrap().id, next.id);
    assert!(
        claim_window_selection(&store, "run", "root/agent", "after-shared")
            .await
            .unwrap()
            .is_none()
    );
}
