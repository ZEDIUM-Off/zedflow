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
use zedflow_ai::utils::abort_signals::AbortController;

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
    dispatch_command_with_controls(runtime, command, RpcDispatchControls::default())
}

#[derive(Clone, Default)]
struct RpcDispatchControls {
    auto_compaction_enabled: Arc<Mutex<bool>>,
    auto_retry_enabled: Arc<Mutex<bool>>,
    bash_abort: Arc<Mutex<Option<AbortController>>>,
}

fn dispatch_command_with_controls(
    runtime: &AgentSessionRuntime,
    command: RpcCommand,
    controls: RpcDispatchControls,
) -> RpcResponse {
    let id = command.id().map(str::to_owned);
    let name = command_name(&command);
    let runtime = runtime.clone();
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())
        .and_then(|executor| {
            executor.block_on(dispatch_command_async(&runtime, command, controls))
        });

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
    let controls = RpcDispatchControls::default();
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
                let controls = controls.clone();
                workers.push(std::thread::spawn(move || {
                    let response = dispatch_command_with_controls(&runtime, command, controls);
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
    controls: RpcDispatchControls,
) -> Result<Option<Value>, String> {
    let session = runtime.session();
    match command {
        RpcCommand::Prompt {
            message,
            images,
            streaming_behavior,
            ..
        } => {
            let images = images
                .map(Value::Array)
                .map(serde_json::from_value)
                .transpose()
                .map_err(|error| format!("Invalid images: {error}"))?;
            let options = Some(AgentHarnessPromptOptions { images });
            match session.prompt(message.clone(), options.clone()).await {
                Ok(_) => Ok(None),
                Err(error) if error.code == AgentHarnessErrorCode::Busy => {
                    match streaming_behavior.as_deref() {
                        Some("steer") => {
                            session
                                .steer(message, options)
                                .await
                                .map_err(|e| e.to_string())?;
                            Ok(None)
                        }
                        Some("followUp") => {
                            session
                                .follow_up(message, options)
                                .await
                                .map_err(|e| e.to_string())?;
                            Ok(None)
                        }
                        Some(value) => Err(format!("Invalid streaming behavior: {value}")),
                        None => Err(error.to_string()),
                    }
                }
                Err(error) => Err(error.to_string()),
            }
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
                .map_err(|e| e.to_string())?;
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
                .map_err(|e| e.to_string())?;
            Ok(None)
        }
        RpcCommand::Abort { .. } => {
            session.abort().await.map_err(|e| e.to_string())?;
            Ok(None)
        }
        RpcCommand::AbortRetry { .. } => {
            // The Rust harness performs retries inside the prompt future; aborting the
            // active turn is the only safe way to stop one at this protocol boundary.
            session.abort().await.map_err(|e| e.to_string())?;
            Ok(None)
        }
        RpcCommand::AbortBash { .. } => {
            if let Some(controller) = controls
                .bash_abort
                .lock()
                .map_err(|_| "RPC bash control lock is poisoned".to_owned())?
                .take()
            {
                controller.abort();
            }
            Ok(None)
        }
        RpcCommand::NewSession { .. } => {
            Err("Session creation is only available when starting the RPC runtime".to_owned())
        }
        RpcCommand::GetState { .. } => {
            let metadata = session.session().get_metadata().await;
            let context = session.session().build_context().await;
            Ok(Some(
                serde_json::to_value(RpcSessionState {
                    model: serde_json::to_value(session.get_model()).ok(),
                    thinking_level: session.get_thinking_level(),
                    is_streaming: false,
                    is_compacting: false,
                    steering_mode: queue_mode_name(session.get_steering_mode()).into(),
                    follow_up_mode: queue_mode_name(session.get_follow_up_mode()).into(),
                    session_file: None,
                    session_id: metadata.id,
                    session_name: None,
                    auto_compaction_enabled: controls
                        .auto_compaction_enabled
                        .lock()
                        .map_err(|_| "RPC state lock is poisoned".to_owned())
                        .map(|enabled| *enabled)?,
                    message_count: context.messages.len(),
                    pending_message_count: 0,
                })
                .map_err(|e| e.to_string())?,
            ))
        }
        RpcCommand::SetModel {
            provider, model_id, ..
        } => {
            let model = zedflow_ai::providers::all::builtin_models()
                .get_model(&provider, &model_id)
                .ok_or_else(|| format!("Model not found: {provider}/{model_id}"))?;
            session
                .set_model(model.clone())
                .await
                .map_err(|e| e.to_string())?;
            Ok(Some(
                serde_json::to_value(model).map_err(|e| e.to_string())?,
            ))
        }
        RpcCommand::CycleModel { .. } => {
            let models = zedflow_ai::providers::all::builtin_models().get_models(None);
            let current = session.get_model();
            let next = models
                .iter()
                .position(|m| m.provider == current.provider && m.id == current.id)
                .and_then(|i| models.get((i + 1) % models.len()))
                .cloned();
            if let Some(model) = next {
                session
                    .set_model(model.clone())
                    .await
                    .map_err(|e| e.to_string())?;
                Ok(Some(
                    serde_json::json!({"model": model, "thinkingLevel": session.get_thinking_level(), "isScoped": false}),
                ))
            } else {
                Ok(Some(Value::Null))
            }
        }
        RpcCommand::GetAvailableModels { .. } => Ok(Some(
            serde_json::json!({"models": zedflow_ai::providers::all::builtin_models().get_models(None)}),
        )),
        RpcCommand::SetThinkingLevel { level, .. } => {
            session
                .set_thinking_level(level)
                .await
                .map_err(|e| e.to_string())?;
            Ok(None)
        }
        RpcCommand::CycleThinkingLevel { .. } => {
            let levels = [
                zedflow_agent::types::ThinkingLevel::Off,
                zedflow_agent::types::ThinkingLevel::Minimal,
                zedflow_agent::types::ThinkingLevel::Low,
                zedflow_agent::types::ThinkingLevel::Medium,
                zedflow_agent::types::ThinkingLevel::High,
                zedflow_agent::types::ThinkingLevel::XHigh,
            ];
            let current = session.get_thinking_level();
            let level = levels[(levels
                .iter()
                .position(|value| *value == current)
                .unwrap_or(0)
                + 1)
                % levels.len()];
            session
                .set_thinking_level(level)
                .await
                .map_err(|e| e.to_string())?;
            Ok(Some(serde_json::json!({"level": level})))
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
                .map_err(|e| e.to_string())?;
            Ok(Some(
                serde_json::to_value(result).map_err(|e| e.to_string())?,
            ))
        }
        RpcCommand::SetAutoCompaction { enabled, .. } => {
            *controls
                .auto_compaction_enabled
                .lock()
                .map_err(|_| "RPC state lock is poisoned".to_owned())? = enabled;
            Ok(None)
        }
        RpcCommand::SetAutoRetry { enabled, .. } => {
            *controls
                .auto_retry_enabled
                .lock()
                .map_err(|_| "RPC retry control lock is poisoned".to_owned())? = enabled;
            Ok(None)
        }
        RpcCommand::Bash {
            command,
            exclude_from_context,
            ..
        } => {
            use zedflow_agent::harness::types::ShellExecOptions;
            let controller = AbortController::new();
            let signal = controller.signal();
            *controls
                .bash_abort
                .lock()
                .map_err(|_| "RPC bash control lock is poisoned".to_owned())? = Some(controller);
            let result = session
                .env()
                .exec(
                    &command,
                    Some(ShellExecOptions {
                        cwd: Some(runtime.cwd().to_owned()),
                        abort_signal: Some(signal.clone()),
                        ..Default::default()
                    }),
                )
                .await;
            controls
                .bash_abort
                .lock()
                .map_err(|_| "RPC bash control lock is poisoned".to_owned())?
                .take();
            let result = result.map_err(|e| e.to_string())?;
            Ok(Some(
                serde_json::json!({"command": command, "output": format!("{}{}", result.stdout, result.stderr), "exitCode": result.exit_code, "cancelled": signal.aborted(), "excludeFromContext": exclude_from_context}),
            ))
        }
        RpcCommand::GetSessionStats { .. } => {
            let messages = session.session().build_context().await.messages;
            let mut stats = serde_json::json!({
                "sessionFile": null,
                "sessionId": session.session().get_metadata().await.id,
                "userMessages": 0,
                "assistantMessages": 0,
                "toolCalls": 0,
                "toolResults": 0,
                "totalMessages": messages.len(),
                "tokens": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0},
                "cost": 0
            });
            for message in messages {
                let value = serde_json::to_value(message).map_err(|e| e.to_string())?;
                match value.get("role").and_then(Value::as_str) {
                    Some("user") => {
                        stats["userMessages"] = stats["userMessages"]
                            .as_u64()
                            .unwrap_or(0)
                            .saturating_add(1)
                            .into()
                    }
                    Some("assistant") => {
                        stats["assistantMessages"] = stats["assistantMessages"]
                            .as_u64()
                            .unwrap_or(0)
                            .saturating_add(1)
                            .into();
                        if let Some(content) = value.get("content").and_then(Value::as_array) {
                            let calls = content
                                .iter()
                                .filter(|block| {
                                    block.get("type").and_then(Value::as_str) == Some("toolCall")
                                })
                                .count() as u64;
                            stats["toolCalls"] = stats["toolCalls"]
                                .as_u64()
                                .unwrap_or(0)
                                .saturating_add(calls)
                                .into();
                        }
                        if let Some(usage) = value.get("usage") {
                            for key in ["input", "output", "cacheRead", "cacheWrite"] {
                                let amount = usage.get(key).and_then(Value::as_u64).unwrap_or(0);
                                stats["tokens"][key] = stats["tokens"][key]
                                    .as_u64()
                                    .unwrap_or(0)
                                    .saturating_add(amount)
                                    .into();
                            }
                            let cost = usage
                                .get("cost")
                                .and_then(|cost| cost.get("total"))
                                .and_then(Value::as_f64)
                                .unwrap_or(0.0);
                            stats["cost"] =
                                serde_json::json!(stats["cost"].as_f64().unwrap_or(0.0) + cost);
                        }
                    }
                    Some("toolResult") => {
                        stats["toolResults"] = stats["toolResults"]
                            .as_u64()
                            .unwrap_or(0)
                            .saturating_add(1)
                            .into()
                    }
                    _ => {}
                }
            }
            let total = ["input", "output", "cacheRead", "cacheWrite"]
                .into_iter()
                .map(|key| stats["tokens"][key].as_u64().unwrap_or(0))
                .sum::<u64>();
            stats["tokens"]["total"] = total.into();
            Ok(Some(stats))
        }
        RpcCommand::ExportHtml { output_path, .. } => {
            let path = output_path.unwrap_or_else(|| "session.html".into());
            let content =
                serde_json::to_string_pretty(&session.session().build_context().await.messages)
                    .map_err(|e| e.to_string())?;
            let html = crate::export_html::export_session_to_html(&content);
            std::fs::write(&path, html).map_err(|e| e.to_string())?;
            Ok(Some(serde_json::json!({"path": path})))
        }
        RpcCommand::SwitchSession { session_path, .. } => Err(format!(
            "Session switching is unavailable in this runtime: {session_path}"
        )),
        RpcCommand::Fork { entry_id, .. } => {
            let result = session
                .navigate_tree(&entry_id, Default::default())
                .await
                .map_err(|e| e.to_string())?;
            Ok(Some(serde_json::json!({
                "text": result.editor_text,
                "cancelled": result.cancelled
            })))
        }
        RpcCommand::Clone { .. } => {
            let leaf = session
                .session()
                .get_leaf_id()
                .await
                .ok_or("Cannot clone session: no current entry selected")?;
            let result = session
                .navigate_tree(&leaf, Default::default())
                .await
                .map_err(|e| e.to_string())?;
            Ok(Some(serde_json::json!({"cancelled": result.cancelled})))
        }
        RpcCommand::GetForkMessages { .. } => {
            let messages = session
                .session()
                .get_entries()
                .await
                .into_iter()
                .filter_map(|entry| {
                    let value = serde_json::to_value(entry).ok()?;
                    let message = value.get("message")?;
                    (message.get("role").and_then(Value::as_str) == Some("user")).then(|| {
                        let text = message
                            .get("content")
                            .and_then(|content| {
                                content.as_str().map(str::to_owned).or_else(|| {
                                    content.as_array().map(|blocks| {
                                        blocks
                                            .iter()
                                            .filter_map(|block| {
                                                (block.get("type").and_then(Value::as_str)
                                                    == Some("text"))
                                                .then(|| block.get("text").and_then(Value::as_str))
                                                .flatten()
                                            })
                                            .collect::<String>()
                                    })
                                })
                            })
                            .unwrap_or_default();
                        serde_json::json!({"entryId": value["id"], "text": text})
                    })
                })
                .filter(|message| {
                    message["text"]
                        .as_str()
                        .is_some_and(|text| !text.is_empty())
                })
                .collect::<Vec<_>>();
            Ok(Some(serde_json::json!({"messages": messages})))
        }
        RpcCommand::GetEntries { since, .. } => {
            let entries = session.session().get_entries().await;
            let values = entries
                .into_iter()
                .map(|entry| serde_json::to_value(entry).map_err(|e| e.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            let values = if let Some(since) = since {
                let index = values
                    .iter()
                    .position(|entry| entry["id"] == since)
                    .ok_or_else(|| format!("Entry not found: {since}"))?;
                values.into_iter().skip(index + 1).collect()
            } else {
                values
            };
            Ok(Some(
                serde_json::json!({"entries": values, "leafId": session.session().get_leaf_id().await}),
            ))
        }
        RpcCommand::GetTree { .. } => {
            let entries = session
                .session()
                .get_entries()
                .await
                .into_iter()
                .map(|entry| serde_json::to_value(entry).map_err(|e| e.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Some(
                serde_json::json!({"tree": entries, "leafId": session.session().get_leaf_id().await}),
            ))
        }
        RpcCommand::GetLastAssistantText { .. } => {
            let text = session
                .session()
                .build_context()
                .await
                .messages
                .into_iter()
                .rev()
                .find_map(|message| {
                    let value = serde_json::to_value(message).ok()?;
                    (value.get("role").and_then(Value::as_str) == Some("assistant"))
                        .then(|| {
                            value
                                .get("content")
                                .and_then(Value::as_array)
                                .map(|blocks| {
                                    blocks
                                        .iter()
                                        .filter_map(|block| {
                                            (block.get("type").and_then(Value::as_str)
                                                == Some("text"))
                                            .then(|| block.get("text").and_then(Value::as_str))
                                            .flatten()
                                        })
                                        .collect::<String>()
                                })
                        })
                        .flatten()
                });
            Ok(Some(serde_json::json!({"text": text})))
        }
        RpcCommand::SetSessionName { name, .. } => {
            let name = name.trim();
            if name.is_empty() {
                return Err("Session name cannot be empty".into());
            }
            session
                .session()
                .append_session_name(name.to_owned())
                .await
                .map_err(|e| e.to_string())?;
            Ok(None)
        }
        RpcCommand::GetMessages { .. } => Ok(Some(
            serde_json::json!({"messages": session.session().build_context().await.messages}),
        )),
        RpcCommand::GetCommands { .. } => {
            let resources = session.get_resources();
            let commands = resources.prompt_templates.unwrap_or_default().into_iter().map(|c| serde_json::json!({"name": c.name, "description": c.description, "source": "prompt"})).chain(resources.skills.unwrap_or_default().into_iter().map(|s| serde_json::json!({"name": format!("skill:{}", s.name), "description": s.description, "source": "skill"}))).collect::<Vec<_>>();
            Ok(Some(serde_json::json!({"commands": commands})))
        }
        RpcCommand::ExtensionUiResponse { .. } => Ok(None),
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
