//! Message normalization helpers ported from Pi's `transform-messages.ts`.

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

const NON_VISION_USER_IMAGE_PLACEHOLDER: &str = "(image omitted: model does not support images)";
const NON_VISION_TOOL_IMAGE_PLACEHOLDER: &str =
    "(tool image omitted: model does not support images)";

/// Callback used to normalize tool-call IDs for the destination model.
pub type ToolCallIdNormalizer<'a> = dyn Fn(&str, &Model, &AssistantMessage) -> String + 'a;

/// Model metadata needed by [`transform_messages`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    /// Model identifier.
    pub id: String,
    /// API identifier.
    pub api: String,
    /// Provider identifier.
    pub provider: String,
    /// Supported input modalities such as `text` and `image`.
    pub input: Vec<String>,
}

/// Text content block.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextContent {
    /// Text body.
    pub text: String,
    /// Optional provider text signature metadata.
    pub text_signature: Option<String>,
}

/// Thinking content block.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingContent {
    /// Thinking text.
    pub thinking: String,
    /// Optional provider thinking signature metadata.
    pub thinking_signature: Option<String>,
    /// Whether the thinking text was redacted and must only replay to the same model.
    pub redacted: bool,
}

/// Image content block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageContent {
    /// Base64-encoded image data.
    pub data: String,
    /// MIME type such as `image/png`.
    pub mime_type: String,
}

/// Tool-call content block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    /// Provider tool-call identifier.
    pub id: String,
    /// Tool name.
    pub name: String,
    /// Tool arguments.
    pub arguments: Value,
    /// Optional provider thought signature metadata.
    pub thought_signature: Option<String>,
}

/// User or tool-result content block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum InputContent {
    /// Text content.
    Text(TextContent),
    /// Image content.
    Image(ImageContent),
}

/// Assistant content block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AssistantContent {
    /// Text content.
    Text(TextContent),
    /// Thinking content.
    Thinking(ThinkingContent),
    /// Tool-call content.
    ToolCall(ToolCall),
}

/// User message content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UserContent {
    /// Plain text user content.
    Text(String),
    /// Rich user content parts.
    Parts(Vec<InputContent>),
}

impl Default for UserContent {
    fn default() -> Self {
        Self::Parts(Vec::new())
    }
}

/// Usage cost breakdown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageCost {
    /// Input-token cost.
    pub input: f64,
    /// Output-token cost.
    pub output: f64,
    /// Cache-read token cost.
    pub cache_read: f64,
    /// Cache-write token cost.
    pub cache_write: f64,
    /// Total cost.
    pub total: f64,
}

/// Token usage and cost metadata preserved on assistant messages.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// Input tokens.
    pub input: u64,
    /// Output tokens.
    pub output: u64,
    /// Cache-read tokens.
    pub cache_read: u64,
    /// Cache-write tokens.
    pub cache_write: u64,
    /// Cache-write tokens retained for one hour.
    pub cache_write_1h: Option<u64>,
    /// Reasoning tokens when reported.
    pub reasoning: Option<u64>,
    /// Total tokens.
    pub total_tokens: u64,
    /// Cost breakdown.
    pub cost: UsageCost,
}

/// Assistant stop reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    /// Normal stop.
    Stop,
    /// Length limit.
    Length,
    /// Tool use requested.
    ToolUse,
    /// Provider/runtime error.
    Error,
    /// Request aborted.
    Aborted,
}

impl Default for StopReason {
    fn default() -> Self {
        Self::Stop
    }
}

/// User message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserMessage {
    /// Message content.
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub content: UserContent,
    /// Unix timestamp in milliseconds.
    pub timestamp: u64,
}

/// Assistant message.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantMessage {
    /// Assistant content blocks.
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub content: Vec<AssistantContent>,
    /// API identifier used to produce the message.
    pub api: String,
    /// Provider identifier used to produce the message.
    pub provider: String,
    /// Model identifier used to produce the message.
    pub model: String,
    /// Concrete upstream response model when it differs from `model`.
    pub response_model: Option<String>,
    /// Provider response identifier.
    pub response_id: Option<String>,
    /// Redacted diagnostics preserved from the source message.
    pub diagnostics: Option<Vec<Value>>,
    /// Token usage metadata.
    pub usage: Usage,
    /// Stop reason.
    pub stop_reason: StopReason,
    /// Error text for failed/aborted messages.
    pub error_message: Option<String>,
    /// Unix timestamp in milliseconds.
    pub timestamp: u64,
}

/// Tool-result message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResultMessage {
    /// Tool-call id being answered.
    pub tool_call_id: String,
    /// Tool name.
    pub tool_name: String,
    /// Tool result content.
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub content: Vec<InputContent>,
    /// Provider/tool-specific details.
    pub details: Option<Value>,
    /// Whether the result is an error.
    pub is_error: bool,
    /// Unix timestamp in milliseconds.
    pub timestamp: u64,
}

/// Conversation message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "camelCase")]
pub enum Message {
    /// User message.
    User(UserMessage),
    /// Assistant message.
    Assistant(AssistantMessage),
    /// Tool-result message.
    ToolResult(ToolResultMessage),
}

fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

fn replace_images_with_placeholder(
    content: &[InputContent],
    placeholder: &str,
) -> Vec<InputContent> {
    let mut result = Vec::with_capacity(content.len());
    let mut previous_was_placeholder = false;

    for block in content {
        match block {
            InputContent::Image(_) => {
                if !previous_was_placeholder {
                    result.push(InputContent::Text(TextContent {
                        text: placeholder.to_string(),
                        text_signature: None,
                    }));
                }
                previous_was_placeholder = true;
            }
            InputContent::Text(text) => {
                result.push(block.clone());
                previous_was_placeholder = text.text == placeholder;
            }
        }
    }

    result
}

fn downgrade_unsupported_images(messages: &[Message], model: &Model) -> Vec<Message> {
    if model.input.iter().any(|input| input == "image") {
        return messages.to_vec();
    }

    messages
        .iter()
        .map(|message| match message {
            Message::User(user) => {
                let mut user = user.clone();
                if let UserContent::Parts(parts) = &user.content {
                    user.content = UserContent::Parts(replace_images_with_placeholder(
                        parts,
                        NON_VISION_USER_IMAGE_PLACEHOLDER,
                    ));
                }
                Message::User(user)
            }
            Message::ToolResult(tool_result) => {
                let mut tool_result = tool_result.clone();
                tool_result.content = replace_images_with_placeholder(
                    &tool_result.content,
                    NON_VISION_TOOL_IMAGE_PLACEHOLDER,
                );
                Message::ToolResult(tool_result)
            }
            Message::Assistant(_) => message.clone(),
        })
        .collect()
}

fn unix_millis() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}

fn insert_synthetic_tool_results(
    result: &mut Vec<Message>,
    pending_tool_calls: &mut Vec<ToolCall>,
    existing_tool_result_ids: &mut HashSet<String>,
) {
    if pending_tool_calls.is_empty() {
        return;
    }

    for tool_call in pending_tool_calls.iter() {
        if !existing_tool_result_ids.contains(&tool_call.id) {
            result.push(Message::ToolResult(ToolResultMessage {
                tool_call_id: tool_call.id.clone(),
                tool_name: tool_call.name.clone(),
                content: vec![InputContent::Text(TextContent {
                    text: "No result provided".to_string(),
                    text_signature: None,
                })],
                details: None,
                is_error: true,
                timestamp: unix_millis(),
            }));
        }
    }

    pending_tool_calls.clear();
    existing_tool_result_ids.clear();
}

/// Normalize replayed messages for the destination model.
///
/// This ports Pi's cross-provider compatibility rules: unsupported image inputs
/// are downgraded, cross-model thinking metadata is removed or converted to
/// plain text, tool-call IDs can be normalized, failed assistant turns are
/// skipped, and missing tool results are synthesized.
#[must_use]
pub fn transform_messages(
    messages: &[Message],
    model: &Model,
    normalize_tool_call_id: Option<&ToolCallIdNormalizer<'_>>,
) -> Vec<Message> {
    let mut tool_call_id_map: HashMap<String, String> = HashMap::new();
    let image_aware_messages = downgrade_unsupported_images(messages, model);

    let transformed: Vec<Message> = image_aware_messages
        .iter()
        .map(|message| match message {
            Message::User(_) => message.clone(),
            Message::ToolResult(tool_result) => {
                if let Some(normalized_id) = tool_call_id_map.get(&tool_result.tool_call_id) {
                    if normalized_id != &tool_result.tool_call_id {
                        let mut tool_result = tool_result.clone();
                        tool_result.tool_call_id = normalized_id.clone();
                        return Message::ToolResult(tool_result);
                    }
                }
                message.clone()
            }
            Message::Assistant(assistant) => {
                let is_same_model = assistant.provider == model.provider
                    && assistant.api == model.api
                    && assistant.model == model.id;
                let mut content = Vec::with_capacity(assistant.content.len());

                for block in &assistant.content {
                    match block {
                        AssistantContent::Thinking(thinking) => {
                            if thinking.redacted {
                                if is_same_model {
                                    content.push(block.clone());
                                }
                                continue;
                            }

                            if is_same_model
                                && thinking
                                    .thinking_signature
                                    .as_deref()
                                    .is_some_and(|signature| !signature.is_empty())
                            {
                                content.push(block.clone());
                                continue;
                            }

                            if thinking.thinking.trim().is_empty() {
                                continue;
                            }

                            if is_same_model {
                                content.push(block.clone());
                            } else {
                                content.push(AssistantContent::Text(TextContent {
                                    text: thinking.thinking.clone(),
                                    text_signature: None,
                                }));
                            }
                        }
                        AssistantContent::Text(text) => {
                            if is_same_model {
                                content.push(block.clone());
                            } else {
                                content.push(AssistantContent::Text(TextContent {
                                    text: text.text.clone(),
                                    text_signature: None,
                                }));
                            }
                        }
                        AssistantContent::ToolCall(tool_call) => {
                            let mut normalized_tool_call = tool_call.clone();

                            if !is_same_model {
                                normalized_tool_call.thought_signature = None;
                            }

                            if !is_same_model {
                                if let Some(normalize) = normalize_tool_call_id {
                                    let normalized_id = normalize(&tool_call.id, model, assistant);
                                    if normalized_id != tool_call.id {
                                        tool_call_id_map
                                            .insert(tool_call.id.clone(), normalized_id.clone());
                                        normalized_tool_call.id = normalized_id;
                                    }
                                }
                            }

                            content.push(AssistantContent::ToolCall(normalized_tool_call));
                        }
                    }
                }

                let mut assistant = assistant.clone();
                assistant.content = content;
                Message::Assistant(assistant)
            }
        })
        .collect();

    let mut result = Vec::with_capacity(transformed.len());
    let mut pending_tool_calls = Vec::new();
    let mut existing_tool_result_ids = HashSet::new();

    for message in transformed {
        match &message {
            Message::Assistant(assistant) => {
                insert_synthetic_tool_results(
                    &mut result,
                    &mut pending_tool_calls,
                    &mut existing_tool_result_ids,
                );

                if matches!(
                    assistant.stop_reason,
                    StopReason::Error | StopReason::Aborted
                ) {
                    continue;
                }

                let tool_calls: Vec<ToolCall> = assistant
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        AssistantContent::ToolCall(tool_call) => Some(tool_call.clone()),
                        _ => None,
                    })
                    .collect();

                if !tool_calls.is_empty() {
                    pending_tool_calls = tool_calls;
                    existing_tool_result_ids.clear();
                }

                result.push(message);
            }
            Message::ToolResult(tool_result) => {
                existing_tool_result_ids.insert(tool_result.tool_call_id.clone());
                result.push(message);
            }
            Message::User(_) => {
                insert_synthetic_tool_results(
                    &mut result,
                    &mut pending_tool_calls,
                    &mut existing_tool_result_ids,
                );
                result.push(message);
            }
        }
    }

    insert_synthetic_tool_results(
        &mut result,
        &mut pending_tool_calls,
        &mut existing_tool_result_ids,
    );

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn model() -> Model {
        Model {
            id: "claude-sonnet-4.6".to_string(),
            api: "anthropic-messages".to_string(),
            provider: "github-copilot".to_string(),
            input: vec!["text".to_string(), "image".to_string()],
        }
    }

    fn assistant(content: Vec<AssistantContent>) -> Message {
        Message::Assistant(AssistantMessage {
            content,
            api: "openai-responses".to_string(),
            provider: "github-copilot".to_string(),
            model: "gpt-5".to_string(),
            stop_reason: StopReason::ToolUse,
            ..AssistantMessage::default()
        })
    }

    fn normalize_tool_call_id(id: &str, _model: &Model, _source: &AssistantMessage) -> String {
        id.chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                    character
                } else {
                    '_'
                }
            })
            .take(64)
            .collect()
    }

    fn user(content: &str) -> Message {
        Message::User(UserMessage {
            content: UserContent::Text(content.to_string()),
            timestamp: 0,
        })
    }

    fn assistant_message(
        content: Vec<AssistantContent>,
        api: &str,
        model_id: &str,
        stop_reason: StopReason,
    ) -> Message {
        Message::Assistant(AssistantMessage {
            content,
            api: api.to_string(),
            provider: "github-copilot".to_string(),
            model: model_id.to_string(),
            stop_reason,
            ..AssistantMessage::default()
        })
    }

    #[test]
    fn converts_thinking_blocks_to_plain_text_when_source_model_differs() {
        let messages = vec![
            user("hello"),
            assistant_message(
                vec![
                    AssistantContent::Thinking(ThinkingContent {
                        thinking: "Let me think about this...".to_string(),
                        thinking_signature: Some("reasoning_content".to_string()),
                        redacted: false,
                    }),
                    AssistantContent::Text(TextContent {
                        text: "Hi there!".to_string(),
                        text_signature: None,
                    }),
                ],
                "openai-completions",
                "gpt-4o",
                StopReason::Stop,
            ),
        ];

        let result = transform_messages(&messages, &model(), Some(&normalize_tool_call_id));
        let assistant = result
            .iter()
            .find_map(|message| match message {
                Message::Assistant(assistant) => Some(assistant),
                _ => None,
            })
            .expect("expected assistant message");

        let thinking_blocks = assistant
            .content
            .iter()
            .filter(|block| matches!(block, AssistantContent::Thinking(_)))
            .count();
        let text_blocks = assistant
            .content
            .iter()
            .filter(|block| matches!(block, AssistantContent::Text(_)))
            .count();
        assert_eq!(thinking_blocks, 0);
        assert!(text_blocks >= 2);
    }

    #[test]
    fn removes_thought_signature_from_tool_calls_when_migrating_between_models() {
        let messages = vec![
            user("run a command"),
            assistant_message(
                vec![AssistantContent::ToolCall(ToolCall {
                    id: "call_123".to_string(),
                    name: "bash".to_string(),
                    arguments: json!({ "command": "ls" }),
                    thought_signature: Some(
                        json!({
                            "type": "reasoning.encrypted",
                            "id": "call_123",
                            "data": "encrypted",
                        })
                        .to_string(),
                    ),
                })],
                "openai-responses",
                "gpt-5",
                StopReason::ToolUse,
            ),
            Message::ToolResult(ToolResultMessage {
                tool_call_id: "call_123".to_string(),
                tool_name: "bash".to_string(),
                content: vec![InputContent::Text(TextContent {
                    text: "output".to_string(),
                    text_signature: None,
                })],
                details: None,
                is_error: false,
                timestamp: 0,
            }),
        ];

        let result = transform_messages(&messages, &model(), Some(&normalize_tool_call_id));
        let assistant = result
            .iter()
            .find_map(|message| match message {
                Message::Assistant(assistant) => Some(assistant),
                _ => None,
            })
            .expect("expected assistant message");
        let tool_call = assistant
            .content
            .iter()
            .find_map(|block| match block {
                AssistantContent::ToolCall(tool_call) => Some(tool_call),
                _ => None,
            })
            .expect("expected tool call");

        assert!(tool_call.thought_signature.is_none());
    }

    #[test]
    fn adds_synthetic_tool_results_for_trailing_orphaned_tool_calls() {
        let messages = vec![
            user("read the file"),
            assistant_message(
                vec![AssistantContent::ToolCall(ToolCall {
                    id: "call_123|fc_123".to_string(),
                    name: "read".to_string(),
                    arguments: json!({ "path": "README.md" }),
                    thought_signature: None,
                })],
                "openai-responses",
                "gpt-5",
                StopReason::ToolUse,
            ),
        ];

        let result = transform_messages(&messages, &model(), Some(&normalize_tool_call_id));
        let Message::ToolResult(last_message) = result.last().expect("expected last message")
        else {
            panic!("expected tool result");
        };

        assert_eq!(last_message.tool_call_id, "call_123_fc_123");
        assert_eq!(last_message.tool_name, "read");
        assert!(last_message.is_error);
        assert_eq!(
            last_message.content,
            vec![InputContent::Text(TextContent {
                text: "No result provided".to_string(),
                text_signature: None,
            })]
        );
    }

    #[test]
    fn adds_synthetic_results_only_for_trailing_tool_calls_still_missing_results() {
        let messages = vec![
            user("run commands"),
            assistant_message(
                vec![
                    AssistantContent::ToolCall(ToolCall {
                        id: "call_1|fc_1".to_string(),
                        name: "read".to_string(),
                        arguments: json!({ "path": "README.md" }),
                        thought_signature: None,
                    }),
                    AssistantContent::ToolCall(ToolCall {
                        id: "call_2|fc_2".to_string(),
                        name: "bash".to_string(),
                        arguments: json!({ "command": "pwd" }),
                        thought_signature: None,
                    }),
                ],
                "openai-responses",
                "gpt-5",
                StopReason::ToolUse,
            ),
            Message::ToolResult(ToolResultMessage {
                tool_call_id: "call_1|fc_1".to_string(),
                tool_name: "read".to_string(),
                content: vec![InputContent::Text(TextContent {
                    text: "done".to_string(),
                    text_signature: None,
                })],
                details: None,
                is_error: false,
                timestamp: 0,
            }),
        ];

        let result = transform_messages(&messages, &model(), Some(&normalize_tool_call_id));
        let synthetic_results: Vec<_> = result
            .iter()
            .filter_map(|message| match message {
                Message::ToolResult(tool_result) if tool_result.is_error => Some(tool_result),
                _ => None,
            })
            .collect();

        assert_eq!(synthetic_results.len(), 1);
        assert_eq!(synthetic_results[0].tool_call_id, "call_2_fc_2");
        assert_eq!(synthetic_results[0].tool_name, "bash");
        assert_eq!(
            synthetic_results[0].content,
            vec![InputContent::Text(TextContent {
                text: "No result provided".to_string(),
                text_signature: None,
            })]
        );
    }

    #[test]
    fn converts_cross_model_thinking_to_text() {
        let result = transform_messages(
            &[assistant(vec![AssistantContent::Thinking(
                ThinkingContent {
                    thinking: "Let me think".to_string(),
                    thinking_signature: Some("reasoning_content".to_string()),
                    redacted: false,
                },
            )])],
            &model(),
            Some(&normalize_tool_call_id),
        );

        let Message::Assistant(assistant) = &result[0] else {
            panic!("expected assistant message");
        };
        assert_eq!(
            assistant.content,
            vec![AssistantContent::Text(TextContent {
                text: "Let me think".to_string(),
                text_signature: None,
            })]
        );
    }

    #[test]
    fn normalizes_tool_call_ids_and_synthesizes_missing_results() {
        let result = transform_messages(
            &[
                assistant(vec![
                    AssistantContent::ToolCall(ToolCall {
                        id: "call_1|fc_1".to_string(),
                        name: "read".to_string(),
                        arguments: json!({ "path": "README.md" }),
                        thought_signature: Some("opaque".to_string()),
                    }),
                    AssistantContent::ToolCall(ToolCall {
                        id: "call_2|fc_2".to_string(),
                        name: "bash".to_string(),
                        arguments: json!({ "command": "pwd" }),
                        thought_signature: None,
                    }),
                ]),
                Message::ToolResult(ToolResultMessage {
                    tool_call_id: "call_1|fc_1".to_string(),
                    tool_name: "read".to_string(),
                    content: vec![InputContent::Text(TextContent {
                        text: "done".to_string(),
                        text_signature: None,
                    })],
                    details: None,
                    is_error: false,
                    timestamp: 0,
                }),
            ],
            &model(),
            Some(&normalize_tool_call_id),
        );

        let Message::Assistant(assistant) = &result[0] else {
            panic!("expected assistant message");
        };
        let AssistantContent::ToolCall(tool_call) = &assistant.content[0] else {
            panic!("expected tool call");
        };
        assert_eq!(tool_call.id, "call_1_fc_1");
        assert_eq!(tool_call.thought_signature, None);

        let Message::ToolResult(existing) = &result[1] else {
            panic!("expected existing tool result");
        };
        assert_eq!(existing.tool_call_id, "call_1_fc_1");

        let Message::ToolResult(synthetic) = &result[2] else {
            panic!("expected synthetic tool result");
        };
        assert_eq!(synthetic.tool_call_id, "call_2_fc_2");
        assert!(synthetic.is_error);
        assert_eq!(
            synthetic.content,
            vec![InputContent::Text(TextContent {
                text: "No result provided".to_string(),
                text_signature: None,
            })]
        );
    }

    #[test]
    fn downgrades_consecutive_images_for_non_vision_models() {
        let mut model = model();
        model.input = vec!["text".to_string()];

        let result = transform_messages(
            &[Message::User(UserMessage {
                content: UserContent::Parts(vec![
                    InputContent::Image(ImageContent {
                        data: "a".to_string(),
                        mime_type: "image/png".to_string(),
                    }),
                    InputContent::Image(ImageContent {
                        data: "b".to_string(),
                        mime_type: "image/png".to_string(),
                    }),
                    InputContent::Text(TextContent {
                        text: "after".to_string(),
                        text_signature: None,
                    }),
                ]),
                timestamp: 0,
            })],
            &model,
            None,
        );

        let Message::User(user) = &result[0] else {
            panic!("expected user message");
        };
        assert_eq!(
            user.content,
            UserContent::Parts(vec![
                InputContent::Text(TextContent {
                    text: NON_VISION_USER_IMAGE_PLACEHOLDER.to_string(),
                    text_signature: None,
                }),
                InputContent::Text(TextContent {
                    text: "after".to_string(),
                    text_signature: None,
                }),
            ])
        );
    }

    #[test]
    fn normalizes_null_and_missing_content_to_empty_arrays() {
        let messages: Vec<Message> = serde_json::from_value(json!([
            { "role": "user", "content": null, "timestamp": 0_u64 },
            {
                "role": "assistant",
                "content": null,
                "api": "openai-completions",
                "provider": "openai",
                "model": "test-model",
                "usage": {
                    "input": 0_u64,
                    "output": 0_u64,
                    "cacheRead": 0_u64,
                    "cacheWrite": 0_u64,
                    "totalTokens": 0_u64,
                    "cost": {
                        "input": 0.0,
                        "output": 0.0,
                        "cacheRead": 0.0,
                        "cacheWrite": 0.0,
                        "total": 0.0,
                    },
                },
                "stopReason": "stop",
                "timestamp": 0_u64,
            },
            {
                "role": "toolResult",
                "toolCallId": "call_1",
                "toolName": "web_search",
                "isError": false,
                "timestamp": 0_u64,
            },
        ]))
        .expect("valid lax message fixture");
        let model = Model {
            id: "test-model".to_string(),
            api: "openai-completions".to_string(),
            provider: "openai".to_string(),
            input: vec!["text".to_string()],
        };

        let result = transform_messages(&messages, &model, None);

        assert_eq!(result.len(), 3);
        for message in result {
            match message {
                Message::User(user) => assert_eq!(user.content, UserContent::Parts(Vec::new())),
                Message::Assistant(assistant) => assert_eq!(assistant.content, Vec::new()),
                Message::ToolResult(tool_result) => assert_eq!(tool_result.content, Vec::new()),
            }
        }
    }
}
