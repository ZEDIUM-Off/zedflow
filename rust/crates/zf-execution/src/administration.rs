//! Admitted context-window and catalogue/archive operations shared by transports.
use crate::{
    commands::{self, Actor, CommandKind, ExecutionError},
    service::{ExecutionContext, ExecutionService, services_for},
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};
use zf_context::{
    context_package::ContextPackage,
    window::{self, WindowPatch},
    window_preparation::WindowSelectionCommand,
};
use zf_core::identity::Revision;
use zf_runtime::{archive_validation::RuntimeArchiveValidation, resources::window_preparation};
use zf_storage::{
    context_store::packages,
    data::{Snapshot, WindowRegistry},
    session_archive,
    workspaces::{self, Workspace},
};

/// Keeps admitted read-only adapter work outside service maintenance.
/// Contents are deliberately opaque: transports cannot construct or mutate an
/// execution context through this guard.
pub struct ReadGuard {
    _context: ExecutionContext,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowQuery {
    pub node_path: String,
    pub alias: String,
    pub revision: Option<Revision>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowEdit {
    pub id: String,
    pub node_path: String,
    pub alias: String,
    pub expected_revision: Revision,
    pub patches: Vec<WindowPatch>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceUpdate {
    pub name: Option<String>,
    pub open: Option<bool>,
}

async fn window_owner(b: &ExecutionContext, id: &str, path: &str, alias: &str) -> Result<()> {
    for record in b.content.records_of_kind(id, "window-owners").await? {
        let owner = b.content.resolve(&record.value_ref).await?;
        if owner["nodePath"] == path && owner["alias"] == alias {
            return Ok(());
        }
    }
    Err(ExecutionError::NotFound("Fenêtre absente pour cet agent".into()).into())
}
fn window_value(path: &str, alias: &str, snapshot: &Snapshot) -> Result<Value> {
    Ok(
        json!({"nodePath":path,"alias":alias,"entityId":snapshot.entity_id,"revision":snapshot.revision,"contentRef":snapshot.content_ref,"window":window::decode(snapshot.value.as_ref())?}),
    )
}

impl ExecutionService {
    /// Authorize a transport-owned read and retain its maintenance lease.
    pub async fn read_scope(&self, actor: &Actor, run_id: Option<&str>) -> Result<ReadGuard> {
        Ok(ReadGuard {
            _context: self.admit(actor, CommandKind::Read, run_id).await?,
        })
    }

    /// Recover committed authoring publications before listing their files.
    /// Recovery changes files, so it shares the authoring lock and survives
    /// cancellation of the caller until its durable work completes.
    pub async fn recover_catalog(&self, actor: &Actor) -> Result<()> {
        let b = self.admit(actor, CommandKind::Read, None).await?;
        tokio::spawn(async move {
            let _authoring = b.authoring_writer.lock().await;
            let workspace = workspaces::get(&b.db, &b.actor.workspace_id).await?;
            crate::live_files::recover(&b.db, &b.home).await?;
            if workspace.path != b.home {
                crate::live_files::recover(&b.db, &workspace.path).await?;
            }
            Ok(())
        })
        .await
        .context("Catalogue recovery owner failed")?
    }

    /// Admitted lightweight projection; detailed contents remain referenced.
    pub async fn projection(&self, actor: &Actor, id: &str) -> Result<Value> {
        let b = self.admit(actor, CommandKind::Read, Some(id)).await?;
        Ok(b.sync.latest(id).await?.0)
    }
    pub async fn context_windows(&self, actor: &Actor, id: &str) -> Result<Value> {
        let b = self.admit(actor, CommandKind::Read, Some(id)).await?;
        let run = commands::definition_run(&b, id).await?;
        let services = services_for(&b, id, &run).await?;
        let registry = WindowRegistry::new(services.data_registry().context("Registre absent")?);
        let mut windows = Vec::new();
        for record in b.content.records_of_kind(id, "window-owners").await? {
            let owner = b.content.resolve(&record.value_ref).await?;
            let path = owner["nodePath"]
                .as_str()
                .context("Agent de fenêtre absent")?;
            let alias = owner["alias"].as_str().context("Alias de fenêtre absent")?;
            let snapshot = registry.read(&window::agent_scope(path), alias).await?;
            let window = window::decode(snapshot.value.as_ref())?;
            windows.push(json!({"nodePath":path,"alias":alias,"entityId":snapshot.entity_id,"revision":snapshot.revision,"programHash":window.program_revision.as_deref().unwrap_or(&window.strategy_revision)}));
        }
        Ok(json!(windows))
    }
    pub async fn context_window(
        &self,
        actor: &Actor,
        id: &str,
        query: WindowQuery,
    ) -> Result<Value> {
        let b = self.admit(actor, CommandKind::Read, Some(id)).await?;
        window_owner(&b, id, &query.node_path, &query.alias).await?;
        let run = commands::definition_run(&b, id).await?;
        let services = services_for(&b, id, &run).await?;
        let registry = WindowRegistry::new(services.data_registry().context("Registre absent")?);
        let scope = window::agent_scope(&query.node_path);
        let snapshot = match &query.revision {
            Some(revision) => registry.revision(&scope, &query.alias, revision).await?,
            None => registry.read(&scope, &query.alias).await?,
        };
        window_value(&query.node_path, &query.alias, &snapshot)
    }
    pub async fn patch_window(
        &self,
        actor: &Actor,
        id: &str,
        command: WindowEdit,
    ) -> Result<Value> {
        let b = self
            .admit(actor, CommandKind::PatchContextWindow, Some(id))
            .await?;
        let id = id.to_owned();
        // An accepted mutation owns its admission until the durable operation
        // finishes even if a transport stops awaiting its response.
        tokio::spawn(async move {
            let _writer = b.writer.lock().await;
            uuid::Uuid::parse_str(&command.id)?;
            window_owner(&b,&id,&command.node_path,&command.alias).await?;
            let run = commands::definition_run(&b,&id).await?;
            let services = services_for(&b,&id,&run).await?;
            services.persist_record("window-edit-commands",&command.id,&serde_json::to_value(&command)?).await?;
            // The intent and its admitted author precede a revision becoming
            // consumable. A failed terminal audit is repaired by the same ID.
            commands::persist_actor_command(&b,&id,&run,&json!({"type":"context_window_edit_requested","command":command})).await?;
            let registry = WindowRegistry::new(services.data_registry().context("Registre absent")?);
            let snapshot = registry.patch_unique(&window::agent_scope(&command.node_path),&command.alias,&command.expected_revision,&command.patches,&format!("ui-window:{}",command.id)).await?;
            let value = window_value(&command.node_path,&command.alias,&snapshot)?;
            commands::persist_actor_command(&b,&id,&run,&json!({"type":"context_window_edited","commandId":command.id,"nodePath":command.node_path,"alias":command.alias,"revision":snapshot.revision})).await?;
            Ok(value)
        }).await.context("Window edit owner failed")?
    }
    pub async fn select_window(
        &self,
        actor: &Actor,
        id: &str,
        command: WindowSelectionCommand,
    ) -> Result<Value> {
        let b = self
            .admit(actor, CommandKind::SelectContextWindow, Some(id))
            .await?;
        let id = id.to_owned();
        tokio::spawn(async move {
            let _writer = b.writer.lock().await;
            window_owner(&b, &id, &command.node_path, &command.alias).await?;
            let run = commands::definition_run(&b, &id).await?;
            let services = services_for(&b, &id, &run).await?;
            uuid::Uuid::parse_str(&command.id)?;
            services
                .persist_record(
                    "window-selection-commands",
                    &command.id,
                    &serde_json::to_value(&command)?,
                )
                .await?;
            commands::persist_actor_command(
                &b,
                &id,
                &run,
                &json!({
                    "type":"context_window_selection_requested", "command":command
                }),
            )
            .await?;
            let value = window_preparation::queue_selection(&services, &command).await?;
            commands::persist_actor_command(
                &b,
                &id,
                &run,
                &json!({"type":"context_window_selection","command":command}),
            )
            .await?;
            Ok(value)
        })
        .await
        .context("Window selection owner failed")?
    }
    /// Export selected inactive runs without interrupting a running visit.
    pub async fn export_sessions(
        &self,
        actor: &Actor,
        ids: &[String],
    ) -> Result<session_archive::ExportResponse> {
        let b = self.admit(actor, CommandKind::ExportSessions, None).await?;
        // Validate the selection before acquiring any per-run write barrier:
        // locking the same journal twice would wait on our own first guard.
        ensure!(
            !ids.is_empty() && ids.len() <= 100,
            ExecutionError::Invalid("Sélectionnez entre 1 et 100 sessions".into())
        );
        ensure!(
            ids.iter().collect::<std::collections::BTreeSet<_>>().len() == ids.len(),
            ExecutionError::Invalid("Session sélectionnée deux fois".into())
        );
        let service = self.clone();
        let ids = ids.to_vec();
        tokio::spawn(async move {
            let _writer = b.writer.lock().await;
            let workspace = workspaces::get(&b.db, &b.actor.workspace_id).await?;
            for id in &ids {
                let _admitted = service
                    .admit(&b.actor, CommandKind::ExportSessions, Some(id))
                    .await?;
                let (run, _) = b.sync.latest(id).await?;
                ensure!(
                    run["status"] != "running" && !service.has_active_run(id),
                    ExecutionError::Busy("Attendez un point d’arrêt avant l’export".into())
                );
            }
            let services: Vec<_> = {
                let services = b.services.lock().await;
                ids.iter()
                    .filter_map(|id| services.get(id).cloned())
                    .collect()
            };
            let mut barriers = Vec::new();
            for service in &services {
                barriers.push(service.export_barrier().await);
            }
            session_archive::export_sessions(
                &b.db,
                &b.data,
                &workspace,
                &ids,
                &RuntimeArchiveValidation,
            )
            .await
        })
        .await
        .context("Session export owner failed")?
    }
    /// Import requires exclusive service maintenance and never launches a run.
    pub async fn import_sessions(
        &self,
        actor: &Actor,
        path: &Path,
    ) -> Result<session_archive::ImportResponse> {
        let (guard, workspace) = self
            .admit_maintenance(actor, CommandKind::ImportSessions)
            .await?;
        let state = self.state.clone();
        let path = path.to_owned();
        tokio::spawn(async move {
            let _maintenance = guard;
            let response = session_archive::import_sessions(
                &state.db,
                &state.data,
                &workspace,
                &path,
                &RuntimeArchiveValidation,
            )
            .await?;
            for run in &response.runs {
                if let Some(id) = run["id"].as_str() {
                    state.sync.seed(id).await?;
                }
            }
            let head: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(seq),0) FROM events")
                .fetch_one(&state.db)
                .await?;
            state.sequence.fetch_max(head, Ordering::SeqCst);
            Ok(response)
        })
        .await
        .context("Session import owner failed")?
    }
    pub async fn download_sessions(&self, actor: &Actor, archive_id: &str) -> Result<Vec<u8>> {
        let b = self.admit(actor, CommandKind::Read, None).await?;
        let workspace = workspaces::get(&b.db, &b.actor.workspace_id).await?;
        session_archive::download(&b.data, archive_id, &workspace).await
    }
    pub async fn open_workspace(&self, actor: &Actor, path: &Path) -> Result<Workspace> {
        let b = self.admit(actor, CommandKind::OpenWorkspace, None).await?;
        let path: PathBuf = path.into();
        tokio::spawn(async move {
            let _writer = b.writer.lock().await;
            workspaces::open(&b.db, &path).await
        })
        .await
        .context("Workspace open owner failed")?
    }
    pub async fn update_workspace(
        &self,
        actor: &Actor,
        request: WorkspaceUpdate,
    ) -> Result<Workspace> {
        let b = self
            .admit(actor, CommandKind::UpdateWorkspace, None)
            .await?;
        tokio::spawn(async move {
            let _writer = b.writer.lock().await;
            let mut workspace = workspaces::get(&b.db, &b.actor.workspace_id).await?;
            if let Some(name) = request.name {
                ensure!(
                    !name.trim().is_empty(),
                    ExecutionError::Invalid("Le nom du workspace est requis".into())
                );
                workspace.name = name.trim().chars().take(160).collect();
            }
            if let Some(open) = request.open {
                workspace.open = open;
            }
            workspaces::save(&b.db, &workspace).await?;
            Ok(workspace)
        })
        .await
        .context("Workspace update owner failed")?
    }
    pub async fn save_type_example(
        &self,
        actor: &Actor,
        data_type: zf_core::types::DataType,
        types: zf_core::types::TypeRegistry,
        label: String,
        value: Value,
    ) -> Result<zf_context::type_examples::TypeExample> {
        let b = self.admit(actor, CommandKind::Authoring, None).await?;
        tokio::spawn(async move {
            let _authoring = b.authoring_writer.lock().await;
            let workspace = workspaces::get(&b.db, &b.actor.workspace_id).await?;
            crate::live_files::recover(&b.db, &workspace.path).await?;
            zf_storage::source_catalog::examples::save(
                workspace.path,
                data_type,
                types,
                label,
                value,
            )
            .await
        })
        .await
        .context("Type example owner failed")?
    }
    pub async fn import_context_package(
        &self,
        actor: &Actor,
        package: &ContextPackage,
    ) -> Result<packages::PackageImport> {
        let b = self.admit(actor, CommandKind::Authoring, None).await?;
        let package = package.clone();
        tokio::spawn(async move {
            let _authoring = b.authoring_writer.lock().await;
            let workspace = workspaces::get(&b.db, &b.actor.workspace_id).await?;
            crate::live_files::recover(&b.db, &workspace.path).await?;
            packages::import_package(workspace.path, &package).await
        })
        .await
        .context("Context package import owner failed")?
    }
}
