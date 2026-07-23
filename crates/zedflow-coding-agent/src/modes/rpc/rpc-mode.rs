//! Headless RPC loop and protocol helpers.

use super::{
    jsonl::serialize_json_line,
    rpc_types::{RpcCommand, RpcResponse},
};
use std::io::{self, BufRead, Write};

/// Handle one already-framed command. Runtime integrations can dispatch the
/// returned command; malformed input gets the same error envelope as Pi.
#[must_use]
pub fn handle_command_line(line: &str) -> RpcResponse {
    match serde_json::from_str::<RpcCommand>(line) {
        Ok(command) => RpcResponse::success(
            command.id().map(str::to_owned),
            command_name(&command),
            None,
        ),
        Err(error) => {
            RpcResponse::error(None, "parse", format!("Failed to parse command: {error}"))
        }
    }
}

fn command_name(command: &RpcCommand) -> &'static str {
    match command {
        RpcCommand::Prompt { .. } => "prompt",
        RpcCommand::Steer { .. } => "steer",
        RpcCommand::FollowUp { .. } => "follow_up",
        RpcCommand::Abort { .. } => "abort",
        RpcCommand::NewSession { .. } => "new_session",
        RpcCommand::GetState { .. } => "get_state",
        RpcCommand::SetModel { .. } => "set_model",
        RpcCommand::CycleModel { .. } => "cycle_model",
        RpcCommand::GetAvailableModels { .. } => "get_available_models",
        RpcCommand::SetThinkingLevel { .. } => "set_thinking_level",
        RpcCommand::CycleThinkingLevel { .. } => "cycle_thinking_level",
        RpcCommand::SetSteeringMode { .. } => "set_steering_mode",
        RpcCommand::SetFollowUpMode { .. } => "set_follow_up_mode",
        RpcCommand::Compact { .. } => "compact",
        RpcCommand::SetAutoCompaction { .. } => "set_auto_compaction",
        RpcCommand::SetAutoRetry { .. } => "set_auto_retry",
        RpcCommand::AbortRetry { .. } => "abort_retry",
        RpcCommand::Bash { .. } => "bash",
        RpcCommand::AbortBash { .. } => "abort_bash",
        RpcCommand::GetSessionStats { .. } => "get_session_stats",
        RpcCommand::ExportHtml { .. } => "export_html",
        RpcCommand::SwitchSession { .. } => "switch_session",
        RpcCommand::Fork { .. } => "fork",
        RpcCommand::Clone { .. } => "clone",
        RpcCommand::GetForkMessages { .. } => "get_fork_messages",
        RpcCommand::GetEntries { .. } => "get_entries",
        RpcCommand::GetTree { .. } => "get_tree",
        RpcCommand::GetLastAssistantText { .. } => "get_last_assistant_text",
        RpcCommand::SetSessionName { .. } => "set_session_name",
        RpcCommand::GetMessages { .. } => "get_messages",
        RpcCommand::GetCommands { .. } => "get_commands",
        RpcCommand::ExtensionUiResponse { .. } => "extension_ui_response",
    }
}

pub fn run_rpc_loop<R: BufRead, W: Write>(reader: R, mut writer: W) -> io::Result<()> {
    for line in reader.lines() {
        let response = handle_command_line(&line?);
        writer.write_all(serialize_json_line(&response).as_bytes())?;
        writer.flush()?;
    }
    Ok(())
}
