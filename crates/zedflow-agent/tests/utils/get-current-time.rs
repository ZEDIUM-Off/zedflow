use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use zedflow_agent::types::{
    AgentFuture, AgentTool, AgentToolExecuteFn, AgentToolResult, AgentToolResultContent, Tool,
};
use zedflow_ai::{TextContent, TextContentType};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCurrentTimeDetails {
    pub utc_timestamp: u64,
}

pub type GetCurrentTimeResult = AgentToolResult<GetCurrentTimeDetails>;

pub fn get_current_time(timezone: Option<&str>) -> Result<GetCurrentTimeResult, String> {
    if let Some(timezone) = timezone {
        if timezone != "UTC" && timezone != "Etc/UTC" {
            return Err(format!(
                "Invalid timezone: {timezone}. Current UTC time: {}",
                now_millis()
            ));
        }
    }

    let utc_timestamp = now_millis();
    Ok(AgentToolResult {
        content: vec![text(format!("Current UTC time: {utc_timestamp}"))],
        details: GetCurrentTimeDetails { utc_timestamp },
        terminate: None,
    })
}

pub fn get_current_time_tool() -> AgentTool<GetCurrentTimeDetails> {
    let execute: AgentToolExecuteFn<GetCurrentTimeDetails> =
        Arc::new(|_tool_call_id, args, _signal, _on_update| {
            let timezone = args
                .get("timezone")
                .and_then(Value::as_str)
                .map(str::to_string);
            Box::pin(async move {
                get_current_time(timezone.as_deref()).unwrap_or_else(|message| AgentToolResult {
                    content: vec![text(message)],
                    details: GetCurrentTimeDetails { utc_timestamp: 0 },
                    terminate: Some(true),
                })
            }) as AgentFuture<'_, AgentToolResult<GetCurrentTimeDetails>>
        });

    AgentTool {
        label: "Current Time".into(),
        tool: Tool {
            name: "get_current_time".into(),
            description: "Get the current date and time".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "timezone": {
                        "type": "string",
                        "description": "Optional timezone (e.g., 'America/New_York', 'Europe/London')"
                    }
                }
            }),
        },
        prepare_arguments: None,
        execute: Some(execute),
        execution_mode: None,
    }
}

fn text(value: impl Into<String>) -> AgentToolResultContent {
    AgentToolResultContent::Text(TextContent {
        content_type: TextContentType::Text,
        text: value.into(),
        text_signature: None,
    })
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            duration.as_millis().try_into().unwrap_or(u64::MAX)
        })
}

#[test]
fn returns_current_utc_time_details() {
    let before = now_millis();
    let result = get_current_time(Some("UTC")).unwrap();
    let after = now_millis();
    assert!(result.details.utc_timestamp >= before);
    assert!(result.details.utc_timestamp <= after);
    assert!(matches!(
        &result.content[0],
        AgentToolResultContent::Text(content) if content.text.starts_with("Current UTC time: ")
    ));
}

#[test]
fn rejects_unsupported_timezones_without_adding_timezone_dependencies() {
    let error = get_current_time(Some("Mars/Base")).unwrap_err();
    assert!(error.starts_with("Invalid timezone: Mars/Base."));
}

#[test]
fn exposes_current_time_tool_metadata_and_executor() {
    let tool = get_current_time_tool();
    assert_eq!(tool.label, "Current Time");
    assert_eq!(tool.tool.name, "get_current_time");
    let execute = tool.execute.expect("current time executor");
    let result = futures::executor::block_on(execute(
        "call-1",
        json!({ "timezone": "Etc/UTC" }),
        None,
        None,
    ));
    assert!(result.details.utc_timestamp > 0);
}
