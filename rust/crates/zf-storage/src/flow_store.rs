//! File-backed flow catalog. Rust sources, not database rows, own definitions.
use crate::workspaces::{Workspace, path_id};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::Arc,
};
#[cfg(test)]
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use zf_flows::{flow_format::SourceValidator, flow_source, schema::Composition};

#[derive(Debug)]
pub struct Conflict(pub &'static str);
impl std::fmt::Display for Conflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for Conflict {}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowFile {
    pub key: String,
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    /// Catalogue revision: source hash for legacy files, full closure revision for packages.
    pub hash: String,
    #[serde(default)]
    pub source_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<zf_flows::package::PackageSnapshot>,
    #[serde(skip)]
    pub preconditions: Vec<crate::source_acceptance::FilePrecondition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub composition: Option<Composition>,
    pub diagnostics: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Clone)]
pub struct FlowStore {
    home: PathBuf,
    validator: Arc<dyn SourceValidator>,
    writer: Arc<Mutex<()>>,
    parsed_sources: Arc<std::sync::Mutex<VecDeque<(String, Composition)>>>,
}

/// A conflict-checked proposal. The accepting writer checks the hash again
/// under the workspace authoring lock before publishing these exact bytes.
pub struct FlowWrite {
    pub path: PathBuf,
    pub lock_workspace: PathBuf,
    pub source: String,
    pub composition: Composition,
    pub create: bool,
    pub package: zf_flows::package::PackageSnapshot,
    pub preconditions: Vec<crate::source_acceptance::FilePrecondition>,
}

/// An exact legacy conversion proposal, without catalogue or run publication.
/// The execution service audits consumers and validates the captured Rust before
/// the lifecycle writer rechecks the whole catalogue under its locks.
pub struct FlowConversionPlan {
    pub legacy: FlowFile,
    pub lock_workspace: PathBuf,
    pub target: PathBuf,
    pub package: zf_flows::package::PackageSnapshot,
}

pub fn hash(source: &[u8]) -> String {
    format!("{:x}", Sha256::digest(source))
}

impl FlowStore {
    pub fn new(home: PathBuf, validator: Arc<dyn SourceValidator>) -> Self {
        Self {
            home,
            validator,
            writer: Arc::new(Mutex::new(())),
            parsed_sources: Arc::new(std::sync::Mutex::new(VecDeque::new())),
        }
    }

    fn parse_source(&self, source: &str) -> Result<Composition> {
        let cached = self
            .parsed_sources
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .iter()
            .find(|(bytes, _)| bytes == source)
            .map(|(_, document)| document.clone());
        if let Some(document) = cached {
            // Only the exact Rust syntax is memoized. Semantic validation may
            // depend on the caller's current capabilities and always runs again.
            self.validator.validate(&document)?;
            return Ok(document);
        }
        let document = flow_source::parse(source, self.validator.as_ref())?;
        let mut cached = self
            .parsed_sources
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        // Retain at most 64 documents and 8 MiB of source per store. Files and
        // package closures are still captured afresh on every catalogue read.
        const MAX_BYTES: usize = 8 * 1024 * 1024;
        let mut bytes: usize = cached.iter().map(|(source, _)| source.len()).sum();
        while cached.len() >= 64 || bytes.saturating_add(source.len()) > MAX_BYTES {
            let Some((removed, _)) = cached.pop_front() else {
                return Ok(document);
            };
            bytes -= removed.len();
        }
        cached.push_back((source.to_owned(), document.clone()));
        Ok(document)
    }

    fn roots(&self, workspace: &Workspace) -> [(PathBuf, &'static str); 4] {
        [
            (workspace.path.join(".zedflow/flows"), "workspace"),
            (workspace.path.join(".agents/flows"), "workspace"),
            (self.home.join(".zedflow/flows"), "global"),
            (self.home.join(".agents/flows"), "global"),
        ]
    }

    pub async fn list(&self, workspace: &Workspace) -> Result<Vec<FlowFile>> {
        let mut flows = self.capture_catalog(workspace).await?;
        // List responses expose revisions and diagnostics, never source bodies.
        for flow in &mut flows {
            flow.package = None;
            flow.source = None;
        }
        Ok(flows)
    }

    /// Acquire the hydrated catalogue once, including invalid entries and identity
    /// diagnostics. Captures are request-local, not atomic against external edits;
    /// admission must recheck the captured preconditions and package revisions.
    pub async fn capture_catalog(&self, workspace: &Workspace) -> Result<Vec<FlowFile>> {
        self.capture_catalog_mode(workspace, true).await
    }

    /// Offline import inspection rejects pending publications without recovering them.
    pub(crate) async fn inspect_catalog(&self, workspace: &Workspace) -> Result<Vec<FlowFile>> {
        self.capture_catalog_mode(workspace, false).await
    }

    async fn capture_catalog_mode(
        &self,
        workspace: &Workspace,
        recover: bool,
    ) -> Result<Vec<FlowFile>> {
        let mut roots = vec![workspace.path.clone(), self.home.clone()];
        roots.sort();
        roots.dedup();
        // Coordinated recovery takes all participant locks. Do it before this
        // read holds any root, never from a nested reader-lock acquisition.
        if recover {
            for root in &roots {
                crate::flow_packages::recover_lifecycle(root.clone()).await?;
            }
        }
        let mut _catalog_guards = Vec::new();
        for root in roots {
            _catalog_guards.push(if recover {
                crate::context_store::reader_lock(root).await?
            } else {
                crate::context_store::clean_reader_lock(root).await?
            });
        }
        let mut flows = Vec::new();
        let mut seen = HashSet::new();
        let mut package_keys = HashSet::new();
        for (root, scope) in self.roots(workspace) {
            let mut pending = vec![root];
            while let Some(directory) = pending.pop() {
                let mut entries = match tokio::fs::read_dir(&directory).await {
                    Ok(entries) => entries,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => {
                        flows.push(diagnostic_file(
                            &directory,
                            scope,
                            workspace,
                            format!("Lecture du dossier impossible : {error}"),
                        ));
                        continue;
                    }
                };
                while let Some(entry) = entries.next_entry().await? {
                    let path = entry.path();
                    let kind = entry.file_type().await?;
                    if kind.is_dir() {
                        pending.push(path);
                        continue;
                    }
                    if path.extension().is_none_or(|extension| extension != "rs") {
                        continue;
                    }
                    if kind.is_symlink() {
                        flows.push(diagnostic_file(&path, scope, workspace, "Le fichier est un lien symbolique ; utilisez un fichier Rust dans ce dossier".into()));
                        continue;
                    }
                    if !kind.is_file() {
                        flows.push(diagnostic_file(
                            &path,
                            scope,
                            workspace,
                            "Le flow doit être un fichier Rust ordinaire".into(),
                        ));
                        continue;
                    }
                    let path = tokio::fs::canonicalize(&path).await.unwrap_or(path);
                    if !seen.insert(path.clone()) {
                        continue;
                    }
                    let mut flow = diagnostic_file(&path, scope, workspace, String::new());
                    flow.diagnostics.clear();
                    match tokio::fs::read_to_string(&path).await {
                        Ok(source) => {
                            flow.hash = hash(source.as_bytes());
                            flow.source_hash.clone_from(&flow.hash);
                            flow.preconditions = vec![crate::source_acceptance::FilePrecondition {
                                path: path.clone(),
                                hash: flow.source_hash.clone(),
                            }];
                            match self.parse_source(&source) {
                                Ok(doc) => {
                                    flow.file_version = Some(doc.format_version);
                                    flow.id = doc.id.clone();
                                    flow.name = doc.name.clone();
                                    flow.composition = Some(doc);
                                }
                                Err(error) => flow.diagnostics.push(format!("{error:#}")),
                            }
                            flow.source = Some(source);
                        }
                        Err(error) => flow
                            .diagnostics
                            .push(format!("Lecture du fichier impossible : {error}")),
                    }
                    flows.push(flow);
                }
            }
        }
        // Package roots are distinct from the four historical file roots.
        // Each immediate directory is one package, not another recursive flow catalogue.
        for (root, scope) in [
            (workspace.path.join(".zedflow/flow"), "workspace"),
            (self.home.join(".zedflow/flow"), "global"),
        ] {
            let mut entries = match tokio::fs::read_dir(&root).await {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    flows.push(diagnostic_file(
                        &root,
                        scope,
                        workspace,
                        format!("Lecture des packages impossible : {error}"),
                    ));
                    continue;
                }
            };
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if !seen.insert(path.clone()) {
                    continue;
                }
                let file = self.read_package(&path, scope, workspace).await;
                if file.package.is_some() {
                    package_keys.insert(file.key.clone());
                }
                flows.push(file);
            }
        }
        let mut identities = std::collections::BTreeMap::<String, Vec<usize>>::new();
        for (index, flow) in flows.iter().enumerate() {
            if flow.composition.is_some() || package_keys.contains(&flow.key) {
                identities.entry(flow.id.clone()).or_default().push(index);
            }
        }
        for (id, entries) in identities {
            if entries.len() > 1
                && entries
                    .iter()
                    .any(|i| flows[*i].hash != flows[entries[0]].hash)
            {
                for index in entries {
                    flows[index].diagnostics.push(format!("Identité de flow concurrente : {id}. Choisissez une conversion ou une identité distincte."));
                }
            }
        }
        flows.sort_by(|a, b| {
            a.scope
                .cmp(&b.scope)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                .then_with(|| a.path.cmp(&b.path))
        });
        Ok(flows)
    }

    pub async fn get(&self, workspace: &Workspace, key: &str) -> Result<FlowFile> {
        let mut flow = self
            .list(workspace)
            .await?
            .into_iter()
            .find(|file| file.key == key)
            .context("Flow introuvable dans ce workspace")?;
        if tokio::fs::symlink_metadata(&flow.path).await?.is_dir() {
            let _guard = crate::context_store::reader_lock(if flow.scope == "global" {
                self.home.clone()
            } else {
                workspace.path.clone()
            })
            .await?;
            let mut loaded = self.read_package(&flow.path, &flow.scope, workspace).await;
            for diagnostic in flow.diagnostics {
                if !loaded.diagnostics.contains(&diagnostic) {
                    loaded.diagnostics.push(diagnostic);
                }
            }
            return Ok(loaded);
        }
        if !tokio::fs::symlink_metadata(&flow.path).await?.is_file() {
            return Ok(flow);
        }
        if let Ok(source) = tokio::fs::read_to_string(&flow.path).await {
            flow.hash = hash(source.as_bytes());
            flow.source_hash.clone_from(&flow.hash);
            flow.preconditions = vec![crate::source_acceptance::FilePrecondition {
                path: flow.path.clone(),
                hash: flow.source_hash.clone(),
            }];
            // Parse the same bytes returned and later frozen by callers.
            flow.composition = self.parse_source(&source).ok();
            flow.file_version = flow.composition.as_ref().map(|doc| doc.format_version);
            if let Some(doc) = &flow.composition {
                flow.id = doc.id.clone();
                flow.name = doc.name.clone();
            }
            if flow.composition.is_none() && flow.diagnostics.is_empty() {
                flow.diagnostics.push(
                    "Le fichier a changé pendant sa lecture ; actualisez le catalogue".into(),
                );
            }
            flow.source = Some(source);
        }
        Ok(flow)
    }

    async fn read_package(&self, path: &Path, scope: &str, workspace: &Workspace) -> FlowFile {
        let mut flow = diagnostic_file(path, scope, workspace, String::new());
        flow.diagnostics.clear();
        let result = async {
            let captured = crate::flow_packages::capture_with_preconditions(path).await?;
            let manifest = captured.snapshot.root_manifest()?;
            let source = captured.snapshot.root_node()?.entry_source()?.to_owned();
            let doc = self.parse_source(&source);
            // Capture remains inspectable even when the visual projection is unsupported.
            flow.id = manifest.id.as_str().to_owned();
            flow.name.clone_from(&manifest.name);
            flow.source_hash = hash(source.as_bytes());
            flow.hash.clone_from(&captured.snapshot.root);
            flow.source = Some(source);
            flow.preconditions = captured.preconditions;
            flow.package = Some(captured.snapshot);
            ensure!(
                path.file_name().and_then(|s| s.to_str()) == Some(flow.id.as_str()),
                "Package directory must match its flow identity"
            );
            let doc = doc?;
            ensure!(
                flow.id == doc.id,
                "Package identity and Rust flow identity disagree"
            );
            flow.name.clone_from(&doc.name);
            flow.file_version = Some(doc.format_version);
            flow.composition = Some(doc);
            Ok::<_, anyhow::Error>(())
        }
        .await;
        if let Err(error) = result {
            flow.diagnostics.push(format!("{error:#}"));
        }
        flow
    }

    /// Capture a legacy entry without rewriting its source or revision.
    /// This method performs no installation and grants no execution capability.
    pub async fn plan_conversion(
        &self,
        workspace: &Workspace,
        key: &str,
        expected_hash: &str,
    ) -> Result<FlowConversionPlan> {
        let legacy = self.get(workspace, key).await?;
        ensure!(legacy.package.is_none(), "Ce flow est déjà un package");
        ensure!(
            legacy.hash == expected_hash && !expected_hash.is_empty(),
            Conflict("Le flow a changé avant sa conversion")
        );
        ensure!(
            legacy.diagnostics.is_empty(),
            "Le flow historique contient des diagnostics : {:?}",
            legacy.diagnostics
        );
        let composition = legacy
            .composition
            .as_ref()
            .context("Le flow historique ne peut pas être converti automatiquement")?;
        let source = legacy
            .source
            .as_ref()
            .context("Source historique absente")?;
        ensure!(
            hash(source.as_bytes()) == expected_hash,
            Conflict("La source historique a changé pendant sa lecture")
        );
        ensure!(
            self.roots(workspace)
                .iter()
                .any(|(root, scope)| legacy.scope == *scope && legacy.path.starts_with(root)),
            "Le flow n’appartient pas à une racine historique reconnue"
        );
        let lock_workspace = if legacy.scope == "global" {
            self.home.clone()
        } else {
            workspace.path.clone()
        };
        let target = lock_workspace.join(".zedflow/flow").join(&composition.id);
        let package = zf_flows::package::PackageSnapshot::capture(
            serde_json::json!({"formatVersion":1,"id":composition.id,"name":composition.name,
                "entry":"flow.rs","files":["flow.rs"]})
            .to_string(),
            std::collections::BTreeMap::from([("flow.rs".into(), source.as_bytes().to_vec())]),
            std::collections::BTreeMap::new(),
        )?;
        ensure!(
            self.list(workspace)
                .await?
                .iter()
                .all(|other| other.key == key || other.id != composition.id),
            "Identité de flow concurrente : {}. Résolvez les doublons avant conversion.",
            composition.id
        );
        match tokio::fs::symlink_metadata(&target).await {
            Ok(_) => anyhow::bail!(
                "La destination du package existe déjà : {}",
                target.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(FlowConversionPlan {
            legacy,
            lock_workspace,
            target,
            package,
        })
    }

    pub async fn store(
        &self,
        workspace: &Workspace,
        doc: Composition,
        scope: &str,
        key: Option<&str>,
        expected_hash: Option<&str>,
    ) -> Result<FlowFile> {
        let _guard = self.writer.lock().await;
        let plan = self.plan(workspace, doc, scope, key, expected_hash).await?;
        let path = plan.path.clone();
        crate::flow_packages::begin(crate::flow_packages::PackageWrite {
            workspace: plan.lock_workspace,
            target: path.clone(),
            snapshot: plan.package,
            expected_revision: expected_hash.map(str::to_owned),
            publication: None,
            preconditions: plan.preconditions,
        })
        .await?
        .finish()
        .await?;
        self.get(workspace, &path_id(&path)).await
    }

    pub async fn plan(
        &self,
        workspace: &Workspace,
        mut doc: Composition,
        scope: &str,
        key: Option<&str>,
        expected_hash: Option<&str>,
    ) -> Result<FlowWrite> {
        ensure!(
            ["global", "workspace"].contains(&scope),
            "Portée de flow invalide"
        );
        let mut previous_package = None;
        let mut preconditions = Vec::new();
        let (path, create, lock_workspace) = if let Some(key) = key {
            let old = self.get(workspace, key).await?;
            ensure!(
                old.package.is_some(),
                "Ce flow historique doit être converti explicitement en package avant modification."
            );
            ensure!(
                old.diagnostics.is_empty(),
                "Le package contient des diagnostics : {:?}",
                old.diagnostics
            );
            previous_package = old.package;
            preconditions = old.preconditions;
            ensure!(
                old.composition.is_some(),
                "Ce fichier ne peut pas être modifié visuellement"
            );
            ensure!(
                Some(old.hash.as_str()) == expected_hash,
                Conflict("Le fichier a été modifié. Rechargez-le avant d’enregistrer.")
            );
            doc.revision = old
                .composition
                .as_ref()
                .map_or(Some(1), |doc| doc.revision.checked_add(1))
                .context("Révision maximale du flow atteinte")?;
            let lock_workspace = if old.scope == "global" {
                self.home.clone()
            } else {
                workspace.path.clone()
            };
            (old.path, false, lock_workspace)
        } else {
            doc.revision = 1;
            let root = if scope == "global" {
                self.home.clone()
            } else {
                workspace.path.clone()
            };
            (root.join(".zedflow/flow").join(&doc.id), true, root)
        };
        let source = flow_source::render(&doc, self.validator.as_ref())?;
        let restored = flow_source::parse(&source, self.validator.as_ref())?;
        ensure!(
            restored.id == doc.id,
            "Le fichier généré ne préserve pas l’identité du flow"
        );
        let package = if let Some(previous) = previous_package {
            let mut snapshot = previous;
            let mut node = snapshot
                .packages
                .remove(&snapshot.root)
                .context("Package root absent")?;
            let mut manifest = node.manifest()?;
            ensure!(
                manifest.id.as_str() == doc.id,
                "Une modification ne peut pas changer l’identité du package"
            );
            if manifest.name != doc.name {
                manifest.name.clone_from(&doc.name);
                node.manifest_source = serde_json::to_string_pretty(&manifest)?;
            }
            node.files.insert(
                zf_flows::package::ENTRY_FILE.into(),
                source.as_bytes().to_vec(),
            );
            snapshot.root = node.revision();
            snapshot.packages.insert(snapshot.root.clone(), node);
            snapshot.validate()?;
            snapshot
        } else {
            let manifest = zf_flows::package::FlowPackageManifest {
                format_version: zf_flows::package::PACKAGE_FORMAT_VERSION,
                id: doc.id.clone().into(),
                name: doc.name.clone(),
                description: None,
                entry: zf_flows::package::ENTRY_FILE.into(),
                files: vec![zf_flows::package::ENTRY_FILE.into()],
                dependencies: Default::default(),
            };
            zf_flows::package::PackageSnapshot::capture(
                serde_json::to_string_pretty(&manifest)?,
                std::collections::BTreeMap::from([(
                    zf_flows::package::ENTRY_FILE.into(),
                    source.as_bytes().to_vec(),
                )]),
                Default::default(),
            )?
        };
        Ok(FlowWrite {
            path,
            lock_workspace,
            source,
            composition: doc,
            create,
            package,
            preconditions,
        })
    }

    pub async fn delete(
        &self,
        workspace: &Workspace,
        key: &str,
        expected_hash: &str,
    ) -> Result<()> {
        let _guard = self.writer.lock().await;
        let old = self.get(workspace, key).await?;
        let _authoring = crate::context_store::writer_lock(if old.scope == "global" {
            self.home.clone()
        } else {
            workspace.path.clone()
        })
        .await?;
        ensure!(
            !old.hash.is_empty() && old.hash == expected_hash,
            Conflict("Le fichier a été modifié. Actualisez avant de le supprimer.")
        );
        ensure!(
            hash(&tokio::fs::read(&old.path).await?) == expected_hash,
            Conflict("Le fichier a changé avant sa suppression.")
        );
        ensure!(
            tokio::fs::symlink_metadata(&old.path).await?.is_file(),
            "La suppression concerne uniquement un fichier de flow"
        );
        tokio::fs::remove_file(old.path).await?;
        Ok(())
    }
}

fn diagnostic_file(
    path: &Path,
    scope: &str,
    workspace: &Workspace,
    diagnostic: String,
) -> FlowFile {
    let key = path_id(path);
    FlowFile {
        id: key.clone(),
        key,
        name: path
            .file_stem()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        path: path.into(),
        scope: scope.into(),
        workspace_id: (scope == "workspace").then(|| workspace.id.clone()),
        hash: String::new(),
        source_hash: String::new(),
        package: None,
        preconditions: vec![],
        file_version: None,
        composition: None,
        diagnostics: vec![diagnostic],
        source: None,
    }
}

#[cfg(test)]
async fn atomic_write(
    path: &Path,
    bytes: &[u8],
    create: bool,
    expected_hash: Option<&str>,
) -> Result<()> {
    let parent = path.parent().context("Dossier parent absent")?;
    let temporary = parent.join(format!(".zedflow-{}.tmp", uuid::Uuid::new_v4()));
    let result = async {
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .await?;
        file.write_all(bytes).await?;
        file.sync_all().await?;
        drop(file);
        if create {
            match tokio::fs::hard_link(&temporary, path).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    return Err(anyhow::Error::new(Conflict(
                        "Un flow existe déjà à cet emplacement.",
                    )));
                }
                Err(error) => return Err(error.into()),
            }
        } else {
            ensure!(
                tokio::fs::symlink_metadata(path).await?.is_file(),
                Conflict("Le fichier a été remplacé. Actualisez avant d’enregistrer.")
            );
            let current = tokio::fs::read(path).await?;
            ensure!(
                expected_hash == Some(hash(&current).as_str()),
                Conflict("Le fichier a été modifié. Rechargez-le avant d’enregistrer.")
            );
            tokio::fs::rename(&temporary, path).await?;
        }
        #[cfg(unix)]
        tokio::fs::File::open(parent).await?.sync_all().await?;
        Ok(())
    }
    .await;
    let _ = tokio::fs::remove_file(temporary).await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn atomic_replacement_preserves_external_edit_and_cleans_temporary_file() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("flow.rs");
        tokio::fs::write(&path, "external").await.unwrap();
        let error = atomic_write(&path, b"canvas", false, Some(&hash(b"loaded")))
            .await
            .unwrap_err();
        assert!(error.downcast_ref::<Conflict>().is_some());
        assert_eq!(tokio::fs::read_to_string(&path).await.unwrap(), "external");
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
    }
}
