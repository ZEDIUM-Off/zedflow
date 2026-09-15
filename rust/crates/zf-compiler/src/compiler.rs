//! Public pure compilation boundary. Physical Cargo packaging is implemented
//! separately; compiling here never invokes Cargo or creates an ADK graph.
use crate::{
    graph_compiler::PrimitiveContracts,
    plan::{self, GraphPlan},
    prepared::{self, CompilationSnapshot, ContextSelection, PreparedRuntime},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use zf_core::diagnostics::Diagnostic;
use zf_flows::composition::ResolveRequest;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompileRequest {
    pub entry: ResolveRequest,
    #[serde(default)]
    pub expected_flow_hashes: BTreeMap<String, String>,
    #[serde(default)]
    pub contexts: BTreeMap<String, ContextSelection>,
}
impl CompileRequest {
    pub fn new(entry: ResolveRequest) -> Self {
        Self {
            entry,
            expected_flow_hashes: BTreeMap::new(),
            contexts: BTreeMap::new(),
        }
    }
}

/// Immutable output of validation and lowering. Mutable/serialized historical
/// definitions must pass through compile again before obtaining this plan.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledPlan {
    revision: String,
    prepared: PreparedRuntime,
    graphs: BTreeMap<String, GraphPlan>,
}
impl CompiledPlan {
    pub fn revision(&self) -> &str {
        &self.revision
    }
    pub fn prepared(&self) -> &PreparedRuntime {
        &self.prepared
    }
    pub fn graphs(&self) -> &BTreeMap<String, GraphPlan> {
        &self.graphs
    }
}

pub fn compile(
    snapshot: &CompilationSnapshot,
    request: &CompileRequest,
    primitives: &dyn PrimitiveContracts,
) -> Result<CompiledPlan, Vec<Diagnostic>> {
    let prepared = prepared::prepare(
        snapshot,
        &request.entry,
        &request.expected_flow_hashes,
        &request.contexts,
        primitives,
    )
    .map_err(|error| {
        if let Some(resolution) = error.downcast_ref::<prepared::ResolutionFailure>() {
            let mut errors = resolution.diagnostics.clone();
            // Keep source diagnostics structured as well; never flatten route
            // codes and their paths into a debug string at the public boundary.
            errors.extend(
                resolution
                    .invalid_sources
                    .iter()
                    .map(|(path, message)| Diagnostic::new("invalid_source", path, message)),
            );
            errors
        } else {
            vec![Diagnostic::new(
                "compilation",
                "$prepare",
                format!("{error:#}"),
            )]
        }
    })?;
    let mut graphs = BTreeMap::new();
    for (instance, flow) in &prepared.flows {
        let graph = plan::lower(&flow.composition, primitives)
            .map_err(|error| vec![Diagnostic::new("lowering", instance, format!("{error:#}"))])?;
        graphs.insert(instance.clone(), graph);
    }
    let encoded = serde_json::to_vec(&(1_u32, &prepared, &graphs))
        .map_err(|error| vec![Diagnostic::new("plan_encoding", "$plan", error.to_string())])?;
    Ok(CompiledPlan {
        revision: format!("{:x}", Sha256::digest(encoded)),
        prepared,
        graphs,
    })
}
