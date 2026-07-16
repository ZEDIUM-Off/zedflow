use serde::{Deserialize, Serialize};
use serde_json::Value;
use zedflow_ai::{
    Message, TextContent, TextContentType, UserContentBlock, UserMessage, UserMessageContent,
    UserMessageRole,
};

use crate::types::AgentMessage;

/// Prefix used when exposing a compaction summary to the model.
pub const COMPACTION_SUMMARY_PREFIX: &str = "The conversation history before this point was compacted into the following summary:\n\n<summary>\n";
/// Suffix used when exposing a compaction summary to the model.
pub const COMPACTION_SUMMARY_SUFFIX: &str = "\n</summary>";
/// Prefix used when exposing a returned branch summary to the model.
pub const BRANCH_SUMMARY_PREFIX: &str =
    "The following is a summary of a branch that this conversation came back from:\n\n<summary>\n";
/// Suffix used when exposing a returned branch summary to the model.
pub const BRANCH_SUMMARY_SUFFIX: &str = "</summary>";

/// Stored shell execution message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BashExecutionMessage {
    /// Discriminator; Pi value is `bashExecution`.
    pub role: String,
    /// Command that was executed.
    pub command: String,
    /// Captured output.
    pub output: String,
    /// Exit code, absent when cancelled.
    pub exit_code: Option<i32>,
    /// Whether execution was cancelled.
    pub cancelled: bool,
    /// Whether output was truncated.
    pub truncated: bool,
    /// Path to complete output, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_output_path: Option<String>,
    /// Unix timestamp in milliseconds.
    pub timestamp: i64,
    /// Exclude this message from LLM context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_from_context: Option<bool>,
}

/// Application-defined user-visible message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomMessage<T = Value> {
    /// Discriminator; Pi value is `custom`.
    pub role: String,
    /// Application custom type.
    pub custom_type: String,
    /// Text or structured content.
    pub content: CustomMessageContent,
    /// Whether the UI should display the message.
    pub display: bool,
    /// Application details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<T>,
    /// Unix timestamp in milliseconds.
    pub timestamp: i64,
}

/// Custom message content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CustomMessageContent {
    /// Plain text custom content.
    Text(String),
    /// Structured text/image blocks.
    Blocks(Vec<UserContentBlock>),
}

/// Branch summary marker message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchSummaryMessage {
    /// Discriminator; Pi value is `branchSummary`.
    pub role: String,
    /// Summary content.
    pub summary: String,
    /// Source branch entry id.
    pub from_id: String,
    /// Unix timestamp in milliseconds.
    pub timestamp: i64,
}

/// Compaction summary marker message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionSummaryMessage {
    /// Discriminator; Pi value is `compactionSummary`.
    pub role: String,
    /// Summary content.
    pub summary: String,
    /// Token count before compaction.
    pub tokens_before: u64,
    /// Unix timestamp in milliseconds.
    pub timestamp: i64,
}

/// Render a shell execution message as LLM text.
#[must_use]
pub fn bash_execution_to_text(msg: &BashExecutionMessage) -> String {
    let mut text = format!("Ran `{}`\n", msg.command);
    if msg.output.is_empty() {
        text.push_str("(no output)");
    } else {
        text.push_str("```\n");
        text.push_str(&msg.output);
        text.push_str("\n```");
    }

    if msg.cancelled {
        text.push_str("\n\n(command cancelled)");
    } else if let Some(exit_code) = msg.exit_code {
        if exit_code != 0 {
            text.push_str(&format!("\n\nCommand exited with code {exit_code}"));
        }
    }

    if msg.truncated {
        if let Some(path) = &msg.full_output_path {
            text.push_str(&format!("\n\n[Output truncated. Full output: {path}]"));
        }
    }
    text
}

/// Create a branch summary custom agent message.
#[must_use]
pub fn create_branch_summary_message(
    summary: String,
    from_id: String,
    timestamp: &str,
) -> AgentMessage {
    custom_to_agent(BranchSummaryMessage {
        role: "branchSummary".to_string(),
        summary,
        from_id,
        timestamp: timestamp_millis(timestamp),
    })
}

/// Create a compaction summary custom agent message.
#[must_use]
pub fn create_compaction_summary_message(
    summary: String,
    tokens_before: u64,
    timestamp: &str,
) -> AgentMessage {
    custom_to_agent(CompactionSummaryMessage {
        role: "compactionSummary".to_string(),
        summary,
        tokens_before,
        timestamp: timestamp_millis(timestamp),
    })
}

/// Create an application custom agent message.
#[must_use]
pub fn create_custom_message(
    custom_type: String,
    content: CustomMessageContent,
    display: bool,
    details: Option<Value>,
    timestamp: &str,
) -> AgentMessage {
    custom_to_agent(CustomMessage {
        role: "custom".to_string(),
        custom_type,
        content,
        display,
        details,
        timestamp: timestamp_millis(timestamp),
    })
}

/// Convert agent messages into model messages.
#[must_use]
pub fn convert_to_llm(messages: &[AgentMessage]) -> Vec<Message> {
    messages.iter().filter_map(convert_one).collect()
}

fn convert_one(message: &AgentMessage) -> Option<Message> {
    match message {
        AgentMessage::Llm(message) => Some(message.clone()),
        AgentMessage::Custom(value) => match role_of(value).as_deref() {
            Some("bashExecution") => {
                let msg: BashExecutionMessage = serde_json::from_value(value.clone()).ok()?;
                if msg.exclude_from_context.unwrap_or(false) {
                    return None;
                }
                Some(user_message(
                    UserMessageContent::Blocks(vec![text_block(bash_execution_to_text(&msg))]),
                    msg.timestamp,
                ))
            }
            Some("custom") => {
                let msg: CustomMessage = serde_json::from_value(value.clone()).ok()?;
                let content = match msg.content {
                    CustomMessageContent::Text(text) => {
                        UserMessageContent::Blocks(vec![text_block(text)])
                    }
                    CustomMessageContent::Blocks(blocks) => UserMessageContent::Blocks(blocks),
                };
                Some(user_message(content, msg.timestamp))
            }
            Some("branchSummary") => {
                let msg: BranchSummaryMessage = serde_json::from_value(value.clone()).ok()?;
                Some(user_message(
                    UserMessageContent::Blocks(vec![text_block(format!(
                        "{BRANCH_SUMMARY_PREFIX}{}{BRANCH_SUMMARY_SUFFIX}",
                        msg.summary
                    ))]),
                    msg.timestamp,
                ))
            }
            Some("compactionSummary") => {
                let msg: CompactionSummaryMessage = serde_json::from_value(value.clone()).ok()?;
                Some(user_message(
                    UserMessageContent::Blocks(vec![text_block(format!(
                        "{COMPACTION_SUMMARY_PREFIX}{}{COMPACTION_SUMMARY_SUFFIX}",
                        msg.summary
                    ))]),
                    msg.timestamp,
                ))
            }
            _ => None,
        },
    }
}

fn role_of(value: &Value) -> Option<String> {
    value.get("role")?.as_str().map(ToOwned::to_owned)
}

fn custom_to_agent(message: impl Serialize) -> AgentMessage {
    AgentMessage::Custom(serde_json::to_value(message).unwrap_or(Value::Null))
}

fn user_message(content: UserMessageContent, timestamp: i64) -> Message {
    Message::User(UserMessage {
        role: UserMessageRole::User,
        content,
        timestamp: timestamp.max(0) as u64,
    })
}

fn text_block(text: String) -> UserContentBlock {
    UserContentBlock::Text(TextContent {
        content_type: TextContentType::Text,
        text,
        text_signature: None,
    })
}

fn timestamp_millis(input: &str) -> i64 {
    parse_rfc3339_millis(input).unwrap_or_else(|| input.parse::<i64>().unwrap_or(0))
}

fn parse_rfc3339_millis(input: &str) -> Option<i64> {
    let bytes = input.as_bytes();
    if bytes.len() < 20 {
        return None;
    }
    let year = parse_digits(input.get(0..4)?)?;
    let month = parse_digits(input.get(5..7)?)?;
    let day = parse_digits(input.get(8..10)?)?;
    let hour = parse_digits(input.get(11..13)?)?;
    let minute = parse_digits(input.get(14..16)?)?;
    let second = parse_digits(input.get(17..19)?)?;
    if input.get(4..5)? != "-"
        || input.get(7..8)? != "-"
        || !matches!(input.get(10..11)?, "T" | "t" | " ")
        || input.get(13..14)? != ":"
        || input.get(16..17)? != ":"
    {
        return None;
    }

    let mut index = 19;
    let mut millis = 0_i64;
    if input.get(index..index + 1) == Some(".") {
        index += 1;
        let start = index;
        while index < input.len() && input.as_bytes()[index].is_ascii_digit() {
            index += 1;
        }
        let fraction = &input[start..index.min(start + 3)];
        if !fraction.is_empty() {
            let mut value = parse_digits(fraction)?;
            for _ in fraction.len()..3 {
                value *= 10;
            }
            millis = value as i64;
        }
    }

    let offset_minutes = match input.get(index..index + 1)? {
        "Z" | "z" => 0,
        "+" | "-" => {
            let sign = if input.get(index..index + 1)? == "+" {
                1
            } else {
                -1
            };
            let h = parse_digits(input.get(index + 1..index + 3)?)? as i64;
            let m = parse_digits(input.get(index + 4..index + 6)?)? as i64;
            if input.get(index + 3..index + 4)? != ":" {
                return None;
            }
            sign * (h * 60 + m)
        }
        _ => return None,
    };

    let days = days_from_civil(year as i64, month as i64, day as i64)?;
    let seconds = days * 86_400 + hour as i64 * 3_600 + minute as i64 * 60 + second as i64
        - offset_minutes * 60;
    Some(seconds * 1_000 + millis)
}

fn parse_digits(value: &str) -> Option<u32> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

fn days_from_civil(year: i64, month: i64, day: i64) -> Option<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let month_prime = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * month_prime + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}
