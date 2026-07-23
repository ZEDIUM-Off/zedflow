//! Headless RPC loop and protocol helpers.

use super::{
    jsonl::serialize_json_line,
    rpc_types::{RpcCommand, RpcResponse, RpcSessionState},
};
use crate::agent_session::{AgentHarnessError, AgentHarnessErrorCode};
use crate::agent_session_runtime::AgentSessionRuntime;
use serde_json::Value;
use std::io::{self, BufRead, Write};
use std::sync::{Arc, Mutex};
use zedflow_agent::harness::types::AgentHarnessPromptOptions;
use zedflow_agent::types::QueueMode;

/// Handle one already-framed command. Runtime integrations can dispatch the
/// returned command; malformed input gets the same error envelope as Pi.
#[must_use]
pub fn handle_command_line(line: &str) -> RpcResponse {
    match parse_command(line) {
        Ok(command) => RpcResponse::success(
            command.id().map(str::to_owned),
            command_name(&command),
            None,
        ),
        Err(response) => response,
    }
}

fn parse_command(line: &str) -> Result<RpcCommand, RpcResponse> {
    let value = serde_json::from_str::<Value>(line).map_err(|error| {
        RpcResponse::error(None, "parse", format!("Failed to parse command: {error}"))
    })?;
    let id = value.get("id").and_then(Value::as_str).map(str::to_owned);
    let command = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("parse")
        .to_owned();
    serde_json::from_value(value).map_err(|error| {
        RpcResponse::error(
            id,
            &command,
            if command == "parse" {
                format!("Failed to parse command: {error}")
            } else {
                format!("Unknown command: {command}")
            },
        )
    })
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

/// Dispatch a parsed command against a live session runtime.
///
/// The legacy [`run_rpc_loop`] remains a framing-only helper for callers that
/// do not construct a session. Embedders with a live runtime must use this
/// function (or [`run_rpc_loop_with_runtime`]); known commands are never
/// acknowledged as successful without reaching the harness.
#[must_use]
pub fn dispatch_command(runtime: &AgentSessionRuntime, command: RpcCommand) -> RpcResponse {
    let id = command.id().map(str::to_owned);
    let name = command_name(&command);
    let runtime = runtime.clone();
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())
        .and_then(|executor| executor.block_on(dispatch_command_async(&runtime, command)));

    match result {
        Ok(data) => RpcResponse::success(id, name, data),
        Err(error) => RpcResponse::error(id, name, error),
    }
}

/// Run JSONL RPC with live [`AgentSessionRuntime`] command dispatch.
pub fn run_rpc_loop_with_runtime<R: BufRead, W: Write + Send + 'static>(
    reader: R,
    writer: W,
    runtime: &AgentSessionRuntime,
) -> io::Result<()> {
    let writer = Arc::new(Mutex::new(writer));
    let event_writer = Arc::clone(&writer);
    let runtime_session = runtime.session();
    let unsubscribe = runtime_session.subscribe(Arc::new(move |event| {
        let event_writer = Arc::clone(&event_writer);
        Box::pin(async move {
            let line = serialize_json_line(&event);
            let mut writer = event_writer.lock().map_err(|_| {
                AgentHarnessError::new(
                    AgentHarnessErrorCode::Hook,
                    "RPC output lock is poisoned",
                    None,
                )
            })?;
            writer.write_all(line.as_bytes()).map_err(|error| {
                AgentHarnessError::new(
                    AgentHarnessErrorCode::Hook,
                    format!("Failed to write RPC event: {error}"),
                    None,
                )
            })?;
            writer.flush().map_err(|error| {
                AgentHarnessError::new(
                    AgentHarnessErrorCode::Hook,
                    format!("Failed to flush RPC event: {error}"),
                    None,
                )
            })?;
            Ok(())
        })
    }));

    let mut workers = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let response = match parse_command(&line) {
            Ok(command) => {
                let runtime = runtime.clone();
                let writer = Arc::clone(&writer);
                workers.push(std::thread::spawn(move || {
                    let response = dispatch_command(&runtime, command);
                    let mut writer = writer
                        .lock()
                        .map_err(|_| io::Error::other("RPC output lock is poisoned"))?;
                    writer.write_all(serialize_json_line(&response).as_bytes())?;
                    writer.flush()
                }));
                continue;
            }
            Err(response) => response,
        };
        let mut writer = writer
            .lock()
            .map_err(|_| io::Error::other("RPC output lock is poisoned"))?;
        writer.write_all(serialize_json_line(&response).as_bytes())?;
        writer.flush()?;
    }
    for worker in workers {
        worker
            .join()
            .map_err(|_| io::Error::other("RPC command worker panicked"))??;
    }
    unsubscribe();
    Ok(())
}

fn queue_mode(value: &str) -> Result<QueueMode, String> {
    match value {
        "all" => Ok(QueueMode::All),
        "one-at-a-time" => Ok(QueueMode::OneAtATime),
        _ => Err(format!("Invalid queue mode: {value}")),
    }
}

async fn dispatch_command_async(
    runtime: &AgentSessionRuntime,
    command: RpcCommand,
) -> Result<Option<Value>, String> {
    let session = runtime.session();
    match command {
        RpcCommand::Prompt {
            message, images, ..
        } => {
            let images = images
                .map(Value::Array)
                .map(serde_json::from_value)
                .transpose()
                .map_err(|error| format!("Invalid images: {error}"))?;
            session
                .prompt(message, Some(AgentHarnessPromptOptions { images }))
                .await
                .map_err(|error| error.to_string())?;
            Ok(None)
        }
        RpcCommand::Steer {
            message, images, ..
        } => {
            let images = images
                .map(Value::Array)
                .map(serde_json::from_value)
                .transpose()
                .map_err(|error| format!("Invalid images: {error}"))?;
            session
                .steer(message, Some(AgentHarnessPromptOptions { images }))
                .await
                .map_err(|error| error.to_string())?;
            Ok(None)
        }
        RpcCommand::FollowUp {
            message, images, ..
        } => {
            let images = images
                .map(Value::Array)
                .map(serde_json::from_value)
                .transpose()
                .map_err(|error| format!("Invalid images: {error}"))?;
            session
                .follow_up(message, Some(AgentHarnessPromptOptions { images }))
                .await
                .map_err(|error| error.to_string())?;
            Ok(None)
        }
        RpcCommand::Abort { .. } => {
            session.abort().await.map_err(|error| error.to_string())?;
            Ok(None)
        }
        RpcCommand::GetState { .. } => {
            let metadata = session.session().get_metadata().await;
            let context = session.session().build_context().await;
            let state = RpcSessionState {
                model: serde_json::to_value(session.get_model()).ok(),
                thinking_level: session.get_thinking_level(),
                is_streaming: false,
                is_compacting: false,
                steering_mode: queue_mode_name(session.get_steering_mode()).into(),
                follow_up_mode: queue_mode_name(session.get_follow_up_mode()).into(),
                session_file: None,
                session_id: metadata.id,
                session_name: None,
                auto_compaction_enabled: true,
                message_count: context.messages.len(),
                pending_message_count: 0,
            };
            Ok(Some(
                serde_json::to_value(state).map_err(|error| error.to_string())?,
            ))
        }
        RpcCommand::SetThinkingLevel { level, .. } => {
            session
                .set_thinking_level(level)
                .await
                .map_err(|error| error.to_string())?;
            Ok(None)
        }
        RpcCommand::SetSteeringMode { mode, .. } => {
            session.set_steering_mode(queue_mode(&mode)?);
            Ok(None)
        }
        RpcCommand::SetFollowUpMode { mode, .. } => {
            session.set_follow_up_mode(queue_mode(&mode)?);
            Ok(None)
        }
        RpcCommand::Compact {
            custom_instructions,
            ..
        } => {
            let result = session
                .compact(custom_instructions.as_deref())
                .await
                .map_err(|error| error.to_string())?;
            Ok(Some(
                serde_json::to_value(result).map_err(|error| error.to_string())?,
            ))
        }
        RpcCommand::GetEntries { since, .. } => {
            let entries = session.session().get_entries().await;
            let entries = entries
                .into_iter()
                .map(|entry| serde_json::to_value(entry).map_err(|error| error.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            let entries = if let Some(since) = since {
                let Some(index) = entries.iter().position(|entry| entry["id"] == since) else {
                    return Err(format!("Entry not found: {since}"));
                };
                entries.into_iter().skip(index + 1).collect()
            } else {
                entries
            };
            let leaf_id = session.session().get_leaf_id().await;
            Ok(Some(
                serde_json::json!({ "entries": entries, "leafId": leaf_id }),
            ))
        }
        _ => Err("Command is not supported by the current runtime".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_commands_keep_pi_correlation_id() {
        let response = handle_command_line(r#"{"id":"request-1","type":"future_command"}"#);
        assert_eq!(response.id.as_deref(), Some("request-1"));
        assert_eq!(response.command, "future_command");
        assert!(!response.success);
    }
}

fn queue_mode_name(mode: QueueMode) -> &'static str {
    match mode {
        QueueMode::All => "all",
        QueueMode::OneAtATime => "one-at-a-time",
    }
}
