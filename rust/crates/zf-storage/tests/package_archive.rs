//! Portable storage contracts; FixtureRuntime interprets only this fixture.
use serde_json::{Value, json};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::{collections::BTreeMap, path::Path};
use zf_flows::package::PackageSnapshot;
use zf_storage::{
    content_store::ContentStore,
    contracts::{ArchiveRuntime, ArchiveSnapshot, CheckpointCodec, DependencyInspection},
    session_archive, session_store,
    workspaces::Workspace,
};
const SOURCE: &str = "fn fixture() {}\n";
struct FixtureRuntime;
impl CheckpointCodec for FixtureRuntime {
    fn decode_legacy_checkpoint(&self, value: Value) -> anyhow::Result<Value> {
        self.validate_checkpoint(&value)?;
        Ok(value)
    }
    fn validate_checkpoint(&self, value: &Value) -> anyhow::Result<()> {
        anyhow::ensure!(value["state"].is_object(), "fixture state absent");
        Ok(())
    }
}
impl ArchiveRuntime for FixtureRuntime {
    fn definition_diagnostics(&self, run: &Value) -> Vec<String> {
        if run["flowSource"] == SOURCE {
            vec![]
        } else {
            vec!["fixture source absent".into()]
        }
    }
    fn dependencies(&self, _: &Value) -> DependencyInspection {
        DependencyInspection {
            blocked: vec![],
            resources: vec![],
        }
    }
    fn resumable_internal_receipt<'a>(
        &'a self,
        _: &'a ContentStore,
        _: ArchiveSnapshot<'a>,
        _: &'a Value,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<bool>> + Send + 'a>>
    {
        Box::pin(async { Ok(false) })
    }
}
async fn database(root: &Path, name: &str) -> (SqlitePool, Workspace) {
    let path = root.join(name);
    std::fs::create_dir_all(&path).unwrap();
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path.join("data.db"))
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::query("CREATE TABLE runs(id TEXT PRIMARY KEY,document TEXT NOT NULL)")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE events(seq INTEGER PRIMARY KEY AUTOINCREMENT,run TEXT NOT NULL,document TEXT NOT NULL)").execute(&db).await.unwrap();
    session_store::initialize(&db).await.unwrap();
    ContentStore::new(db.clone()).await.unwrap();
    (
        db,
        Workspace {
            id: name.into(),
            name: name.into(),
            path,
            open: true,
        },
    )
}
fn package() -> PackageSnapshot {
    let child=PackageSnapshot::capture(json!({"formatVersion":1,"id":"shared","name":"Shared","entry":"flow.rs","files":["flow.rs","data.bin"],"dependencies":{}}).to_string(),BTreeMap::from([("flow.rs".into(),SOURCE.as_bytes().to_vec()),("data.bin".into(),vec![0,255,42,128])]),BTreeMap::new()).unwrap();
    PackageSnapshot::capture(json!({"formatVersion":1,"id":"fixture","name":"Fixture","entry":"flow.rs","files":["flow.rs","README.md"],"dependencies":{"a":{"path":"../shared"},"b":{"path":"../shared"}}}).to_string(),BTreeMap::from([("flow.rs".into(),SOURCE.as_bytes().to_vec()),("README.md".into(),"Données figées é🙂\n".as_bytes().to_vec())]),BTreeMap::from([("a".into(),child.clone()),("b".into(),child)])).unwrap()
}
fn run(workspace: &Workspace, id: &str) -> Value {
    json!({"id":id,"workspaceId":workspace.id,"workspacePath":workspace.path,"status":"completed","state":{"history":[{"text":"é🙂"}],"literal":"sha256:not-a-reference"},"context":{"cwd":workspace.path,"instructions":[],"skills":[]},"composition":{"id":"fixture","name":"Fixture","nodes":[],"edges":[]},"flowSource":SOURCE,"messages":[]})
}

#[tokio::test]
async fn package_closure_survives_archive_move_without_original_catalogue_or_database() {
    let temp = tempfile::tempdir().unwrap();
    let (db, from) = database(temp.path(), "source").await;
    let (target, to) = database(temp.path(), "target").await;
    let id = uuid::Uuid::new_v4().to_string();
    let snapshot = package();
    assert_eq!(snapshot.packages.len(), 2);
    let mut doc = run(&from, &id);
    doc["flowPackage"] = serde_json::to_value(&snapshot).unwrap();
    doc["flowRef"] = json!({"key":"pkg:workspace:fixture","hash":snapshot.root});
    session_store::save(&db, &id, &doc).await.unwrap();
    let projection = session_store::load_projection(&db, &id).await.unwrap();
    assert!(projection["flowPackageRef"].is_string());
    assert!(projection["flowPackage"].is_null());
    let exported = session_archive::export_sessions(
        &db,
        &from.path,
        &from,
        std::slice::from_ref(&id),
        &FixtureRuntime,
    )
    .await
    .unwrap();
    let portable = temp.path().join("portable");
    std::fs::rename(&exported.exports[0].path, &portable).unwrap();
    db.close().await;
    std::fs::remove_dir_all(&from.path).unwrap();
    let imported =
        session_archive::import_sessions(&target, &to.path, &to, &portable, &FixtureRuntime)
            .await
            .unwrap();
    assert_eq!(imported.imported, 1);
    let recovered: PackageSnapshot =
        serde_json::from_value(imported.runs[0]["flowPackage"].clone()).unwrap();
    recovered.validate().unwrap();
    assert_eq!(recovered, snapshot);
    assert_eq!(imported.runs[0]["flowSource"], SOURCE);
    assert_eq!(imported.runs[0]["state"], doc["state"]);
    let store = ContentStore::from_pool(target.clone());
    let restored = session_store::load_projection(&target, &id).await.unwrap();
    assert_eq!(restored["flowPackageRef"], projection["flowPackageRef"]);
    let reference = restored["flowPackageRef"].as_str().unwrap();
    assert_eq!(store.resolve(reference).await.unwrap(), doc["flowPackage"]);
    let manifest: session_archive::Manifest =
        serde_json::from_slice(&std::fs::read(portable.join("manifest.json")).unwrap()).unwrap();
    let paths: std::collections::BTreeSet<_> = manifest.files.iter().map(|f| &f.path).collect();
    assert_eq!(paths.len(), manifest.files.len());
    let repeat =
        session_archive::import_sessions(&target, &to.path, &to, &portable, &FixtureRuntime)
            .await
            .unwrap();
    assert_eq!((repeat.imported, repeat.unchanged), (0, 1));
    target.close().await;
}

#[tokio::test]
async fn window_selection_command_identity_and_arguments_survive_archive_roundtrip() {
    let temp = tempfile::tempdir().unwrap();
    let (db, from) = database(temp.path(), "source").await;
    let (target, to) = database(temp.path(), "target").await;
    let id = uuid::Uuid::new_v4().to_string();
    session_store::save(&db, &id, &run(&from, &id))
        .await
        .unwrap();
    let store = ContentStore::from_pool(db.clone());
    let command_id = uuid::Uuid::new_v4().to_string();
    let command = json!({"id":command_id,"nodePath":"root/model","alias":"window","revision":"unchanged-revision","programHash":"frozen-program"});
    store
        .put_record(&id, "window-selection-commands", &command_id, &command)
        .await
        .unwrap();
    let exported = session_archive::export_sessions(
        &db,
        &from.path,
        &from,
        std::slice::from_ref(&id),
        &FixtureRuntime,
    )
    .await
    .unwrap();
    session_archive::import_sessions(
        &target,
        &to.path,
        &to,
        &exported.exports[0].path,
        &FixtureRuntime,
    )
    .await
    .unwrap();
    let target_store = ContentStore::from_pool(target.clone());
    let records = target_store
        .records_of_kind(&id, "window-selection-commands")
        .await
        .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].key, command_id);
    assert_eq!(
        target_store.resolve(&records[0].value_ref).await.unwrap(),
        command
    );
    db.close().await;
    target.close().await;
}

#[tokio::test]
async fn archive_hash_does_not_substitute_for_the_nested_package_revision() {
    let temp = tempfile::tempdir().unwrap();
    let (db, from) = database(temp.path(), "source").await;
    let (target, to) = database(temp.path(), "target").await;
    let id = uuid::Uuid::new_v4().to_string();
    let snapshot = package();
    let mut doc = run(&from, &id);
    let mut corrupted = serde_json::to_value(&snapshot).unwrap();
    corrupted["packages"][&snapshot.root]["files"]["README.md"] =
        json!("altered with unchanged package revision");
    doc["flowPackage"] = corrupted;
    session_store::save(&db, &id, &doc).await.unwrap();
    // Content-store and archive hashes are internally valid for these bytes;
    // the stale package revision must still be rejected before publication.
    let exported = session_archive::export_sessions(
        &db,
        &from.path,
        &from,
        std::slice::from_ref(&id),
        &FixtureRuntime,
    )
    .await
    .unwrap();
    let error = session_archive::import_sessions(
        &target,
        &to.path,
        &to,
        &exported.exports[0].path,
        &FixtureRuntime,
    )
    .await
    .unwrap_err();
    assert!(format!("{error:#}").contains("package"), "{error:#}");
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM runs")
        .fetch_one(&target)
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert!(!to.path.join("runs").join(&id).exists());
    assert!(!to.path.join(".session-imports").exists());
    db.close().await;
    target.close().await;
}
