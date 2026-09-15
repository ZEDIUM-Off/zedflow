//! Explicit offline maintenance. The source directory is never edited in place.
use crate::{
    content_store::ContentStore,
    contracts::{CheckpointHeader, CheckpointStore, CheckpointValidation},
    session_store,
};
use anyhow::{Context, Result, ensure};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{SqlitePool, sqlite::SqlitePoolOptions};
use std::{
    collections::BTreeMap,
    fs::File,
    path::{Path, PathBuf},
};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MaintenanceReport {
    pub backup: PathBuf,
    pub removed_session_ids: Vec<String>,
    pub removed_session_evidence: BTreeMap<String, Value>,
    pub retained_session_ids: Vec<String>,
    pub verified_runs: usize,
    pub verified_events: usize,
    pub verified_checkpoints: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Marker {
    version: u32,
    data: PathBuf,
    staging: PathBuf,
    retired: PathBuf,
    staging_inventory: BTreeMap<String, String>,
    backup_inventory: BTreeMap<String, String>,
    report: MaintenanceReport,
}

fn absolute_data(data: &Path) -> Result<PathBuf> {
    let data = if data.is_absolute() {
        data.to_owned()
    } else {
        std::env::current_dir()?.join(data)
    };
    let name = data
        .file_name()
        .context("Le dossier de données doit avoir un nom")?;
    let parent = data.parent().context("Dossier parent absent")?;
    Ok(std::fs::canonicalize(parent)?.join(name))
}

fn sibling(data: &Path, suffix: &str) -> Result<PathBuf> {
    let name = data
        .file_name()
        .and_then(|name| name.to_str())
        .context("Nom de dossier non UTF-8")?;
    Ok(data.with_file_name(format!("{name}.{suffix}")))
}

/// Hold this file for the entire daemon lifetime, or the whole maintenance.
/// The lock lives outside the data directory so an atomic directory swap cannot
/// replace its inode and let a second process enter.
pub fn lock(data: &Path) -> Result<File> {
    let data = absolute_data(data)?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(sibling(&data, "lock")?)?;
    file.try_lock().map_err(|error| {
        anyhow::anyhow!(
            "Le dossier de données est utilisé par un daemon ou une maintenance : {error}"
        )
    })?;
    Ok(file)
}

async fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    tokio::fs::File::open(path).await?.sync_all().await?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

async fn json_file(path: &Path, value: &impl Serialize) -> Result<()> {
    let pending = path.with_extension(format!("{}.pending", Uuid::new_v4()));
    let mut file = tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&pending)
        .await?;
    file.write_all(&serde_json::to_vec_pretty(value)?).await?;
    file.sync_all().await?;
    tokio::fs::rename(&pending, path).await?;
    sync_directory(path.parent().context("Fichier sans parent")?).await
}

async fn pool(path: &Path, writable: bool) -> Result<SqlitePool> {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(path)
                .read_only(!writable)
                .create_if_missing(false),
        )
        .await
        .map_err(Into::into)
}

async fn integrity(path: &Path) -> Result<()> {
    let db = pool(path, false).await?;
    let rows: Vec<String> = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_all(&db)
        .await?;
    db.close().await;
    ensure!(
        rows == ["ok"],
        "Sauvegarde SQLite invalide : {} : {}",
        path.display(),
        rows.join(" ; ")
    );
    Ok(())
}

/// Snapshot SQLite through VACUUM INTO; copy every other owned entry exactly.
/// Symlinks are preserved as links and are never followed during the snapshot.
async fn copy_tree(source: &Path, destination: &Path, snapshot_sqlite: bool) -> Result<()> {
    tokio::fs::create_dir(destination).await?;
    let mut pending = vec![(source.to_owned(), destination.to_owned())];
    let mut directories = vec![destination.to_owned()];
    while let Some((source, target)) = pending.pop() {
        let mut entries = tokio::fs::read_dir(&source).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name();
            let name_text = name.to_string_lossy();
            let original = entry.path();
            let copied = target.join(&name);
            let kind = entry.file_type().await?;
            if kind.is_symlink() {
                let link = tokio::fs::read_link(&original).await?;
                #[cfg(unix)]
                tokio::fs::symlink(link, &copied).await?;
                #[cfg(not(unix))]
                anyhow::bail!("La sauvegarde des liens symboliques requiert Unix");
            } else if kind.is_dir() {
                tokio::fs::create_dir(&copied).await?;
                directories.push(copied.clone());
                pending.push((original, copied));
            } else {
                ensure!(
                    kind.is_file(),
                    "Fichier spécial dans les données : {}",
                    original.display()
                );
                // Every database copy is made from a consistent logical snapshot.
                // Its transient journal/SHM/WAL must not be applied to that copy.
                if snapshot_sqlite
                    && [".db-wal", ".db-shm", ".db-journal"]
                        .iter()
                        .any(|suffix| name_text.ends_with(suffix))
                {
                    continue;
                }
                if snapshot_sqlite && name_text.ends_with(".db") {
                    let db = pool(&original, false).await?;
                    sqlx::query("VACUUM INTO ?")
                        .bind(copied.to_str().context("Chemin non UTF-8")?)
                        .execute(&db)
                        .await?;
                    db.close().await;
                    integrity(&copied).await?;
                } else {
                    tokio::fs::copy(&original, &copied).await?;
                }
                tokio::fs::set_permissions(
                    &copied,
                    tokio::fs::metadata(&original).await?.permissions(),
                )
                .await?;
                tokio::fs::File::open(&copied).await?.sync_all().await?;
            }
        }
    }
    for directory in directories.into_iter().rev() {
        let relative = directory.strip_prefix(destination)?;
        tokio::fs::set_permissions(
            &directory,
            tokio::fs::metadata(source.join(relative))
                .await?
                .permissions(),
        )
        .await?;
        sync_directory(&directory).await?;
    }
    sync_directory(destination.parent().context("Destination sans parent")?).await
}

async fn inventory(root: &Path) -> Result<BTreeMap<String, String>> {
    let mut inventory = BTreeMap::new();
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        let mut entries = tokio::fs::read_dir(directory).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)?
                .to_str()
                .context("Chemin non UTF-8")?
                .to_owned();
            let kind = entry.file_type().await?;
            if kind.is_symlink() {
                inventory.insert(
                    relative,
                    format!("symlink:{}", tokio::fs::read_link(path).await?.display()),
                );
            } else if kind.is_dir() {
                pending.push(path);
            } else {
                ensure!(kind.is_file(), "Fichier spécial inattendu");
                use tokio::io::AsyncReadExt;
                let mut file = tokio::fs::File::open(path).await?;
                let mut hash = Sha256::new();
                let mut buffer = vec![0_u8; 65536];
                loop {
                    let n = file.read(&mut buffer).await?;
                    if n == 0 {
                        break;
                    }
                    hash.update(&buffer[..n]);
                }
                inventory.insert(relative, format!("sha256:{:x}", hash.finalize()));
            }
        }
    }
    Ok(inventory)
}

fn verify_marker(data: &Path, marker: &Marker) -> Result<()> {
    ensure!(
        marker.version == 1 && marker.data == data,
        "Marqueur de maintenance incompatible"
    );
    for (path, label) in [
        (&marker.staging, "staging"),
        (&marker.retired, "retired"),
        (&marker.report.backup, "backup"),
    ] {
        let prefix = sibling(data, &format!("{label}-"))?;
        let prefix = prefix
            .file_name()
            .and_then(|x| x.to_str())
            .context("Nom invalide")?;
        let name = path
            .file_name()
            .and_then(|x| x.to_str())
            .context("Nom invalide")?;
        ensure!(
            path.parent() == data.parent()
                && name
                    .strip_prefix(prefix)
                    .is_some_and(|id| Uuid::parse_str(id).is_ok()),
            "Chemin de récupération invalide"
        );
    }
    Ok(())
}

/// Resume only a previously verified installation. Caller must hold `lock`.
pub async fn recover(data: &Path) -> Result<()> {
    let data = absolute_data(data)?;
    let marker_path = sibling(&data, "maintenance.json")?;
    if !tokio::fs::try_exists(&marker_path).await? {
        return Ok(());
    }
    let metadata = tokio::fs::symlink_metadata(&marker_path).await?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "Marqueur de maintenance non ordinaire"
    );
    let marker: Marker = serde_json::from_slice(&tokio::fs::read(&marker_path).await?)?;
    verify_marker(&data, &marker)?;
    let parent = data.parent().context("Données sans parent")?;
    for path in [
        &data,
        &marker.staging,
        &marker.retired,
        &marker.report.backup,
    ] {
        if tokio::fs::try_exists(path).await? {
            let metadata = tokio::fs::symlink_metadata(path).await?;
            ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "Dossier de maintenance non ordinaire"
            );
        }
    }
    ensure!(
        inventory(&marker.report.backup).await? == marker.backup_inventory,
        "La sauvegarde vérifiée a été modifiée"
    );
    let staged = tokio::fs::try_exists(&marker.staging).await?;
    if staged {
        ensure!(
            inventory(&marker.staging).await? == marker.staging_inventory,
            "Le staging vérifié a été modifié ; sauvegarde conservée"
        );
        if tokio::fs::try_exists(&data).await? {
            ensure!(
                !tokio::fs::try_exists(&marker.retired).await?,
                "Deux sources présentes pendant la récupération"
            );
            tokio::fs::rename(&data, &marker.retired).await?;
            sync_directory(parent).await?;
        } else {
            ensure!(
                tokio::fs::try_exists(&marker.retired).await?,
                "Source originale de maintenance absente"
            );
        }
        tokio::fs::rename(&marker.staging, &data).await?;
        sync_directory(parent).await?;
    }
    ensure!(
        tokio::fs::try_exists(&data).await? && tokio::fs::try_exists(&marker.report.backup).await?,
        "Installation ou sauvegarde absente"
    );
    ensure!(
        inventory(&data).await? == marker.staging_inventory,
        "L’installation ne correspond pas au staging vérifié"
    );
    if tokio::fs::try_exists(&marker.retired).await? {
        tokio::fs::remove_dir_all(&marker.retired).await?;
    }
    json_file(&sibling(&data, "maintenance-result.json")?, &marker.report).await?;
    tokio::fs::remove_file(marker_path).await?;
    sync_directory(parent).await
}

/// Delete only explicitly older sessions, preserve keep/newer sessions, migrate
/// the retained data in staging and atomically install it after full equality checks.
/// The daemon must be stopped. This function also acquires the shared data lock.
pub async fn maintain(
    data: &Path,
    keep_id: &str,
    cutoff_ms: u64,
    validator: &dyn CheckpointValidation,
) -> Result<MaintenanceReport> {
    let _lock = lock(data)?;
    recover(data).await?;
    let data = absolute_data(data)?;
    let meta = tokio::fs::symlink_metadata(&data).await?;
    ensure!(
        meta.is_dir() && !meta.file_type().is_symlink(),
        "Dossier de données non ordinaire"
    );
    let imports = data.join(".session-imports");
    if tokio::fs::try_exists(&imports).await? {
        ensure!(
            tokio::fs::read_dir(imports)
                .await?
                .next_entry()
                .await?
                .is_none(),
            "Un import inachevé doit être récupéré avant la maintenance"
        );
    }
    let source = pool(&data.join("zedflow.db"), false).await?;
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT id,document FROM runs ORDER BY id")
        .fetch_all(&source)
        .await?;
    ensure!(
        rows.iter().any(|(id, _)| id == keep_id),
        "La session à conserver est absente"
    );
    let mut removed = Vec::new();
    let mut evidence = BTreeMap::new();
    let mut retained = Vec::new();
    for (id, raw) in rows {
        Uuid::parse_str(&id).context("Identité de session invalide")?;
        let value: Value = serde_json::from_str(&raw)?;
        ensure!(
            value["status"] != "running",
            "Une session est encore running ; arrêtez le daemon puis inspectez-la"
        );
        let proof = if let Some(created) = value["createdAt"].as_u64() {
            (created < cutoff_ms).then(|| json!({"kind":"createdAt","createdAt":created}))
        } else if value["createdAt"].is_null() && id != keep_id {
            // Legacy runs predate creation timestamps. A dated checkpoint proves
            // existence before cutoff; absence of evidence always preserves it.
            let checkpoints =
                crate::session_archive::checkpoints(&data.join("checkpoints.db"), &id, validator)
                    .await?;
            let mut dated = Vec::with_capacity(checkpoints.len());
            for checkpoint in checkpoints {
                let timestamp = checkpoint["created_at"]
                    .as_str()
                    .context("Checkpoint timestamp absent")?;
                let date = chrono::DateTime::parse_from_rfc3339(timestamp)?;
                let millis = date.timestamp_millis();
                if millis >= 0 && (millis as u64) < cutoff_ms {
                    dated.push((date, json!({"kind":"legacyCheckpoint","checkpointId":checkpoint["checkpoint_id"],"threadId":checkpoint["thread_id"],"createdAt":millis,"timestamp":date.to_rfc3339()})));
                }
            }
            dated
                .into_iter()
                .min_by_key(|(date, _)| *date)
                .map(|(_, proof)| proof)
        } else {
            None
        };
        if id != keep_id
            && let Some(proof) = proof
        {
            evidence.insert(id.clone(), proof);
            removed.push(id);
        } else {
            retained.push(id);
        }
    }
    source.close().await;
    let suffix = Uuid::new_v4().to_string();
    let backup = sibling(&data, &format!("backup-{suffix}"))?;
    let staging = sibling(&data, &format!("staging-{suffix}"))?;
    copy_tree(&data, &backup, true).await?;
    let backup_inventory = inventory(&backup).await?;
    json_file(&sibling(&backup, "inventory.json")?, &backup_inventory).await?;
    copy_tree(&backup, &staging, false).await?;
    let mut report = MaintenanceReport {
        backup,
        removed_session_ids: removed,
        removed_session_evidence: evidence,
        retained_session_ids: retained,
        verified_runs: 0,
        verified_events: 0,
        verified_checkpoints: 0,
    };
    let backup = report.backup.clone();
    normalize_and_verify(&staging, &backup, &data, &mut report, validator).await?;
    integrity(&staging.join("zedflow.db")).await?;
    let marker = Marker {
        version: 1,
        data: data.clone(),
        staging_inventory: inventory(&staging).await?,
        backup_inventory,
        staging,
        retired: sibling(&data, &format!("retired-{suffix}"))?,
        report: report.clone(),
    };
    json_file(&sibling(&data, "maintenance.json")?, &marker).await?;
    recover(&data).await?;
    Ok(report)
}

fn logical(mut value: Value) -> Value {
    if let Some(object) = value.as_object_mut() {
        object.remove("storageVersion");
        for collection in session_store::COLLECTIONS {
            if object
                .get(*collection)
                .is_some_and(|v| v.as_array().is_some_and(Vec::is_empty))
            {
                object.remove(*collection);
            }
        }
    }
    value
}

async fn has_table(db: &SqlitePool, name: &str) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?)",
    )
    .bind(name)
    .fetch_one(db)
    .await?
        != 0)
}

async fn normalize_and_verify(
    staging: &Path,
    backup: &Path,
    original: &Path,
    report: &mut MaintenanceReport,
    validator: &dyn CheckpointValidation,
) -> Result<()> {
    let source = pool(&backup.join("zedflow.db"), false).await?;
    let db = pool(&staging.join("zedflow.db"), true).await?;
    session_store::initialize(&db).await?;
    let store = ContentStore::new(db.clone()).await?;
    let cp = CheckpointStore::new(store.clone()).await?;
    let source_store = ContentStore::from_pool(source.clone());
    let legacy = staging.join("checkpoints.db");
    if tokio::fs::try_exists(&legacy).await? {
        let legacy_db = pool(&legacy, false).await?;
        let threads: Vec<String> =
            sqlx::query_scalar("SELECT DISTINCT thread_id FROM graph_checkpoints")
                .fetch_all(&legacy_db)
                .await?;
        for thread in threads {
            let root = thread.split('/').next().context("Thread ADK vide")?;
            ensure!(
                report
                    .retained_session_ids
                    .iter()
                    .chain(&report.removed_session_ids)
                    .any(|id| id == root),
                "Checkpoint orphelin {thread} : maintenance refusée sans le supprimer"
            );
        }
        legacy_db.close().await;
    }
    for id in &report.removed_session_ids {
        for table in ["events", "run_entities", "run_changes"] {
            sqlx::query(&format!("DELETE FROM {table} WHERE run=?"))
                .bind(id)
                .execute(&db)
                .await?;
        }
        sqlx::query("DELETE FROM runs WHERE id=?")
            .bind(id)
            .execute(&db)
            .await?;
        sqlx::query("DELETE FROM zf_records WHERE scope=?")
            .bind(id)
            .execute(&db)
            .await?;
        sqlx::query("DELETE FROM zf_checkpoints WHERE thread_id=? OR substr(thread_id,1,length(?)+1)=? || '/' ").bind(id).bind(id).bind(id).execute(&db).await?;
        let assets = staging.join("runs").join(id);
        if tokio::fs::try_exists(&assets).await? {
            tokio::fs::remove_dir_all(assets).await?;
        }
    }
    for id in &report.retained_session_ids {
        let before = session_store::load(&source, id).await?;
        session_store::save(&db, id, &before).await?;
        ensure!(
            logical(session_store::load(&db, id).await?) == logical(before),
            "La session {id} a changé pendant la migration"
        );
        report.verified_runs += 1;
        let events: Vec<(i64, String)> =
            sqlx::query_as("SELECT seq,document FROM events WHERE run=? ORDER BY seq")
                .bind(id)
                .fetch_all(&source)
                .await?;
        for (seq, raw) in events {
            let before: Value = serde_json::from_str(&raw)?;
            let before = session_store::hydrate_event(&source_store, &before).await?;
            let compact = session_store::compact_event(&store, &before).await?;
            ensure!(
                session_store::hydrate_event(&store, &compact).await? == before,
                "Événement {seq} altéré"
            );
            sqlx::query("UPDATE events SET document=? WHERE seq=? AND run=?")
                .bind(compact.to_string())
                .bind(seq)
                .bind(id)
                .execute(&db)
                .await?;
            report.verified_events += 1;
        }
        for checkpoint in
            crate::session_archive::checkpoints(&staging.join("checkpoints.db"), id, validator)
                .await?
        {
            cp.save(&checkpoint).await?;
            let after = cp
                .load_by_id(
                    checkpoint["checkpoint_id"]
                        .as_str()
                        .context("Checkpoint identity absent")?,
                )
                .await?
                .context("Checkpoint perdu")?;
            ensure!(after == checkpoint, "Checkpoint modifié");
            report.verified_checkpoints += 1;
        }
        if has_table(&source, "zf_checkpoints").await? {
            let headers:Vec<String>=sqlx::query_scalar("SELECT header FROM zf_checkpoints WHERE thread_id=? OR substr(thread_id,1,length(?)+1)=? || '/' ORDER BY created_at,seq").bind(id).bind(id).bind(id).fetch_all(&source).await?;
            for raw in headers {
                let header: CheckpointHeader = serde_json::from_str(&raw)?;
                ensure!(
                    source_store.resolve(&header.checkpoint_ref).await?
                        == store.resolve(&header.checkpoint_ref).await?,
                    "Checkpoint CAS modifié"
                );
                report.verified_checkpoints += 1;
            }
        }
        // Migrate actual contents, including uncertain receipts. Keep compatibility
        // files for historical paths; future execution reads the durable record.
        for kind in [
            "receipts",
            "capability-snapshots",
            "model-calls",
            "model-requests",
        ] {
            let directory = staging.join("runs").join(id).join(kind);
            if !tokio::fs::try_exists(&directory).await? {
                continue;
            }
            let mut entries = tokio::fs::read_dir(directory).await?;
            while let Some(entry) = entries.next_entry().await? {
                ensure!(
                    entry.file_type().await?.is_file(),
                    "Donnée runtime non ordinaire"
                );
                let path = entry.path();
                if path.extension().is_none_or(|e| e != "json") {
                    continue;
                }
                let key = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .context("Clé runtime invalide")?;
                let value: Value = serde_json::from_slice(&tokio::fs::read(&path).await?)?;
                if let Some(existing) = store.record(id, kind, key).await? {
                    ensure!(existing == value, "Collision runtime {id}/{kind}/{key}");
                } else {
                    store.put_record(id, kind, key, &value).await?;
                }
                ensure!(
                    store.record(id, kind, key).await? == Some(value),
                    "Runtime modifié"
                );
            }
        }
        let directory = staging.join("runs").join(id).join("tool-output");
        if tokio::fs::try_exists(&directory).await? {
            let mut entries = tokio::fs::read_dir(directory).await?;
            while let Some(entry) = entries.next_entry().await? {
                ensure!(
                    entry.file_type().await?.is_file(),
                    "Sortie runtime non ordinaire"
                );
                let bytes = tokio::fs::read(entry.path()).await?;
                let chunks: Vec<_> = bytes
                    .chunks(64 * 1024)
                    .map(|chunk| base64::engine::general_purpose::STANDARD.encode(chunk))
                    .collect();
                let relative = entry.path().strip_prefix(staging)?.to_owned();
                let value = json!({"path":original.join(relative),"content":{"encoding":"base64","chunks":chunks,"byteLength":bytes.len()}});
                // Historical file names need not equal call IDs. A separate legacy
                // key preserves every byte without replacing newer canonical records.
                let key = format!("legacy:{}", entry.file_name().to_string_lossy());
                store.put_record(id, "tool-output", &key, &value).await?;
                ensure!(
                    store.record(id, "tool-output", &key).await? == Some(value),
                    "Sortie complète modifiée"
                );
            }
        }
    }
    let actual: Vec<String> = sqlx::query_scalar("SELECT id FROM runs ORDER BY id")
        .fetch_all(&db)
        .await?;
    ensure!(
        actual == report.retained_session_ids,
        "Ensemble des sessions conservées incorrect"
    );
    // Legacy checkpoints are now verified in the central CAS. Only the staging
    // copy is removed; the coherent backup retains the original database.
    for suffix in [
        "checkpoints.db",
        "checkpoints.db-wal",
        "checkpoints.db-shm",
        "checkpoints.db-journal",
    ] {
        let path = staging.join(suffix);
        if tokio::fs::try_exists(&path).await? {
            tokio::fs::remove_file(path).await?;
        }
    }
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&db)
        .await?;
    sqlx::query("VACUUM").execute(&db).await?;
    db.close().await;
    source.close().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct FixtureValidation;
    impl CheckpointValidation for FixtureValidation {
        fn validate_checkpoint(&self, value: &Value) -> Result<()> {
            ensure!(value["state"].is_object(), "invalid fixture state");
            ensure!(
                value["pending_nodes"].is_array(),
                "invalid fixture pending nodes"
            );
            Ok(())
        }
    }

    async fn fixture(root: &Path) -> (PathBuf, String, String, String, Value, Vec<Value>) {
        let data = root.join("data");
        tokio::fs::create_dir(&data).await.unwrap();
        let db = SqlitePool::connect(&format!(
            "sqlite://{}?mode=rwc",
            data.join("zedflow.db").display()
        ))
        .await
        .unwrap();
        sqlx::query("CREATE TABLE runs(id TEXT PRIMARY KEY,document TEXT NOT NULL)")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE events(seq INTEGER PRIMARY KEY AUTOINCREMENT,run TEXT NOT NULL,document TEXT NOT NULL)").execute(&db).await.unwrap();
        let keep = Uuid::new_v4().to_string();
        let old = Uuid::new_v4().to_string();
        let newer = Uuid::new_v4().to_string();
        let payload = "Contexte conservé exact. ".repeat(4096);
        let value = json!({"id":keep,"status":"waiting","workspaceId":"fixture-workspace","createdAt":100,"updatedAt":250,"messages":[{"id":"message","text":"conserver"}],"state":{"large":payload,"literal":"sha256:not-a-reference"},"activities":[{"occurrenceId":"node:1","input":{"large":payload},"output":{"large":payload}}],"context":{"cwd":root,"instructions":[]},"composition":{"nodes":[]},"flowSource":"fn fixture() {}"});
        for (id, created) in [(&keep, 100), (&old, 50), (&newer, 300)] {
            let mut run = value.clone();
            run["id"] = json!(id);
            run["createdAt"] = json!(created);
            sqlx::query("INSERT INTO runs VALUES(?,?)")
                .bind(id)
                .bind(run.to_string())
                .execute(&db)
                .await
                .unwrap();
            for n in 0..3 {
                sqlx::query("INSERT INTO events(run,document) VALUES(?,?)")
                    .bind(id)
                    .bind(
                        json!({"type":"node_completed","output":{"large":payload},"n":n})
                            .to_string(),
                    )
                    .execute(&db)
                    .await
                    .unwrap();
            }
            let dir = data.join("runs").join(id).join("receipts");
            tokio::fs::create_dir_all(&dir).await.unwrap();
            tokio::fs::write(
                dir.join("fixture.json"),
                b"{\"status\":\"started\",\"id\":\"uncertain\",\"nodePath\":\"tool\"}",
            )
            .await
            .unwrap();
        }
        db.close().await;
        // Historical SQLite fixture; actual ADK serialization is exercised by
        // the runtime adapter's compatibility tests in P4.1.
        let cp = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(data.join("checkpoints.db"))
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        sqlx::query("CREATE TABLE graph_checkpoints(id TEXT,thread_id TEXT,step INTEGER,created_at TEXT,state TEXT,pending_nodes TEXT,metadata TEXT,attempts TEXT,child_ledger TEXT,cleared_interrupt TEXT)").execute(&cp).await.unwrap();
        let mut checkpoints = vec![];
        for thread in [&keep, &format!("{keep}/child"), &old] {
            let checkpoint = json!({"checkpoint_id":Uuid::new_v4().to_string(),"thread_id":thread,
                "step":4,"created_at":"2026-09-15T12:00:00Z","state":{"large":payload},
                "pending_nodes":["inbox"],"metadata":{},"attempts":{"retry":2},
                "child_ledger":{"child":{"effect":"already done"}},"cleared_interrupt":"inbox"});
            sqlx::query("INSERT INTO graph_checkpoints VALUES(?,?,4,?,?,?,?,?,?,?)")
                .bind(checkpoint["checkpoint_id"].as_str().unwrap())
                .bind(thread)
                .bind(checkpoint["created_at"].as_str().unwrap())
                .bind(checkpoint["state"].to_string())
                .bind(checkpoint["pending_nodes"].to_string())
                .bind("{}")
                .bind(checkpoint["attempts"].to_string())
                .bind(checkpoint["child_ledger"].to_string())
                .bind("inbox")
                .execute(&cp)
                .await
                .unwrap();
            if thread != &old {
                checkpoints.push(checkpoint);
            }
        }
        cp.close().await;
        tokio::fs::create_dir_all(data.join("flows")).await.unwrap();
        tokio::fs::write(data.join("flows/keep.rs"), "// do not alter")
            .await
            .unwrap();
        tokio::fs::create_dir_all(data.join("runs").join(&keep).join("tool-output"))
            .await
            .unwrap();
        tokio::fs::write(
            data.join("runs").join(&keep).join("tool-output/full.log"),
            [0, 255, 128, 65],
        )
        .await
        .unwrap();
        (data, keep, old, newer, value, checkpoints)
    }

    #[tokio::test]
    async fn maintenance_preserves_exact_state_and_newer_sessions_with_complete_backup() {
        let temp = tempfile::tempdir().unwrap();
        let (data, keep, old, newer, expected, checkpoints) = fixture(temp.path()).await;
        let report = maintain(&data, &keep, 200, &FixtureValidation)
            .await
            .unwrap();
        assert_eq!(report.removed_session_ids, vec![old.clone()]);
        assert_eq!(report.verified_runs, 2);
        assert_eq!(report.verified_events, 6);
        assert_eq!(report.verified_checkpoints, 2);
        let db = pool(&data.join("zedflow.db"), false).await.unwrap();
        assert_eq!(
            logical(session_store::load(&db, &keep).await.unwrap()),
            logical(expected)
        );
        let mut expected_ids = vec![keep.clone(), newer];
        expected_ids.sort();
        assert_eq!(report.retained_session_ids, expected_ids);
        let store = ContentStore::from_pool(db.clone());
        for checkpoint in checkpoints {
            let raw: String =
                sqlx::query_scalar("SELECT header FROM zf_checkpoints WHERE checkpoint_id=?")
                    .bind(checkpoint["checkpoint_id"].as_str().unwrap())
                    .fetch_one(&db)
                    .await
                    .unwrap();
            let header: CheckpointHeader = serde_json::from_str(&raw).unwrap();
            assert_eq!(
                store.resolve(&header.checkpoint_ref).await.unwrap(),
                serde_json::to_value(checkpoint).unwrap()
            );
        }
        assert_eq!(
            store
                .record(&keep, "receipts", "fixture")
                .await
                .unwrap()
                .unwrap()["status"],
            "started"
        );
        assert_eq!(
            store
                .record(&keep, "tool-output", "legacy:full.log")
                .await
                .unwrap()
                .unwrap()["content"]["chunks"][0],
            "AP+AQQ=="
        );
        assert!(!data.join("runs").join(&old).exists());
        assert!(!data.join("checkpoints.db").exists());
        assert_eq!(
            tokio::fs::read(data.join("flows/keep.rs")).await.unwrap(),
            b"// do not alter"
        );
        let backup = pool(&report.backup.join("zedflow.db"), false)
            .await
            .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM runs")
                .fetch_one(&backup)
                .await
                .unwrap(),
            3
        );
        assert!(report.backup.join("runs").join(old).exists());
        assert!(report.backup.join("checkpoints.db").exists());
        let compact = session_store::load_projection(&db, &keep).await.unwrap();
        assert_eq!(
            compact["activities"][0]["inputRef"], compact["activities"][0]["outputRef"],
            "identical occurrence states share their content"
        );
        let metadata: String = sqlx::query_scalar("SELECT document FROM runs WHERE id=?")
            .bind(&keep)
            .fetch_one(&db)
            .await
            .unwrap();
        assert!(
            metadata.len() < 4096,
            "large state is absent from the session metadata"
        );
        db.close().await;
        backup.close().await;
    }

    #[tokio::test]
    async fn undated_legacy_requires_checkpoint_proof_and_preserves_unknown_sessions() {
        let temp = tempfile::tempdir().unwrap();
        let (data, keep, old, newer, _, _) = fixture(temp.path()).await;
        let db = pool(&data.join("zedflow.db"), true).await.unwrap();
        sqlx::query("UPDATE runs SET document=json_remove(document,'$.createdAt') WHERE id<>?")
            .bind(&keep)
            .execute(&db)
            .await
            .unwrap();
        db.close().await;
        let checkpoints = pool(&data.join("checkpoints.db"), true).await.unwrap();
        sqlx::query("UPDATE graph_checkpoints SET created_at='1970-01-01T00:00:00.050+00:00' WHERE thread_id=?").bind(&old).execute(&checkpoints).await.unwrap();
        checkpoints.close().await;
        let report = maintain(&data, &keep, 200, &FixtureValidation)
            .await
            .unwrap();
        assert_eq!(report.removed_session_ids, vec![old.clone()]);
        assert_eq!(
            report.removed_session_evidence[&old]["kind"],
            "legacyCheckpoint"
        );
        assert!(report.retained_session_ids.contains(&newer));
    }

    #[tokio::test]
    async fn lock_and_invalid_checkpoint_refuse_without_modifying_source() {
        let temp = tempfile::tempdir().unwrap();
        let (data, keep, _, _, _, _) = fixture(temp.path()).await;
        let held = lock(&data).unwrap();
        assert!(
            maintain(&data, &keep, 200, &FixtureValidation)
                .await
                .is_err()
        );
        drop(held);
        let cp = pool(&data.join("checkpoints.db"), true).await.unwrap();
        sqlx::query("UPDATE graph_checkpoints SET state='invalid' WHERE thread_id=?")
            .bind(&keep)
            .execute(&cp)
            .await
            .unwrap();
        cp.close().await;
        let before = inventory(&data).await.unwrap();
        assert!(
            maintain(&data, &keep, 200, &FixtureValidation)
                .await
                .is_err()
        );
        assert_eq!(inventory(&data).await.unwrap(), before);
    }

    #[tokio::test]
    async fn recovery_finishes_each_atomic_swap_boundary_and_rejects_modified_staging() {
        for boundary in 0..3 {
            let temp = tempfile::tempdir().unwrap();
            let data = temp.path().join("data");
            tokio::fs::create_dir(&data).await.unwrap();
            tokio::fs::write(data.join("original"), "old")
                .await
                .unwrap();
            let suffix = Uuid::new_v4().to_string();
            let backup = sibling(&data, &format!("backup-{suffix}")).unwrap();
            let staging = sibling(&data, &format!("staging-{suffix}")).unwrap();
            let retired = sibling(&data, &format!("retired-{suffix}")).unwrap();
            copy_tree(&data, &backup, false).await.unwrap();
            tokio::fs::create_dir(&staging).await.unwrap();
            tokio::fs::write(staging.join("migrated"), "new")
                .await
                .unwrap();
            let marker = Marker {
                version: 1,
                data: data.clone(),
                staging: staging.clone(),
                retired: retired.clone(),
                staging_inventory: inventory(&staging).await.unwrap(),
                backup_inventory: inventory(&backup).await.unwrap(),
                report: MaintenanceReport {
                    backup: backup.clone(),
                    removed_session_ids: vec![],
                    removed_session_evidence: BTreeMap::new(),
                    retained_session_ids: vec![],
                    verified_runs: 0,
                    verified_events: 0,
                    verified_checkpoints: 0,
                },
            };
            json_file(&sibling(&data, "maintenance.json").unwrap(), &marker)
                .await
                .unwrap();
            if boundary == 0 {
                tokio::fs::write(staging.join("tampered"), "bad")
                    .await
                    .unwrap();
                assert!(recover(&data).await.is_err());
                assert!(data.join("original").exists());
                tokio::fs::remove_file(staging.join("tampered"))
                    .await
                    .unwrap();
            }
            if boundary >= 1 {
                tokio::fs::rename(&data, &retired).await.unwrap();
            }
            if boundary == 2 {
                tokio::fs::rename(&staging, &data).await.unwrap();
            }
            recover(&data).await.unwrap();
            assert!(data.join("migrated").exists());
            assert!(backup.join("original").exists());
            assert!(!retired.exists());
            recover(&data).await.unwrap();
        }
    }
}
