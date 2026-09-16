//! Rust source catalogues share one checked writer and optimistic file revision.
//! Parsing remains specific to strategies, libraries or bridge definitions.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use zf_context::{
    context::{ContextLibrary, ContextStrategy, valid_id},
    context_source,
};
use zf_core::diagnostics::Diagnostic;

#[derive(Debug)]
pub struct Conflict(pub &'static str);
impl std::fmt::Display for Conflict {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}
impl std::error::Error for Conflict {}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFile {
    pub key: String,
    pub path: PathBuf,
    pub hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug)]
pub struct SourceStore {
    workspace: PathBuf,
    components: Vec<String>,
}
impl SourceStore {
    /// Segments are simple names below `.zedflow`; callers cannot supply paths.
    pub fn new(workspace: PathBuf, segments: &[&str]) -> Result<Self> {
        ensure!(
            !segments.is_empty() && segments.iter().all(|s| valid_id(s)),
            "Invalid source catalogue segments"
        );
        Ok(Self {
            workspace,
            components: segments.iter().map(|s| (*s).into()).collect(),
        })
    }
    pub async fn list(&self) -> Result<Vec<SourceFile>> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            let Some(_guard) = workspace_lock(&store.workspace, false, false)? else {
                return Ok(vec![]);
            };
            let Some(directory) = directory(&store.workspace, &store.components, false)? else {
                return Ok(vec![]);
            };
            let mut files = vec![];
            for entry in fs::read_dir(directory)? {
                let path = entry?.path();
                if path.extension().is_some_and(|extension| extension == "rs") {
                    files.push(read_source_path(&path));
                }
            }
            files.sort_by(|a, b| a.key.cmp(&b.key));
            Ok(files)
        })
        .await
        .context("Source catalogue reader failed")?
    }
    pub async fn read(&self, key: &str) -> Result<SourceFile> {
        ensure!(valid_id(key), "Invalid source key");
        let store = self.clone();
        let key = key.to_owned();
        tokio::task::spawn_blocking(move || {
            let _guard = workspace_lock(&store.workspace, false, false)?
                .context("Source catalogue does not exist")?;
            let directory = directory(&store.workspace, &store.components, false)?
                .context("Source catalogue does not exist")?;
            let path = directory.join(format!("{key}.rs"));
            fs::symlink_metadata(&path).with_context(|| format!("Source not found: {key}"))?;
            Ok(read_source_path(&path))
        })
        .await
        .context("Source reader failed")?
    }
    /// Codecs must validate and round-trip before this byte-preserving write.
    pub async fn save(
        &self,
        key: &str,
        source: &str,
        expected_hash: Option<&str>,
    ) -> Result<SourceFile> {
        super::source_acceptance::begin(
            self.workspace.clone(),
            self.components.clone(),
            key.into(),
            source.into(),
            expected_hash.map(str::to_owned),
            None,
            vec![],
        )
        .await?
        .finish()
        .await
    }
    /// Resolve only an explicitly accepted head; a known authored ancestor follows
    /// accepted saves while an unknown pin or external edit remains a conflict.
    pub async fn resolve(&self, key: &str, requested: Option<&str>) -> Result<SourceFile> {
        self.resolve_inner(key, requested, None).await
    }
    pub async fn preflight(
        &self,
        key: &str,
        requested: Option<&str>,
        expected: &str,
    ) -> Result<SourceFile> {
        self.resolve_inner(key, requested, Some(expected)).await
    }
    async fn resolve_inner(
        &self,
        key: &str,
        requested: Option<&str>,
        expected: Option<&str>,
    ) -> Result<SourceFile> {
        ensure!(valid_id(key), "Invalid source key");
        let store = self.clone();
        let key = key.to_owned();
        let requested = requested.map(str::to_owned);
        let expected = expected.map(str::to_owned);
        tokio::task::spawn_blocking(move || {
            let _guard = workspace_lock(&store.workspace, false, false)?
                .context("Source workspace absent")?;
            let directory = directory(&store.workspace, &store.components, false)?
                .context("Source catalogue absent")?;
            let file = read_source_path(&directory.join(format!("{key}.rs")));
            ensure!(
                file.diagnostics.is_empty(),
                "Source cannot be resolved: {:?}",
                file.diagnostics
            );
            if let Some(expected) = &expected {
                ensure!(
                    file.hash == *expected,
                    Conflict("Source changed during dependent preflight")
                );
            }
            super::source_acceptance::check_reference_locked(
                &store.workspace,
                &store.components,
                &key,
                &file.hash,
                requested.as_deref(),
                expected.is_some(),
            )?;
            Ok(file)
        })
        .await
        .context("Source resolution failed")?
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextFile {
    pub key: String,
    pub path: PathBuf,
    pub hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<ContextStrategy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}
#[derive(Clone, Debug)]
pub struct ContextStore {
    source: SourceStore,
}
impl ContextStore {
    pub fn new(workspace: PathBuf) -> Self {
        Self {
            source: SourceStore {
                workspace,
                components: vec!["context".into()],
            },
        }
    }
    pub async fn list(&self) -> Result<Vec<ContextFile>> {
        Ok(self
            .source
            .list()
            .await?
            .into_iter()
            .map(|f| decode_context(f, false))
            .collect())
    }
    pub async fn read(&self, key: &str) -> Result<ContextFile> {
        Ok(decode_context(self.source.read(key).await?, true))
    }
    pub async fn resolve(&self, key: &str, requested: Option<&str>) -> Result<ContextFile> {
        Ok(decode_context(
            self.source.resolve(key, requested).await?,
            true,
        ))
    }
    pub async fn preflight(
        &self,
        key: &str,
        requested: Option<&str>,
        expected: &str,
    ) -> Result<ContextFile> {
        Ok(decode_context(
            self.source.preflight(key, requested, expected).await?,
            true,
        ))
    }
    pub async fn save(
        &self,
        strategy: &ContextStrategy,
        expected_hash: Option<&str>,
    ) -> Result<ContextFile> {
        let source = context_source::generate(strategy).map_err(diagnostics_error)?;
        ensure!(
            context_source::parse(&source).map_err(diagnostics_error)? == *strategy,
            "Generated context source does not round-trip"
        );
        Ok(decode_context(
            self.source
                .save(&strategy.id, &source, expected_hash)
                .await?,
            true,
        ))
    }
}
fn decode_context(file: SourceFile, include_source: bool) -> ContextFile {
    let mut result = ContextFile {
        key: file.key,
        path: file.path,
        hash: file.hash,
        source: None,
        strategy: None,
        diagnostics: file.diagnostics,
    };
    if let Some(source) = file.source {
        match context_source::parse(&source) {
            Ok(strategy) if strategy.id == result.key => result.strategy = Some(strategy),
            Ok(_) => result.diagnostics.push(Diagnostic::new(
                "context_identity",
                "$source",
                "Strategy identity must match its filename",
            )),
            Err(errors) => result.diagnostics.extend(errors),
        }
        if include_source {
            result.source = Some(source);
        }
    }
    result
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFile {
    pub key: String,
    pub path: PathBuf,
    pub hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library: Option<ContextLibrary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}
#[derive(Clone, Debug)]
pub struct LibraryStore {
    source: SourceStore,
}
impl LibraryStore {
    pub fn new(workspace: PathBuf) -> Self {
        Self {
            source: SourceStore {
                workspace,
                components: vec!["context".into(), "libraries".into()],
            },
        }
    }
    pub async fn list(&self) -> Result<Vec<LibraryFile>> {
        Ok(self
            .source
            .list()
            .await?
            .into_iter()
            .map(|f| decode_library(f, false))
            .collect())
    }
    pub async fn read(&self, key: &str) -> Result<LibraryFile> {
        Ok(decode_library(self.source.read(key).await?, true))
    }
    pub async fn resolve(&self, key: &str, requested: Option<&str>) -> Result<LibraryFile> {
        Ok(decode_library(
            self.source.resolve(key, requested).await?,
            true,
        ))
    }
    pub async fn preflight(
        &self,
        key: &str,
        requested: Option<&str>,
        expected: &str,
    ) -> Result<LibraryFile> {
        Ok(decode_library(
            self.source.preflight(key, requested, expected).await?,
            true,
        ))
    }
    pub async fn save(
        &self,
        key: &str,
        library: &ContextLibrary,
        expected_hash: Option<&str>,
    ) -> Result<LibraryFile> {
        let source = context_source::generate_library(library).map_err(diagnostics_error)?;
        ensure!(
            context_source::parse_library(&source).map_err(diagnostics_error)? == *library,
            "Generated library source does not round-trip"
        );
        Ok(decode_library(
            self.source.save(key, &source, expected_hash).await?,
            true,
        ))
    }
}
fn decode_library(file: SourceFile, include_source: bool) -> LibraryFile {
    let mut result = LibraryFile {
        key: file.key,
        path: file.path,
        hash: file.hash,
        source: None,
        library: None,
        diagnostics: file.diagnostics,
    };
    if let Some(source) = file.source {
        match context_source::parse_library(&source) {
            Ok(library) => result.library = Some(library),
            Err(errors) => result.diagnostics.extend(errors),
        }
        if include_source {
            result.source = Some(source);
        }
    }
    result
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeFile {
    pub key: String,
    pub path: PathBuf,
    pub hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub types: Option<zf_core::types::TypeRegistry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}
#[derive(Clone, Debug)]
pub struct TypeStore {
    source: SourceStore,
}
impl TypeStore {
    pub fn new(workspace: PathBuf) -> Self {
        Self {
            source: SourceStore {
                workspace,
                components: vec!["types".into()],
            },
        }
    }
    pub async fn list(&self) -> Result<Vec<TypeFile>> {
        Ok(self
            .source
            .list()
            .await?
            .into_iter()
            .map(|f| decode_types(f, false))
            .collect())
    }
    pub async fn read(&self, key: &str) -> Result<TypeFile> {
        Ok(decode_types(self.source.read(key).await?, true))
    }
    pub async fn resolve(&self, key: &str, requested: Option<&str>) -> Result<TypeFile> {
        Ok(decode_types(
            self.source.resolve(key, requested).await?,
            true,
        ))
    }
    pub async fn preflight(
        &self,
        key: &str,
        requested: Option<&str>,
        expected: &str,
    ) -> Result<TypeFile> {
        Ok(decode_types(
            self.source.preflight(key, requested, expected).await?,
            true,
        ))
    }
    pub async fn save(
        &self,
        key: &str,
        types: &zf_core::types::TypeRegistry,
        expected: Option<&str>,
    ) -> Result<TypeFile> {
        let source = context_source::generate_types(types).map_err(diagnostics_error)?;
        ensure!(
            context_source::parse_types(&source).map_err(diagnostics_error)? == *types,
            "Type source does not round-trip"
        );
        Ok(decode_types(
            self.source.save(key, &source, expected).await?,
            true,
        ))
    }
}
fn decode_types(file: SourceFile, include_source: bool) -> TypeFile {
    let mut diagnostics = file.diagnostics;
    let types =
        file.source
            .as_deref()
            .and_then(|source| match context_source::parse_types(source) {
                Ok(types) => Some(types),
                Err(errors) => {
                    diagnostics.extend(errors);
                    None
                }
            });
    TypeFile {
        key: file.key,
        path: file.path,
        hash: file.hash,
        types,
        source: if include_source { file.source } else { None },
        diagnostics,
    }
}

pub(super) fn read_source_path(path: &Path) -> SourceFile {
    let key = path
        .file_stem()
        .map(|v| v.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut result = SourceFile {
        key,
        path: path.into(),
        hash: String::new(),
        source: None,
        diagnostics: vec![],
    };
    match bytes(path) {
        Err(error) => result.diagnostics.push(Diagnostic::new(
            "context_file",
            path.to_string_lossy(),
            error.to_string(),
        )),
        Ok(bytes) => {
            result.hash = hash(&bytes);
            match String::from_utf8(bytes) {
                Ok(source) => result.source = Some(source),
                Err(error) => result.diagnostics.push(Diagnostic::new(
                    "context_encoding",
                    "$source",
                    error.to_string(),
                )),
            }
        }
    }
    result
}
pub(crate) fn diagnostics_error(diagnostics: Vec<Diagnostic>) -> anyhow::Error {
    anyhow::anyhow!(
        "{}",
        diagnostics
            .iter()
            .map(|d| format!("{}: {} ({})", d.path, d.message, d.code))
            .collect::<Vec<_>>()
            .join("; ")
    )
}
pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(super) fn no_follow(options: &mut OpenOptions) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(nix::libc::O_NOFOLLOW);
    }
    #[cfg(not(unix))]
    let _ = options;
}
fn bytes(path: &Path) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "Context sources must be regular files, not symlinks"
    );
    ensure!(
        metadata.len() <= context_source::MAX_SOURCE_BYTES as u64,
        "Context source exceeds 1 MiB"
    );
    let mut options = OpenOptions::new();
    options.read(true);
    no_follow(&mut options);
    let mut bytes = Vec::new();
    options
        .open(path)?
        .take(context_source::MAX_SOURCE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= context_source::MAX_SOURCE_BYTES,
        "Context source exceeds 1 MiB"
    );
    Ok(bytes)
}
pub(super) fn existing_hash(path: &Path) -> Result<Option<String>> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(Some(hash(&bytes(path)?))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}
pub(super) fn directory(
    workspace: &Path,
    components: &[String],
    create: bool,
) -> Result<Option<PathBuf>> {
    namespace_directory(workspace, ".zedflow", components, create)
}
pub(super) fn namespace_directory(
    workspace: &Path,
    namespace: &str,
    components: &[String],
    create: bool,
) -> Result<Option<PathBuf>> {
    ensure!(
        matches!(namespace, ".zedflow" | ".agents"),
        "Invalid source namespace"
    );
    let metadata = fs::symlink_metadata(workspace).context("Workspace is inaccessible")?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "Workspace must be a canonical directory"
    );
    let canonical = fs::canonicalize(workspace)?;
    let mut directory = canonical.clone();
    for component in std::iter::once(namespace).chain(components.iter().map(String::as_str)) {
        directory.push(component);
        match fs::symlink_metadata(&directory) {
            Ok(metadata) => ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "Context catalog directories cannot be symlinks"
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if !create {
                    return Ok(None);
                }
                match fs::create_dir(&directory) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error.into()),
                }
                let metadata = fs::symlink_metadata(&directory)?;
                ensure!(
                    metadata.is_dir() && !metadata.file_type().is_symlink(),
                    "Context catalog directories cannot be symlinks"
                );
            }
            Err(error) => return Err(error.into()),
        }
    }
    ensure!(
        fs::canonicalize(&directory)?.starts_with(canonical),
        "Context catalog escaped its workspace"
    );
    Ok(Some(directory))
}
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

const IMPORT_MARKER: &str = ".source-import.json";
const MAX_IMPORT_FILES: usize = 256;
const MAX_IMPORT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct SourceInstall {
    pub segments: Vec<String>,
    pub key: String,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstallEntry {
    segments: Vec<String>,
    key: String,
    hash: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ImportPhase {
    Installing,
    Committed,
    RolledBack,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportJournal {
    version: u32,
    id: String,
    phase: ImportPhase,
    entries: Vec<InstallEntry>,
    #[serde(default)]
    external_changes: bool,
}

/// Shared locks cover complete catalogue reads. An interrupted import is repaired
/// under the exclusive lock before any reader can observe its partial contents.
pub(super) fn workspace_lock_raw(
    workspace: &Path,
    create: bool,
    exclusive: bool,
) -> Result<Option<File>> {
    if !create {
        match fs::symlink_metadata(workspace) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
            Ok(_) => {}
        }
    }
    let Some(root) = directory(workspace, &[], create)? else {
        return Ok(None);
    };
    let lock_path = root.join(".sources.lock");
    if let Ok(metadata) = fs::symlink_metadata(&lock_path) {
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "Source workspace lock must be a regular file"
        );
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    no_follow(&mut options);
    let lock = options
        .open(&lock_path)
        .context("Cannot open source workspace lock")?;
    if exclusive {
        lock.lock()?;
    } else {
        lock.lock_shared()?;
    }
    Ok(Some(lock))
}
pub(super) fn workspace_lock(
    workspace: &Path,
    create: bool,
    exclusive: bool,
) -> Result<Option<File>> {
    let Some(lock) = workspace_lock_raw(workspace, create, exclusive)? else {
        return Ok(None);
    };
    let root = directory(workspace, &[], false)?.context("Source workspace disappeared")?;
    super::flow_packages::ensure_no_lifecycle_locked(&root)?;
    if exclusive {
        recover_locked(workspace, &root)?;
        super::source_acceptance::recover_files_locked(workspace, &root)?;
        super::flow_packages::recover_files_locked(workspace, &root)?;
        return Ok(Some(lock));
    }
    loop {
        super::flow_packages::ensure_no_lifecycle_locked(&root)?;
        if fs::symlink_metadata(root.join(IMPORT_MARKER)).is_err()
            && fs::symlink_metadata(root.join(super::source_acceptance::MARKER)).is_err()
            && fs::symlink_metadata(root.join(super::flow_packages::MARKER)).is_err()
        {
            break;
        }
        lock.unlock()?;
        lock.lock()?;
        super::flow_packages::ensure_no_lifecycle_locked(&root)?;
        recover_locked(workspace, &root)?;
        super::source_acceptance::recover_files_locked(workspace, &root)?;
        super::flow_packages::recover_files_locked(workspace, &root)?;
        lock.unlock()?;
        lock.lock_shared()?;
    }
    Ok(Some(lock))
}
/// The FlowStore authoring path shares this lock with context/library/bridge saves.
/// Acquire only after asynchronous preflight; do not recursively read a SourceStore while held.
pub async fn writer_lock(workspace: PathBuf) -> Result<File> {
    tokio::task::spawn_blocking(move || {
        workspace_lock(&workspace, true, true)?.context("Source workspace absent")
    })
    .await
    .context("Source writer lock failed")?
}
pub async fn reader_lock(workspace: PathBuf) -> Result<Option<File>> {
    tokio::task::spawn_blocking(move || workspace_lock(&workspace, false, false))
        .await
        .context("Source reader lock failed")?
}
fn check_entry(entry: &InstallEntry) -> Result<()> {
    ensure!(
        valid_id(&entry.key)
            && !entry.segments.is_empty()
            && entry.segments.len() <= 8
            && entry.segments.iter().all(|s| valid_id(s)),
        "Invalid source import location"
    );
    ensure!(
        entry.hash.len() == 64 && entry.hash.bytes().all(|b| b.is_ascii_hexdigit()),
        "Invalid source import hash"
    );
    Ok(())
}
fn entry_path(workspace: &Path, entry: &InstallEntry, create: bool) -> Result<Option<PathBuf>> {
    check_entry(entry)?;
    let extension = if entry.segments == ["examples"] {
        "json"
    } else {
        "rs"
    };
    Ok(directory(workspace, &entry.segments, create)?
        .map(|dir| dir.join(format!("{}.{extension}", entry.key))))
}
fn journal(root: &Path) -> Result<Option<ImportJournal>> {
    let path = root.join(IMPORT_MARKER);
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
        Ok(_) => {}
    }
    let journal: ImportJournal =
        serde_json::from_slice(&bytes(&path)?).context("Invalid source import recovery marker")?;
    ensure!(
        matches!(journal.version, 1 | 2)
            && valid_id(&journal.id)
            && !journal.entries.is_empty()
            && journal.entries.len() <= MAX_IMPORT_FILES,
        "Invalid source import recovery marker"
    );
    let mut paths = std::collections::BTreeSet::new();
    for entry in &journal.entries {
        ensure!(
            journal.version >= 2 || entry.segments != ["examples"],
            "JSON examples require an import marker v2"
        );
        check_entry(entry)?;
        ensure!(
            paths.insert((entry.segments.clone(), entry.key.clone())),
            "Duplicate source recovery target"
        );
    }
    Ok(Some(journal))
}
fn stage_directory(root: &Path, journal: &ImportJournal) -> PathBuf {
    root.join(format!(".source-import-{}", journal.id))
}
fn save_journal(root: &Path, journal: &ImportJournal) -> Result<()> {
    let temporary = Temporary(root.join(format!(".source-journal-{}.tmp", uuid::Uuid::new_v4())));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary.0)?;
    file.write_all(&serde_json::to_vec(journal)?)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary.0, root.join(IMPORT_MARKER))?;
    File::open(root)?.sync_all()?;
    Ok(())
}
fn clean_journal(root: &Path, journal: &ImportJournal) -> Result<()> {
    let stage = stage_directory(root, journal);
    match fs::symlink_metadata(&stage) {
        Ok(metadata) => {
            ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "Import staging directory is no longer regular"
            );
            // Remove only the files declared by this transaction, never user files.
            for index in 0..journal.entries.len() {
                let path = stage.join(format!("{index}.rs"));
                match fs::remove_file(path) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.into()),
                }
            }
            fs::remove_dir(&stage)?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    fs::remove_file(root.join(IMPORT_MARKER))?;
    File::open(root)?.sync_all()?;
    Ok(())
}
pub(super) fn recover_locked(workspace: &Path, root: &Path) -> Result<()> {
    let Some(mut journal) = journal(root)? else {
        return Ok(());
    };
    let mut changed = journal.external_changes;
    if journal.phase == ImportPhase::Installing {
        let stage = stage_directory(root, &journal);
        let metadata = fs::symlink_metadata(&stage).context("Import staging proof is absent")?;
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "Import staging proof must be a regular directory"
        );
        for (index, entry) in journal.entries.iter().enumerate() {
            let Some(path) = entry_path(workspace, entry, false)? else {
                continue;
            };
            match existing_hash(&path) {
                Ok(Some(actual))
                    if actual == entry.hash
                        && bytes(&stage.join(format!("{index}.rs")))
                            .is_ok_and(|bytes| hash(&bytes) == entry.hash)
                        && same_file::is_same_file(&path, stage.join(format!("{index}.rs")))
                            .unwrap_or(false) =>
                {
                    fs::remove_file(&path)?;
                    File::open(path.parent().context("Source parent missing")?)?.sync_all()?;
                }
                Ok(None) => {}
                // External changes belong to their author and must survive rollback.
                Ok(Some(_)) | Err(_) => changed = true,
            }
        }
        // Record rollback completion before removing the staging proof. Recovery
        // must still work after a second crash during that final cleanup.
        journal.phase = ImportPhase::RolledBack;
        journal.external_changes = changed;
        save_journal(root, &journal)?;
    }
    clean_journal(root, &journal)?;
    ensure!(
        !changed,
        Conflict("Interrupted import rolled back; externally changed files were preserved")
    );
    Ok(())
}
fn begin_install(
    workspace: &Path,
    root: &Path,
    sources: &[SourceInstall],
) -> Result<ImportJournal> {
    ensure!(
        !sources.is_empty() && sources.len() <= MAX_IMPORT_FILES,
        "Import requires 1–256 source files"
    );
    let mut entries = vec![];
    let mut total = 0usize;
    let mut seen = std::collections::BTreeSet::new();
    for source in sources {
        ensure!(
            source.source.len() <= context_source::MAX_SOURCE_BYTES,
            "Source exceeds 1 MiB"
        );
        total = total
            .checked_add(source.source.len())
            .context("Import size overflow")?;
        ensure!(total <= MAX_IMPORT_BYTES, "Import exceeds 64 MiB");
        let entry = InstallEntry {
            segments: source.segments.clone(),
            key: source.key.clone(),
            hash: hash(source.source.as_bytes()),
        };
        check_entry(&entry)?;
        ensure!(
            seen.insert((entry.segments.clone(), entry.key.clone())),
            "Duplicate source import target"
        );
        let path = entry_path(workspace, &entry, true)?.context("Source directory absent")?;
        ensure!(
            fs::symlink_metadata(&path).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
            Conflict("Import target already exists; files are never overwritten")
        );
        entries.push(entry);
    }
    let journal = ImportJournal {
        version: if sources.iter().any(|source| source.segments == ["examples"]) {
            2
        } else {
            1
        },
        id: uuid::Uuid::new_v4().to_string(),
        phase: ImportPhase::Installing,
        entries,
        external_changes: false,
    };
    let stage = stage_directory(root, &journal);
    fs::create_dir(&stage)?;
    for (index, source) in sources.iter().enumerate() {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(stage.join(format!("{index}.rs")))?;
        file.write_all(source.source.as_bytes())?;
        file.sync_all()?;
    }
    File::open(&stage)?.sync_all()?;
    save_journal(root, &journal)?;
    Ok(journal)
}
fn install_entries(
    workspace: &Path,
    root: &Path,
    journal: &mut ImportJournal,
) -> Result<Vec<SourceFile>> {
    let stage = stage_directory(root, journal);
    for (index, entry) in journal.entries.iter().enumerate() {
        let staged = stage.join(format!("{index}.rs"));
        ensure!(
            hash(&bytes(&staged)?) == entry.hash,
            Conflict("Staged import source changed")
        );
        let path = entry_path(workspace, entry, true)?.context("Source directory absent")?;
        fs::hard_link(&staged, &path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                anyhow::Error::new(Conflict("Import target was created by another writer"))
            } else {
                e.into()
            }
        })?;
        File::open(path.parent().context("Source parent absent")?)?.sync_all()?;
    }
    let mut files = vec![];
    for entry in &journal.entries {
        let path = entry_path(workspace, entry, false)?.context("Source directory absent")?;
        let file = read_source_path(&path);
        ensure!(
            file.hash == entry.hash && file.diagnostics.is_empty(),
            Conflict("Imported source changed before commit")
        );
        files.push(file);
    }
    journal.phase = ImportPhase::Committed;
    save_journal(root, journal)?;
    clean_journal(root, journal)?;
    Ok(files)
}
/// Installs only absent files. The workspace lock hides the entire transaction
/// from source readers; recovery completes or rolls back before later reads.
pub async fn install_sources(
    workspace: PathBuf,
    sources: Vec<SourceInstall>,
) -> Result<Vec<SourceFile>> {
    tokio::task::spawn_blocking(move || {
        let _guard = workspace_lock(&workspace, true, true)?.context("Source workspace absent")?;
        let root = directory(&workspace, &[], false)?.context("Source workspace absent")?;
        let mut journal = begin_install(&workspace, &root, &sources)?;
        match install_entries(&workspace, &root, &mut journal) {
            Ok(files) => Ok(files),
            Err(error) => {
                recover_locked(&workspace, &root).context(error.to_string())?;
                Err(error)
            }
        }
    })
    .await
    .context("Source package importer failed")?
}
/// One shared-lock snapshot across different source catalogues.
pub async fn read_sources(
    workspace: PathBuf,
    requests: Vec<(Vec<String>, String)>,
) -> Result<Vec<SourceFile>> {
    tokio::task::spawn_blocking(move || {
        let _guard =
            workspace_lock(&workspace, false, false)?.context("Source workspace absent")?;
        let mut files = vec![];
        for (segments, key) in requests {
            let entry = InstallEntry {
                segments,
                key,
                hash: "0".repeat(64),
            };
            let path = entry_path(&workspace, &entry, false)?.context("Source catalogue absent")?;
            fs::symlink_metadata(&path).context("Requested source absent")?;
            files.push(read_source_path(&path));
        }
        Ok(files)
    })
    .await
    .context("Source package exporter failed")?
}

#[cfg(test)]
mod import_tests {
    use super::*;
    fn sources() -> Vec<SourceInstall> {
        vec![
            SourceInstall {
                segments: vec!["context".into()],
                key: "first".into(),
                source: "// first source".into(),
            },
            SourceInstall {
                segments: vec!["bridges".into()],
                key: "second".into(),
                source: "// second source".into(),
            },
        ]
    }
    fn link_entry(workspace: &Path, root: &Path, journal: &ImportJournal, index: usize) {
        let path = entry_path(workspace, &journal.entries[index], true)
            .unwrap()
            .unwrap();
        fs::hard_link(
            stage_directory(root, journal).join(format!("{index}.rs")),
            &path,
        )
        .unwrap();
        File::open(path.parent().unwrap())
            .unwrap()
            .sync_all()
            .unwrap();
    }
    #[tokio::test]
    async fn mixed_json_and_rust_import_recovers_without_exposing_partial_examples() {
        let workspace = tempfile::tempdir().unwrap();
        let guard = workspace_lock(workspace.path(), true, true)
            .unwrap()
            .unwrap();
        let root = directory(workspace.path(), &[], false).unwrap().unwrap();
        let mut files = sources();
        files.push(SourceInstall {
            segments: vec!["examples".into()],
            key: "example".into(),
            source: "{}".into(),
        });
        let journal = begin_install(workspace.path(), &root, &files).unwrap();
        assert_eq!(journal.version, 2);
        link_entry(workspace.path(), &root, &journal, 0);
        link_entry(workspace.path(), &root, &journal, 2);
        assert!(root.join("examples/example.json").exists());
        drop(guard);
        let _reader = reader_lock(workspace.path().into()).await.unwrap();
        assert!(!root.join("examples/example.json").exists());
        assert!(!root.join("context/first.rs").exists());
        assert!(!root.join(IMPORT_MARKER).exists());
    }

    #[tokio::test]
    async fn reader_recovers_crash_between_artifacts_before_exposing_catalogue() {
        let workspace = tempfile::tempdir().unwrap();
        let guard = workspace_lock(workspace.path(), true, true)
            .unwrap()
            .unwrap();
        let root = directory(workspace.path(), &[], false).unwrap().unwrap();
        let journal = begin_install(workspace.path(), &root, &sources()).unwrap();
        link_entry(workspace.path(), &root, &journal, 0);
        // Simulate process loss after the first durable file: no destructor rolls
        // back the journal. A new reader must repair the persisted state itself.
        drop(guard);
        assert!(
            SourceStore::new(workspace.path().into(), &["context"])
                .unwrap()
                .list()
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            SourceStore::new(workspace.path().into(), &["bridges"])
                .unwrap()
                .list()
                .await
                .unwrap()
                .is_empty()
        );
        assert!(!root.join(IMPORT_MARKER).exists());
        assert!(!stage_directory(&root, &journal).exists());
    }
    #[tokio::test]
    async fn reader_finishes_a_proven_commit_without_rolling_back_new_files() {
        let workspace = tempfile::tempdir().unwrap();
        let guard = workspace_lock(workspace.path(), true, true)
            .unwrap()
            .unwrap();
        let root = directory(workspace.path(), &[], false).unwrap().unwrap();
        let mut journal = begin_install(workspace.path(), &root, &sources()).unwrap();
        link_entry(workspace.path(), &root, &journal, 0);
        link_entry(workspace.path(), &root, &journal, 1);
        journal.phase = ImportPhase::Committed;
        save_journal(&root, &journal).unwrap();
        drop(guard);
        let files = SourceStore::new(workspace.path().into(), &["context"])
            .unwrap()
            .list()
            .await
            .unwrap();
        assert_eq!(files[0].source.as_deref(), Some("// first source"));
        assert_eq!(
            SourceStore::new(workspace.path().into(), &["bridges"])
                .unwrap()
                .list()
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(!root.join(IMPORT_MARKER).exists());
    }
    #[tokio::test]
    async fn recovery_survives_a_second_crash_after_rollback_staging_was_removed() {
        let workspace = tempfile::tempdir().unwrap();
        let guard = workspace_lock(workspace.path(), true, true)
            .unwrap()
            .unwrap();
        let root = directory(workspace.path(), &[], false).unwrap().unwrap();
        let mut journal = begin_install(workspace.path(), &root, &sources()).unwrap();
        journal.phase = ImportPhase::RolledBack;
        save_journal(&root, &journal).unwrap();
        let stage = stage_directory(&root, &journal);
        for index in 0..journal.entries.len() {
            fs::remove_file(stage.join(format!("{index}.rs"))).unwrap();
        }
        fs::remove_dir(stage).unwrap();
        drop(guard);
        assert!(
            SourceStore::new(workspace.path().into(), &["context"])
                .unwrap()
                .list()
                .await
                .unwrap()
                .is_empty()
        );
        assert!(!root.join(IMPORT_MARKER).exists());
    }

    #[tokio::test]
    async fn matching_bytes_without_import_file_identity_cannot_authorize_deletion() {
        let workspace = tempfile::tempdir().unwrap();
        let guard = workspace_lock(workspace.path(), true, true)
            .unwrap()
            .unwrap();
        let root = directory(workspace.path(), &[], false).unwrap().unwrap();
        let journal = begin_install(workspace.path(), &root, &sources()).unwrap();
        let path = entry_path(workspace.path(), &journal.entries[0], false)
            .unwrap()
            .unwrap();
        // An external writer created the exact same bytes after preflight. The
        // import never linked this file, so recovery must not remove it.
        fs::write(&path, &sources()[0].source).unwrap();
        drop(guard);
        let store = SourceStore::new(workspace.path().into(), &["context"]).unwrap();
        assert!(
            store
                .list()
                .await
                .unwrap_err()
                .downcast_ref::<Conflict>()
                .is_some()
        );
        assert_eq!(fs::read_to_string(path).unwrap(), sources()[0].source);
    }

    #[tokio::test]
    async fn recovery_preserves_external_edits_and_never_reports_a_partial_import_success() {
        let workspace = tempfile::tempdir().unwrap();
        let guard = workspace_lock(workspace.path(), true, true)
            .unwrap()
            .unwrap();
        let root = directory(workspace.path(), &[], false).unwrap().unwrap();
        let journal = begin_install(workspace.path(), &root, &sources()).unwrap();
        link_entry(workspace.path(), &root, &journal, 0);
        link_entry(workspace.path(), &root, &journal, 1);
        let edited = entry_path(workspace.path(), &journal.entries[0], false)
            .unwrap()
            .unwrap();
        fs::write(&edited, "// external edit").unwrap();
        drop(guard);
        let store = SourceStore::new(workspace.path().into(), &["context"]).unwrap();
        let error = store.list().await.unwrap_err();
        assert!(error.downcast_ref::<Conflict>().is_some());
        assert_eq!(fs::read_to_string(edited).unwrap(), "// external edit");
        assert_eq!(store.list().await.unwrap().len(), 1);
        assert!(
            SourceStore::new(workspace.path().into(), &["bridges"])
                .unwrap()
                .list()
                .await
                .unwrap()
                .is_empty()
        );
    }
}

/// Portable definition packages using checked source installation and domain validators.
pub mod packages {
    use super::{SourceFile, SourceInstall};
    use crate::context_store;
    use anyhow::{Result, ensure};
    use serde::Serialize;
    use std::path::PathBuf;
    use zf_context::context_package::{
        ArtifactKind, ArtifactSelection, ContextPackage, PACKAGE_VERSION, PackagePrerequisite,
        SourceArtifact, validate_package,
    };
    use zf_flows::bridge_source::PackageBridgeValidator;

    #[derive(Clone, Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct PackageImport {
        pub files: Vec<SourceFile>,
        pub prerequisites: Vec<PackagePrerequisite>,
    }
    pub async fn export_selection(
        workspace: PathBuf,
        selection: &[ArtifactSelection],
    ) -> Result<ContextPackage> {
        ensure!(
            !selection.is_empty() && selection.len() <= 256,
            "Select 1–256 source artifacts"
        );
        let mut selection = selection.to_vec();
        selection.sort();
        ensure!(
            selection.windows(2).all(|p| p[0] != p[1]),
            "Duplicate source selection"
        );
        let requests = selection
            .iter()
            .map(|s| (s.kind.segments(), s.key.clone()))
            .collect();
        let files = context_store::read_sources(workspace, requests).await?;
        let mut artifacts = vec![];
        for (selection, file) in selection.into_iter().zip(files) {
            ensure!(
                file.diagnostics.is_empty(),
                "Source {} is invalid: {:?}",
                file.key,
                file.diagnostics
            );
            artifacts.push(SourceArtifact {
                kind: selection.kind,
                key: file.key,
                hash: file.hash,
                source: file
                    .source
                    .ok_or_else(|| anyhow::anyhow!("Source contents absent"))?,
            });
        }
        let package = ContextPackage {
            version: if artifacts
                .iter()
                .any(|artifact| artifact.kind == ArtifactKind::Example)
            {
                2
            } else {
                PACKAGE_VERSION
            },
            artifacts,
        };
        let validated = validate_package(&package, &PackageBridgeValidator);
        ensure!(
            validated.valid,
            "Package dependencies or sources are invalid: {:?}",
            validated.diagnostics
        );
        Ok(package)
    }

    pub async fn import_package(
        workspace: PathBuf,
        package: &ContextPackage,
    ) -> Result<PackageImport> {
        let validated = validate_package(package, &PackageBridgeValidator);
        ensure!(
            validated.valid,
            "Invalid source package: {:?}",
            validated.diagnostics
        );
        let sources = package
            .artifacts
            .iter()
            .map(|a| SourceInstall {
                segments: a.kind.segments(),
                key: a.key.clone(),
                source: a.source.clone(),
            })
            .collect();
        let files = context_store::install_sources(workspace, sources).await?;
        Ok(PackageImport {
            files,
            prerequisites: validated.prerequisites,
        })
    }
}
