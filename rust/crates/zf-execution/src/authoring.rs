//! Admitted authoring commands shared by local and transport adapters.
use anyhow::Result;
use serde::Deserialize;
use zf_compiler::programs::SourceOverride;
use zf_flows::{composition::BridgeDefinition, schema::Composition};
use zf_storage::{
    context_store::SourceFile,
    flow_store::FlowFile,
    workspaces::{self, Workspace},
};

use crate::{
    commands::{Actor, CommandKind},
    live_files, live_flows,
    service::{ExecutionContext, ExecutionService},
};

/// Flow creation or update in the admitted actor's workspace or global catalogue.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoreFlow {
    pub composition: Composition,
    pub scope: Option<String>,
    pub key: Option<String>,
    pub expected_hash: Option<String>,
}

/// Bridge creation or update in the admitted actor's workspace.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoreBridge {
    pub key: String,
    pub bridge: BridgeDefinition,
    pub expected_hash: Option<String>,
}

impl ExecutionService {
    /// Accepts a flow after recovery, source preflight and live-run compatibility checks.
    ///
    /// # Errors
    /// Returns admission, invalid definition, revision conflict or persistence errors.
    pub async fn store_flow(&self, actor: &Actor, request: StoreFlow) -> Result<FlowFile> {
        let b = self.admit(actor, CommandKind::Authoring, None).await?;
        // This async mutex protects the complete recovery/preflight/acceptance
        // transaction against other service authoring commands, including CLI calls.
        let _authoring = b.authoring_writer.lock().await;
        let workspace = authoring_workspace(&b).await?;
        let plan = b
            .flows
            .plan(
                &workspace,
                request.composition,
                request.scope.as_deref().unwrap_or("workspace"),
                request.key.as_deref(),
                request.expected_hash.as_deref(),
            )
            .await?;
        let global = plan.lock_workspace == b.home;
        live_flows::accept(
            &b.db,
            &b.content,
            &b.flows,
            &workspace,
            plan,
            request.expected_hash,
            global,
        )
        .await
    }

    /// Accepts exact strategy, library or type source bytes and publishes affected heads.
    ///
    /// # Errors
    /// Returns admission, invalid source, incompatible dependency, revision conflict
    /// or persistence errors. No source is accepted before admission succeeds.
    pub async fn store_source(&self, actor: &Actor, request: SourceOverride) -> Result<SourceFile> {
        let b = self.admit(actor, CommandKind::Authoring, None).await?;
        let _authoring = b.authoring_writer.lock().await;
        let workspace = authoring_workspace(&b).await?;
        live_files::accept_source(&b.db, &b.flows, &workspace, request).await
    }

    /// Accepts a bridge using the existing composition and live-graph preflight policy.
    ///
    /// # Errors
    /// Returns admission, invalid bridge, incompatible composition, revision conflict
    /// or persistence errors.
    pub async fn store_bridge(&self, actor: &Actor, request: StoreBridge) -> Result<SourceFile> {
        let b = self.admit(actor, CommandKind::Authoring, None).await?;
        let _authoring = b.authoring_writer.lock().await;
        let workspace = authoring_workspace(&b).await?;
        live_files::accept_bridge(
            &b.db,
            &b.flows,
            &workspace,
            &request.key,
            &request.bridge,
            request.expected_hash,
        )
        .await
    }
}

async fn authoring_workspace(b: &ExecutionContext) -> Result<Workspace> {
    let workspace = workspaces::get(&b.db, &b.actor.workspace_id).await?;
    live_files::recover(&b.db, &b.home).await?;
    if workspace.path != b.home {
        live_files::recover(&b.db, &workspace.path).await?;
    }
    Ok(workspace)
}
