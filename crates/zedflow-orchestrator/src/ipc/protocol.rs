use crate::types::{InstanceRecord, InstanceStatus};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OrchestratorRequest {
    Spawn {
        cwd: String,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        provider: Option<String>,
        #[serde(default)]
        model: Option<String>,
    },
    List,
    Stop {
        instance_id: String,
    },
    Status {
        instance_id: String,
    },
    Rpc {
        instance_id: String,
        command: Value,
    },
    RpcStream {
        instance_id: String,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceSummary {
    pub id: String,
    pub status: InstanceStatus,
    pub cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius_pi_id: Option<String>,
}
impl From<InstanceRecord> for InstanceSummary {
    fn from(value: InstanceRecord) -> Self {
        Self {
            id: value.id,
            status: value.status,
            cwd: value.cwd,
            label: value.label,
            session_id: value.session_id,
            session_file: value.session_file,
            radius_pi_id: value.radius_pi_id,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OrchestratorResponse {
    SpawnResult {
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        instance: Option<InstanceSummary>,
    },
    ListResult {
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        instances: Option<Vec<InstanceSummary>>,
    },
    StopResult {
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        instance_id: Option<String>,
    },
    StatusResult {
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        instance: Option<InstanceSummary>,
    },
    RpcResult {
        ok: bool,
        response: Value,
    },
    RpcReady {
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        instance: Option<InstanceSummary>,
    },
    Error {
        ok: bool,
        error: String,
    },
}
pub fn encode_message<T: Serialize>(message: &T) -> serde_json::Result<String> {
    serde_json::to_string(message).map(|line| line + "\n")
}
pub fn parse_request_line(line: &str) -> serde_json::Result<OrchestratorRequest> {
    serde_json::from_str(line)
}
pub fn parse_response_line(line: &str) -> serde_json::Result<OrchestratorResponse> {
    serde_json::from_str(line)
}
