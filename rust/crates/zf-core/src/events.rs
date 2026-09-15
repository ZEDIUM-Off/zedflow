//! Provenance shared by runtime observations and persisted records.
use crate::identity::{InvocationId, NodePath, OccurrenceId};
use serde::{Deserialize, Serialize};

/// Exact passage producing an element. Absence in historical records stays absent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventOrigin {
    pub node_path: NodePath,
    pub occurrence_id: OccurrenceId,
}

/// Locator into a persisted invocation's calls and granted tools.
/// Possessing this value alone grants no capability; the runtime must check it
/// against that immutable record before executing an effect.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallProvenance {
    pub invocation_id: InvocationId,
    pub agent_path: NodePath,
    pub call_index: u64,
}
