//! Explicitly accepted source revisions, with a durable filesystem/SQLite handoff.
//! Metadata links accepted hashes; the Rust file remains the source definition.
use super::context_store::{self, Conflict, SourceFile};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub(crate) const MARKER: &str = ".source-acceptance.json";
const MAX_JOURNAL_BYTES: usize = 16 * 1024 * 1024;
fn source_limit(segments: &[String]) -> usize {
    if segments.first().is_some_and(|s| s == "flows") {
        2 * 1024 * 1024
    } else {
        zf_context::context_source::MAX_SOURCE_BYTES
    }
}
fn source_hash(path: &Path, limit: usize) -> Result<Option<String>> {
    match regular_bytes(path, limit) {
        Ok(bytes) => Ok(Some(context_store::hash(&bytes))),
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FilePrecondition {
    pub path: PathBuf,
    pub hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AcceptedRevision {
    version: u32,
    id: String,
    namespace: String,
    segments: Vec<String>,
    key: String,
    hash: String,
    parent: Option<String>,
    previous_hash: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AcceptanceIntent {
    version: u32,
    revision: AcceptedRevision,
    source: String,
    publication: Option<Value>,
    installed: bool,
}

pub struct PendingAcceptance {
    workspace: PathBuf,
    intent: AcceptanceIntent,
    _lock: File,
}
impl PendingAcceptance {
    pub fn id(&self) -> &str {
        &self.intent.revision.id
    }
    pub fn publication(&self) -> Option<&Value> {
        self.intent.publication.as_ref()
    }
    pub fn source_hash(&self) -> &str {
        &self.intent.revision.hash
    }
    pub async fn finish(self) -> Result<SourceFile> {
        tokio::task::spawn_blocking(move || {
            // The worker outlives a cancelled caller; it must own the lock too.
            let _lock = self._lock;
            let root = context_store::directory(&self.workspace, &[], false)?
                .context("Source root disappeared")?;
            let path = source_path(&self.workspace, &self.intent.revision)?;
            // The accepted definition may already be in SQLite. A subsequent
            // external edit remains unaccepted and survives this final cleanup.
            let bytes = regular_bytes(&path, source_limit(&self.intent.revision.segments));
            let result = bytes.and_then(|bytes| {
                Ok(SourceFile {
                    key: self.intent.revision.key.clone(),
                    path: path.clone(),
                    hash: context_store::hash(&bytes),
                    source: Some(String::from_utf8(bytes)?),
                    diagnostics: vec![],
                })
            });
            fs::remove_file(root.join(MARKER))?;
            File::open(root)?.sync_all()?;
            let result = result?;
            ensure!(
                result.hash == self.intent.revision.hash && result.diagnostics.is_empty(),
                Conflict("Source changed after acceptance; external bytes were preserved")
            );
            Ok(result)
        })
        .await
        .context("Source acceptance finalizer failed")?
    }
}

fn regular_bytes(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() <= limit as u64,
        "Invalid source acceptance metadata file"
    );
    let mut options = OpenOptions::new();
    options.read(true);
    context_store::no_follow(&mut options);
    let mut bytes = Vec::new();
    options
        .open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= limit,
        "Source acceptance metadata exceeds its byte limit"
    );
    Ok(bytes)
}
fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    match regular_bytes(path, MAX_JOURNAL_BYTES) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}
fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("Metadata parent absent")?;
    let temporary = parent.join(format!(".accepted-{}.tmp", uuid::Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    if let Ok(meta) = fs::symlink_metadata(path) {
        ensure!(
            meta.is_file() && !meta.file_type().is_symlink(),
            "Acceptance target is not a regular file"
        );
    }
    fs::rename(&temporary, path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}
fn metadata_root(workspace: &Path, create: bool) -> Result<Option<PathBuf>> {
    context_store::directory(workspace, &["source-history".into()], create)
}
fn head_name(namespace: &str, segments: &[String], key: &str) -> Result<String> {
    Ok(format!(
        "{}.head.json",
        context_store::hash(&serde_json::to_vec(&(namespace, segments, key))?)
    ))
}
fn valid_location(segments: &[String], key: &str) -> bool {
    if segments.first().is_some_and(|s| s == "flows") {
        let normal =
            |s: &str| !s.is_empty() && s != "." && s != ".." && !s.contains(['/', '\\', '\0']);
        segments.len() <= 256 && segments.iter().all(|s| normal(s)) && normal(key)
    } else {
        segments.len() <= 8
            && segments.iter().all(|s| zf_context::context::valid_id(s))
            && zf_context::context::valid_id(key)
    }
}
fn validate_revision(revision: &AcceptedRevision) -> Result<()> {
    ensure!(
        revision.version == 1
            && matches!(revision.namespace.as_str(), ".zedflow" | ".agents")
            && !revision.segments.is_empty()
            && valid_location(&revision.segments, &revision.key),
        "Invalid accepted source identity"
    );
    uuid::Uuid::parse_str(&revision.id)?;
    if let Some(parent) = &revision.parent {
        uuid::Uuid::parse_str(parent)?;
    }
    for hash in std::iter::once(&revision.hash).chain(revision.previous_hash.as_ref()) {
        ensure!(
            hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
            "Invalid accepted source hash"
        );
    }
    Ok(())
}
fn source_path(workspace: &Path, revision: &AcceptedRevision) -> Result<PathBuf> {
    validate_revision(revision)?;
    Ok(context_store::namespace_directory(
        workspace,
        &revision.namespace,
        &revision.segments,
        true,
    )?
    .context("Source directory absent")?
    .join(format!("{}.rs", revision.key)))
}
fn intent(root: &Path) -> Result<Option<AcceptanceIntent>> {
    let Some(intent): Option<AcceptanceIntent> = read(&root.join(MARKER))? else {
        return Ok(None);
    };
    validate_revision(&intent.revision)?;
    ensure!(
        intent.version == 1
            && intent.source.len() <= source_limit(&intent.revision.segments)
            && context_store::hash(intent.source.as_bytes()) == intent.revision.hash,
        "Source acceptance intent is invalid"
    );
    Ok(Some(intent))
}
fn save_intent(root: &Path, intent: &AcceptanceIntent) -> Result<()> {
    write(&root.join(MARKER), &serde_json::to_vec(intent)?)
}

fn install(workspace: &Path, root: &Path, intent: &mut AcceptanceIntent) -> Result<()> {
    if intent.installed {
        return Ok(());
    }
    let path = source_path(workspace, &intent.revision)?;
    let actual = source_hash(&path, source_limit(&intent.revision.segments))?;
    // A crash may have happened after the rename and before the installed bit.
    if actual.as_deref() != Some(&intent.revision.hash) {
        ensure!(
            actual == intent.revision.previous_hash,
            Conflict("Source changed before acceptance; external bytes were preserved")
        );
        if actual.is_none() {
            let parent = path.parent().context("Source parent absent")?;
            let staged = parent.join(format!(".accepted-source-{}", intent.revision.id));
            if !staged.exists() {
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&staged)?;
                file.write_all(intent.source.as_bytes())?;
                file.sync_all()?;
            }
            ensure!(
                context_store::hash(&regular_bytes(
                    &staged,
                    source_limit(&intent.revision.segments)
                )?) == intent.revision.hash,
                "Acceptance staging source changed"
            );
            fs::hard_link(&staged, &path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    anyhow::Error::new(Conflict("Source was concurrently created"))
                } else {
                    e.into()
                }
            })?;
            fs::remove_file(staged)?;
            File::open(parent)?.sync_all()?;
        } else {
            let parent = path.parent().context("Source parent absent")?;
            let staged = parent.join(format!(".accepted-replace-{}.tmp", uuid::Uuid::new_v4()));
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&staged)?;
            file.write_all(intent.source.as_bytes())?;
            file.sync_all()?;
            drop(file);
            if source_hash(&path, source_limit(&intent.revision.segments))?
                != intent.revision.previous_hash
            {
                fs::remove_file(staged)?;
                anyhow::bail!(Conflict("Source changed while staging acceptance"));
            }
            fs::rename(staged, &path)?;
            File::open(parent)?.sync_all()?;
        }
    }
    let history = metadata_root(workspace, true)?.context("Acceptance history absent")?;
    let revision_path = history.join(format!("{}.json", intent.revision.id));
    let bytes = serde_json::to_vec(&intent.revision)?;
    if let Some(previous) = read::<Value>(&revision_path)? {
        ensure!(
            previous == serde_json::to_value(&intent.revision)?,
            "Accepted revision identity was reused"
        );
    } else {
        write(&revision_path, &bytes)?;
    }
    write(
        &history.join(head_name(
            &intent.revision.namespace,
            &intent.revision.segments,
            &intent.revision.key,
        )?),
        &bytes,
    )?;
    intent.installed = true;
    save_intent(root, intent)?;
    Ok(())
}

/// Called only while the workspace file lock is exclusive. Pure file saves can
/// finish here; an intent with a SQLite batch requires the daemon recovery hook.
pub(crate) fn recover_files_locked(workspace: &Path, root: &Path) -> Result<()> {
    let Some(mut intent) = intent(root)? else {
        return Ok(());
    };
    install(workspace, root, &mut intent)?;
    ensure!(
        intent.publication.is_none(),
        Conflict("A source publication requires daemon recovery before catalogue access")
    );
    fs::remove_file(root.join(MARKER))?;
    File::open(root)?.sync_all()?;
    Ok(())
}

pub async fn begin(
    workspace: PathBuf,
    segments: Vec<String>,
    key: String,
    source: String,
    expected_hash: Option<String>,
    publication: Option<Value>,
    preconditions: Vec<FilePrecondition>,
) -> Result<PendingAcceptance> {
    begin_internal(
        workspace,
        ".zedflow".into(),
        segments,
        key,
        source,
        expected_hash,
        publication,
        preconditions,
    )
    .await
}

/// Existing `.agents/flows` files and global flow roots keep their exact location.
/// The FlowStore resolves and authorizes the target before calling this writer.
pub async fn begin_flow(
    target: PathBuf,
    lock_workspace: PathBuf,
    source: String,
    expected_hash: Option<String>,
    publication: Option<Value>,
    preconditions: Vec<FilePrecondition>,
) -> Result<PendingAcceptance> {
    let relative = target
        .strip_prefix(&lock_workspace)
        .context("Flow target lies outside its source root")?;
    let parts: Vec<_> = relative
        .components()
        .map(|part| match part {
            std::path::Component::Normal(value) => value.to_str().context("Non UTF-8 flow path"),
            _ => anyhow::bail!("Invalid flow target component"),
        })
        .collect::<Result<_>>()?;
    ensure!(
        parts.len() >= 3 && matches!(parts[0], ".zedflow" | ".agents") && parts[1] == "flows",
        "Flow target must remain in a recognized flow namespace"
    );
    let key = parts
        .last()
        .context("Flow filename absent")?
        .strip_suffix(".rs")
        .context("Flow source must end in .rs")?
        .to_owned();
    let namespace = parts[0].to_owned();
    let segments = parts[1..parts.len() - 1]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    begin_internal(
        lock_workspace,
        namespace,
        segments,
        key,
        source,
        expected_hash,
        publication,
        preconditions,
    )
    .await
}
#[allow(clippy::too_many_arguments)] // Two public, checked location forms share this transaction implementation.
async fn begin_internal(
    workspace: PathBuf,
    namespace: String,
    segments: Vec<String>,
    key: String,
    source: String,
    expected_hash: Option<String>,
    publication: Option<Value>,
    preconditions: Vec<FilePrecondition>,
) -> Result<PendingAcceptance> {
    ensure!(
        !segments.is_empty() && valid_location(&segments, &key),
        "Invalid source location"
    );
    ensure!(
        source.len() <= source_limit(&segments),
        "Source exceeds 1 MiB"
    );
    tokio::task::spawn_blocking(move || {
        let lock = context_store::workspace_lock(&workspace, true, true)?
            .context("Source workspace absent")?;
        let root =
            context_store::directory(&workspace, &[], false)?.context("Source workspace absent")?;
        let history = metadata_root(&workspace, true)?.context("Acceptance history absent")?;
        let prior: Option<AcceptedRevision> =
            read(&history.join(head_name(&namespace, &segments, &key)?))?;
        if let Some(prior) = &prior {
            validate_revision(prior)?;
            ensure!(
                prior.namespace == namespace && prior.segments == segments && prior.key == key,
                "Accepted source head belongs to another artifact"
            );
        }
        let revision = AcceptedRevision {
            version: 1,
            id: uuid::Uuid::new_v4().to_string(),
            namespace,
            segments,
            key,
            hash: context_store::hash(source.as_bytes()),
            parent: prior.map(|p| p.id),
            previous_hash: expected_hash,
        };
        validate_revision(&revision)?;
        ensure!(
            source.len() <= source_limit(&revision.segments),
            "Source exceeds 1 MiB"
        );
        let path = source_path(&workspace, &revision)?;
        ensure!(
            source_hash(&path, source_limit(&revision.segments))? == revision.previous_hash,
            Conflict("Source changed since preflight")
        );
        for condition in preconditions {
            ensure!(
                source_hash(&condition.path, zf_flows::package::MAX_PACKAGE_FILE_BYTES)?.as_deref()
                    == Some(condition.hash.as_str()),
                Conflict("A dependent definition changed during source preflight")
            );
        }
        let mut intent = AcceptanceIntent {
            version: 1,
            revision,
            source,
            publication,
            installed: false,
        };
        save_intent(&root, &intent)?;
        install(&workspace, &root, &mut intent)?;
        Ok(PendingAcceptance {
            workspace,
            intent,
            _lock: lock,
        })
    })
    .await
    .context("Source acceptance writer failed")?
}

/// Acquire a pending filesystem/SQLite handoff without trying to consume its own
/// marker through the ordinary catalogue reader. The caller must replay the batch.
pub async fn recover(workspace: PathBuf) -> Result<Option<PendingAcceptance>> {
    tokio::task::spawn_blocking(move || {
        let Some(lock) = context_store::workspace_lock_raw(&workspace, false, true)? else {
            return Ok(None);
        };
        let root =
            context_store::directory(&workspace, &[], false)?.context("Source workspace absent")?;
        crate::flow_packages::ensure_no_lifecycle_locked(&root)?;
        context_store::recover_locked(&workspace, &root)?;
        let Some(mut intent) = intent(&root)? else {
            return Ok(None);
        };
        install(&workspace, &root, &mut intent)?;
        Ok(Some(PendingAcceptance {
            workspace,
            intent,
            _lock: lock,
        }))
    })
    .await
    .context("Source publication recovery failed")?
}

/// Explicit Save may accept externally edited bytes. Its dependent references
/// must still name this artifact's known lineage, or these exact external bytes.
pub(crate) fn check_reference_locked(
    workspace: &Path,
    segments: &[String],
    key: &str,
    current: &str,
    requested: Option<&str>,
    accepting: bool,
) -> Result<()> {
    let Some(history) = metadata_root(workspace, false)? else {
        ensure!(
            requested.is_none_or(|r| r == current),
            Conflict("Selected source revision changed")
        );
        return Ok(());
    };
    let Some(mut accepted): Option<AcceptedRevision> =
        read(&history.join(head_name(".zedflow", segments, key)?))?
    else {
        ensure!(
            requested.is_none_or(|r| r == current),
            Conflict("Selected source revision changed")
        );
        return Ok(());
    };
    ensure!(
        accepting || accepted.hash == current,
        Conflict("External source changes must be explicitly accepted before execution")
    );
    if requested.is_none() || (accepting && requested == Some(current)) {
        return Ok(());
    }
    let mut seen = BTreeSet::new();
    for _ in 0..10000 {
        validate_revision(&accepted)?;
        ensure!(
            accepted.namespace == ".zedflow"
                && accepted.key == key
                && accepted.segments == segments
                && seen.insert(accepted.id.clone()),
            "Acceptance history identity or cycle is invalid"
        );
        if requested == Some(accepted.hash.as_str())
            || requested == accepted.previous_hash.as_deref()
        {
            return Ok(());
        }
        let Some(parent) = accepted.parent else { break };
        accepted = read(&history.join(format!("{parent}.json")))?
            .context("Accepted source parent is absent")?;
    }
    anyhow::bail!(Conflict(
        "Selected hash is not an accepted ancestor of this source"
    ))
}
