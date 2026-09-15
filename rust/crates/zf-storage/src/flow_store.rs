//! File-backed flow catalog. Rust sources, not database rows, own definitions.
use crate::workspaces::{Workspace, path_id};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{io::AsyncWriteExt, sync::Mutex};
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
    pub hash: String,
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
}

/// A conflict-checked proposal. The accepting writer checks the hash again
/// under the workspace authoring lock before publishing these exact bytes.
pub struct FlowWrite {
    pub path: PathBuf,
    pub lock_workspace: PathBuf,
    pub source: String,
    pub composition: Composition,
    pub create: bool,
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
        }
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
        let mut roots = vec![workspace.path.clone(), self.home.clone()];
        roots.sort();
        roots.dedup();
        let mut _catalog_guards = Vec::new();
        for root in roots {
            _catalog_guards.push(crate::context_store::reader_lock(root).await?);
        }
        let mut flows = Vec::new();
        let mut seen = HashSet::new();
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
                            match flow_source::parse(&source, self.validator.as_ref()) {
                                Ok(doc) => {
                                    flow.file_version = Some(doc.format_version);
                                    flow.id = doc.id.clone();
                                    flow.name = doc.name.clone();
                                    flow.composition = Some(doc);
                                }
                                Err(error) => flow.diagnostics.push(format!("{error:#}")),
                            }
                        }
                        Err(error) => flow
                            .diagnostics
                            .push(format!("Lecture du fichier impossible : {error}")),
                    }
                    flows.push(flow);
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
        if !tokio::fs::symlink_metadata(&flow.path).await?.is_file() {
            return Ok(flow);
        }
        if let Ok(source) = tokio::fs::read_to_string(&flow.path).await {
            flow.hash = hash(source.as_bytes());
            // Parse the same bytes returned and later frozen by callers.
            flow.composition = flow_source::parse(&source, self.validator.as_ref()).ok();
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
        let _authoring = crate::context_store::writer_lock(plan.lock_workspace).await?;
        atomic_write(
            &plan.path,
            plan.source.as_bytes(),
            plan.create,
            expected_hash,
        )
        .await?;
        drop(_authoring);
        self.get(workspace, &path_id(&plan.path)).await
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
        let (path, create, lock_workspace) = if let Some(key) = key {
            let old = self.get(workspace, key).await?;
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
            let directory = root.join(".zedflow/flows");
            tokio::fs::create_dir_all(&directory).await?;
            let directory = tokio::fs::canonicalize(directory).await?;
            (directory.join(filename(&doc)), true, root)
        };
        let source = flow_source::render(&doc, self.validator.as_ref())?;
        let restored = flow_source::parse(&source, self.validator.as_ref())?;
        ensure!(
            restored.id == doc.id,
            "Le fichier généré ne préserve pas l’identité du flow"
        );
        Ok(FlowWrite {
            path,
            lock_workspace,
            source,
            composition: doc,
            create,
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
        file_version: None,
        composition: None,
        diagnostics: vec![diagnostic],
        source: None,
    }
}

fn filename(doc: &Composition) -> String {
    let slug: String = doc
        .name
        .chars()
        .take(64)
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug.trim_matches('-');
    let id = hash(doc.id.as_bytes());
    format!(
        "{}-{}.rs",
        if slug.is_empty() { "flow" } else { slug },
        &id[..12]
    )
}

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
