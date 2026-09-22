//! Explicit, portable session exports. SQLite remains the live source of truth.
use crate::{
    content_store::{ContentBlob, ContentRecord, ContentStore},
    contracts::{
        ArchiveRuntime, ArchiveSnapshot, CheckpointCodec, CheckpointHeader, CheckpointStore,
    },
    session_store,
    workspaces::Workspace,
};
use anyhow::{Context, Result, bail, ensure};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read, Write},
    path::{Component, Path, PathBuf},
};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;
use zf_flows::schema::Composition;

const FORMAT: &str = "zedflow-session";
const VERSION: u32 = 3;
const ADK_VERSION: &str = "2.2.0";
const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_FILES: usize = 10_000;
const RUNTIME_DIRS: &[&str] = &[
    "receipts",
    "tool-output",
    "tool-output-fragments",
    "capability-snapshots",
    "model-calls",
    "model-requests",
    "inference-raw",
    "inference-dispatch",
    "prepared-requests",
];
fn runtime_record_kind(kind: &str) -> bool {
    RUNTIME_DIRS.contains(&kind)
        || [
            "route-runtime",
            "route-inputs",
            "route-visits",
            "route-status",
            "route-resume",
            "route-bindings",
            "route-output-heads",
            "route-publications",
            "route-channel-views",
            "context-productions",
            "context-production-results",
            "context-production-status",
            "window-owners",
            "window-captures",
            "window-invocation-results",
            "window-invocation-selection",
            "window-selection-used",
            "window-edit-commands",
            "window-selection-commands",
            "revision-definitions",
            "revision-heads",
            "revision-active",
            "revision-steps",
            "revision-boundaries",
            "runtime-graph-definitions",
            "runtime-graph-heads",
            "runtime-graph-steps",
            "route-dispatches",
        ]
        .contains(&kind)
        || kind
            .strip_prefix("window-selections:")
            .is_some_and(|hash| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
}

#[derive(Debug)]
pub struct Conflict(pub String);
impl std::fmt::Display for Conflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for Conflict {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InventoryFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    pub session_id: String,
    pub archive_hash: String,
    pub files: Vec<InventoryFile>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedSession {
    pub session_id: String,
    pub path: PathBuf,
    pub archive_hash: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResponse {
    pub exports: Vec<ExportedSession>,
    pub download_url: String,
}

#[derive(Debug, Serialize)]
pub struct ImportResponse {
    pub runs: Vec<Value>,
    pub imported: usize,
    pub unchanged: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Header {
    #[serde(rename = "type")]
    kind: String,
    version: u32,
    zedflow_version: String,
    adk_version: String,
    run: Value,
    runtime_root: PathBuf,
    context_files: Vec<ContextFile>,
    resume_blocked: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ContextFile {
    source_path: PathBuf,
    archive_path: String,
    sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content_ref: Option<String>,
}

/// Storage-owned envelope: only fields used for archive identity/path handling
/// are projected. Every other ADK field is retained and validated by the runtime.
#[derive(Clone, Serialize, Deserialize)]
struct ArchivedCheckpoint {
    checkpoint_id: String,
    thread_id: String,
    state: Value,
    #[serde(flatten)]
    fields: serde_json::Map<String, Value>,
}

struct Bundle {
    manifest: Manifest,
    files: BTreeMap<String, Vec<u8>>,
    header: Header,
    events: Vec<(i64, Value)>,
    checkpoints: Vec<ArchivedCheckpoint>,
    checkpoint_headers: Vec<CheckpointHeader>,
    records: Vec<ContentRecord>,
    content: Vec<ContentBlob>,
    registry: crate::data_archive::RegistryArchive,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportMarker {
    sessions: Vec<ImportingSession>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportingSession {
    id: String,
    archive_hash: String,
    checkpoint_ids: Vec<String>,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn valid_id(id: &str) -> Result<()> {
    ensure!(
        Uuid::parse_str(id).is_ok(),
        "identifiant de session invalide"
    );
    Ok(())
}

fn relative_path(path: &str) -> Result<&Path> {
    let value = Path::new(path);
    ensure!(
        !path.is_empty()
            && !path.contains('\\')
            && !path.contains('\0')
            && value
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "chemin d’archive invalide : {path}"
    );
    Ok(value)
}

async fn regular_bytes(path: &Path) -> Result<Vec<u8>> {
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .with_context(|| format!("fichier absent : {}", path.display()))?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "fichier ordinaire requis : {}",
        path.display()
    );
    ensure!(
        metadata.len() <= MAX_BYTES,
        "fichier trop volumineux pour l’export : {}",
        path.display()
    );
    let bytes = tokio::fs::read(path).await?;
    ensure!(
        bytes.len() as u64 <= MAX_BYTES,
        "fichier trop volumineux pour l’export"
    );
    Ok(bytes)
}

pub(crate) async fn owned_directory(path: &Path) -> Result<()> {
    match tokio::fs::symlink_metadata(path).await {
        Ok(meta) => ensure!(
            meta.is_dir() && !meta.file_type().is_symlink(),
            "dossier ordinaire requis : {}",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            tokio::fs::create_dir(path).await?;
            #[cfg(unix)]
            if let Some(parent) = path.parent() {
                tokio::fs::File::open(parent).await?.sync_all().await?;
            }
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

async fn collect_files(root: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_owned()];
    let mut bytes = 0_u64;
    while let Some(directory) = pending.pop() {
        let meta = tokio::fs::symlink_metadata(&directory).await?;
        ensure!(
            meta.is_dir() && !meta.file_type().is_symlink(),
            "lien symbolique interdit dans une archive"
        );
        let mut entries = tokio::fs::read_dir(&directory).await?;
        while let Some(entry) = entries.next_entry().await? {
            let kind = entry.file_type().await?;
            ensure!(
                !kind.is_symlink(),
                "lien symbolique interdit dans une archive"
            );
            if kind.is_dir() {
                pending.push(entry.path());
                continue;
            }
            ensure!(kind.is_file(), "fichier spécial interdit dans une archive");
            let path = entry
                .path()
                .strip_prefix(root)?
                .to_str()
                .context("chemin non UTF-8")?
                .to_owned();
            relative_path(&path)?;
            let content = regular_bytes(&entry.path()).await?;
            bytes = bytes
                .checked_add(content.len() as u64)
                .context("taille d’archive invalide")?;
            ensure!(
                bytes <= MAX_BYTES && files.len() < MAX_FILES,
                "archive trop volumineuse"
            );
            files.insert(path, content);
        }
    }
    Ok(files)
}

async fn write_files(root: &Path, files: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    owned_directory(root).await?;
    let mut directories = BTreeSet::from([root.to_owned()]);
    for (name, bytes) in files {
        let path = root.join(relative_path(name)?);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
            let mut ancestor = Some(parent);
            while let Some(directory) = ancestor.filter(|directory| directory.starts_with(root)) {
                directories.insert(directory.to_owned());
                ancestor = directory.parent();
            }
        }
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await?;
        file.write_all(bytes).await?;
        file.sync_all().await?;
    }
    #[cfg(unix)]
    for directory in directories.into_iter().rev() {
        tokio::fs::File::open(directory).await?.sync_all().await?;
    }
    Ok(())
}

async fn sync_directories(root: &Path) -> Result<()> {
    let mut pending = vec![root.to_owned()];
    let mut directories = Vec::new();
    while let Some(directory) = pending.pop() {
        let mut entries = tokio::fs::read_dir(&directory).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                pending.push(entry.path());
            }
        }
        directories.push(directory);
    }
    #[cfg(unix)]
    for directory in directories.into_iter().rev() {
        tokio::fs::File::open(directory).await?.sync_all().await?;
    }
    Ok(())
}

/// Read the historical SQLite layout without creating a database. The runtime
/// validates each complete document before any caller can migrate or import it.
pub async fn checkpoints(
    db_path: &Path,
    id: &str,
    validator: &dyn CheckpointCodec,
) -> Result<Vec<Value>> {
    use sqlx::Row;
    if !tokio::fs::try_exists(db_path).await? {
        return Ok(vec![]);
    }
    let pool = SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(db_path)
            .read_only(true),
    )
    .await?;
    let rows = sqlx::query("SELECT * FROM graph_checkpoints WHERE thread_id=? OR substr(thread_id,1,length(?)+1)=? || '/' ORDER BY created_at,id")
        .bind(id).bind(id).bind(id).fetch_all(&pool).await?;
    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        let mut value = json!({"checkpoint_id":row.try_get::<String,_>("id")?,"thread_id":row.try_get::<String,_>("thread_id")?,"step":row.try_get::<i64,_>("step")?,"created_at":row.try_get::<String,_>("created_at")?});
        for field in [
            "state",
            "pending_nodes",
            "metadata",
            "attempts",
            "child_ledger",
        ] {
            let raw = row.try_get::<Option<String>, _>(field).or_else(|error| {
                if matches!(error, sqlx::Error::ColumnNotFound(_)) {
                    Ok(None)
                } else {
                    Err(error)
                }
            })?;
            value[field] = match raw {
                Some(raw) => serde_json::from_str(&raw)?,
                None => json!({}),
            };
        }
        value["cleared_interrupt"] = json!(
            row.try_get::<Option<String>, _>("cleared_interrupt")
                .or_else(|error| if matches!(error, sqlx::Error::ColumnNotFound(_)) {
                    Ok(None)
                } else {
                    Err(error)
                })?
        );
        result.push(validator.decode_legacy_checkpoint(value)?);
    }
    pool.close().await;
    Ok(result)
}

fn add_json_line(bytes: &mut Vec<u8>, value: &impl Serialize) -> Result<()> {
    serde_json::to_writer(&mut *bytes, value)?;
    bytes.push(b'\n');
    Ok(())
}

async fn capture_context(
    run: &Value,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(Vec<ContextFile>, Vec<String>)> {
    let mut context_files = Vec::new();
    let mut blocked = Vec::new();
    for (category, entries) in [
        ("instructions", &run["context"]["instructions"]),
        ("skills", &run["context"]["skills"]),
    ] {
        for entry in entries.as_array().into_iter().flatten() {
            let Some(source) = entry["path"].as_str() else {
                continue;
            };
            let expected = entry["hash"].as_str().unwrap_or_default();
            let bytes = if let Some(content) =
                entry["content"].as_str().or_else(|| entry["body"].as_str())
            {
                content.as_bytes().to_vec()
            } else {
                match regular_bytes(Path::new(source)).await {
                    Ok(bytes) if digest(&bytes) == expected => bytes,
                    _ => {
                        blocked.push(format!("Contenu historique indisponible pour {source}"));
                        continue;
                    }
                }
            };
            let hash = digest(&bytes);
            let archive_path = format!(
                "context/{category}/{hash}/{}",
                if category == "skills" {
                    "SKILL.md"
                } else {
                    "AGENTS.md"
                }
            );
            files.insert(archive_path.clone(), bytes);
            context_files.push(ContextFile {
                source_path: source.into(),
                archive_path,
                sha256: hash,
                content_ref: None,
            });
        }
    }
    Ok((context_files, blocked))
}

fn binary_content(bytes: &[u8]) -> Value {
    let chunks: Vec<_> = bytes
        .chunks(64 * 1024)
        .map(|chunk| base64::engine::general_purpose::STANDARD.encode(chunk))
        .collect();
    json!({"encoding":"base64","chunks":chunks,"byteLength":bytes.len()})
}
fn asset_bytes(value: &Value) -> Result<Vec<u8>> {
    let bytes = if let Some(text) = value.as_str() {
        text.as_bytes().to_vec()
    } else {
        crate::content_store::decode_full_output(value)?
    };
    ensure!(bytes.len() as u64 <= MAX_BYTES, "fichier trop volumineux");
    Ok(bytes)
}
async fn output_content(
    store: &ContentStore,
    value: &Value,
    records: &[ContentRecord],
) -> Result<Value> {
    if let Some(key) = value["fragmentKey"].as_str() {
        let record = records
            .iter()
            .find(|record| record.kind == "tool-output-fragments" && record.key == key)
            .context("fragments de sortie absents")?;
        store.resolve(&record.value_ref).await
    } else {
        value
            .get("content")
            .cloned()
            .context("contenu de sortie absent")
    }
}

async fn memory_store() -> Result<ContentStore> {
    ContentStore::new(
        sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?,
    )
    .await
}

fn projection_refs(value: &Value) -> Vec<String> {
    let mut refs = BTreeSet::new();
    if let Some(reference) = value["preview"]["sourceCompositionRef"].as_str() {
        refs.insert(reference.to_owned());
    }
    // Only explicitly owned storage fields are links. No traversal enters user
    // arguments, model text, compositions or hydrated application state.
    for key in [
        "projectionRef",
        "stateRef",
        "contextRef",
        "compositionRef",
        "flowSourceRef",
        "flowPackageRef",
        "runtimeGraphRef",
        "graphRef",
        "inputRef",
        "outputRef",
        "snapshotRef",
        "resultRef",
        "argumentsRef",
        "contentRef",
        "textRef",
        "chunkRef",
        "recordRef",
        "dataRef",
        "updatesRef",
        "requestRef",
        "fullOutputRef",
        "checkpointRef",
        "definitionRef",
        "sourceRef",
    ] {
        if let Some(reference) = value[key].as_str() {
            refs.insert(reference.to_owned());
        }
    }
    for collection in ["activities", "contextSnapshots", "toolActivities"] {
        for item in value[collection].as_array().into_iter().flatten() {
            refs.extend(projection_refs(item));
        }
    }
    for item in value["timeline"].as_array().into_iter().flatten() {
        if item["kind"] == "tool" {
            refs.extend(projection_refs(&item["activity"]));
        }
    }
    for key in ["snapshot", "result", "flowRevision"] {
        if value[key].is_object() {
            refs.extend(projection_refs(&value[key]));
        }
    }
    refs.into_iter().collect()
}

async fn capture(
    db: &SqlitePool,
    data: &Path,
    workspace: &Workspace,
    id: &str,
    runtime: &dyn ArchiveRuntime,
) -> Result<Bundle> {
    valid_id(id)?;
    let mut run = session_store::load_projection(db, id).await?;
    ensure!(
        run["workspaceId"] == workspace.id,
        "cette session appartient à un autre workspace"
    );
    if run["status"] == "running" {
        return Err(
            Conflict("Attendez un point d’arrêt de la session avant de l’exporter".into()).into(),
        );
    }
    let scratch = memory_store().await?;
    let source = ContentStore::from_pool(db.clone());
    let mut source_roots = projection_refs(&run);
    let registry = crate::data_archive::capture(db, id).await?;
    source_roots.extend(registry.roots());
    let runtime_root = data.join("runs").join(id);
    let mut files = BTreeMap::new();
    for name in RUNTIME_DIRS {
        let root = runtime_root.join(name);
        if tokio::fs::try_exists(&root).await? {
            for (path, bytes) in collect_files(&root).await? {
                files.insert(format!("runtime/{name}/{path}"), bytes);
            }
        }
    }
    let normalized = run["storageVersion"] == 2;
    if normalized {
        scratch
            .import_blobs(&source.export_blobs(&source_roots).await?)
            .await?;
    } else {
        run = session_store::compact_run(&scratch, &run).await?;
    }
    let context = if let Some(reference) = run["contextRef"].as_str() {
        scratch.resolve(reference).await?
    } else {
        Value::Null
    };
    let (mut context_files, resume_blocked) =
        capture_context(&json!({"context":context}), &mut files).await?;
    let rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT seq,document FROM events WHERE run=? ORDER BY seq")
            .bind(id)
            .fetch_all(db)
            .await?;
    let mut events = Vec::with_capacity(rows.len());
    for (seq, raw) in rows {
        let event: Value = serde_json::from_str(&raw)?;
        source_roots.extend(projection_refs(&event));
        if event["type"] == "context_snapshot"
            && let Some(reference) = event["snapshotRef"].as_str()
        {
            source_roots.extend(projection_refs(&source.resolve(reference).await?));
        }
        events.push((seq, event));
    }
    let has_cas: i64 = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='zf_records')",
    )
    .fetch_one(db)
    .await?;
    let mut records = if has_cas != 0 {
        source.records(id).await?
    } else {
        vec![]
    };
    source_roots.extend(records.iter().map(|r| r.value_ref.clone()));
    for record in &records {
        if record.kind.starts_with("revision-")
            || record.kind.starts_with("route-")
            || record.kind.starts_with("runtime-graph-")
            || record.kind == "window-captures"
        {
            source_roots.extend(projection_refs(&source.resolve(&record.value_ref).await?));
        }
    }
    let has_cp: i64 = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='zf_checkpoints')",
    )
    .fetch_one(db)
    .await?;
    let mut checkpoint_headers = Vec::new();
    if has_cp != 0 {
        let rows:Vec<String>=sqlx::query_scalar("SELECT header FROM zf_checkpoints WHERE thread_id=? OR substr(thread_id,1,length(?)+1)=? || '/' ORDER BY created_at,seq").bind(id).bind(id).bind(id).fetch_all(db).await?;
        for raw in rows {
            let h: CheckpointHeader = serde_json::from_str(&raw)?;
            source_roots.extend([h.checkpoint_ref.clone(), h.state_ref.clone()]);
            checkpoint_headers.push(h);
        }
    }
    if !source_roots.is_empty() {
        scratch
            .import_blobs(&source.export_blobs(&source_roots).await?)
            .await?;
    }
    for (_, event) in &mut events {
        *event = session_store::compact_event(&scratch, event).await?;
    }
    let legacy_cp = checkpoints(&data.join("checkpoints.db"), id, runtime).await?;
    if !legacy_cp.is_empty() {
        let cp = CheckpointStore::new(scratch.clone()).await?;
        for checkpoint in legacy_cp {
            if !checkpoint_headers
                .iter()
                .any(|h| checkpoint["checkpoint_id"] == h.checkpoint_id)
            {
                cp.save(&checkpoint).await?;
            }
        }
        checkpoint_headers.extend(cp.list_run_headers(id).await?);
    }
    // Encode legacy assets into the same closure. The archive never carries a
    // second physical copy of a runtime record beside its content reference.
    for (name, bytes) in &files {
        let Some(relative) = name.strip_prefix("runtime/") else {
            continue;
        };
        let Some((kind, file)) = relative.split_once('/') else {
            continue;
        };
        if kind == "tool-output" {
            let path = runtime_root.join(relative);
            let mut existing = false;
            for record in records.iter().filter(|r| r.kind == "tool-output") {
                if scratch.resolve(&record.value_ref).await?["path"] == json!(path) {
                    existing = true;
                    break;
                }
            }
            if !existing {
                let value = json!({"path":path,"content":binary_content(bytes)});
                records.push(ContentRecord {
                    scope: id.into(),
                    kind: kind.into(),
                    key: format!("legacy:{file}"),
                    value_ref: scratch.intern(&value).await?,
                });
            }
        } else if file.ends_with(".json") {
            let key = file.trim_end_matches(".json");
            if records.iter().any(|r| r.kind == kind && r.key == key) {
                continue;
            }
            let value: Value = serde_json::from_slice(bytes)?;
            records.push(ContentRecord {
                scope: id.into(),
                kind: kind.into(),
                key: key.into(),
                value_ref: scratch.intern(&value).await?,
            });
        } else {
            bail!("asset runtime non pris en charge : {relative}");
        }
    }
    for file in &mut context_files {
        let bytes = files.get(&file.archive_path).context("contexte absent")?;
        let value = match std::str::from_utf8(bytes) {
            Ok(text) => json!(text),
            Err(_) => binary_content(bytes),
        };
        file.content_ref = Some(scratch.intern(&value).await?);
    }
    files.retain(|path, _| !path.starts_with("runtime/") && !path.starts_with("context/"));
    records.sort_by(|a, b| (&a.kind, &a.key).cmp(&(&b.kind, &b.key)));
    let run_roots = projection_refs(&run);
    let run = json!({"id":run["id"],"status":run["status"],"workspaceId":run["workspaceId"],"projectionRef":scratch.intern(&run).await?});
    let header = Header {
        kind: FORMAT.into(),
        version: VERSION,
        zedflow_version: env!("CARGO_PKG_VERSION").into(),
        adk_version: ADK_VERSION.into(),
        run,
        runtime_root,
        context_files,
        resume_blocked,
    };
    let mut roots = run_roots;
    roots.extend(registry.roots());
    roots.extend(projection_refs(&header.run));
    for (_, event) in &events {
        roots.extend(projection_refs(event));
    }
    roots.extend(records.iter().map(|r| r.value_ref.clone()));
    for record in &records {
        if record.kind.starts_with("revision-")
            || record.kind.starts_with("route-")
            || record.kind.starts_with("runtime-graph-")
            || record.kind == "window-captures"
        {
            roots.extend(projection_refs(&scratch.resolve(&record.value_ref).await?));
        }
    }
    roots.extend(
        header
            .context_files
            .iter()
            .filter_map(|f| f.content_ref.clone()),
    );
    for checkpoint in &checkpoint_headers {
        roots.extend([
            checkpoint.checkpoint_ref.clone(),
            checkpoint.state_ref.clone(),
        ]);
    }
    let content = scratch.export_blobs(&roots).await?;
    let mut lines = Vec::new();
    add_json_line(&mut lines, &header)?;
    add_json_line(&mut lines, &json!({"type":"registry","registry":registry}))?;
    for (seq, event) in &events {
        add_json_line(&mut lines, &json!({"type":"event","seq":seq,"event":event}))?;
    }
    for checkpoint in &checkpoint_headers {
        add_json_line(
            &mut lines,
            &json!({"type":"checkpointRef","checkpoint":checkpoint}),
        )?;
    }
    for record in &records {
        add_json_line(&mut lines, &json!({"type":"record","record":record}))?;
    }
    files.insert("session.jsonl".into(), lines);
    let mut bytes = Vec::new();
    for blob in &content {
        add_json_line(&mut bytes, blob)?;
    }
    files.insert("contents.jsonl".into(), bytes);
    let inventory: Vec<_> = files
        .iter()
        .map(|(path, bytes)| InventoryFile {
            path: path.clone(),
            sha256: digest(bytes),
            bytes: bytes.len() as u64,
        })
        .collect();
    ensure!(
        inventory.iter().map(|f| f.bytes).sum::<u64>() <= MAX_BYTES,
        "archive trop volumineuse"
    );
    let manifest = Manifest {
        format: FORMAT.into(),
        version: VERSION,
        session_id: id.into(),
        archive_hash: digest(&serde_json::to_vec(&inventory)?),
        files: inventory,
    };
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec_pretty(&manifest)?,
    );
    Ok(Bundle {
        manifest,
        files,
        header,
        events,
        checkpoints: vec![],
        checkpoint_headers,
        records,
        content,
        registry,
    })
}

fn validate_bundle(files: BTreeMap<String, Vec<u8>>) -> Result<Bundle> {
    let manifest: Manifest =
        serde_json::from_slice(files.get("manifest.json").context("manifest.json absent")?)?;
    ensure!(
        manifest.format == FORMAT && (1..=VERSION).contains(&manifest.version),
        "format d’archive non pris en charge"
    );
    valid_id(&manifest.session_id)?;
    ensure!(
        digest(&serde_json::to_vec(&manifest.files)?) == manifest.archive_hash,
        "empreinte d’inventaire invalide"
    );
    let mut names = BTreeSet::new();
    let mut size = 0_u64;
    for entry in &manifest.files {
        relative_path(&entry.path)?;
        ensure!(
            entry.path != "manifest.json" && names.insert(entry.path.clone()),
            "entrée d’inventaire dupliquée"
        );
        let bytes = files
            .get(&entry.path)
            .with_context(|| format!("fichier manquant : {}", entry.path))?;
        ensure!(
            bytes.len() as u64 == entry.bytes && digest(bytes) == entry.sha256,
            "fichier altéré : {}",
            entry.path
        );
        size = size.checked_add(entry.bytes).context("taille invalide")?;
    }
    ensure!(
        size <= MAX_BYTES && names.len() < MAX_FILES && files.len() == names.len() + 1,
        "inventaire incomplet ou archive trop volumineuse"
    );
    let text = std::str::from_utf8(files.get("session.jsonl").context("session.jsonl absent")?)?;
    ensure!(text.ends_with('\n'), "journal exporté incomplet");
    let mut lines = text.lines();
    let header: Header = serde_json::from_str(lines.next().context("journal vide")?)?;
    ensure!(
        header.kind == FORMAT
            && header.version == manifest.version
            && header.run["id"] == manifest.session_id,
        "en-tête d’archive invalide"
    );
    ensure!(
        header.run["status"] != "running",
        "archive prise pendant une exécution"
    );
    if header.version == 1 {
        let _: Composition = serde_json::from_value(header.run["composition"].clone())
            .context("composition exportée invalide")?;
    }
    let mut events = Vec::new();
    let mut checkpoints = Vec::new();
    let mut checkpoint_ids = BTreeSet::new();
    let mut checkpoint_headers = Vec::new();
    let mut records = Vec::new();
    let mut record_keys = BTreeSet::new();
    let mut registry = None;
    let mut last_seq = 0;
    for line in lines {
        let entry: Value = serde_json::from_str(line).context("ligne d’archive invalide")?;
        match entry["type"].as_str() {
            Some("registry") if header.version >= 3 => {
                ensure!(registry.is_none(), "registre dupliqué dans l’archive");
                let value: crate::data_archive::RegistryArchive =
                    serde_json::from_value(entry["registry"].clone())?;
                value.validate()?;
                registry = Some(value);
            }
            Some("event") => {
                let seq = entry["seq"].as_i64().context("séquence manquante")?;
                ensure!(
                    seq > last_seq && entry["event"].is_object(),
                    "ordre des événements invalide"
                );
                last_seq = seq;
                events.push((seq, entry["event"].clone()));
            }
            Some("checkpoint") => {
                let checkpoint: ArchivedCheckpoint =
                    serde_json::from_value(entry["checkpoint"].clone())?;
                valid_id(&checkpoint.checkpoint_id)?;
                ensure!(
                    (checkpoint.thread_id == manifest.session_id
                        || checkpoint
                            .thread_id
                            .starts_with(&format!("{}/", manifest.session_id)))
                        && checkpoint_ids.insert(checkpoint.checkpoint_id.clone()),
                    "checkpoint étranger ou dupliqué"
                );
                checkpoints.push(checkpoint);
            }
            Some("checkpointRef") if header.version >= 2 => {
                let checkpoint: CheckpointHeader =
                    serde_json::from_value(entry["checkpoint"].clone())?;
                valid_id(&checkpoint.checkpoint_id)?;
                ensure!(
                    (checkpoint.thread_id == manifest.session_id
                        || checkpoint
                            .thread_id
                            .starts_with(&format!("{}/", manifest.session_id)))
                        && checkpoint_ids.insert(checkpoint.checkpoint_id.clone()),
                    "checkpoint étranger ou dupliqué"
                );
                checkpoint_headers.push(checkpoint);
            }
            Some("record") if header.version >= 2 => {
                let record: ContentRecord = serde_json::from_value(entry["record"].clone())?;
                ensure!(
                    record.scope == manifest.session_id
                        && runtime_record_kind(&record.kind)
                        && !record.key.is_empty()
                        && !record.key.contains('\0')
                        && record_keys.insert((record.kind.clone(), record.key.clone())),
                    "identité de record invalide"
                );
                records.push(record);
            }
            _ => bail!("type de ligne d’archive inconnu"),
        }
    }
    for file in &header.context_files {
        relative_path(&file.archive_path)?;
        ensure!(
            file.archive_path.starts_with("context/"),
            "chemin de contexte invalide"
        );
        ensure!(
            (header.version >= 2 && file.content_ref.is_some())
                || files
                    .get(&file.archive_path)
                    .is_some_and(|bytes| digest(bytes) == file.sha256),
            "contexte exporté incomplet"
        );
    }
    for (name, bytes) in &files {
        if name.starts_with("runtime/receipts/") {
            let receipt: Value = serde_json::from_slice(bytes).context("reçu d’outil invalide")?;
            let call = receipt["id"].as_str().context("identité de reçu absente")?;
            let node = receipt["nodePath"]
                .as_str()
                .context("nœud du reçu absent")?;
            let expected = digest(format!("{}\0{node}\0{call}", manifest.session_id).as_bytes());
            ensure!(
                name == &format!("runtime/receipts/{expected}.json")
                    && ["started", "waiting", "completed", "failed"]
                        .contains(&receipt["status"].as_str().unwrap_or_default()),
                "identité ou statut de reçu invalide"
            );
        }
    }
    let mut content = Vec::new();
    if header.version >= 2 {
        let bytes = files
            .get("contents.jsonl")
            .context("contents.jsonl absent")?;
        ensure!(
            bytes.is_empty() || bytes.ends_with(b"\n"),
            "journal de contenu incomplet"
        );
        let mut seen = BTreeSet::new();
        for line in std::str::from_utf8(bytes)?.lines() {
            let blob: ContentBlob = serde_json::from_str(line)?;
            ensure!(
                seen.insert(blob.reference.clone()),
                "contenu dupliqué dans l’archive"
            );
            content.push(blob);
        }
    }
    ensure!(
        header.version < 3 || registry.is_some(),
        "registre manquant dans l’archive"
    );
    Ok(Bundle {
        manifest,
        files,
        header,
        events,
        checkpoints,
        checkpoint_headers,
        records,
        content,
        registry: registry.unwrap_or_default(),
    })
}

fn validate_flow_package(run: &Value) -> Result<()> {
    if let Some(value) = run.get("flowPackage").filter(|value| !value.is_null()) {
        let package: zf_flows::package::PackageSnapshot =
            serde_json::from_value(value.clone()).context("invalid frozen flow package")?;
        package.validate().context("invalid frozen flow package")?;
    }
    Ok(())
}

/// Validate content before opening a transaction or publishing any target files.
async fn hydrate_bundle(mut bundle: Bundle, runtime: &dyn ArchiveRuntime) -> Result<Bundle> {
    for checkpoint in &bundle.checkpoints {
        runtime.validate_checkpoint(&serde_json::to_value(checkpoint)?)?;
    }
    if bundle.header.version == 1 {
        validate_flow_package(&bundle.header.run)?;
        return Ok(bundle);
    }
    let store = memory_store().await?;
    store.import_blobs(&bundle.content).await?;
    bundle.registry.validate_contents(&store).await?;
    let reference = bundle.header.run["projectionRef"]
        .as_str()
        .context("projectionRef absent")?;
    let projection = store.resolve_with_limit(reference, MAX_BYTES).await?;
    let mut roots = projection_refs(&projection);
    roots.extend(bundle.registry.roots());
    for (_, event) in &bundle.events {
        roots.extend(projection_refs(event));
    }
    roots.extend(bundle.records.iter().map(|r| r.value_ref.clone()));
    for record in &bundle.records {
        if record.kind.starts_with("revision-")
            || record.kind.starts_with("route-")
            || record.kind.starts_with("runtime-graph-")
            || record.kind == "window-captures"
        {
            roots.extend(projection_refs(&store.resolve(&record.value_ref).await?));
        }
    }
    roots.extend(
        bundle
            .header
            .context_files
            .iter()
            .filter_map(|f| f.content_ref.clone()),
    );
    for header in &bundle.checkpoint_headers {
        roots.extend([header.checkpoint_ref.clone(), header.state_ref.clone()]);
    }
    roots.sort();
    roots.dedup();
    for reference in roots {
        ensure!(
            store.expanded_size(&reference).await? <= MAX_BYTES,
            "contenu matérialisé trop volumineux dans l’archive"
        );
    }
    ensure!(
        projection["id"] == bundle.manifest.session_id
            && projection["status"] == bundle.header.run["status"]
            && projection["workspaceId"] == bundle.header.run["workspaceId"],
        "projection de session incohérente"
    );
    bundle.header.run = session_store::hydrate_run(&store, &projection).await?;
    validate_flow_package(&bundle.header.run)?;
    let _: Composition = serde_json::from_value(bundle.header.run["composition"].clone())
        .context("composition exportée invalide")?;
    for (_, event) in &mut bundle.events {
        *event = session_store::hydrate_event(&store, event).await?;
    }
    CheckpointStore::new(store.clone())
        .await?
        .install_headers(&bundle.checkpoint_headers)
        .await?;
    for header in &bundle.checkpoint_headers {
        runtime.validate_checkpoint(&store.resolve(&header.checkpoint_ref).await?)?;
    }
    for file in &bundle.header.context_files {
        let reference = file
            .content_ref
            .as_deref()
            .context("référence contexte absente")?;
        ensure!(
            digest(&asset_bytes(&store.resolve(reference).await?)?) == file.sha256,
            "contexte exporté altéré"
        );
    }
    for record in &bundle.records {
        let value = store.resolve(&record.value_ref).await?;
        if record.kind == "tool-output" {
            let path = Path::new(value["path"].as_str().context("chemin de sortie absent")?);
            let relative = path
                .strip_prefix(&bundle.header.runtime_root)
                .context("sortie étrangère au runtime exporté")?;
            relative_path(relative.to_str().context("chemin non UTF-8")?)?;
            ensure!(
                relative.starts_with("tool-output"),
                "chemin de sortie invalide"
            );
            asset_bytes(&output_content(&store, &value, &bundle.records).await?)?;
        }
        if record.kind == "tool-output-fragments" {
            asset_bytes(&value)?;
        }
        if record.kind == "receipts" {
            let call = value["id"].as_str().context("identité de reçu absente")?;
            let node = value["nodePath"].as_str().context("nœud du reçu absent")?;
            ensure!(
                record.key
                    == digest(format!("{}\0{node}\0{call}", bundle.manifest.session_id).as_bytes())
                    && ["started", "waiting", "completed", "failed"]
                        .contains(&value["status"].as_str().unwrap_or_default()),
                "identité ou statut de reçu invalide"
            );
            if matches!(value["status"].as_str(), Some("started" | "waiting"))
                && !runtime
                    .resumable_internal_receipt(
                        &store,
                        ArchiveSnapshot {
                            session_id: &bundle.manifest.session_id,
                            run: &bundle.header.run,
                            records: &bundle.records,
                            registry: &bundle.registry,
                        },
                        &value,
                    )
                    .await?
            {
                bundle.header.resume_blocked.push(format!(
                    "Effet d’outil incertain conservé : {}",
                    value["id"]
                ));
            }
        }
    }
    Ok(bundle)
}

fn remap_content(
    blobs: &[ContentBlob],
    roots: &[String],
    old: &Path,
    new: &Path,
) -> Result<(Vec<ContentBlob>, BTreeMap<String, String>)> {
    let mut bodies: BTreeMap<String, Value> = blobs
        .iter()
        .map(|b| (b.reference.clone(), b.body.clone()))
        .collect();
    let mut mapped = BTreeMap::<(String, u8), String>::new();
    for root in roots {
        let mut pending = vec![(root.clone(), 0_u8, false)];
        while let Some((id, mode, ready)) = pending.pop() {
            if mapped.contains_key(&(id.clone(), mode)) {
                continue;
            }
            if mode == 1 {
                mapped.insert((id.clone(), mode), id);
                continue;
            }
            let mut body = bodies
                .get(&id)
                .context("contenu manquant lors du remappage")?
                .clone();
            let mut children = Vec::new();
            match body["kind"].as_str() {
                Some("object") => {
                    for (key, value) in body["entries"]
                        .as_object()
                        .context("objet de contenu invalide")?
                    {
                        let child = value.as_str().context("référence invalide")?.to_owned();
                        let mode = if ["arguments", "args", "config", "composition", "source"]
                            .contains(&key.as_str())
                        {
                            1
                        } else if ["fullOutputPath", "snapshotPath", "recordPath"]
                            .contains(&key.as_str())
                        {
                            2
                        } else {
                            0
                        };
                        children.push((key.clone(), child, mode));
                    }
                }
                Some("sequence") => {
                    for key in ["parent", "item"] {
                        children.push((
                            key.into(),
                            body[key].as_str().context("séquence invalide")?.to_owned(),
                            0,
                        ));
                    }
                }
                Some("string") => children.push((
                    "head".into(),
                    body["head"].as_str().context("chaîne invalide")?.to_owned(),
                    1,
                )),
                Some("scalar" | "array") => {}
                _ => bail!("type de contenu invalide"),
            }
            if !ready && !children.is_empty() {
                pending.push((id, mode, true));
                for (_, child, mode) in children {
                    if !mapped.contains_key(&(child.clone(), mode)) {
                        pending.push((child, mode, false));
                    }
                }
                continue;
            }
            for (key, child, child_mode) in children {
                let next = mapped
                    .get(&(child, child_mode))
                    .context("référence non ordonnée")?;
                if body["kind"] == "object" {
                    body["entries"][key] = json!(next);
                } else {
                    body[key] = json!(next);
                }
            }
            if mode == 2
                && body["kind"] == "scalar"
                && let Some(path) = body["value"].as_str()
                && let Ok(relative) = Path::new(path).strip_prefix(old)
            {
                body["value"] = json!(new.join(relative));
            }
            let next = format!("sha256:{}", digest(&serde_json::to_vec(&body)?));
            bodies.entry(next.clone()).or_insert(body);
            mapped.insert((id, mode), next);
        }
    }
    let result = roots
        .iter()
        .map(|root| {
            Ok((
                root.clone(),
                mapped
                    .get(&(root.clone(), 0))
                    .context("racine manquante")?
                    .clone(),
            ))
        })
        .collect::<Result<_>>()?;
    Ok((
        bodies
            .into_iter()
            .map(|(reference, body)| ContentBlob { reference, body })
            .collect(),
        result,
    ))
}

async fn install_export(target: &Path, files: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    let parent = target.parent().context("export sans dossier parent")?;
    let staging = parent.join(format!(".export-{}", Uuid::new_v4()));
    write_files(&staging, files).await?;
    let backup = parent.join(format!(".previous-{}", Uuid::new_v4()));
    let exists = tokio::fs::symlink_metadata(target).await.is_ok();
    if exists {
        // Never replace a user-edited export or an unrelated directory.
        validate_bundle(collect_files(target).await?).context(
            "l’export existant a été modifié ; conservez-le ailleurs avant de réexporter",
        )?;
        tokio::fs::rename(target, &backup).await?;
    }
    if let Err(error) = tokio::fs::rename(&staging, target).await {
        if exists {
            tokio::fs::rename(&backup, target).await?;
        }
        return Err(error.into());
    }
    #[cfg(unix)]
    tokio::fs::File::open(parent).await?.sync_all().await?;
    if exists {
        tokio::fs::remove_dir_all(backup).await?;
    }
    Ok(())
}

pub async fn export_sessions(
    db: &SqlitePool,
    data: &Path,
    workspace: &Workspace,
    ids: &[String],
    runtime: &dyn ArchiveRuntime,
) -> Result<ExportResponse> {
    ensure!(
        !ids.is_empty() && ids.len() <= 100,
        "sélectionnez entre 1 et 100 sessions"
    );
    ensure!(
        ids.iter().collect::<BTreeSet<_>>().len() == ids.len(),
        "session sélectionnée deux fois"
    );
    let mut bundles = Vec::new();
    let mut total_bytes = 0_u64;
    let mut total_files = 0_usize;
    for id in ids {
        let bundle = capture(db, data, workspace, id, runtime).await?;
        total_bytes += bundle
            .files
            .values()
            .map(|bytes| bytes.len() as u64)
            .sum::<u64>();
        total_files += bundle.files.len();
        ensure!(
            total_bytes <= MAX_BYTES && total_files <= MAX_FILES,
            "sélection trop volumineuse pour une archive unique"
        );
        bundles.push(bundle);
    }
    crate::workspaces::ensure_metadata(&workspace.path).await?;
    let sessions = workspace.path.join(".zedflow/sessions");
    owned_directory(&sessions).await?;
    let mut exports = Vec::new();
    let mut zip_files = BTreeMap::new();
    for bundle in bundles {
        let path = sessions.join(&bundle.manifest.session_id);
        install_export(&path, &bundle.files).await?;
        for (name, bytes) in bundle.files {
            zip_files.insert(format!("{}/{name}", bundle.manifest.session_id), bytes);
        }
        exports.push(ExportedSession {
            session_id: bundle.manifest.session_id,
            path,
            archive_hash: bundle.manifest.archive_hash,
        });
    }
    let archive = tokio::task::spawn_blocking(move || make_zip(zip_files)).await??;
    let downloads = data.join("session-downloads");
    owned_directory(&downloads).await?;
    let id = Uuid::new_v4().to_string();
    let files = BTreeMap::from([
        (format!("{id}.zip"), archive),
        (
            format!("{id}.json"),
            serde_json::to_vec(&json!({"workspaceId":workspace.id}))?,
        ),
    ]);
    for (name, bytes) in files {
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(downloads.join(name))
            .await?;
        file.write_all(&bytes).await?;
        file.sync_all().await?;
    }
    Ok(ExportResponse {
        exports,
        download_url: format!(
            "/api/sessions/exports/{id}.zip?workspaceId={}",
            workspace.id
        ),
    })
}

fn make_zip(files: BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in files {
        writer.start_file(name, options)?;
        writer.write_all(&bytes)?;
    }
    let bytes = writer.finish()?.into_inner();
    ensure!(
        bytes.len() as u64 <= MAX_BYTES,
        "archive ZIP trop volumineuse"
    );
    Ok(bytes)
}

#[derive(Debug)]
pub struct ExportNotFound;
impl std::fmt::Display for ExportNotFound {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Export absent de ce workspace")
    }
}
impl std::error::Error for ExportNotFound {}

async fn download_bytes(path: &Path) -> Result<Vec<u8>> {
    regular_bytes(path).await.map_err(|error| {
        if error.chain().any(|cause| {
            cause
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound)
        }) {
            ExportNotFound.into()
        } else {
            error
        }
    })
}

pub async fn download(data: &Path, id: &str, workspace: &Workspace) -> Result<Vec<u8>> {
    valid_id(id)?;
    let root = data.join("session-downloads");
    let metadata: Value =
        serde_json::from_slice(&download_bytes(&root.join(format!("{id}.json"))).await?)?;
    let owner = metadata
        .get("workspaceId")
        .and_then(Value::as_str)
        .filter(|owner| !owner.is_empty())
        .context("invalid session export metadata owner")?;
    ensure!(owner == workspace.id, ExportNotFound);
    download_bytes(&root.join(format!("{id}.zip"))).await
}

fn read_zip(bytes: Vec<u8>) -> Result<Vec<Bundle>> {
    let mut reader = zip::ZipArchive::new(Cursor::new(bytes))?;
    ensure!(reader.len() <= MAX_FILES, "archive trop volumineuse");
    let mut groups = BTreeMap::<String, BTreeMap<String, Vec<u8>>>::new();
    let mut size = 0_u64;
    for index in 0..reader.len() {
        let mut file = reader.by_index(index)?;
        ensure!(
            file.enclosed_name().is_some()
                && file
                    .unix_mode()
                    .is_none_or(|mode| mode & 0o170000 != 0o120000),
            "chemin ZIP ou lien invalide"
        );
        if file.is_dir() {
            continue;
        }
        let name = file.name().to_owned();
        relative_path(&name)?;
        let (id, path) = name
            .split_once('/')
            .context("ZIP attendu avec un dossier par session")?;
        valid_id(id)?;
        relative_path(path)?;
        size = size
            .checked_add(file.size())
            .context("taille ZIP invalide")?;
        ensure!(size <= MAX_BYTES, "archive trop volumineuse");
        let mut bytes = Vec::new();
        file.by_ref().take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 == file.size(),
            "taille de fichier ZIP invalide"
        );
        ensure!(
            groups
                .entry(id.into())
                .or_default()
                .insert(path.into(), bytes)
                .is_none(),
            "entrée ZIP dupliquée"
        );
    }
    ensure!(!groups.is_empty(), "archive vide");
    groups
        .into_iter()
        .map(|(id, files)| {
            let bundle = validate_bundle(files)?;
            ensure!(bundle.manifest.session_id == id, "identité ZIP incohérente");
            Ok(bundle)
        })
        .collect()
}

async fn read_bundles(path: &Path) -> Result<Vec<Bundle>> {
    let meta = tokio::fs::symlink_metadata(path).await?;
    ensure!(
        !meta.file_type().is_symlink(),
        "lien symbolique non accepté pour l’import"
    );
    if meta.is_dir() {
        return Ok(vec![validate_bundle(collect_files(path).await?)?]);
    }
    if path
        .file_name()
        .is_some_and(|name| name == "session.jsonl" || name == "manifest.json")
    {
        return Ok(vec![validate_bundle(
            collect_files(path.parent().context("dossier d’archive absent")?).await?,
        )?]);
    }
    let bytes = regular_bytes(path).await?;
    tokio::task::spawn_blocking(move || read_zip(bytes)).await?
}

// Only persistence-owned anchors refer to SQL event identities. A model/tool
// payload may itself contain a field named `seq`; its contents stay untouched.
fn remap_sequences(run: &mut Value, mapping: &BTreeMap<i64, i64>) {
    if run["revision"].is_i64() {
        run["revision"] = json!(mapping.values().max().copied().unwrap_or(0));
    }
    for (collection, keys) in [
        ("timeline", ["seq", "updatedSeq"]),
        ("activities", ["startedSeq", "endedSeq"]),
    ] {
        for entry in run[collection].as_array_mut().into_iter().flatten() {
            for key in keys {
                if let Some(seq) = entry[key].as_i64()
                    && let Some(next) = mapping.get(&seq)
                {
                    entry[key] = json!(next);
                }
            }
        }
    }
}

fn remap_runtime_paths(value: &mut Value, old: &Path, new: &Path) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                if matches!(
                    key.as_str(),
                    "fullOutputPath" | "snapshotPath" | "recordPath"
                ) {
                    if let Some(path) = value.as_str()
                        && let Ok(relative) = Path::new(path).strip_prefix(old)
                    {
                        *value = json!(new.join(relative));
                    }
                } else if !matches!(
                    key.as_str(),
                    "arguments" | "args" | "config" | "composition"
                ) {
                    remap_runtime_paths(value, old, new);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                remap_runtime_paths(item, old, new);
            }
        }
        _ => {}
    }
}

fn resume_diagnostics(bundle: &Bundle, runtime: &dyn ArchiveRuntime) -> Vec<String> {
    let mut blocked = bundle.header.resume_blocked.clone();
    if bundle.header.adk_version != ADK_VERSION {
        blocked.push(format!(
            "Version ADK incompatible : {}",
            bundle.header.adk_version
        ));
    }
    if bundle.header.zedflow_version != env!("CARGO_PKG_VERSION") {
        blocked.push(format!(
            "Version Zedflow incompatible : {}",
            bundle.header.zedflow_version
        ));
    }
    let run = &bundle.header.run;
    if run["status"] != "completed"
        && !bundle.checkpoints.iter().any(|cp| {
            run["checkpoint"] == cp.checkpoint_id && cp.thread_id == bundle.manifest.session_id
        })
        && !bundle.checkpoint_headers.iter().any(|cp| {
            run["checkpoint"] == cp.checkpoint_id && cp.thread_id == bundle.manifest.session_id
        })
    {
        blocked.push("Checkpoint de reprise absent".into());
    }
    blocked.extend(runtime.definition_diagnostics(run));
    for (name, bytes) in &bundle.files {
        if name.starts_with("runtime/receipts/")
            && let Ok(receipt) = serde_json::from_slice::<Value>(bytes)
            && receipt["status"] == "started"
        {
            blocked.push(format!(
                "Effet d’outil incertain conservé : {}",
                receipt["id"]
            ));
        }
    }
    blocked.sort();
    blocked.dedup();
    blocked
}

const MISSING_PIECE: &str = "Pièce indisponible dans le workspace cible";

fn refresh_dependency_diagnostics(run: &mut Value, runtime: &dyn ArchiveRuntime) {
    let dependencies = runtime.dependencies(run);
    run["import"]["resourceDiagnostics"] = json!(dependencies.resources);
    let mut blocked = run["import"]["resumeBlocked"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|reason| !reason.starts_with(MISSING_PIECE))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    blocked.extend(dependencies.blocked);
    run["import"]["resumeBlocked"] = json!(blocked);
}

pub fn ensure_resume_allowed(run: &Value) -> Result<()> {
    if let Some(blocked) = run["import"]["resumeBlocked"].as_array()
        && !blocked.is_empty()
    {
        return Err(Conflict(format!(
            "Reprise de l’import indisponible : {}",
            blocked
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" ; ")
        ))
        .into());
    }
    Ok(())
}

/// Finish or roll back an interrupted import before accepting API commands.
/// The marker names only newly allocated identities, never existing sessions.
pub async fn recover_imports(db: &SqlitePool, data: &Path) -> Result<()> {
    let directory = data.join(".session-imports");
    if !tokio::fs::try_exists(&directory).await? {
        return Ok(());
    }
    owned_directory(&directory).await?;
    let mut entries = tokio::fs::read_dir(&directory).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "pending")
        {
            valid_id(
                path.file_stem()
                    .and_then(|stem| stem.to_str())
                    .context("nom de marqueur invalide")?,
            )?;
            ensure!(
                entry.file_type().await?.is_file(),
                "marqueur d’import non ordinaire"
            );
            // A pending marker is written before any imported state. Only its
            // atomic rename to .json authorizes the subsequent side effects.
            tokio::fs::remove_file(path).await?;
            continue;
        }
        ensure!(
            path.extension().is_some_and(|ext| ext == "json"),
            "marqueur d’import inconnu"
        );
        let marker: ImportMarker = serde_json::from_slice(&regular_bytes(&entry.path()).await?)?;
        let mut committed = 0;
        for session in &marker.sessions {
            valid_id(&session.id)?;
            for id in &session.checkpoint_ids {
                valid_id(id)?;
            }
            let existing: Option<String> =
                sqlx::query_scalar("SELECT document FROM runs WHERE id=?")
                    .bind(&session.id)
                    .fetch_optional(db)
                    .await?;
            if let Some(existing) = existing {
                let run: Value = serde_json::from_str(&existing)?;
                ensure!(
                    run["import"]["archiveHash"] == session.archive_hash,
                    "collision pendant la récupération d’un import"
                );
                committed += 1;
            }
        }
        ensure!(
            committed == 0 || committed == marker.sessions.len(),
            "commit d’import partiel inattendu"
        );
        if committed == 0 {
            let cp_path = data.join("checkpoints.db");
            if tokio::fs::try_exists(&cp_path).await? {
                let cp =
                    SqlitePool::connect(&format!("sqlite://{}?mode=rw", cp_path.display())).await?;
                for session in &marker.sessions {
                    for checkpoint in &session.checkpoint_ids {
                        sqlx::query("DELETE FROM graph_checkpoints WHERE id=? AND (thread_id=? OR thread_id LIKE ?)").bind(checkpoint).bind(&session.id).bind(format!("{}/%",session.id)).execute(&cp).await?;
                    }
                }
                cp.close().await;
            }
            let cas:i64=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='zf_records')").fetch_one(db).await?;
            for session in &marker.sessions {
                if cas != 0 {
                    sqlx::query("DELETE FROM zf_records WHERE scope=?")
                        .bind(&session.id)
                        .execute(db)
                        .await?;
                    sqlx::query("DELETE FROM zf_checkpoints WHERE thread_id=? OR substr(thread_id,1,length(?)+1)=? || '/' ").bind(&session.id).bind(&session.id).bind(&session.id).execute(db).await?;
                }
                let target = data.join("runs").join(&session.id);
                if tokio::fs::try_exists(&target).await? {
                    tokio::fs::remove_dir_all(target).await?;
                }
            }
        }
        tokio::fs::remove_file(entry.path()).await?;
    }
    Ok(())
}

/// The caller serializes application commands; import never starts an executor.
pub async fn import_sessions(
    db: &SqlitePool,
    data: &Path,
    workspace: &Workspace,
    path: &Path,
    runtime: &dyn ArchiveRuntime,
) -> Result<ImportResponse> {
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        workspace.path.join(path)
    };
    let bundles = read_bundles(&path).await?;
    let mut pending = Vec::new();
    let mut result = ImportResponse {
        runs: vec![],
        imported: 0,
        unchanged: 0,
    };
    for bundle in bundles {
        let bundle = hydrate_bundle(bundle, runtime).await?;
        let existing: Option<String> = sqlx::query_scalar("SELECT document FROM runs WHERE id=?")
            .bind(&bundle.manifest.session_id)
            .fetch_optional(db)
            .await?;
        if existing.is_some() {
            let mut run: Value = session_store::load(db, &bundle.manifest.session_id).await?;
            if run["workspaceId"] == workspace.id
                && (run["import"]["archiveHash"] == bundle.manifest.archive_hash
                    || capture(db, data, workspace, &bundle.manifest.session_id, runtime)
                        .await
                        .is_ok_and(|current| {
                            current.manifest.archive_hash == bundle.manifest.archive_hash
                        }))
            {
                if run["import"].is_object() {
                    refresh_dependency_diagnostics(&mut run, runtime);
                    session_store::save(db, &bundle.manifest.session_id, &run).await?;
                }
                result.unchanged += 1;
                result.runs.push(run);
                continue;
            }
            return Err(Conflict(format!("La session {} existe déjà avec une autre origine ou version ; aucune donnée remplacée",bundle.manifest.session_id)).into());
        }
        pending.push(bundle);
    }
    if pending.is_empty() {
        return Ok(result);
    }
    let store = ContentStore::new(db.clone()).await?;
    crate::data::DataRegistry::new(db.clone(), store.clone(), "archive-schema").await?;
    let cp = CheckpointStore::new(store.clone()).await?;
    let mut checkpoint_ids = BTreeSet::new();
    for bundle in &pending {
        let ids = bundle
            .checkpoints
            .iter()
            .map(|cp| (&cp.checkpoint_id, &cp.thread_id))
            .chain(
                bundle
                    .checkpoint_headers
                    .iter()
                    .map(|cp| (&cp.checkpoint_id, &cp.thread_id)),
            );
        for (id, thread) in ids {
            ensure!(
                checkpoint_ids.insert(id),
                "checkpoint partagé entre plusieurs sessions de l’archive"
            );
            let exists: i64 = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM zf_checkpoints WHERE checkpoint_id=? OR thread_id=?)",
            )
            .bind(id)
            .bind(thread)
            .fetch_one(db)
            .await?;
            if exists != 0 {
                return Err(Conflict("Identité de checkpoint déjà utilisée".into()).into());
            }
        }
        let target = data.join("runs").join(&bundle.manifest.session_id);
        ensure!(
            !tokio::fs::try_exists(&target).await?,
            "données d’exécution déjà présentes pour cette identité"
        );
        ensure!(
            store.records(&bundle.manifest.session_id).await?.is_empty(),
            "records déjà présents pour cette identité"
        );
    }
    owned_directory(&data.join("runs")).await?;
    let marker_directory = data.join(".session-imports");
    owned_directory(&marker_directory).await?;
    let marker_path = marker_directory.join(format!("{}.json", Uuid::new_v4()));
    let marker = ImportMarker {
        sessions: pending
            .iter()
            .map(|bundle| ImportingSession {
                id: bundle.manifest.session_id.clone(),
                archive_hash: bundle.manifest.archive_hash.clone(),
                checkpoint_ids: bundle
                    .checkpoints
                    .iter()
                    .map(|cp| cp.checkpoint_id.clone())
                    .chain(
                        bundle
                            .checkpoint_headers
                            .iter()
                            .map(|cp| cp.checkpoint_id.clone()),
                    )
                    .collect(),
            })
            .collect(),
    };
    let mut marker_file = tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(marker_path.with_extension("pending"))
        .await?;
    marker_file.write_all(&serde_json::to_vec(&marker)?).await?;
    marker_file.sync_all().await?;
    tokio::fs::rename(marker_path.with_extension("pending"), &marker_path).await?;
    #[cfg(unix)]
    tokio::fs::File::open(&marker_directory)
        .await?
        .sync_all()
        .await?;
    let operation: Result<()> = async {
        let base:i64=sqlx::query_scalar("SELECT COALESCE(MAX(seq),0) FROM events").fetch_one(db).await?;
        let mut next_seq=base;
        let mut prepared_runs=Vec::new();
        let mut prepared_registries=Vec::new();
        let mut prepared_events=Vec::new();
        for bundle in pending {
            let id = &bundle.manifest.session_id;
            let target = data.join("runs").join(id);
            let mut runtime_files = BTreeMap::new();
            for (name,bytes) in &bundle.files {
                if let Some(name) = name.strip_prefix("runtime/") {
                    let bytes=if name.ends_with(".json") {
                        let mut value: Value=serde_json::from_slice(bytes)?;
                        remap_runtime_paths(&mut value,&bundle.header.runtime_root,&target);
                        serde_json::to_vec(&value)?
                    } else { bytes.clone() };
                    runtime_files.insert(name.into(),bytes);
                }
                else if name.starts_with("context/") { runtime_files.insert(name.clone(),bytes.clone()); }
            }
            write_files(&target,&runtime_files).await?;
            if bundle.header.version>=2 {
                let mut roots=Vec::new();
                for header in &bundle.checkpoint_headers {roots.extend([header.checkpoint_ref.clone(),header.state_ref.clone()]);}
                roots.extend(bundle.records.iter().map(|r|r.value_ref.clone()));
                let (content,mapping)=remap_content(&bundle.content,&roots,&bundle.header.runtime_root,&target)?;
                store.import_blobs(&content).await?;
                let mut headers=bundle.checkpoint_headers.clone();
                for header in &mut headers {header.checkpoint_ref=mapping[&header.checkpoint_ref].clone();header.state_ref=mapping[&header.state_ref].clone();}
                cp.install_headers(&headers).await?;
                for record in &bundle.records {
                    if record.kind=="tool-output" {
                        let mut value=store.resolve(&mapping[&record.value_ref]).await?;
                        let relative=Path::new(value["path"].as_str().context("sortie sans chemin")?).strip_prefix(&bundle.header.runtime_root)?;
                        let path=target.join(relative);tokio::fs::create_dir_all(path.parent().context("sortie sans parent")?).await?;
                        let mut file=tokio::fs::OpenOptions::new().create_new(true).write(true).open(&path).await?;
                        file.write_all(&asset_bytes(&output_content(&store,&value,&bundle.records).await?)?).await?;file.sync_all().await?;
                        value["path"]=json!(path);store.put_record(id,&record.kind,&record.key,&value).await?;
                    } else {
                        sqlx::query("INSERT INTO zf_records(scope,kind,key,value_ref) VALUES(?,?,?,?)").bind(id).bind(&record.kind).bind(&record.key).bind(&mapping[&record.value_ref]).execute(db).await?;
                    }
                }
                for file in &bundle.header.context_files {
                    let bytes=asset_bytes(&store.resolve(file.content_ref.as_deref().context("référence contexte absente")?).await?)?;
                    let path=target.join(&file.archive_path);tokio::fs::create_dir_all(path.parent().context("contexte sans parent")?).await?;
                    tokio::fs::write(&path,bytes).await?;tokio::fs::File::open(path).await?.sync_all().await?;
                }
            }
            sync_directories(&target).await?;
            for checkpoint in &bundle.checkpoints {
                let mut checkpoint = checkpoint.clone();
                let mut state = serde_json::to_value(&checkpoint.state)?;
                remap_runtime_paths(&mut state,&bundle.header.runtime_root,&target);
                checkpoint.state = state;
                let value = serde_json::to_value(&checkpoint)?;
                runtime.validate_checkpoint(&value)?;
                cp.save(&value).await?;
            }
            let mut mapping = BTreeMap::from([(0,0)]);
            for (old,event) in &bundle.events {
                let mut event = event.clone();
                remap_runtime_paths(&mut event,&bundle.header.runtime_root,&target);
                next_seq+=1;
                let compact=session_store::compact_event(&store,&event).await?;
                prepared_events.push((next_seq,id.clone(),compact.to_string()));
                mapping.insert(*old,next_seq);
            }
            let mut run = bundle.header.run.clone();
            let blocked = resume_diagnostics(&bundle, runtime);
            remap_sequences(&mut run,&mapping);
            remap_runtime_paths(&mut run,&bundle.header.runtime_root,&target);
            for category in ["instructions","skills"] {
                if let Some(entries) = run["context"][category].as_array_mut() {
                    for entry in entries {
                        if let Some(file) = bundle.header.context_files.iter().find(|file|entry["path"].as_str().is_some_and(|path|Path::new(path)==file.source_path)) { entry["path"]=json!(target.join(&file.archive_path)); }
                    }
                }
            }
            run["import"]=json!({"archiveHash":bundle.manifest.archive_hash,"sourceSessionId":id,"sourceWorkspace":{"id":run["workspaceId"],"path":run["workspacePath"]},"importedAt":now(),"resumeBlocked":blocked});
            run["workspaceId"]=json!(workspace.id);run["workspacePath"]=json!(workspace.path);run["context"]["cwd"]=json!(workspace.path);
            run["activeNodes"]=json!([]);run["activeNode"]=Value::Null;
            refresh_dependency_diagnostics(&mut run, runtime);
            prepared_runs.push((id.clone(),session_store::prepare(&store,&run).await?));
            prepared_registries.push((id.clone(),bundle.registry));
            result.runs.push(run);result.imported+=1;
        }
        let mut tx=db.begin().await?;
        let current:i64=sqlx::query_scalar("SELECT COALESCE(MAX(seq),0) FROM events").fetch_one(&mut *tx).await?;
        ensure!(current==base,"Le journal a évolué pendant l’import ; réessayez au prochain point d’arrêt");
        for (id,registry) in prepared_registries {registry.install(&mut tx,&id).await?;}
        for (seq,id,event) in prepared_events {sqlx::query("INSERT INTO events(seq,run,document) VALUES(?,?,?)").bind(seq).bind(id).bind(event).execute(&mut *tx).await?;}
        for (id,prepared) in prepared_runs {session_store::write(&mut tx,&id,&prepared).await?;}
        tx.commit().await?;
        Ok(())
    }.await;
    if let Err(error) = operation {
        recover_imports(db, data)
            .await
            .context("récupération de l’import inachevée ; marqueur conservé")?;
        return Err(error);
    }
    tokio::fs::remove_file(marker_path).await?;
    Ok(result)
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rebase_only_persistence_fields_and_keep_user_payloads_exact() {
        let mut run = json!({
            "timeline":[{"seq":3,"updatedSeq":5,"activity":{"arguments":{"seq":3,"recordPath":"/source/record.json"},"result":{"fullOutputPath":"/source/tool-output/log.txt"}}}],
            "activities":[{"startedSeq":2,"endedSeq":5,"output":{"seq":3}}],
            "composition":{"nodes":[{"data":{"config":{"seq":3,"snapshotPath":"/source/user-file"}}}]},
            "state":{"seq":3},"messages":[{"text":"/source/tool-output/log.txt"}]
        });
        remap_sequences(&mut run, &BTreeMap::from([(2, 102), (3, 103), (5, 105)]));
        remap_runtime_paths(&mut run, Path::new("/source"), Path::new("/target"));
        assert_eq!(run["timeline"][0]["seq"], 103);
        assert_eq!(run["timeline"][0]["updatedSeq"], 105);
        assert_eq!(run["activities"][0]["startedSeq"], 102);
        assert_eq!(run["activities"][0]["endedSeq"], 105);
        assert_eq!(run["activities"][0]["output"]["seq"], 3);
        assert_eq!(
            run["timeline"][0]["activity"]["arguments"],
            json!({"seq":3,"recordPath":"/source/record.json"})
        );
        assert_eq!(
            run["timeline"][0]["activity"]["result"]["fullOutputPath"],
            "/target/tool-output/log.txt"
        );
        assert_eq!(run["state"]["seq"], 3);
        assert_eq!(
            run["composition"]["nodes"][0]["data"]["config"]["snapshotPath"],
            "/source/user-file"
        );
        assert_eq!(run["messages"][0]["text"], "/source/tool-output/log.txt");
    }

    #[tokio::test]
    async fn archive_closure_shares_history_and_remaps_only_technical_paths() {
        let store = memory_store().await.unwrap();
        let mut history = Vec::new();
        let mut roots = Vec::new();
        let mut logical_bytes = 0;
        let large = "Texte modèle exact /source/output.log 🦀".repeat(200);
        for index in 0..40 {
            history.push(json!({"index":index,"fullOutputPath":"/source/tool-output/full.log","arguments":{"fullOutputPath":"/source/user-file"},"text":large}));
            let value = json!({"history":history,"config":{"snapshotPath":"/source/keep"}});
            logical_bytes += serde_json::to_vec(&value).unwrap().len();
            roots.push(store.intern(&value).await.unwrap());
        }
        let blobs = store.export_blobs(&roots).await.unwrap();
        assert!(
            serde_json::to_vec(&blobs).unwrap().len() < logical_bytes / 3,
            "archive grows with unique content, not repeated history snapshots"
        );
        let (remapped, mapping) =
            remap_content(&blobs, &roots, Path::new("/source"), Path::new("/target")).unwrap();
        let target = memory_store().await.unwrap();
        target.import_blobs(&remapped).await.unwrap();
        let last = roots.last().unwrap();
        let mut expected = store.resolve(last).await.unwrap();
        remap_runtime_paths(&mut expected, Path::new("/source"), Path::new("/target"));
        let restored = target.resolve(&mapping[last]).await.unwrap();
        assert_eq!(restored, expected);
        assert_eq!(
            restored["history"][0]["arguments"]["fullOutputPath"],
            "/source/user-file"
        );
        assert_eq!(restored["history"][0]["text"], large);
    }

    #[test]
    fn unsafe_zip_paths_and_symbolic_links_are_rejected_before_extraction() {
        let bytes = make_zip(BTreeMap::from([(
            "../escape".into(),
            b"never write".to_vec(),
        )]))
        .unwrap();
        assert!(
            read_zip(bytes)
                .err()
                .unwrap()
                .to_string()
                .contains("invalide")
        );
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .add_symlink("link", "/tmp", zip::write::SimpleFileOptions::default())
            .unwrap();
        assert!(
            read_zip(writer.finish().unwrap().into_inner())
                .err()
                .unwrap()
                .to_string()
                .contains("invalide")
        );
    }

    #[tokio::test]
    async fn recovery_rolls_back_uncommitted_imports_and_preserves_committed_sessions() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path();
        let db = SqlitePool::connect(&format!(
            "sqlite://{}?mode=rwc",
            data.join("zedflow.db").display()
        ))
        .await
        .unwrap();
        sqlx::query("CREATE TABLE runs(id TEXT PRIMARY KEY, document TEXT NOT NULL)")
            .execute(&db)
            .await
            .unwrap();
        let cp = CheckpointStore::new(ContentStore::new(db.clone()).await.unwrap())
            .await
            .unwrap();
        let mut imported = Vec::new();
        for committed in [false, true] {
            let id = Uuid::new_v4().to_string();
            let root_checkpoint = json!({"checkpoint_id":Uuid::new_v4().to_string(),"thread_id":id,"state":{},"step":1,"pending_nodes":["child"],"created_at":"2026-09-15T12:00:00Z"});
            let child_checkpoint = json!({"checkpoint_id":Uuid::new_v4().to_string(),"thread_id":format!("{id}/child@1"),"state":{},"step":2,"pending_nodes":["effect"],"created_at":"2026-09-15T12:00:00Z"});
            for checkpoint in [&root_checkpoint, &child_checkpoint] {
                cp.save(checkpoint).await.unwrap();
            }
            let target = data.join("runs").join(&id);
            tokio::fs::create_dir_all(&target).await.unwrap();
            tokio::fs::write(target.join("kept.txt"), b"imported")
                .await
                .unwrap();
            let marker = ImportMarker {
                sessions: vec![ImportingSession {
                    id: id.clone(),
                    archive_hash: "hash".into(),
                    checkpoint_ids: vec![
                        root_checkpoint["checkpoint_id"]
                            .as_str()
                            .unwrap()
                            .to_owned(),
                        child_checkpoint["checkpoint_id"]
                            .as_str()
                            .unwrap()
                            .to_owned(),
                    ],
                }],
            };
            tokio::fs::create_dir_all(data.join(".session-imports"))
                .await
                .unwrap();
            tokio::fs::write(
                data.join(".session-imports")
                    .join(format!("{}.json", Uuid::new_v4())),
                serde_json::to_vec(&marker).unwrap(),
            )
            .await
            .unwrap();
            if committed {
                sqlx::query("INSERT INTO runs VALUES(?,?)")
                    .bind(&id)
                    .bind(json!({"id":id,"import":{"archiveHash":"hash"}}).to_string())
                    .execute(&db)
                    .await
                    .unwrap();
            }
            imported.push((committed, target, root_checkpoint, child_checkpoint));
        }
        // Interrupted marker writes cannot have started an import.
        tokio::fs::write(
            data.join(".session-imports")
                .join(format!("{}.pending", Uuid::new_v4())),
            b"{partial",
        )
        .await
        .unwrap();
        recover_imports(&db, data).await.unwrap();
        for (committed, target, root_checkpoint, child_checkpoint) in imported {
            assert_eq!(target.exists(), committed);
            for checkpoint in [root_checkpoint, child_checkpoint] {
                assert_eq!(
                    cp.load_by_id(checkpoint["checkpoint_id"].as_str().unwrap())
                        .await
                        .unwrap()
                        .is_some(),
                    committed
                );
            }
        }
        assert_eq!(
            std::fs::read_dir(data.join(".session-imports"))
                .unwrap()
                .count(),
            0
        );
        recover_imports(&db, data).await.unwrap();
    }
}
