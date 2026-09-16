//! Catalogue-only conversion and deletion. Participant markers bar cooperative
//! readers until every rename and verified cleanup has completed; no run or SQL
//! revision is published here.
use super::{
    Inventory, MAX_ENTRIES, MAX_JOURNAL_BYTES, capture_sync, expected, io, layout, revision,
};
use crate::context_store::{self, Conflict};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::File,
    path::{Component, Path, PathBuf},
};
use zf_flows::package::{MAX_PACKAGE_FILE_BYTES, MAX_SNAPSHOT_BYTES, PackageSnapshot};

pub(crate) const LIFECYCLE_MARKER: &str = ".package-lifecycle.json";
pub(crate) const LIFECYCLE_PARTICIPANT_MARKER: &str = ".package-lifecycle-participant.json";
const CATALOGUES: [&str; 4] = [
    ".zedflow/bridges",
    ".zedflow/flows",
    ".agents/flows",
    ".zedflow/flow",
];
const MAX_PARTICIPANTS: usize = 256;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LegacyRetirement {
    pub path: PathBuf,
    pub before: Vec<u8>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BridgeMutation {
    pub workspace: PathBuf,
    pub path: PathBuf,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}
/// An exact directory inventory, including absence. Capture before auditing the
/// catalogue: this detects new consumers and collisions as well as edited files.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CataloguePrecondition {
    workspace: PathBuf,
    catalogue: String,
    inventory: Option<Inventory>,
}
/// Full closure revision observed while auditing another catalogue package.
/// Reacquisition also validates dependency inventories outside the catalogues.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackagePrecondition {
    pub path: PathBuf,
    pub revision: String,
}
#[derive(Clone, Debug)]
pub struct PackageConversion {
    pub workspace: PathBuf,
    pub snapshot: PackageSnapshot,
    pub legacy: LegacyRetirement,
    pub bridges: Vec<BridgeMutation>,
    pub preconditions: Vec<CataloguePrecondition>,
    pub package_preconditions: Vec<PackagePrecondition>,
}
#[derive(Clone, Debug)]
pub struct PackageDeletion {
    pub workspace: PathBuf,
    pub target: PathBuf,
    pub expected_revision: String,
    pub preconditions: Vec<CataloguePrecondition>,
    pub package_preconditions: Vec<PackagePrecondition>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
enum Operation {
    Convert {
        snapshot: PackageSnapshot,
        legacy: LegacyRetirement,
        bridges: Vec<BridgeMutation>,
    },
    Delete {
        snapshot: PackageSnapshot,
        inventory: Inventory,
    },
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Phase {
    Staging,
    Ready,
    Installed,
    Finishing,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Intent {
    version: u32,
    id: String,
    workspace: PathBuf,
    participants: Vec<PathBuf>,
    preconditions: Vec<CataloguePrecondition>,
    package_preconditions: Vec<PackagePrecondition>,
    operation: Operation,
    phase: Phase,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Participant {
    version: u32,
    id: String,
    coordinator: PathBuf,
}

fn canonical(path: &Path) -> bool {
    path.is_absolute()
        && path.components().count() <= 256
        && path
            .components()
            .all(|c| matches!(c, Component::RootDir | Component::Normal(_)))
        && path.to_str().is_some_and(|s| {
            !s.contains(['\\', '\0'])
                && !s.contains("//")
                && !s.contains("/./")
                && !s.ends_with("/.")
                && (s == "/" || !s.ends_with('/'))
        })
}
fn missing(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<std::io::Error>()
        .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound)
        || error.downcast_ref::<nix::errno::Errno>() == Some(&nix::errno::Errno::ENOENT)
}
fn optional<T>(result: Result<T>) -> Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(e) if missing(&e) => Ok(None),
        Err(e) => Err(e),
    }
}
fn inventory(path: &Path) -> Result<Option<Inventory>> {
    optional(io::inventory(path))
}
fn bytes(path: &Path) -> Result<Option<Vec<u8>>> {
    optional(io::bytes(path, MAX_PACKAGE_FILE_BYTES))
}
fn root(workspace: &Path) -> PathBuf {
    workspace.join(".zedflow")
}
fn marker(workspace: &Path) -> PathBuf {
    root(workspace).join(LIFECYCLE_MARKER)
}
fn participant_marker(workspace: &Path) -> PathBuf {
    root(workspace).join(LIFECYCLE_PARTICIPANT_MARKER)
}
fn target(intent: &Intent) -> Result<PathBuf> {
    Ok(root(&intent.workspace)
        .join("flow")
        .join(snapshot(intent).root_manifest()?.id.as_str()))
}
fn snapshot(intent: &Intent) -> &PackageSnapshot {
    match &intent.operation {
        Operation::Convert { snapshot, .. } | Operation::Delete { snapshot, .. } => snapshot,
    }
}
fn private(intent: &Intent, workspace: &Path, suffix: &str) -> PathBuf {
    root(workspace).join(format!(".package-lifecycle-{}-{suffix}", intent.id))
}
fn package_stage(intent: &Intent) -> PathBuf {
    private(intent, &intent.workspace, "package")
}
fn legacy_backup(intent: &Intent) -> PathBuf {
    private(intent, &intent.workspace, "legacy")
}
fn bridge_stage(intent: &Intent, bridge: &BridgeMutation, index: usize) -> PathBuf {
    private(intent, &bridge.workspace, &format!("bridge-{index}"))
}
fn valid_inventory(inventory: &Inventory) -> Result<()> {
    ensure!(
        inventory.len() <= MAX_ENTRIES,
        "lifecycle inventory exceeds entry limit"
    );
    for (path, hash) in inventory {
        ensure!(
            !path.is_empty()
                && !path.contains(['\\', '\0'])
                && Path::new(path)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_)))
                && path
                    .split('/')
                    .all(|p| !p.is_empty() && p != "." && p != ".."),
            "invalid lifecycle inventory path"
        );
        ensure!(
            hash.as_deref().is_none_or(revision),
            "invalid lifecycle inventory hash"
        );
    }
    Ok(())
}
fn validate(intent: &Intent) -> Result<()> {
    ensure!(
        intent.version == 1 && uuid::Uuid::parse_str(&intent.id)?.to_string() == intent.id,
        "invalid lifecycle identity"
    );
    ensure!(canonical(&intent.workspace), "invalid lifecycle workspace");
    ensure!(
        !intent.participants.is_empty()
            && intent.participants.len() <= MAX_PARTICIPANTS
            && intent.participants.windows(2).all(|w| w[0] < w[1])
            && intent.participants.iter().all(|p| canonical(p))
            && intent.participants.contains(&intent.workspace),
        "invalid lifecycle participants"
    );
    ensure!(
        intent.preconditions.len() == intent.participants.len() * CATALOGUES.len(),
        "lifecycle requires every participant catalogue inventory"
    );
    let mut keys = BTreeSet::new();
    let mut catalogue_entries = 0usize;
    for condition in &intent.preconditions {
        ensure!(
            intent.participants.contains(&condition.workspace)
                && CATALOGUES.contains(&condition.catalogue.as_str())
                && keys.insert((&condition.workspace, &condition.catalogue)),
            "invalid or duplicate lifecycle precondition"
        );
        if let Some(inventory) = &condition.inventory {
            valid_inventory(inventory)?;
            catalogue_entries = catalogue_entries
                .checked_add(inventory.len())
                .context("catalogue inventory size overflow")?;
            ensure!(
                catalogue_entries <= MAX_ENTRIES,
                "lifecycle catalogue audit exceeds entry limit"
            );
        }
    }
    ensure!(
        intent.package_preconditions.len() <= MAX_ENTRIES,
        "too many package audit preconditions"
    );
    let mut packages = BTreeSet::new();
    for condition in &intent.package_preconditions {
        ensure!(
            canonical(&condition.path)
                && revision(&condition.revision)
                && packages.insert(&condition.path)
                && intent
                    .participants
                    .iter()
                    .any(|root| condition.path.parent()
                        == Some(root.join(".zedflow/flow").as_path())),
            "invalid package audit precondition"
        );
        ensure!(
            condition.path != target(intent)?,
            "package audit cannot depend on lifecycle target"
        );
    }
    snapshot(intent).validate()?;
    let destination = target(intent)?;
    let materialized = layout(&destination, snapshot(intent))?;
    // External dependencies are frozen inputs, never deletion targets. Their
    // exact bytes are checked before publication; their location is unrestricted.
    if let Operation::Convert {
        snapshot,
        legacy,
        bridges,
    } = &intent.operation
    {
        ensure!(
            canonical(&legacy.path)
                && legacy.path.extension().is_some_and(|x| x == "rs")
                && [".zedflow/flows", ".agents/flows"]
                    .iter()
                    .any(|d| legacy.path.starts_with(intent.workspace.join(d))),
            "invalid legacy retirement path"
        );
        ensure!(
            legacy.before.len() <= MAX_PACKAGE_FILE_BYTES,
            "legacy source exceeds byte limit"
        );
        let entry = snapshot.root_manifest()?.entry;
        ensure!(
            snapshot.root_node()?.files.get(&entry) == Some(&legacy.before),
            "conversion must preserve exact legacy source bytes"
        );
        ensure!(bridges.len() <= MAX_ENTRIES, "too many bridge mutations");
        let mut mutation_bytes = legacy.before.len();
        let mut paths = BTreeSet::new();
        for bridge in bridges {
            ensure!(
                intent.participants.contains(&bridge.workspace)
                    && canonical(&bridge.path)
                    && bridge.path.parent()
                        == Some(bridge.workspace.join(".zedflow/bridges").as_path())
                    && bridge.path.extension().is_some_and(|x| x == "rs")
                    && paths.insert(&bridge.path),
                "invalid or duplicate bridge mutation path"
            );
            ensure!(
                bridge.before.len() <= MAX_PACKAGE_FILE_BYTES
                    && bridge.after.len() <= MAX_PACKAGE_FILE_BYTES
                    && bridge.before != bridge.after,
                "invalid bridge mutation bytes"
            );
            mutation_bytes = mutation_bytes
                .checked_add(bridge.before.len())
                .and_then(|n| n.checked_add(bridge.after.len()))
                .context("bridge mutation size overflow")?;
            ensure!(
                mutation_bytes <= MAX_SNAPSHOT_BYTES,
                "lifecycle mutations exceed byte limit"
            );
        }
    } else if let Operation::Delete { inventory, .. } = &intent.operation {
        valid_inventory(inventory)?;
        ensure!(
            *inventory == expected(&materialized.files),
            "deletion inventory differs from captured package"
        );
    }
    Ok(())
}
fn participants(workspace: &Path, conditions: &[CataloguePrecondition]) -> Vec<PathBuf> {
    std::iter::once(workspace.to_owned())
        .chain(conditions.iter().map(|c| c.workspace.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
fn locks(workspaces: &[PathBuf], create: bool) -> Result<Vec<File>> {
    let mut result = Vec::with_capacity(workspaces.len());
    for workspace in workspaces {
        ensure!(canonical(workspace), "invalid lifecycle workspace path");
        io::directory(workspace, false)?;
        result.push(
            context_store::workspace_lock_raw(workspace, create, true)?
                .context("lifecycle workspace absent")?,
        );
    }
    Ok(result)
}
/// This hook only checks markers while a reader owns a catalogue lock. Recovery
/// needs all participant locks in canonical order and must run after releasing it.
pub(crate) fn ensure_no_lifecycle_locked(root: &Path) -> Result<()> {
    for name in [LIFECYCLE_MARKER, LIFECYCLE_PARTICIPANT_MARKER] {
        ensure!(
            !optional(io::exists(&root.join(name)))?.unwrap_or(false),
            Conflict("package lifecycle requires coordinated recovery before catalogue access")
        );
    }
    Ok(())
}
fn recover_other_writes(workspaces: &[PathBuf]) -> Result<()> {
    for workspace in workspaces {
        let root = root(workspace);
        ensure_no_lifecycle_locked(&root)?;
        context_store::recover_locked(workspace, &root)?;
        crate::source_acceptance::recover_files_locked(workspace, &root)?;
        super::recover_files_locked(workspace, &root)?;
    }
    Ok(())
}
/// Capture these immediately before the host's consumer/collision audit, then
/// pass the unchanged values to conversion/deletion. No absent directory is made.
pub async fn capture_catalogue_preconditions(
    mut workspaces: Vec<PathBuf>,
) -> Result<Vec<CataloguePrecondition>> {
    tokio::task::spawn_blocking(move || {
        workspaces.sort();
        workspaces.dedup();
        ensure!(
            !workspaces.is_empty() && workspaces.len() <= MAX_PARTICIPANTS,
            "invalid catalogue participant count"
        );
        let mut result = Vec::new();
        let mut entries = 0usize;
        for workspace in workspaces {
            ensure!(canonical(&workspace), "invalid catalogue workspace");
            io::directory(&workspace, false)?;
            let _lock = context_store::workspace_lock(&workspace, false, true)?;
            for catalogue in CATALOGUES {
                let inventory = inventory(&workspace.join(catalogue))?;
                entries = entries
                    .checked_add(inventory.as_ref().map_or(0, Inventory::len))
                    .context("catalogue inventory size overflow")?;
                ensure!(
                    entries <= MAX_ENTRIES,
                    "lifecycle catalogue audit exceeds entry limit"
                );
                result.push(CataloguePrecondition {
                    inventory,
                    workspace: workspace.clone(),
                    catalogue: catalogue.to_owned(),
                });
            }
        }
        Ok(result)
    })
    .await
    .context("catalogue capture worker failed")?
}

fn save(intent: &Intent) -> Result<()> {
    struct BoundedJournal(Vec<u8>);
    impl std::io::Write for BoundedJournal {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.0.len().saturating_add(bytes.len()) > MAX_JOURNAL_BYTES {
                return Err(std::io::Error::other(
                    "lifecycle journal exceeds byte limit",
                ));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut buffer = BoundedJournal(Vec::new());
    serde_json::to_writer(&mut buffer, intent)?;
    let serialized = buffer.0;
    let path = marker(&intent.workspace);
    let previous = optional(io::bytes(&path, MAX_JOURNAL_BYTES))?;
    if let Some(previous) = &previous {
        let previous: Intent = serde_json::from_slice(previous)?;
        let mut expected = intent.clone();
        expected.phase = previous.phase;
        ensure!(
            previous == expected,
            Conflict("lifecycle journal was externally modified")
        );
    }
    let temporary = root(&intent.workspace).join(format!(
        ".package-lifecycle-journal-{}",
        uuid::Uuid::new_v4()
    ));
    io::create_file(&temporary, &serialized)?;
    io::rename(&temporary, &path, previous.is_some())?;
    if let Some(previous) = previous {
        ensure!(
            io::bytes(&temporary, MAX_JOURNAL_BYTES)? == previous,
            Conflict("lifecycle journal changed during exchange; backup preserved")
        );
        io::unlink(&temporary, false)?;
    }
    Ok(())
}
fn read(workspace: &Path) -> Result<Option<Intent>> {
    let Some(bytes) = optional(io::bytes(&marker(workspace), MAX_JOURNAL_BYTES))? else {
        return Ok(None);
    };
    let intent: Intent = serde_json::from_slice(&bytes)?;
    validate(&intent)?;
    ensure!(
        intent.workspace == workspace,
        "lifecycle journal coordinator mismatch"
    );
    Ok(Some(intent))
}
fn participant(intent: &Intent) -> Participant {
    Participant {
        version: 1,
        id: intent.id.clone(),
        coordinator: intent.workspace.clone(),
    }
}
fn install_markers(intent: &Intent) -> Result<()> {
    let expected = participant(intent);
    for workspace in &intent.participants {
        if *workspace == intent.workspace {
            continue;
        }
        ensure!(
            optional(io::bytes(&marker(workspace), MAX_JOURNAL_BYTES))?.is_none(),
            Conflict("participant has another lifecycle coordinator")
        );
        let path = participant_marker(workspace);
        if let Some(bytes) = optional(io::bytes(&path, MAX_JOURNAL_BYTES))? {
            ensure!(
                serde_json::from_slice::<Participant>(&bytes)? == expected,
                Conflict("participant has another lifecycle transaction")
            );
        } else {
            stage_file(intent, workspace, &path, &serde_json::to_vec(&expected)?)?;
        }
    }
    ensure!(
        optional(io::bytes(
            &participant_marker(&intent.workspace),
            MAX_JOURNAL_BYTES
        ))?
        .is_none(),
        Conflict("coordinator is participating in another lifecycle")
    );
    Ok(())
}
fn save_phase(intent: &mut Intent, phase: Phase) -> Result<()> {
    intent.phase = phase;
    save(intent)
}

fn check_preconditions(intent: &Intent, progressing: bool) -> Result<()> {
    for condition in &intent.package_preconditions {
        ensure!(
            capture_sync(&condition.path)?.snapshot.root == condition.revision,
            Conflict("package dependency closure changed since lifecycle audit")
        );
    }
    for condition in &intent.preconditions {
        let path = condition.workspace.join(&condition.catalogue);
        let mut wanted = condition.inventory.clone();
        if progressing {
            // Authorize only our exact completed renames; all other inventory
            // additions/removals/edits (including new consumers) still conflict.
            match &intent.operation {
                Operation::Convert {
                    legacy, bridges, ..
                } => {
                    if bytes(&legacy.path)?.is_none() {
                        remove_entry(&mut wanted, &path, &legacy.path, false)?;
                    }
                    let destination = target(intent)?;
                    if inventory(&destination)?.is_some() {
                        add_package(
                            &mut wanted,
                            &path,
                            &destination,
                            &expected(&layout(&destination, snapshot(intent))?.files),
                        )?;
                    }
                    for bridge in bridges {
                        if bytes(&bridge.path)?.as_ref() == Some(&bridge.after)
                            && let Ok(relative) = bridge.path.strip_prefix(&path)
                        {
                            wanted
                                .as_mut()
                                .context("missing bridge catalogue precondition")?
                                .insert(
                                    relative
                                        .to_str()
                                        .context("non UTF-8 bridge path")?
                                        .to_owned(),
                                    Some(context_store::hash(&bridge.after)),
                                );
                        }
                    }
                }
                Operation::Delete { .. } => {
                    let destination = target(intent)?;
                    if inventory(&destination)?.is_none() {
                        remove_entry(&mut wanted, &path, &destination, true)?;
                    }
                }
            }
        }
        let actual = inventory(&path)?;
        // Installing a first package creates its previously absent catalogue.
        let empty_created = progressing
            && condition.catalogue == ".zedflow/flow"
            && condition.workspace == intent.workspace
            && wanted.is_none()
            && actual.as_ref().is_some_and(|i| i.is_empty());
        ensure!(
            actual == wanted || empty_created,
            Conflict("catalogue changed since lifecycle audit")
        );
    }
    Ok(())
}
fn remove_entry(
    wanted: &mut Option<Inventory>,
    root: &Path,
    path: &Path,
    subtree: bool,
) -> Result<()> {
    if let Ok(relative) = path.strip_prefix(root) {
        let relative = relative.to_str().context("non UTF-8 inventory path")?;
        if let Some(wanted) = wanted {
            wanted.remove(relative);
            if subtree {
                let prefix = format!("{relative}/");
                wanted.retain(|name, _| !name.starts_with(&prefix));
            }
        }
    }
    Ok(())
}
fn add_package(
    wanted: &mut Option<Inventory>,
    root: &Path,
    path: &Path,
    package: &Inventory,
) -> Result<()> {
    if let Ok(relative) = path.strip_prefix(root) {
        let relative = relative.to_str().context("non UTF-8 inventory path")?;
        let wanted = wanted.get_or_insert_with(Inventory::new);
        wanted.insert(relative.to_owned(), None);
        for (name, hash) in package {
            wanted.insert(format!("{relative}/{name}"), hash.clone());
        }
    }
    Ok(())
}
fn stage_file(intent: &Intent, workspace: &Path, path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(actual) = bytes(path)? {
        ensure!(
            actual == contents,
            Conflict("lifecycle staged file changed")
        );
    } else {
        let parent = path.parent().context("staging parent absent")?;
        io::directory(parent, true)?;
        // The journal identity owns a deterministic fragment outside the
        // package inventory. A crash during its write leaves a recoverable
        // prefix rather than an unexplained file inside the package stage.
        let key = context_store::hash(path.to_str().context("non UTF-8 staging path")?.as_bytes());
        let fragment = private(intent, workspace, &format!("fragment-{key}"));
        if let Some(actual) = bytes(&fragment)? {
            ensure!(
                contents.starts_with(&actual),
                Conflict("lifecycle fragment changed; preserved")
            );
            if actual != contents {
                io::unlink(&fragment, false)?;
            }
        }
        if bytes(&fragment)?.is_none() {
            io::create_file(&fragment, contents)?;
        }
        io::rename(&fragment, path, false)?;
    }
    Ok(())
}
fn prepare(intent: &Intent) -> Result<()> {
    if let Operation::Convert { bridges, .. } = &intent.operation {
        let destination = target(intent)?;
        let layout = layout(&destination, snapshot(intent))?;
        super::check_dependencies(&layout)?;
        let stage = package_stage(intent);
        io::directory(&stage, true)?;
        let wanted = expected(&layout.files);
        ensure!(
            io::inventory(&stage)?
                .iter()
                .all(|(p, h)| wanted.get(p) == Some(h)),
            Conflict("lifecycle package stage changed")
        );
        for (name, contents) in layout.files {
            stage_file(intent, &intent.workspace, &stage.join(name), contents)?;
        }
        ensure!(
            io::inventory(&stage)? == wanted,
            Conflict("lifecycle stage differs from package")
        );
        for (index, bridge) in bridges.iter().enumerate() {
            stage_file(
                intent,
                &bridge.workspace,
                &bridge_stage(intent, bridge, index),
                &bridge.after,
            )?;
        }
    }
    Ok(())
}
fn check_initial(intent: &Intent) -> Result<()> {
    check_preconditions(intent, false)?;
    let destination = target(intent)?;
    let materialized = layout(&destination, snapshot(intent))?;
    super::check_dependencies(&materialized)?;
    match &intent.operation {
        Operation::Convert {
            legacy, bridges, ..
        } => {
            ensure!(
                inventory(&destination)?.is_none(),
                Conflict("conversion package target already exists")
            );
            ensure!(
                bytes(&legacy.path)?.as_ref() == Some(&legacy.before),
                Conflict("legacy source changed since conversion audit")
            );
            for bridge in bridges {
                ensure!(
                    bytes(&bridge.path)?.as_ref() == Some(&bridge.before),
                    Conflict("bridge source changed since conversion audit")
                );
            }
        }
        Operation::Delete {
            snapshot,
            inventory: wanted,
        } => {
            ensure!(
                capture_sync(&destination)?.snapshot == *snapshot
                    && io::inventory(&destination)? == *wanted,
                Conflict("package changed since deletion audit")
            );
        }
    }
    Ok(())
}
fn install(intent: &Intent) -> Result<()> {
    let destination = target(intent)?;
    let stage = package_stage(intent);
    let materialized = layout(&destination, snapshot(intent))?;
    super::check_dependencies(&materialized)?;
    check_preconditions(intent, true)?;
    match &intent.operation {
        Operation::Convert {
            legacy, bridges, ..
        } => {
            let backup = legacy_backup(intent);
            match (bytes(&legacy.path)?, bytes(&backup)?) {
                (Some(actual), None) if actual == legacy.before => {
                    io::rename(&legacy.path, &backup, false)?;
                    ensure!(
                        bytes(&backup)?.as_ref() == Some(&legacy.before),
                        Conflict("legacy source changed during retirement; backup preserved")
                    );
                }
                (None, Some(actual)) if actual == legacy.before => {}
                _ => anyhow::bail!(Conflict(
                    "legacy retirement state changed; external bytes preserved"
                )),
            }
            let wanted = expected(&materialized.files);
            match (inventory(&destination)?, inventory(&stage)?) {
                (None, Some(actual)) if actual == wanted => {
                    io::directory(destination.parent().context("package parent absent")?, true)?;
                    io::rename(&stage, &destination, false)?;
                    ensure!(
                        io::inventory(&destination)? == wanted,
                        Conflict("package changed during conversion")
                    );
                }
                (Some(actual), None) if actual == wanted => {}
                _ => anyhow::bail!(Conflict(
                    "conversion package state changed; external bytes preserved"
                )),
            }
            for (index, bridge) in bridges.iter().enumerate() {
                let staged = bridge_stage(intent, bridge, index);
                match (bytes(&bridge.path)?, bytes(&staged)?) {
                    (Some(actual), Some(staged_bytes))
                        if actual == bridge.before && staged_bytes == bridge.after =>
                    {
                        io::rename(&staged, &bridge.path, true)?;
                        ensure!(
                            bytes(&bridge.path)?.as_ref() == Some(&bridge.after)
                                && bytes(&staged)?.as_ref() == Some(&bridge.before),
                            Conflict("bridge changed during conversion; backup preserved")
                        );
                    }
                    (Some(actual), Some(staged_bytes))
                        if actual == bridge.after && staged_bytes == bridge.before => {}
                    _ => anyhow::bail!(Conflict(
                        "conversion bridge state changed; external bytes preserved"
                    )),
                }
            }
        }
        Operation::Delete {
            inventory: wanted, ..
        } => match (inventory(&destination)?, inventory(&stage)?) {
            (Some(actual), None) if actual == *wanted => {
                io::rename(&destination, &stage, false)?;
                ensure!(
                    io::inventory(&stage)? == *wanted,
                    Conflict("package changed during quarantine; backup preserved")
                );
            }
            (None, Some(actual)) if actual == *wanted => {}
            _ => anyhow::bail!(Conflict(
                "deletion package state changed; external bytes preserved"
            )),
        },
    }
    Ok(())
}
fn check_installed(intent: &Intent) -> Result<()> {
    check_preconditions(intent, true)?;
    match &intent.operation {
        Operation::Convert {
            snapshot,
            legacy,
            bridges,
        } => {
            ensure!(
                capture_sync(&target(intent)?)?.snapshot == *snapshot
                    && bytes(&legacy.path)?.is_none(),
                Conflict("converted package or legacy location changed")
            );
            for bridge in bridges {
                ensure!(
                    bytes(&bridge.path)?.as_ref() == Some(&bridge.after),
                    Conflict("converted bridge changed; external bytes preserved")
                );
            }
        }
        Operation::Delete { .. } => ensure!(
            inventory(&target(intent)?)?.is_none(),
            Conflict("deleted package was externally recreated")
        ),
    }
    Ok(())
}
fn cleanup_file(path: &Path, expected: &[u8], partial: bool) -> Result<()> {
    if let Some(actual) = bytes(path)? {
        ensure!(
            actual == expected,
            Conflict("lifecycle backup changed; external bytes preserved")
        );
        io::unlink(path, false)?;
    } else {
        ensure!(
            partial,
            Conflict("lifecycle backup disappeared before cleanup")
        );
    }
    Ok(())
}
fn check_backups(intent: &Intent) -> Result<()> {
    match &intent.operation {
        Operation::Convert {
            legacy, bridges, ..
        } => {
            ensure!(
                bytes(&legacy_backup(intent))?.as_ref() == Some(&legacy.before),
                Conflict("legacy backup changed; preserved")
            );
            for (index, bridge) in bridges.iter().enumerate() {
                ensure!(
                    bytes(&bridge_stage(intent, bridge, index))?.as_ref() == Some(&bridge.before),
                    Conflict("bridge backup changed; preserved")
                );
            }
        }
        Operation::Delete {
            inventory: wanted, ..
        } => ensure!(
            inventory(&package_stage(intent))?.as_ref() == Some(wanted),
            Conflict("package quarantine changed; preserved")
        ),
    }
    Ok(())
}
fn cleanup(intent: &Intent) -> Result<()> {
    match &intent.operation {
        Operation::Convert {
            legacy, bridges, ..
        } => {
            cleanup_file(&legacy_backup(intent), &legacy.before, true)?;
            for (index, bridge) in bridges.iter().enumerate() {
                cleanup_file(&bridge_stage(intent, bridge, index), &bridge.before, true)?;
            }
        }
        Operation::Delete {
            inventory: wanted, ..
        } => {
            let stage = package_stage(intent);
            if let Some(actual) = inventory(&stage)? {
                ensure!(
                    actual.iter().all(|(p, h)| wanted.get(p) == Some(h)),
                    Conflict("quarantine changed during cleanup; preserved")
                );
                for (name, hash) in actual.iter().rev() {
                    let path = stage.join(name);
                    if let Some(hash) = hash {
                        ensure!(
                            context_store::hash(&io::bytes(&path, MAX_PACKAGE_FILE_BYTES)?)
                                == *hash,
                            Conflict("quarantine file changed during cleanup")
                        );
                    }
                    io::unlink(&path, hash.is_none())?;
                }
                io::unlink(&stage, true)?;
            }
        }
    }
    // The coordinator is removed last. Recovery can always find its complete
    // participant list if interrupted while removing these small barriers.
    for workspace in &intent.participants {
        if *workspace == intent.workspace {
            continue;
        }
        let path = participant_marker(workspace);
        if let Some(bytes) = optional(io::bytes(&path, MAX_JOURNAL_BYTES))? {
            // After a cleanup crash, a participant whose barrier was already
            // removed can start another transaction. Never remove that marker.
            if serde_json::from_slice::<Participant>(&bytes)? == participant(intent) {
                io::unlink(&path, false)?;
            }
        }
    }
    ensure!(
        read(&intent.workspace)?.as_ref() == Some(intent),
        Conflict("lifecycle journal changed before cleanup")
    );
    io::unlink(&marker(&intent.workspace), false)
}
fn execute(mut intent: Intent) -> Result<()> {
    // All public changes and backup validation were durable before Finishing.
    // Only owned cleanup remains; released participants may have newer writes.
    if intent.phase == Phase::Finishing {
        return cleanup(&intent);
    }
    install_markers(&intent)?;
    if intent.phase == Phase::Staging {
        check_initial(&intent)?;
        prepare(&intent)?;
        check_initial(&intent)?;
        save_phase(&mut intent, Phase::Ready)?;
    }
    if intent.phase == Phase::Ready {
        install(&intent)?;
        save_phase(&mut intent, Phase::Installed)?;
    }
    check_installed(&intent)?;
    if intent.phase != Phase::Finishing {
        check_backups(&intent)?;
        save_phase(&mut intent, Phase::Finishing)?;
    }
    cleanup(&intent)
}

/// Convert a captured legacy source and exact bridge mutations as one operation
/// for cooperative catalogue readers. The worker owns every lock to completion.
pub async fn convert_package(request: PackageConversion) -> Result<PackageSnapshot> {
    ensure!(
        cfg!(target_os = "linux"),
        "atomic package conversion requires Linux renameat2"
    );
    tokio::task::spawn_blocking(move || {
        let intent = Intent {
            version: 1,
            id: uuid::Uuid::new_v4().to_string(),
            participants: participants(&request.workspace, &request.preconditions),
            workspace: request.workspace,
            preconditions: request.preconditions,
            package_preconditions: request.package_preconditions,
            operation: Operation::Convert {
                snapshot: request.snapshot.clone(),
                legacy: request.legacy,
                bridges: request.bridges,
            },
            phase: Phase::Staging,
        };
        validate(&intent)?;
        let _locks = locks(&intent.participants, true)?;
        recover_other_writes(&intent.participants)?;
        check_initial(&intent)?;
        save(&intent)?;
        execute(intent)?;
        Ok(request.snapshot)
    })
    .await
    .context("package conversion worker failed")?
}
/// Quarantine a package after exact closure/inventory CAS, then delete only its
/// verified inventory. Historical snapshots and run heads remain untouched.
pub async fn delete_package(request: PackageDeletion) -> Result<()> {
    ensure!(
        cfg!(target_os = "linux"),
        "atomic package deletion requires Linux renameat2"
    );
    tokio::task::spawn_blocking(move || {
        ensure!(
            canonical(&request.workspace)
                && canonical(&request.target)
                && revision(&request.expected_revision),
            "invalid package deletion request"
        );
        ensure!(
            request.target.parent() == Some(root(&request.workspace).join("flow").as_path()),
            "deletion target is outside package catalogue"
        );
        let participants = participants(&request.workspace, &request.preconditions);
        ensure!(
            !participants.is_empty() && participants.len() <= MAX_PARTICIPANTS,
            "invalid lifecycle participants"
        );
        let _locks = locks(&participants, true)?;
        recover_other_writes(&participants)?;
        let captured = capture_sync(&request.target)?.snapshot;
        ensure!(
            captured.root == request.expected_revision,
            Conflict("package revision changed before deletion")
        );
        let intent = Intent {
            version: 1,
            id: uuid::Uuid::new_v4().to_string(),
            workspace: request.workspace,
            participants,
            preconditions: request.preconditions,
            package_preconditions: request.package_preconditions,
            operation: Operation::Delete {
                snapshot: captured,
                inventory: io::inventory(&request.target)?,
            },
            phase: Phase::Staging,
        };
        validate(&intent)?;
        ensure!(
            request.target == target(&intent)?,
            "deletion target differs from manifest identity"
        );
        check_initial(&intent)?;
        save(&intent)?;
        execute(intent)
    })
    .await
    .context("package deletion worker failed")?
}
/// Recover from any participant. The initial lookup lock is released before
/// acquiring the entire sorted participant set; this never recurses through the
/// ordinary reader lock, and never holds one root while discovering lock order.
pub async fn recover_lifecycle(workspace: PathBuf) -> Result<bool> {
    tokio::task::spawn_blocking(move || {
        ensure!(
            canonical(&workspace),
            "invalid lifecycle recovery workspace"
        );
        let pointer = {
            let Some(_lock) = context_store::workspace_lock_raw(&workspace, false, true)? else {
                return Ok(false);
            };
            if let Some(intent) = read(&workspace)? {
                Some(participant(&intent))
            } else {
                optional(io::bytes(
                    &participant_marker(&workspace),
                    MAX_JOURNAL_BYTES,
                ))?
                .map(|bytes| serde_json::from_slice::<Participant>(&bytes))
                .transpose()?
            }
        };
        let Some(pointer) = pointer else {
            return Ok(false);
        };
        ensure!(
            pointer.version == 1
                && canonical(&pointer.coordinator)
                && uuid::Uuid::parse_str(&pointer.id)?.to_string() == pointer.id,
            "invalid lifecycle participant pointer"
        );
        let discovered = {
            let _lock = context_store::workspace_lock_raw(&pointer.coordinator, false, true)?
                .context("lifecycle coordinator absent")?;
            read(&pointer.coordinator)?
        };
        let Some(discovered) = discovered else {
            // Another worker may have finished while the lookup lock was released.
            let _lock = context_store::workspace_lock_raw(&workspace, false, true)?;
            ensure_no_lifecycle_locked(&root(&workspace))?;
            return Ok(false);
        };
        ensure!(
            discovered.id == pointer.id && discovered.participants.contains(&workspace),
            "lifecycle participant does not belong to coordinator"
        );
        let _locks = locks(&discovered.participants, false)?;
        let Some(intent) = read(&pointer.coordinator)? else {
            ensure_no_lifecycle_locked(&root(&workspace))?;
            return Ok(false);
        };
        ensure!(
            intent.id == discovered.id
                && intent.participants == discovered.participants
                && intent.operation == discovered.operation
                && intent.preconditions == discovered.preconditions
                && intent.package_preconditions == discovered.package_preconditions,
            Conflict("lifecycle intent changed during lock acquisition")
        );
        execute(intent)?;
        Ok(true)
    })
    .await
    .context("package lifecycle recovery worker failed")?
}
