//! JSON-serializable RPC protocol contracts.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use zedflow_agent::types::ThinkingLevel;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RpcCommand {
    #[serde(rename = "prompt")]
    Prompt {
        id: Option<String>,
        message: String,
        images: Option<Vec<Value>>,
        streaming_behavior: Option<String>,
    },
    #[serde(rename = "steer")]
    Steer {
        id: Option<String>,
        message: String,
        images: Option<Vec<Value>>,
    },
    #[serde(rename = "follow_up")]
    FollowUp {
        id: Option<String>,
        message: String,
        images: Option<Vec<Value>>,
    },
    #[serde(rename = "abort")]
    Abort { id: Option<String> },
    #[serde(rename = "new_session")]
    NewSession {
        id: Option<String>,
        parent_session: Option<String>,
    },
    #[serde(rename = "get_state")]
    GetState { id: Option<String> },
    #[serde(rename = "set_model")]
    SetModel {
        id: Option<String>,
        provider: String,
        model_id: String,
    },
    #[serde(rename = "cycle_model")]
    CycleModel { id: Option<String> },
    #[serde(rename = "get_available_models")]
    GetAvailableModels { id: Option<String> },
    #[serde(rename = "set_thinking_level")]
    SetThinkingLevel {
        id: Option<String>,
        level: ThinkingLevel,
    },
    #[serde(rename = "cycle_thinking_level")]
    CycleThinkingLevel { id: Option<String> },
    #[serde(rename = "set_steering_mode")]
    SetSteeringMode { id: Option<String>, mode: String },
    #[serde(rename = "set_follow_up_mode")]
    SetFollowUpMode { id: Option<String>, mode: String },
    #[serde(rename = "compact")]
    Compact {
        id: Option<String>,
        custom_instructions: Option<String>,
    },
    #[serde(rename = "set_auto_compaction")]
    SetAutoCompaction { id: Option<String>, enabled: bool },
    #[serde(rename = "set_auto_retry")]
    SetAutoRetry { id: Option<String>, enabled: bool },
    #[serde(rename = "abort_retry")]
    AbortRetry { id: Option<String> },
    #[serde(rename = "bash")]
    Bash {
        id: Option<String>,
        command: String,
        exclude_from_context: Option<bool>,
    },
    #[serde(rename = "abort_bash")]
    AbortBash { id: Option<String> },
    #[serde(rename = "get_session_stats")]
    GetSessionStats { id: Option<String> },
    #[serde(rename = "export_html")]
    ExportHtml {
        id: Option<String>,
        output_path: Option<String>,
    },
    #[serde(rename = "switch_session")]
    SwitchSession {
        id: Option<String>,
        session_path: String,
    },
    #[serde(rename = "fork")]
    Fork {
        id: Option<String>,
        entry_id: String,
    },
    #[serde(rename = "clone")]
    Clone { id: Option<String> },
    #[serde(rename = "get_fork_messages")]
    GetForkMessages { id: Option<String> },
    #[serde(rename = "get_entries")]
    GetEntries {
        id: Option<String>,
        since: Option<String>,
    },
    #[serde(rename = "get_tree")]
    GetTree { id: Option<String> },
    #[serde(rename = "get_last_assistant_text")]
    GetLastAssistantText { id: Option<String> },
    #[serde(rename = "set_session_name")]
    SetSessionName { id: Option<String>, name: String },
    #[serde(rename = "get_messages")]
    GetMessages { id: Option<String> },
    #[serde(rename = "get_commands")]
    GetCommands { id: Option<String> },
    #[serde(rename = "extension_ui_response")]
    ExtensionUiResponse {
        id: String,
        value: Option<String>,
        confirmed: Option<bool>,
        cancelled: Option<bool>,
    },
}

impl RpcCommand {
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        match self {
            Self::ExtensionUiResponse { id, .. } => Some(id),
            Self::Prompt { id, .. }
            | Self::Steer { id, .. }
            | Self::FollowUp { id, .. }
            | Self::Abort { id }
            | Self::NewSession { id, .. }
            | Self::GetState { id }
            | Self::SetModel { id, .. }
            | Self::CycleModel { id }
            | Self::GetAvailableModels { id }
            | Self::SetThinkingLevel { id, .. }
            | Self::CycleThinkingLevel { id }
            | Self::SetSteeringMode { id, .. }
            | Self::SetFollowUpMode { id, .. }
            | Self::Compact { id, .. }
            | Self::SetAutoCompaction { id, .. }
            | Self::SetAutoRetry { id, .. }
            | Self::AbortRetry { id }
            | Self::Bash { id, .. }
            | Self::AbortBash { id }
            | Self::GetSessionStats { id }
            | Self::ExportHtml { id, .. }
            | Self::SwitchSession { id, .. }
            | Self::Fork { id, .. }
            | Self::Clone { id }
            | Self::GetForkMessages { id }
            | Self::GetEntries { id, .. }
            | Self::GetTree { id }
            | Self::GetLastAssistantText { id }
            | Self::SetSessionName { id, .. }
            | Self::GetMessages { id }
            | Self::GetCommands { id } => id.as_deref(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcSessionState {
    pub model: Option<Value>,
    pub thinking_level: ThinkingLevel,
    pub is_streaming: bool,
    pub is_compacting: bool,
    pub steering_mode: String,
    pub follow_up_mode: String,
    pub session_file: Option<String>,
    pub session_id: String,
    pub session_name: Option<String>,
    pub auto_compaction_enabled: bool,
    pub message_count: usize,
    pub pending_message_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcResponse {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub response_type: String,
    pub command: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl RpcResponse {
    #[must_use]
    pub fn success(id: Option<String>, command: impl Into<String>, data: Option<Value>) -> Self {
        Self {
            id,
            response_type: "response".into(),
            command: command.into(),
            success: true,
            data,
            error: None,
        }
    }
    #[must_use]
    pub fn error(id: Option<String>, command: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            id,
            response_type: "response".into(),
            command: command.into(),
            success: false,
            data: None,
            error: Some(error.into()),
        }
    }
}

pub type RpcExtensionUiRequest = Value;
pub type RpcExtensionUiResponse = Value;
