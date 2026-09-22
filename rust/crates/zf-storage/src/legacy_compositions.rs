//! Explicit offline import of SQLite definitions. The source table and sessions
//! remain untouched. Binary rollback still requires the complete pre-upgrade snapshot.
use crate::{
    flow_packages,
    flow_store::{FlowStore, FlowWrite, hash},
    migration,
    workspaces::{Workspace, path_id},
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use zf_flows::schema::Composition;

const RECEIPT: &str = "legacy-compositions-import.json";
const LIMIT: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportReceipt {
    pub format_version: u32,
    pub data: PathBuf,
    pub workspace: PathBuf,
    pub flow_home: PathBuf,
    pub source_hash: String,
    pub packages: BTreeMap<String, String>,
    pub backup: Option<PathBuf>,
    pub backup_hash: Option<String>,
    pub complete: bool,
}

fn directory(path: &Path) -> Result<PathBuf> {
    ensure!(
        path.is_absolute(),
        "Import requires absolute data/workspace/flow-home paths"
    );
    let canonical = std::fs::canonicalize(path)?;
    ensure!(
        canonical == path && path.is_dir(),
        "Import roots must be ordinary canonical directories without symlinks"
    );
    Ok(canonical)
}
fn regular(path: &Path) -> Result<()> {
    let m = std::fs::symlink_metadata(path)?;
    ensure!(
        m.is_file() && !m.file_type().is_symlink(),
        "Import file must be regular: {}",
        path.display()
    );
    Ok(())
}
async fn open(path: &Path, readonly: bool) -> Result<SqlitePool> {
    regular(path)?;
    for suffix in ["-wal", "-shm"] {
        let sibling = PathBuf::from(format!("{}{suffix}", path.display()));
        if sibling.symlink_metadata().is_ok() {
            regular(&sibling)?;
        }
    }
    Ok(SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(false)
                .read_only(readonly)
                .busy_timeout(std::time::Duration::from_secs(1)),
        )
        .await?)
}
async fn rows(db: &SqlitePool) -> Result<Vec<(String, String)>> {
    let exists: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='compositions'",
    )
    .fetch_one(db)
    .await?;
    if exists == 0 {
        return Ok(Vec::new());
    }
    let (count, bytes): (i64, i64) = sqlx::query_as("SELECT count(*), COALESCE(sum(length(CAST(id AS BLOB))+length(CAST(document AS BLOB))),0) FROM compositions").fetch_one(db).await?;
    ensure!(
        count <= 4096 && bytes <= LIMIT as i64,
        "Legacy compositions exceed import limits"
    );
    Ok(
        sqlx::query_as("SELECT id,document FROM compositions ORDER BY id")
            .fetch_all(db)
            .await?,
    )
}
fn fingerprint(rows: &[(String, String)]) -> Result<String> {
    Ok(hash(&serde_json::to_vec(rows)?))
}
async fn receipt(data: &Path) -> Result<Option<ImportReceipt>> {
    let path = data.join(RECEIPT);
    if !tokio::fs::try_exists(&path).await? {
        return Ok(None);
    }
    regular(&path)?;
    ensure!(
        tokio::fs::metadata(&path).await?.len() <= LIMIT,
        "Import receipt exceeds size limit"
    );
    let value: ImportReceipt = serde_json::from_slice(&tokio::fs::read(path).await?)?;
    ensure!(
        value.format_version == 1 && value.data == data,
        "Import receipt identity/version mismatch"
    );
    Ok(Some(value))
}
async fn save(value: &ImportReceipt) -> Result<()> {
    let target = value.data.join(RECEIPT);
    if target.symlink_metadata().is_ok() {
        regular(&target)?;
    }
    let temp = value
        .data
        .join(format!(".legacy-import-{}.pending", uuid::Uuid::new_v4()));
    let mut file = tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)
        .await?;
    file.write_all(&serde_json::to_vec_pretty(value)?).await?;
    file.sync_all().await?;
    tokio::fs::rename(&temp, target).await?;
    sync_dir(&value.data).await
}
async fn sync_dir(path: &Path) -> Result<()> {
    #[cfg(unix)]
    tokio::fs::File::open(path).await?.sync_all().await?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
async fn file_hash(path: &Path) -> Result<String> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}
async fn verify_backup(value: &ImportReceipt, expected: &[(String, String)]) -> Result<()> {
    let path = value
        .backup
        .as_ref()
        .context("Import receipt lacks backup")?;
    ensure!(
        path.parent() == Some(value.data.as_path()),
        "Import backup is outside data directory"
    );
    regular(path)?;
    ensure!(
        Some(file_hash(path).await?) == value.backup_hash,
        "Import backup changed"
    );
    let db = open(path, true).await?;
    let checks: Vec<String> = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_all(&db)
        .await?;
    let saved = rows(&db).await?;
    db.close().await;
    ensure!(
        checks == ["ok"] && saved == expected,
        "Import backup integrity/source mismatch"
    );
    Ok(())
}

/// Called before startup initializes sessions or admits execution. Completed
/// imports ignore later package edits; they never resurrect deleted definitions.
pub async fn require_imported(db: &SqlitePool, data: &Path) -> Result<()> {
    let source = rows(db).await?;
    if let Some(value) = receipt(data).await? {
        ensure!(
            value.complete,
            "Legacy import is incomplete; stop the daemon and resume `zf --workspace <absolute-workspace> migrate-compositions --data <absolute-data> --flow-home <absolute-home>`"
        );
        ensure!(
            value.source_hash == fingerprint(&source)?,
            "Legacy import source changed; inspect the import receipt before startup"
        );
    } else {
        ensure!(
            source.is_empty(),
            "Legacy SQLite compositions require explicit offline import: `zf --workspace <absolute-workspace> migrate-compositions --data <absolute-data> --flow-home <absolute-home>`; preserve a complete snapshot for rollback"
        );
    }
    Ok(())
}

async fn audit(
    flows: &FlowStore,
    workspace: &Workspace,
    plans: &[FlowWrite],
    resume: bool,
) -> Result<Vec<bool>> {
    let catalogue = flows.inspect_catalog(workspace).await?;
    let mut present = Vec::new();
    for plan in plans {
        let matches: Vec<_> = catalogue
            .iter()
            .filter(|f| f.id == plan.composition.id)
            .collect();
        ensure!(
            matches.is_empty()
                || (resume
                    && matches.len() == 1
                    && matches[0].path == plan.path
                    && matches[0].package.as_ref() == Some(&plan.package)
                    && matches[0].diagnostics.is_empty()),
            "Legacy import collision for flow {}; existing bytes preserved",
            plan.composition.id
        );
        ensure!(
            !plan.path.exists() || !matches.is_empty(),
            "Legacy import target already exists: {}",
            plan.path.display()
        );
        present.push(!matches.is_empty());
    }
    Ok(present)
}

/// No database/schema write, model call, session conversion, or legacy-file edit.
/// The existing data ownership lock is held until all publications are verified.
/// Dry runs may acquire existing catalogue locks, but publish no backup/receipt/flow.
pub async fn import(
    data: &Path,
    workspace: &Path,
    flow_home: &Path,
    flows: &FlowStore,
    dry_run: bool,
) -> Result<ImportReceipt> {
    ensure!(
        cfg!(target_os = "linux"),
        "Legacy package import requires Linux atomic publication"
    );
    let data = directory(data)?;
    let workspace_path = directory(workspace)?;
    let flow_home = directory(flow_home)?;
    let _owner = migration::lock(&data)?;
    let path = data.join("zedflow.db");
    let db = open(&path, true).await?;
    // Keep an immediate transaction on a separate connection to bar legacy SQL
    // writers throughout backup/publication. No source statement changes data.
    let writable = open(&path, false).await?;
    let transaction = writable.begin_with("BEGIN IMMEDIATE").await?;
    let source = rows(&db).await?;
    let source_hash = fingerprint(&source)?;
    let workspace = Workspace {
        id: path_id(&workspace_path),
        name: "Legacy import".into(),
        path: workspace_path.clone(),
        open: false,
    };
    let previous = receipt(&data).await?;
    if let Some(value) = &previous {
        ensure!(
            value.workspace == workspace_path
                && value.flow_home == flow_home
                && value.source_hash == source_hash,
            "Legacy import source or roots changed; existing packages preserved"
        );
        verify_backup(value, &source).await?;
        if value.complete {
            return Ok(value.clone());
        }
    }
    // Validate every row before reading/recovering catalogues or creating backups.
    let mut plans = Vec::new();
    let mut packages = BTreeMap::new();
    for (id, raw) in &source {
        let doc: Composition = serde_json::from_str(raw)
            .with_context(|| format!("Invalid legacy composition {id}"))?;
        ensure!(
            doc.id == *id && !packages.contains_key(id),
            "Legacy row identity mismatch/duplicate: {id}"
        );
        let plan = flows.plan(&workspace, doc, "workspace", None, None).await?;
        packages.insert(id.clone(), plan.package.root.clone());
        plans.push(plan);
    }
    if let Some(value) = &previous {
        ensure!(value.packages == packages, "Legacy import plan changed");
    }
    // Only a create-only package journal matching this pending receipt may be
    // recovered. Previews and unrelated authoring journals stay untouched.
    for root in [&workspace.path, &flow_home] {
        for marker in [
            ".package-lifecycle.json",
            ".package-lifecycle-participant.json",
            ".source-acceptance.json",
            ".source-import.json",
        ] {
            ensure!(
                root.join(".zedflow")
                    .join(marker)
                    .symlink_metadata()
                    .is_err(),
                "Pending authoring publication requires recovery before import"
            );
        }
        if root
            .join(".zedflow/.package-acceptance.json")
            .symlink_metadata()
            .is_ok()
        {
            ensure!(
                previous.is_some() && !dry_run && root == &workspace.path,
                "Pending authoring publication requires recovery before import"
            );
            flow_packages::recover_import(workspace.path.clone(), packages.clone()).await?;
        }
    }
    audit(flows, &workspace, &plans, previous.is_some()).await?;
    let mut value = previous.clone().unwrap_or(ImportReceipt {
        format_version: 1,
        data: data.clone(),
        workspace: workspace_path,
        flow_home: flow_home.clone(),
        source_hash,
        packages,
        backup: None,
        backup_hash: None,
        complete: false,
    });
    if dry_run {
        return Ok(value);
    }
    if previous.is_none() {
        let backup = data.join(format!("before-file-flows-{}.sqlite", uuid::Uuid::new_v4()));
        sqlx::query("VACUUM INTO ?")
            .bind(backup.to_str().context("Backup path must be UTF-8")?)
            .execute(&db)
            .await?;
        tokio::fs::File::open(&backup).await?.sync_all().await?;
        sync_dir(&data).await?;
        value.backup_hash = Some(file_hash(&backup).await?);
        value.backup = Some(backup);
        verify_backup(&value, &source).await?;
        save(&value).await?;
    }
    for (index, plan) in plans.iter().enumerate() {
        let conditions = flow_packages::inspect_catalogue_preconditions(vec![
            workspace.path.clone(),
            flow_home.clone(),
        ])
        .await?;
        let present = audit(flows, &workspace, &plans, true).await?;
        if present[index] {
            continue;
        }
        flow_packages::begin_checked(
            flow_packages::PackageWrite {
                workspace: plan.lock_workspace.clone(),
                target: plan.path.clone(),
                snapshot: plan.package.clone(),
                expected_revision: None,
                publication: None,
                preconditions: plan.preconditions.clone(),
            },
            conditions,
        )
        .await?
        .finish()
        .await?;
    }
    let final_conditions = flow_packages::inspect_catalogue_preconditions(vec![
        workspace.path.clone(),
        flow_home.clone(),
    ])
    .await?;
    ensure!(
        audit(flows, &workspace, &plans, true)
            .await?
            .iter()
            .all(|p| *p),
        "Legacy import publication incomplete"
    );
    ensure!(
        fingerprint(&rows(&db).await?)? == value.source_hash,
        "Legacy import source changed"
    );
    let _catalogues = flow_packages::guard_catalogues(final_conditions).await?;
    value.complete = true;
    save(&value).await?;
    transaction.rollback().await?;
    db.close().await;
    writable.close().await;
    Ok(value)
}
