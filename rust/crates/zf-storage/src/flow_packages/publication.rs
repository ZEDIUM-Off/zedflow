//! Durable package replacement and the filesystem/SQLite publication handoff.
mod lifecycle;
use super::acquire::capture_sync;
use crate::{
    context_store::{self, Conflict},
    source_acceptance::FilePrecondition,
};
use anyhow::{Context, Result, ensure};
pub use lifecycle::{
    BridgeMutation, CataloguePrecondition, LegacyRetirement, PackageConversion, PackageDeletion,
    PackagePrecondition, capture_catalogue_preconditions, convert_package, delete_package,
    recover_lifecycle,
};
pub(crate) use lifecycle::{ensure_no_lifecycle_locked, inspect_catalogue_preconditions};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs::File,
    path::{Component, Path, PathBuf},
};
use zf_flows::package::{
    MANIFEST_FILE, MAX_PACKAGE_FILE_BYTES, MAX_SNAPSHOT_BYTES, PackageSnapshot,
};

pub(crate) const MARKER: &str = ".package-acceptance.json";
// UTF-8 control bytes may each become a six-byte JSON escape (\u0000).
// The remaining allowance covers the envelope, inventory and publication data;
// their aggregate is still checked before the journal is written.
const MAX_JOURNAL_BYTES: usize = MAX_SNAPSHOT_BYTES * 6 + 16 * 1024 * 1024;
const MAX_ENTRIES: usize = 32768;
type Inventory = BTreeMap<String, Option<String>>;

pub struct PackageWrite {
    pub workspace: PathBuf,
    pub target: PathBuf,
    pub snapshot: PackageSnapshot,
    pub expected_revision: Option<String>,
    pub publication: Option<Value>,
    pub preconditions: Vec<FilePrecondition>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Phase {
    Staging,
    Ready,
    Installed,
    Finishing,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Intent {
    version: u32,
    id: String,
    snapshot: PackageSnapshot,
    previous_revision: Option<String>,
    previous_inventory: Option<Inventory>,
    publication: Option<Value>,
    preconditions: Vec<FilePrecondition>,
    phase: Phase,
}
pub struct PendingPackage {
    workspace: PathBuf,
    intent: Intent,
    _lock: Vec<File>,
}
impl PendingPackage {
    pub fn id(&self) -> &str {
        &self.intent.id
    }
    pub fn publication(&self) -> Option<&Value> {
        self.intent.publication.as_ref()
    }
    /// Call only after the host has durably and idempotently applied `publication`.
    pub async fn finish(self) -> Result<PackageSnapshot> {
        tokio::task::spawn_blocking(move || {
            // Precise closure capture must include catalogue ownership, even
            // when the caller drops its future while this worker is queued.
            let _lock = self._lock;
            let root = root(&self.workspace)?;
            finish(&self.workspace, &root, self.intent)
        })
        .await
        .context("package publication finalizer failed")?
    }
}

// Every path is opened component-by-component; no ancestor or final symlink is
// followed. Descriptors also keep publication anchored during directory renames.
#[cfg(unix)]
mod io {
    use super::*;
    use nix::{
        dir::Dir,
        fcntl::{AtFlags, OFlag, open, openat},
        sys::stat::{Mode, SFlag, fstatat, mkdirat},
        unistd::{UnlinkatFlags, unlinkat},
    };
    use std::io::{Read, Write};
    fn flags() -> OFlag {
        OFlag::O_RDONLY | OFlag::O_DIRECTORY | OFlag::O_CLOEXEC | OFlag::O_NOFOLLOW
    }
    pub(super) fn directory(path: &Path, create: bool) -> Result<File> {
        ensure!(
            path.is_absolute(),
            "package publication paths must be absolute"
        );
        ensure!(
            path.components().count() <= 256,
            "package publication path is too deep"
        );
        let mut dir = File::from(open(Path::new("/"), flags(), Mode::empty())?);
        for component in path.components() {
            let name = match component {
                Component::RootDir => continue,
                Component::Normal(name) => Path::new(name),
                _ => anyhow::bail!("noncanonical publication path"),
            };
            if create {
                match mkdirat(&dir, name, Mode::from_bits_truncate(0o700)) {
                    Ok(()) => dir.sync_all()?,
                    Err(nix::errno::Errno::EEXIST) => {}
                    Err(e) => return Err(e.into()),
                }
            }
            dir = File::from(openat(&dir, name, flags(), Mode::empty())?);
        }
        Ok(dir)
    }
    pub(super) fn exists(path: &Path) -> Result<bool> {
        let dir = directory(path.parent().context("missing parent")?, false)?;
        match fstatat(
            &dir,
            path.file_name().context("missing filename")?,
            AtFlags::AT_SYMLINK_NOFOLLOW,
        ) {
            Ok(_) => Ok(true),
            Err(nix::errno::Errno::ENOENT) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
    pub(super) fn bytes(path: &Path, limit: usize) -> Result<Vec<u8>> {
        let parent = directory(path.parent().context("missing parent")?, false)?;
        let file = File::from(openat(
            &parent,
            path.file_name().context("missing filename")?,
            OFlag::O_RDONLY | OFlag::O_CLOEXEC | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK,
            Mode::empty(),
        )?);
        ensure!(
            file.metadata()?.is_file() && file.metadata()?.len() <= limit as u64,
            "invalid or excessive publication file"
        );
        let mut bytes = Vec::new();
        file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= limit, "publication file exceeds byte limit");
        Ok(bytes)
    }
    pub(super) fn create_file(path: &Path, bytes: &[u8]) -> Result<()> {
        let dir = directory(path.parent().context("missing parent")?, true)?;
        let mut file = File::from(openat(
            &dir,
            path.file_name().context("missing filename")?,
            OFlag::O_WRONLY | OFlag::O_CLOEXEC | OFlag::O_NOFOLLOW | OFlag::O_CREAT | OFlag::O_EXCL,
            Mode::from_bits_truncate(0o600),
        )?);
        file.write_all(bytes)?;
        file.sync_all()?;
        dir.sync_all()?;
        Ok(())
    }
    pub(super) fn rename(from: &Path, to: &Path, exchange: bool) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            let source = directory(from.parent().context("missing parent")?, false)?;
            let target = directory(to.parent().context("missing parent")?, false)?;
            nix::fcntl::renameat2(
                &source,
                from.file_name().context("missing filename")?,
                &target,
                to.file_name().context("missing filename")?,
                if exchange {
                    nix::fcntl::RenameFlags::RENAME_EXCHANGE
                } else {
                    nix::fcntl::RenameFlags::RENAME_NOREPLACE
                },
            )?;
            source.sync_all()?;
            target.sync_all()?;
            Ok(())
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (from, to, exchange);
            anyhow::bail!("atomic package publication requires Linux renameat2")
        }
    }
    pub(super) fn unlink(path: &Path, directory_flag: bool) -> Result<()> {
        let parent = directory(path.parent().context("missing parent")?, false)?;
        unlinkat(
            &parent,
            path.file_name().context("missing filename")?,
            if directory_flag {
                UnlinkatFlags::RemoveDir
            } else {
                UnlinkatFlags::NoRemoveDir
            },
        )?;
        parent.sync_all()?;
        Ok(())
    }
    pub(super) fn inventory(root: &Path) -> Result<Inventory> {
        fn walk(
            root: &Path,
            relative: &str,
            result: &mut Inventory,
            total: &mut usize,
            depth: usize,
        ) -> Result<()> {
            ensure!(depth <= 64, "publication inventory is too deep");
            let path = root.join(relative);
            let dir = directory(&path, false)?;
            let mut entries = Dir::openat(&dir, ".", flags(), Mode::empty())?;
            for entry in entries.iter() {
                let entry = entry?;
                let name = entry.file_name().to_str()?;
                if matches!(name, "." | "..") {
                    continue;
                }
                let child = if relative.is_empty() {
                    name.to_owned()
                } else {
                    format!("{relative}/{name}")
                };
                ensure!(
                    result.len() < MAX_ENTRIES,
                    "publication inventory exceeds entry limit"
                );
                let stat = fstatat(&dir, name, AtFlags::AT_SYMLINK_NOFOLLOW)?;
                match SFlag::from_bits_truncate(stat.st_mode) {
                    SFlag::S_IFDIR => {
                        result.insert(child.clone(), None);
                        walk(root, &child, result, total, depth + 1)?;
                    }
                    SFlag::S_IFREG => {
                        let bytes = bytes(&root.join(&child), MAX_PACKAGE_FILE_BYTES)?;
                        *total = total
                            .checked_add(bytes.len())
                            .context("inventory size overflow")?;
                        ensure!(
                            *total <= MAX_SNAPSHOT_BYTES,
                            "publication inventory exceeds byte limit"
                        );
                        result.insert(child, Some(context_store::hash(&bytes)));
                    }
                    _ => anyhow::bail!("publication inventory contains a symlink or special file"),
                }
            }
            Ok(())
        }
        let mut result = BTreeMap::new();
        walk(root, "", &mut result, &mut 0, 0)?;
        Ok(result)
    }
}

#[cfg(not(unix))]
mod io {
    use super::*;
    macro_rules! unavailable {
        ($name:ident($($arg:ident: $ty:ty),*) -> $result:ty) => {
            pub(super) fn $name($($arg: $ty),*) -> Result<$result> {
                let _ = ($($arg),*);
                anyhow::bail!("safe package publication requires Linux")
            }
        };
    }
    unavailable!(directory(path: &Path, create: bool) -> File);
    unavailable!(exists(path: &Path) -> bool);
    unavailable!(bytes(path: &Path, limit: usize) -> Vec<u8>);
    unavailable!(create_file(path: &Path, bytes: &[u8]) -> ());
    unavailable!(rename(from: &Path, to: &Path, exchange: bool) -> ());
    unavailable!(unlink(path: &Path, directory: bool) -> ());
    unavailable!(inventory(path: &Path) -> Inventory);
}

fn root(workspace: &Path) -> Result<PathBuf> {
    let root =
        context_store::directory(workspace, &[], false)?.context("package workspace absent")?;
    io::directory(&root, false)?;
    Ok(root)
}
fn target(root: &Path, intent: &Intent) -> Result<PathBuf> {
    Ok(root
        .join("flow")
        .join(intent.snapshot.root_manifest()?.id.as_str()))
}
fn stage(root: &Path, intent: &Intent) -> PathBuf {
    root.join(format!(".package-stage-{}", intent.id))
}
fn revision(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn validate(intent: &Intent) -> Result<()> {
    ensure!(
        intent.version == 1 && uuid::Uuid::parse_str(&intent.id)?.to_string() == intent.id,
        "invalid package publication identity"
    );
    intent.snapshot.validate()?;
    ensure!(
        intent.previous_revision.is_some() == intent.previous_inventory.is_some(),
        "invalid previous package state"
    );
    if let Some(hash) = &intent.previous_revision {
        ensure!(revision(hash), "invalid previous package revision");
    }
    if let Some(inventory) = &intent.previous_inventory {
        ensure!(
            inventory.len() <= MAX_ENTRIES,
            "previous inventory too large"
        );
        for (path, hash) in inventory {
            ensure!(
                !path.is_empty()
                    && Path::new(path)
                        .components()
                        .all(|c| matches!(c, Component::Normal(_)))
                    && !path.contains('\\'),
                "invalid previous inventory path"
            );
            if let Some(hash) = hash {
                ensure!(revision(hash), "invalid inventory hash");
            }
        }
    }
    ensure!(
        intent.preconditions.len() <= MAX_ENTRIES,
        "too many publication preconditions"
    );
    for p in &intent.preconditions {
        ensure!(
            p.path.is_absolute()
                && p.path
                    .components()
                    .all(|c| matches!(c, Component::RootDir | Component::Normal(_)))
                && revision(&p.hash),
            "invalid publication precondition"
        );
    }
    Ok(())
}
fn save(root: &Path, intent: &Intent) -> Result<()> {
    let bytes = serde_json::to_vec(intent)?;
    ensure!(
        bytes.len() <= MAX_JOURNAL_BYTES,
        "package journal exceeds byte limit"
    );
    let temporary = root.join(format!(".package-journal-{}", uuid::Uuid::new_v4()));
    io::create_file(&temporary, &bytes)?;
    let marker = root.join(MARKER);
    if io::exists(&marker)? {
        // Check marker type before exchange; the old regular journal is then removed.
        io::bytes(&marker, MAX_JOURNAL_BYTES)?;
        io::rename(&temporary, &marker, true)?;
        io::unlink(&temporary, false)?;
    } else {
        io::rename(&temporary, &marker, false)?;
    }
    Ok(())
}
fn read(root: &Path) -> Result<Option<Intent>> {
    if !io::exists(&root.join(MARKER))? {
        return Ok(None);
    }
    let intent = serde_json::from_slice(&io::bytes(&root.join(MARKER), MAX_JOURNAL_BYTES)?)?;
    validate(&intent)?;
    Ok(Some(intent))
}

struct Layout<'a> {
    files: BTreeMap<String, &'a [u8]>,
    external: BTreeMap<PathBuf, String>,
}
fn normalize(path: &Path) -> Result<PathBuf> {
    let mut result = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                ensure!(result.pop(), "dependency escapes filesystem root");
            }
            Component::RootDir | Component::Normal(_) => result.push(part.as_os_str()),
            _ => anyhow::bail!("invalid dependency location"),
        }
    }
    Ok(result)
}
fn layout<'a>(target: &Path, snapshot: &'a PackageSnapshot) -> Result<Layout<'a>> {
    let mut result = Layout {
        files: BTreeMap::new(),
        external: BTreeMap::new(),
    };
    let mut pending = vec![(target.to_owned(), snapshot.root.clone())];
    let mut seen = BTreeMap::new();
    let mut total = 0usize;
    while let Some((path, revision)) = pending.pop() {
        ensure!(seen.len() < MAX_ENTRIES, "too many dependency locations");
        if let Some(previous) = seen.insert(path.clone(), revision.clone()) {
            ensure!(previous == revision, "conflicting dependency locations");
            continue;
        }
        let node = snapshot
            .packages
            .get(&revision)
            .context("missing package node")?;
        let manifest = node.manifest()?;
        if let Ok(relative) = path.strip_prefix(target) {
            for (name, bytes) in std::iter::once((MANIFEST_FILE, node.manifest_source.as_bytes()))
                .chain(
                    node.files
                        .iter()
                        .map(|(name, bytes)| (name.as_str(), bytes.as_slice())),
                )
            {
                let name = relative
                    .join(name)
                    .to_str()
                    .context("non UTF-8 inventory")?
                    .to_owned();
                total = total
                    .checked_add(bytes.len())
                    .context("materialized package size overflow")?;
                ensure!(
                    total <= MAX_SNAPSHOT_BYTES && result.files.len() < MAX_ENTRIES,
                    "materialized package exceeds limits"
                );
                ensure!(
                    result.files.insert(name, bytes).is_none(),
                    "overlapping package files"
                );
            }
        } else {
            result.external.insert(path.clone(), revision);
        }
        for (alias, dependency) in manifest.dependencies {
            let mut normal_seen = false;
            for part in dependency.path.split('/') {
                if part == ".." {
                    ensure!(
                        !normal_seen,
                        "dependency path must not cancel a directory component"
                    );
                } else {
                    normal_seen = true;
                }
            }
            pending.push((
                normalize(&path.join(dependency.path))?,
                node.dependencies
                    .get(&alias)
                    .context("unresolved dependency")?
                    .clone(),
            ));
        }
    }
    for name in result.files.keys() {
        for (index, _) in name.match_indices('/') {
            ensure!(
                !result.files.contains_key(&name[..index]),
                "package file overlaps a dependency directory"
            );
        }
    }
    Ok(result)
}
fn expected(files: &BTreeMap<String, &[u8]>) -> Inventory {
    let mut result = Inventory::new();
    for (name, bytes) in files {
        result.insert(name.clone(), Some(context_store::hash(bytes)));
        for (index, _) in name.match_indices('/') {
            result.entry(name[..index].to_owned()).or_insert(None);
        }
    }
    result
}
fn check_dependencies(layout: &Layout<'_>) -> Result<()> {
    for (path, revision) in &layout.external {
        ensure!(
            capture_sync(path)?.snapshot.root == *revision,
            Conflict("external package dependency changed")
        );
    }
    Ok(())
}
fn check_preconditions(intent: &Intent) -> Result<()> {
    for p in &intent.preconditions {
        ensure!(
            context_store::hash(&io::bytes(&p.path, MAX_PACKAGE_FILE_BYTES)?) == p.hash,
            Conflict("dependent definition changed before package publication")
        );
    }
    Ok(())
}
fn prepare_stage(root: &Path, intent: &Intent, layout: &Layout<'_>) -> Result<()> {
    let path = stage(root, intent);
    io::directory(&path, true)?;
    let wanted = expected(&layout.files);
    let actual = io::inventory(&path)?;
    ensure!(
        actual
            .iter()
            .all(|(name, hash)| wanted.get(name) == Some(hash)),
        Conflict("staged package inventory changed")
    );
    for (name, bytes) in &layout.files {
        let destination = path.join(name);
        io::directory(destination.parent().context("missing parent")?, true)?;
        if io::exists(&destination)? {
            ensure!(
                io::bytes(&destination, MAX_PACKAGE_FILE_BYTES)? == *bytes,
                Conflict("staged package bytes changed")
            );
        } else {
            // A crash during this write can leave a private fragment outside both
            // the catalog and the staging inventory. Never truncate or remove
            // such fragments on recovery: an external edit must survive.
            let temporary = root.join(format!(".package-part-{}", uuid::Uuid::new_v4()));
            io::create_file(&temporary, bytes)?;
            io::rename(&temporary, &destination, false)?;
        }
    }
    ensure!(
        io::inventory(&path)? == wanted,
        "staged package inventory differs from snapshot"
    );
    Ok(())
}
fn install(workspace: &Path, root: &Path, intent: &mut Intent) -> Result<()> {
    let destination = target(root, intent)?;
    let staged = stage(root, intent);
    let layout = layout(&destination, &intent.snapshot)?;
    let wanted = expected(&layout.files);
    if matches!(intent.phase, Phase::Installed | Phase::Finishing) {
        return Ok(());
    }
    if intent.phase == Phase::Staging {
        check_dependencies(&layout)?;
        check_preconditions(intent)?;
        check_previous(&destination, intent)?;
        prepare_stage(root, intent, &layout)?;
        intent.phase = Phase::Ready;
        save(root, intent)?;
    }
    // EXCHANGE moves the old tree to the staging name. This shape proves a
    // completed rename even when the process died before recording Installed.
    let actual = if io::exists(&destination)? {
        Some(io::inventory(&destination)?)
    } else {
        None
    };
    let staged_actual = if io::exists(&staged)? {
        Some(io::inventory(&staged)?)
    } else {
        None
    };
    let already_installed =
        actual.as_ref() == Some(&wanted) && staged_actual == intent.previous_inventory;
    if !already_installed {
        check_dependencies(&layout)?;
        check_preconditions(intent)?;
        check_previous(&destination, intent)?;
        ensure!(
            staged_actual.as_ref() == Some(&wanted),
            Conflict("staged package changed before publication")
        );
        io::rename(&staged, &destination, intent.previous_revision.is_some())?;
        ensure!(
            io::inventory(&destination)? == wanted,
            Conflict("package changed during atomic publication")
        );
        if let Some(previous) = &intent.previous_inventory {
            ensure!(
                io::inventory(&staged)? == *previous,
                Conflict("replaced package changed during publication; backup preserved")
            );
        }
    }
    intent.phase = Phase::Installed;
    save(root, intent)?;
    let _ = workspace;
    Ok(())
}
fn check_previous(target: &Path, intent: &Intent) -> Result<()> {
    let actual = if io::exists(target)? {
        Some(capture_sync(target)?.snapshot.root)
    } else {
        None
    };
    ensure!(
        actual == intent.previous_revision,
        Conflict("package changed since preflight")
    );
    if let Some(previous) = &intent.previous_inventory {
        ensure!(
            io::inventory(target)? == *previous,
            Conflict("package inventory changed since preflight")
        );
    }
    Ok(())
}
fn cleanup(root: &Path, intent: &Intent) -> Result<()> {
    let staged = stage(root, intent);
    if io::exists(&staged)? {
        let previous = intent
            .previous_inventory
            .as_ref()
            .context("unexpected package backup")?;
        let actual = io::inventory(&staged)?;
        // Finishing is persisted before deletion, so recovery accepts only a
        // subset of the old inventory, and never deletes an added/edited file.
        ensure!(
            actual
                .iter()
                .all(|(name, hash)| previous.get(name) == Some(hash)),
            Conflict("package backup was externally changed; preserved")
        );
        for (name, hash) in actual.iter().rev() {
            let path = staged.join(name);
            if let Some(hash) = hash {
                ensure!(
                    context_store::hash(&io::bytes(&path, MAX_PACKAGE_FILE_BYTES)?) == *hash,
                    Conflict("backup changed during cleanup")
                );
            }
            io::unlink(&path, hash.is_none())?;
        }
        io::unlink(&staged, true)?;
    }
    io::unlink(&root.join(MARKER), false)
}
fn finish(workspace: &Path, root: &Path, mut intent: Intent) -> Result<PackageSnapshot> {
    let destination = target(root, &intent)?;
    let observed = capture_sync(&destination).map(|capture| capture.snapshot);
    if intent.phase != Phase::Finishing {
        // Verify the complete backup before authorizing interruptible cleanup.
        if let Some(previous) = &intent.previous_inventory {
            ensure!(
                io::inventory(&stage(root, &intent))? == *previous,
                Conflict("package backup changed; preserved")
            );
        }
        intent.phase = Phase::Finishing;
        save(root, &intent)?;
    }
    cleanup(root, &intent)?;
    let observed = observed?;
    ensure!(
        observed == intent.snapshot,
        Conflict("package changed after publication; external bytes preserved")
    );
    let _ = workspace;
    Ok(observed)
}

/// Start a journaled package replacement under the shared catalog lock. A
/// cancelled future leaves its owned worker to finish installation and releases
/// the lock only afterwards; the marker then supports a subsequent recovery.
pub async fn begin(write: PackageWrite) -> Result<PendingPackage> {
    begin_checked(write, Vec::new()).await
}

/// Import admission checks the audited catalogues under the existing writer locks.
/// The ordinary package journal still owns publication and recovery.
pub(crate) async fn begin_checked(
    write: PackageWrite,
    conditions: Vec<CataloguePrecondition>,
) -> Result<PendingPackage> {
    tokio::task::spawn_blocking(move || {
        ensure!(
            cfg!(target_os = "linux"),
            "atomic package publication requires Linux renameat2"
        );
        write.snapshot.validate()?;
        io::directory(&write.workspace, false)?;
        let locks = lock_catalogues(&conditions, Some(&write.workspace))?;
        let root = root(&write.workspace)?;
        let destination = root
            .join("flow")
            .join(write.snapshot.root_manifest()?.id.as_str());
        ensure!(
            write.target == destination,
            "package target must be workspace/.zedflow/flow/<manifest.id>"
        );
        io::directory(destination.parent().context("missing catalog")?, true)?;
        let previous_inventory = if io::exists(&destination)? {
            Some(io::inventory(&destination)?)
        } else {
            None
        };
        ensure!(
            previous_inventory.is_some() == write.expected_revision.is_some(),
            Conflict("package target was concurrently created or removed")
        );
        let mut intent = Intent {
            version: 1,
            id: uuid::Uuid::new_v4().to_string(),
            snapshot: write.snapshot,
            previous_revision: write.expected_revision,
            previous_inventory,
            publication: write.publication,
            preconditions: write.preconditions,
            phase: Phase::Staging,
        };
        validate(&intent)?;
        check_previous(&destination, &intent)?;
        check_preconditions(&intent)?;
        check_dependencies(&layout(&destination, &intent.snapshot)?)?;
        save(&root, &intent)?;
        install(&write.workspace, &root, &mut intent)?;
        Ok(PendingPackage {
            workspace: write.workspace,
            intent,
            _lock: locks,
        })
    })
    .await
    .context("package publication worker failed")?
}
fn lock_catalogues(
    conditions: &[CataloguePrecondition],
    workspace: Option<&Path>,
) -> Result<Vec<File>> {
    let mut roots: Vec<_> = conditions
        .iter()
        .map(|c| c.workspace().to_owned())
        .collect();
    roots.extend(workspace.map(Path::to_owned));
    roots.sort();
    roots.dedup();
    let mut locks = Vec::new();
    for root in roots {
        let lock = if conditions.is_empty() {
            context_store::workspace_lock(&root, true, true)?
        } else {
            context_store::clean_workspace_lock(&root, true, true)?
        };
        locks.push(lock.context("package workspace absent")?);
    }
    for condition in conditions {
        condition.check()?;
    }
    Ok(locks)
}

/// Keep final import verification and its completion receipt under the same locks.
pub(crate) async fn guard_catalogues(conditions: Vec<CataloguePrecondition>) -> Result<Vec<File>> {
    tokio::task::spawn_blocking(move || lock_catalogues(&conditions, None)).await?
}

/// Recover only a create-only publication whose exact revision belongs to this
/// pending SQLite import. Other authoring journals need their own recovery owner.
pub(crate) async fn recover_import(
    workspace: PathBuf,
    expected: BTreeMap<String, String>,
) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        let Some(_lock) = context_store::workspace_lock_raw(&workspace, false, true)? else {
            return Ok(());
        };
        let root = root(&workspace)?;
        context_store::require_clean_publications(&root, true)?;
        let Some(mut intent) = read(&root)? else {
            return Ok(());
        };
        let manifest = intent.snapshot.root_manifest()?;
        let id = manifest.id.as_str();
        ensure!(
            intent.publication.is_none()
                && intent.previous_revision.is_none()
                && expected.get(id) == Some(&intent.snapshot.root),
            Conflict("pending package publication does not belong to this legacy import")
        );
        install(&workspace, &root, &mut intent)?;
        finish(&workspace, &root, intent)?;
        Ok(())
    })
    .await?
}

/// Recover a filesystem/SQLite handoff; replay the payload by `id` before finish.
pub async fn recover(workspace: PathBuf) -> Result<Option<PendingPackage>> {
    tokio::task::spawn_blocking(move || {
        let Some(lock) = context_store::workspace_lock_raw(&workspace, false, true)? else {
            return Ok(None);
        };
        let root = root(&workspace)?;
        ensure_no_lifecycle_locked(&root)?;
        context_store::recover_locked(&workspace, &root)?;
        let Some(mut intent) = read(&root)? else {
            return Ok(None);
        };
        install(&workspace, &root, &mut intent)?;
        Ok(Some(PendingPackage {
            workspace,
            intent,
            _lock: vec![lock],
        }))
    })
    .await
    .context("package publication recovery failed")?
}
/// Called while the ordinary catalog holds its exclusive workspace lock.
pub(crate) fn recover_files_locked(workspace: &Path, root: &Path) -> Result<()> {
    let Some(mut intent) = read(root)? else {
        return Ok(());
    };
    install(workspace, root, &mut intent)?;
    ensure!(
        intent.publication.is_none(),
        Conflict("package publication requires daemon recovery before catalogue access")
    );
    finish(workspace, root, intent)?;
    Ok(())
}

#[cfg(test)]
mod import_tests {
    use super::*;

    #[tokio::test]
    async fn checked_import_refuses_catalogue_added_after_audit_before_publication() {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace");
        let home = root.path().join("home");
        for path in [&workspace, &home] {
            std::fs::create_dir(path).unwrap();
        }
        let conditions = capture_catalogue_preconditions(vec![workspace.clone(), home.clone()])
            .await
            .unwrap();
        let outside = home.join(".agents/flows");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("concurrent.rs"), "concurrent authoring").unwrap();
        let snapshot = PackageSnapshot::capture(
            serde_json::json!({"formatVersion":1,"id":"test","name":"Test","entry":"flow.rs","files":["flow.rs"]}).to_string(),
            BTreeMap::from([("flow.rs".into(), b"// preserved source".to_vec())]), Default::default(),
        ).unwrap();
        let target = workspace.join(".zedflow/flow/test");
        let result = begin_checked(
            PackageWrite {
                workspace: workspace.clone(),
                target: target.clone(),
                snapshot,
                expected_revision: None,
                publication: None,
                preconditions: vec![],
            },
            conditions.clone(),
        )
        .await;
        assert!(
            result
                .err()
                .unwrap()
                .to_string()
                .contains("catalogue changed")
        );
        assert!(!target.exists());
        assert!(!workspace.join(".zedflow/.package-acceptance.json").exists());
        assert!(guard_catalogues(conditions).await.is_err());
        assert_eq!(
            std::fs::read_to_string(outside.join("concurrent.rs")).unwrap(),
            "concurrent authoring"
        );
    }
}

#[cfg(test)]
mod import_race_tests {
    use super::*;

    fn snapshot(id: &str) -> PackageSnapshot {
        PackageSnapshot::capture(
            serde_json::json!({"formatVersion":1,"id":id,"name":id,"entry":"flow.rs","files":["flow.rs"]}).to_string(),
            BTreeMap::from([("flow.rs".into(), b"// exact concurrent source".to_vec())]), Default::default(),
        ).unwrap()
    }
    #[tokio::test]
    async fn import_inspection_and_admission_never_recover_a_journal_arriving_after_precheck() {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace");
        let home = root.path().join("home");
        for path in [&workspace, &home] {
            std::fs::create_dir(path).unwrap();
        }
        let conditions = inspect_catalogue_preconditions(vec![workspace.clone(), home.clone()])
            .await
            .unwrap();
        // Another writer installs and leaves its journal after the import's audit.
        let pending = begin(PackageWrite {
            workspace: workspace.clone(),
            target: workspace.join(".zedflow/flow/another"),
            snapshot: snapshot("another"),
            expected_revision: None,
            publication: None,
            preconditions: vec![],
        })
        .await
        .unwrap();
        drop(pending);
        let marker = workspace.join(".zedflow/.package-acceptance.json");
        let before = std::fs::read(&marker).unwrap();
        let store = crate::flow_store::FlowStore::new(
            home.clone(),
            std::sync::Arc::new(|_: &zf_flows::schema::Composition| Ok(())),
        );
        let scoped = crate::workspaces::Workspace {
            id: crate::workspaces::path_id(&workspace),
            name: "fixture".into(),
            path: workspace.clone(),
            open: false,
        };
        assert!(store.inspect_catalog(&scoped).await.is_err());
        assert!(
            inspect_catalogue_preconditions(vec![workspace.clone(), home])
                .await
                .is_err()
        );
        assert!(
            begin_checked(
                PackageWrite {
                    workspace: workspace.clone(),
                    target: workspace.join(".zedflow/flow/imported"),
                    snapshot: snapshot("imported"),
                    expected_revision: None,
                    publication: None,
                    preconditions: vec![]
                },
                conditions.clone()
            )
            .await
            .is_err()
        );
        assert!(guard_catalogues(conditions).await.is_err());
        assert_eq!(std::fs::read(&marker).unwrap(), before);
        assert!(!workspace.join(".zedflow/flow/imported").exists());
        assert_eq!(
            std::fs::read(workspace.join(".zedflow/flow/another/flow.rs")).unwrap(),
            b"// exact concurrent source"
        );
    }
}
