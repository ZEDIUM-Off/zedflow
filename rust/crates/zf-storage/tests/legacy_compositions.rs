use serde_json::json;
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use std::{path::PathBuf, sync::Arc};
use zf_flows::schema::Composition;
use zf_storage::{
    flow_store::FlowStore,
    legacy_compositions::{self, ImportReceipt},
};

struct Fixture {
    _root: tempfile::TempDir,
    data: PathBuf,
    workspace: PathBuf,
    home: PathBuf,
    db: SqlitePool,
    flows: FlowStore,
}
fn doc(id: &str) -> serde_json::Value {
    json!({"formatVersion":3,"id":id,"name":id,"revision":19,
        "nodes":[{"id":"start","position":{"x":0,"y":0},"data":{"kind":"start","label":"Start"}},
        {"id":"end","position":{"x":100,"y":0},"data":{"kind":"end","label":"End"}}],
        "edges":[{"id":"done","source":"start","target":"end"}]})
}
impl Fixture {
    async fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("data");
        let workspace = root.path().join("workspace");
        let home = root.path().join("home");
        for path in [&data, &workspace, &home] {
            std::fs::create_dir(path).unwrap();
        }
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(data.join("zedflow.db"))
                    .create_if_missing(true)
                    .journal_mode(SqliteJournalMode::Wal),
            )
            .await
            .unwrap();
        sqlx::query("PRAGMA wal_autocheckpoint=0")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE compositions(id TEXT,document TEXT NOT NULL)")
            .execute(&db)
            .await
            .unwrap();
        for name in ["sessions", "events", "checkpoints", "receipts"] {
            sqlx::query(&format!("CREATE TABLE {name}(payload BLOB)"))
                .execute(&db)
                .await
                .unwrap();
            sqlx::query(&format!("INSERT INTO {name} VALUES(X'000102FF')"))
                .execute(&db)
                .await
                .unwrap();
        }
        let flows = FlowStore::new(home.clone(), Arc::new(|_: &Composition| Ok(())));
        Self {
            _root: root,
            data,
            workspace,
            home,
            db,
            flows,
        }
    }
    async fn add(&self, id: &str) {
        sqlx::query("INSERT INTO compositions VALUES(?,?)")
            .bind(id)
            .bind(doc(id).to_string())
            .execute(&self.db)
            .await
            .unwrap();
    }
    async fn import(&self, dry: bool) -> anyhow::Result<ImportReceipt> {
        legacy_compositions::import(&self.data, &self.workspace, &self.home, &self.flows, dry).await
    }
    async fn startup(&self) -> anyhow::Result<()> {
        legacy_compositions::require_imported(&self.db, &self.data).await
    }
    fn package(&self, id: &str) -> PathBuf {
        self.workspace.join(".zedflow/flow").join(id)
    }
    fn pending(&self, receipt: &ImportReceipt) {
        let mut value = receipt.clone();
        value.complete = false;
        std::fs::write(
            self.data.join("legacy-compositions-import.json"),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }
    fn no_publication(&self) {
        assert!(!self.workspace.join(".zedflow").exists());
        assert!(!self.data.join("legacy-compositions-import.json").exists());
        assert!(!std::fs::read_dir(&self.data).unwrap().any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("before-file-flows")
        }));
    }
}

#[tokio::test]
async fn explicit_import_snapshots_wal_preserves_rows_and_never_resurrects_deleted_packages() {
    let f = Fixture::new().await;
    f.add("legacy").await;
    assert!(f.data.join("zedflow.db-wal").metadata().unwrap().len() > 0);
    assert!(
        f.startup()
            .await
            .unwrap_err()
            .to_string()
            .contains("migrate-compositions")
    );
    let dry = f.import(true).await.unwrap();
    assert!(!dry.complete && dry.backup.is_none());
    f.no_publication();
    let value = f.import(false).await.unwrap();
    assert!(value.complete);
    let backup = SqlitePool::connect(&format!(
        "sqlite://{}?mode=ro",
        value.backup.as_ref().unwrap().display()
    ))
    .await
    .unwrap();
    let raw: String = sqlx::query_scalar("SELECT document FROM compositions")
        .fetch_one(&backup)
        .await
        .unwrap();
    assert_eq!(raw, doc("legacy").to_string());
    for name in ["sessions", "events", "checkpoints", "receipts"] {
        let sql = format!("SELECT hex(payload) FROM {name}");
        let before: String = sqlx::query_scalar(&sql).fetch_one(&backup).await.unwrap();
        let after: String = sqlx::query_scalar(&sql).fetch_one(&f.db).await.unwrap();
        assert_eq!(before, "000102FF");
        assert_eq!(before, after);
    }
    let raw_after: String = sqlx::query_scalar("SELECT document FROM compositions")
        .fetch_one(&f.db)
        .await
        .unwrap();
    assert_eq!(raw_after, raw);
    assert!(f.package("legacy").join("flow.rs").is_file());
    assert!(!f.workspace.join(".zedflow/flows").exists());
    f.startup().await.unwrap();
    std::fs::remove_dir_all(f.package("legacy")).unwrap();
    assert_eq!(f.import(false).await.unwrap(), value);
    assert!(!f.package("legacy").exists());
    f.startup().await.unwrap();
    backup.close().await;
}

#[tokio::test]
async fn all_rows_are_validated_before_any_package_backup_or_receipt() {
    let f = Fixture::new().await;
    f.add("a-valid").await;
    sqlx::query("INSERT INTO compositions VALUES('z-invalid','not json')")
        .execute(&f.db)
        .await
        .unwrap();
    assert!(f.import(false).await.is_err());
    f.no_publication();
    sqlx::query("DELETE FROM compositions WHERE id='z-invalid'")
        .execute(&f.db)
        .await
        .unwrap();
    f.add("a-valid").await;
    assert!(
        f.import(false)
            .await
            .unwrap_err()
            .to_string()
            .contains("duplicate")
    );
    f.no_publication();
}

#[tokio::test]
async fn pending_import_resumes_missing_packages_and_blocks_startup_until_complete() {
    let f = Fixture::new().await;
    f.add("a").await;
    f.add("b").await;
    let value = f.import(false).await.unwrap();
    let first = std::fs::read(f.package("a").join("flow.rs")).unwrap();
    f.pending(&value);
    std::fs::remove_dir_all(f.package("b")).unwrap();
    assert!(
        f.startup()
            .await
            .unwrap_err()
            .to_string()
            .contains("incomplete")
    );
    assert_eq!(f.import(false).await.unwrap(), value);
    assert_eq!(
        std::fs::read(f.package("a").join("flow.rs")).unwrap(),
        first
    );
    assert!(f.package("b").is_dir());
    f.startup().await.unwrap();
}

#[tokio::test]
async fn pending_import_preserves_external_edit_and_refuses_changed_source() {
    let f = Fixture::new().await;
    f.add("a").await;
    f.add("b").await;
    let value = f.import(false).await.unwrap();
    f.pending(&value);
    std::fs::remove_dir_all(f.package("b")).unwrap();
    std::fs::write(f.package("a").join("flow.rs"), "externally edited").unwrap();
    assert!(
        f.import(false)
            .await
            .unwrap_err()
            .to_string()
            .contains("collision")
    );
    assert_eq!(
        std::fs::read_to_string(f.package("a").join("flow.rs")).unwrap(),
        "externally edited"
    );
    assert!(!f.package("b").exists());
    f.add("c").await;
    assert!(
        f.import(false)
            .await
            .unwrap_err()
            .to_string()
            .contains("source or roots changed")
    );
}

#[tokio::test]
async fn fresh_collision_in_global_legacy_catalogue_is_rejected_without_backup() {
    let f = Fixture::new().await;
    f.add("same").await;
    let dir = f.home.join(".agents/flows");
    std::fs::create_dir_all(&dir).unwrap();
    let source = zf_flows::flow_source::render(
        &serde_json::from_value(doc("same")).unwrap(),
        &|_: &Composition| Ok(()),
    )
    .unwrap();
    std::fs::write(dir.join("unrelated-name.rs"), &source).unwrap();
    assert!(
        f.import(false)
            .await
            .unwrap_err()
            .to_string()
            .contains("collision")
    );
    f.no_publication();
    assert_eq!(
        std::fs::read_to_string(dir.join("unrelated-name.rs")).unwrap(),
        source
    );
}

#[tokio::test]
async fn offline_owner_and_backup_tampering_are_refused() {
    let f = Fixture::new().await;
    f.add("one").await;
    let owner = zf_storage::migration::lock(&f.data).unwrap();
    assert!(
        f.import(false)
            .await
            .unwrap_err()
            .to_string()
            .contains("daemon")
    );
    f.no_publication();
    drop(owner);
    let value = f.import(false).await.unwrap();
    std::fs::write(value.backup.unwrap(), "corrupted").unwrap();
    assert!(
        f.import(false)
            .await
            .unwrap_err()
            .to_string()
            .contains("backup changed")
    );
}

#[tokio::test]
async fn pending_import_reuses_existing_package_journal_after_interrupted_finish() {
    let f = Fixture::new().await;
    f.add("one").await;
    let receipt = f.import(false).await.unwrap();
    f.pending(&receipt);
    std::fs::remove_dir_all(f.package("one")).unwrap();
    let workspace = zf_storage::workspaces::Workspace {
        id: zf_storage::workspaces::path_id(&f.workspace),
        name: "fixture".into(),
        path: f.workspace.clone(),
        open: false,
    };
    let plan = f
        .flows
        .plan(
            &workspace,
            serde_json::from_value(doc("one")).unwrap(),
            "workspace",
            None,
            None,
        )
        .await
        .unwrap();
    let pending = zf_storage::flow_packages::begin(zf_storage::flow_packages::PackageWrite {
        workspace: plan.lock_workspace,
        target: plan.path,
        snapshot: plan.package,
        expected_revision: None,
        publication: None,
        preconditions: plan.preconditions,
    })
    .await
    .unwrap();
    drop(pending); // installation finished, its durable finalization has not run
    let marker = f.workspace.join(".zedflow/.package-acceptance.json");
    assert!(marker.exists());
    assert!(f.import(true).await.is_err()); // previews never recover publications
    assert!(marker.exists());
    assert_eq!(f.import(false).await.unwrap(), receipt);
    assert!(!marker.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn symlink_database_and_destination_are_rejected_without_following_them() {
    let f = Fixture::new().await;
    f.add("one").await;
    let outside = f._root.path().join("outside.sqlite");
    let original = f.data.join("zedflow.db");
    f.db.close().await;
    std::fs::rename(&original, &outside).unwrap();
    std::os::unix::fs::symlink(&outside, &original).unwrap();
    assert!(
        f.import(false)
            .await
            .unwrap_err()
            .to_string()
            .contains("regular")
    );
    f.no_publication();
    std::fs::remove_file(&original).unwrap();
    std::fs::rename(&outside, &original).unwrap();
    std::fs::create_dir_all(f.package("one").parent().unwrap()).unwrap();
    let target = f._root.path().join("outside-directory");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("sentinel"), "untouched").unwrap();
    std::os::unix::fs::symlink(&target, f.package("one")).unwrap();
    assert!(f.import(false).await.is_err());
    assert_eq!(
        std::fs::read_to_string(target.join("sentinel")).unwrap(),
        "untouched"
    );
    assert!(!f.data.join("legacy-compositions-import.json").exists());
}

#[tokio::test]
async fn pending_import_refuses_another_authors_package_journal() {
    let f = Fixture::new().await;
    f.add("one").await;
    let receipt = f.import(false).await.unwrap();
    f.pending(&receipt);
    let workspace = zf_storage::workspaces::Workspace {
        id: zf_storage::workspaces::path_id(&f.workspace),
        name: "fixture".into(),
        path: f.workspace.clone(),
        open: false,
    };
    let plan = f
        .flows
        .plan(
            &workspace,
            serde_json::from_value(doc("another")).unwrap(),
            "workspace",
            None,
            None,
        )
        .await
        .unwrap();
    let pending = zf_storage::flow_packages::begin(zf_storage::flow_packages::PackageWrite {
        workspace: plan.lock_workspace,
        target: plan.path,
        snapshot: plan.package,
        expected_revision: None,
        publication: None,
        preconditions: plan.preconditions,
    })
    .await
    .unwrap();
    drop(pending);
    let marker = f.workspace.join(".zedflow/.package-acceptance.json");
    let before = std::fs::read(&marker).unwrap();
    assert!(
        f.import(false)
            .await
            .unwrap_err()
            .to_string()
            .contains("does not belong")
    );
    assert_eq!(std::fs::read(marker).unwrap(), before);
}
