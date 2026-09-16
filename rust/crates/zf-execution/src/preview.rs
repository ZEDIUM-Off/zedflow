//! Admit previews against their origin and execute captured definitions in a
//! separate workspace. HTTP and CLI callers share this boundary.
use adk_graph::State;
use anyhow::Result;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;
use zf_compiler::{graph_compiler, programs};
use zf_flows::schema::Composition;
use zf_runtime::{materialize::RuntimePrimitives, workspace_context::ContextSnapshot};
use zf_storage::workspaces;

use crate::{
    commands::{Actor, CommandKind, validate_bindings},
    preparation,
    service::ExecutionService,
    start::{StartDefinition, StartRequest},
};

/// An inline preview requested by an authenticated caller. The actor supplies
/// the source workspace; captured context and provenance are service-owned.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewRun {
    pub composition: Composition,
    #[serde(default)]
    pub input: State,
    #[serde(default = "empty_bindings")]
    pub model_bindings: Value,
}

fn empty_bindings() -> Value {
    json!({})
}

impl ExecutionService {
    /// Freeze source programs and workspace context before starting an isolated
    /// preview. Its closed temporary workspace remains available for history.
    ///
    /// # Errors
    /// Returns admission, source capture, validation, storage or launch errors.
    pub async fn preview(&self, actor: &Actor, request: PreviewRun) -> Result<Value> {
        let b = self.admit(actor, CommandKind::Start, None).await?;
        let authoring = b.authoring_writer.clone();
        let guard = authoring.lock().await;
        let origin = workspaces::get(&b.db, &actor.workspace_id).await?;
        crate::live_files::recover(&b.db, &b.home).await?;
        if origin.path != b.home {
            crate::live_files::recover(&b.db, &origin.path).await?;
        }
        let source_composition = request.composition.clone();
        let mut composition = request.composition;
        let sources = crate::sources::program_sources(&composition, &origin.path, &[]).await?;
        programs::freeze(&mut composition, &sources)?;
        graph_compiler::validate(&composition, &RuntimePrimitives)?;
        validate_bindings(&composition, &request.model_bindings)?;
        let conditions = crate::sources::captured_sources(&composition, &origin.path)?
            .into_iter()
            .map(|condition| (condition.path, condition.hash))
            .collect();
        keep_frozen(&mut composition)?;
        let context =
            ContextSnapshot::load_with_home(&origin.path, &b.skill_dirs, b.context_home.as_deref())
                .await?;
        preparation::recheck(&conditions).await?;
        let path = b
            .data
            .join("previews")
            .join(Uuid::new_v4().to_string())
            .join("workspace");
        tokio::fs::create_dir_all(&path).await?;
        let mut workspace = workspaces::open(&b.db, &path).await?;
        workspace.open = false;
        workspaces::save(&b.db, &workspace).await?;
        let source_ref = b
            .content
            .intern(&serde_json::to_value(source_composition)?)
            .await?;
        let metadata = json!({
            "sourceWorkspaceId": origin.id,
            "sourceWorkspacePath": origin.path,
            "sourceCompositionRef": source_ref,
            "temporaryWorkspace": true
        });
        drop(guard);
        // The lease and actor stay attached to the originating workspace. The
        // service selected the temporary destination, so do not authorize again
        // against a workspace the caller could not know before this command.
        Self::start_admitted(
            b,
            &workspace.id,
            StartRequest {
                definition: StartDefinition::Inline(composition),
                input: request.input,
                model_bindings: request.model_bindings,
                node_path: None,
                prepared_context: Some(context),
                preview_metadata: Some(metadata),
            },
        )
        .await
    }
}

fn keep_frozen(doc: &mut Composition) -> Result<()> {
    for node in &mut doc.nodes {
        if node.data.kind == "subgraph" {
            let mut child = serde_json::from_value(node.data.config["composition"].clone())?;
            keep_frozen(&mut child)?;
            node.data.config["composition"] = serde_json::to_value(child)?;
        }
        if matches!(node.data.kind.as_str(), "agent" | "context")
            && node.data.config.get("contextProgram").is_some()
            && let Some(config) = node.data.config.as_object_mut()
        {
            for key in ["contextStrategy", "contextLibraryRef", "contextTypesRef"] {
                config.remove(key);
            }
        }
    }
    Ok(())
}
