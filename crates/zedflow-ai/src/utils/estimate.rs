//! Context token-estimation helpers ported from Pi's `packages/ai/src/utils/estimate.ts`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

const CHARS_PER_TOKEN: usize = 4;
const ESTIMATED_IMAGE_CHARS: usize = 4_800;

/// Estimated context-token usage for a message list or context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextUsageEstimate {
    /// Estimated total context tokens.
    pub tokens: u64,
    /// Tokens reported by the most recent assistant usage block.
    pub usage_tokens: u64,
    /// Estimated tokens after the most recent assistant usage block.
    pub trailing_tokens: u64,
    /// Index of the message that provided usage, or [`None`] when none exists.
    pub last_usage_index: Option<usize>,
}

/// Token usage reported by a provider.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Usage {
    /// Input tokens.
    pub input: u64,
    /// Output tokens.
    pub output: u64,
    /// Prompt-cache read tokens.
    pub cache_read: u64,
    /// Prompt-cache write tokens.
    pub cache_write: u64,
    /// Total context tokens when the provider reports them.
    pub total_tokens: u64,
}

/// Text content block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextContent {
    /// Text payload.
    pub text: String,
}

/// Image content block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageContent {
    /// Base64 image payload.
    pub data: String,
    /// Image MIME type.
    pub mime_type: String,
}

/// Provider thinking content block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThinkingContent {
    /// Thinking payload.
    pub thinking: String,
}

/// Tool-call content block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Tool/function name.
    pub name: String,
    /// JSON arguments supplied to the tool/function.
    pub arguments: Value,
}

/// Text or image content accepted by user and tool-result messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextAndImageContentBlock {
    /// Text content.
    Text(TextContent),
    /// Image content.
    Image(ImageContent),
}

/// User or tool-result message content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextAndImageContent {
    /// Plain text content.
    Text(String),
    /// Structured text/image content blocks.
    Blocks(Vec<TextAndImageContentBlock>),
}

/// Assistant message content.
#[derive(Debug, Clone, PartialEq)]
pub enum AssistantContentBlock {
    /// Text content.
    Text(TextContent),
    /// Thinking content.
    Thinking(ThinkingContent),
    /// Tool-call content.
    ToolCall(ToolCall),
}

/// Assistant stop reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StopReason {
    /// Natural stop.
    Stop,
    /// Length limit stop.
    Length,
    /// Tool-use stop.
    ToolUse,
    /// Error termination.
    Error,
    /// Aborted termination.
    Aborted,
}

/// User-authored message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserMessage {
    /// User content.
    pub content: TextAndImageContent,
}

/// Assistant-authored message.
#[derive(Debug, Clone, PartialEq)]
pub struct AssistantMessage {
    /// Assistant content blocks.
    pub content: Vec<AssistantContentBlock>,
    /// Provider usage reported for this assistant message.
    pub usage: Usage,
    /// Why the assistant stopped.
    pub stop_reason: StopReason,
}

/// Tool-result message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResultMessage {
    /// Tool output content.
    pub content: TextAndImageContent,
}

/// Conversation message.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// User-authored message.
    User(UserMessage),
    /// Assistant-authored message.
    Assistant(AssistantMessage),
    /// Tool-result message.
    ToolResult(ToolResultMessage),
}

/// Tool declaration included in a context prefix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tool {
    /// Tool name.
    pub name: String,
    /// Tool description.
    pub description: String,
    /// Tool parameters schema.
    pub parameters: Value,
}

/// Conversation context.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Context {
    /// Optional system prompt prepended by providers.
    pub system_prompt: Option<String>,
    /// Conversation messages.
    pub messages: Vec<Message>,
    /// Available tool declarations.
    pub tools: Vec<Tool>,
}

/// Input accepted by [`estimate_context_tokens`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContextInput<'a> {
    /// Estimate a full context.
    Context(&'a Context),
    /// Estimate a message slice without system prompt or tools.
    Messages(&'a [Message]),
}

impl<'a> From<&'a Context> for ContextInput<'a> {
    fn from(context: &'a Context) -> Self {
        Self::Context(context)
    }
}

impl<'a> From<&'a [Message]> for ContextInput<'a> {
    fn from(messages: &'a [Message]) -> Self {
        Self::Messages(messages)
    }
}

impl<'a> From<&'a Vec<Message>> for ContextInput<'a> {
    fn from(messages: &'a Vec<Message>) -> Self {
        Self::Messages(messages)
    }
}

/// Calculates context tokens from provider usage, falling back to component sums when total is zero.
#[must_use]
pub const fn calculate_context_tokens(usage: &Usage) -> u64 {
    if usage.total_tokens != 0 {
        usage.total_tokens
    } else {
        usage
            .input
            .saturating_add(usage.output)
            .saturating_add(usage.cache_read)
            .saturating_add(usage.cache_write)
    }
}

/// Estimates tokens for plain text using Pi's four-chars-per-token heuristic.
#[must_use]
pub fn estimate_text_tokens(text: &str) -> u64 {
    tokens_for_chars(text.len())
}

/// Estimates tokens for text/image content using Pi's fixed image-character heuristic.
#[must_use]
pub fn estimate_text_and_image_content_tokens(content: &TextAndImageContent) -> u64 {
    tokens_for_chars(estimate_text_and_image_content_chars(content))
}

/// Estimates tokens for a single message.
#[must_use]
pub fn estimate_message_tokens(message: &Message) -> u64 {
    match message {
        Message::User(message) => estimate_text_and_image_content_tokens(&message.content),
        Message::ToolResult(message) => estimate_text_and_image_content_tokens(&message.content),
        Message::Assistant(message) => tokens_for_chars(
            message
                .content
                .iter()
                .map(|block| match block {
                    AssistantContentBlock::Text(content) => content.text.len(),
                    AssistantContentBlock::Thinking(content) => content.thinking.len(),
                    AssistantContentBlock::ToolCall(call) => {
                        call.name.len() + safe_json_stringify(&call.arguments).len()
                    }
                })
                .sum(),
        ),
    }
}

/// Estimates total context tokens for either a full context or a message slice.
#[must_use]
pub fn estimate_context_tokens<'a>(context: impl Into<ContextInput<'a>>) -> ContextUsageEstimate {
    match context.into() {
        ContextInput::Messages(messages) => estimate_messages(messages),
        ContextInput::Context(context) => {
            let estimate = estimate_messages(&context.messages);
            if estimate.last_usage_index.is_some() {
                return estimate;
            }

            let mut prefix_tokens = context
                .system_prompt
                .as_deref()
                .map_or(0, estimate_text_tokens);
            if !context.tools.is_empty() {
                prefix_tokens = prefix_tokens
                    .saturating_add(estimate_text_tokens(&safe_json_stringify(&context.tools)));
            }

            ContextUsageEstimate {
                tokens: estimate.tokens.saturating_add(prefix_tokens),
                usage_tokens: estimate.usage_tokens,
                trailing_tokens: estimate.trailing_tokens.saturating_add(prefix_tokens),
                last_usage_index: estimate.last_usage_index,
            }
        }
    }
}

fn estimate_text_and_image_content_chars(content: &TextAndImageContent) -> usize {
    match content {
        TextAndImageContent::Text(text) => text.len(),
        TextAndImageContent::Blocks(blocks) => blocks
            .iter()
            .map(|block| match block {
                TextAndImageContentBlock::Text(content) => content.text.len(),
                TextAndImageContentBlock::Image(_) => ESTIMATED_IMAGE_CHARS,
            })
            .sum(),
    }
}

fn get_last_assistant_usage_info(messages: &[Message]) -> Option<(&Usage, usize)> {
    messages
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, message)| match message {
            Message::Assistant(assistant)
                if !matches!(
                    assistant.stop_reason,
                    StopReason::Aborted | StopReason::Error
                ) && calculate_context_tokens(&assistant.usage) > 0 =>
            {
                Some((&assistant.usage, index))
            }
            _ => None,
        })
}

fn estimate_messages(messages: &[Message]) -> ContextUsageEstimate {
    if let Some((usage, index)) = get_last_assistant_usage_info(messages) {
        let usage_tokens = calculate_context_tokens(usage);
        let trailing_tokens = messages[index + 1..]
            .iter()
            .map(estimate_message_tokens)
            .fold(0_u64, u64::saturating_add);
        return ContextUsageEstimate {
            tokens: usage_tokens.saturating_add(trailing_tokens),
            usage_tokens,
            trailing_tokens,
            last_usage_index: Some(index),
        };
    }

    let tokens = messages
        .iter()
        .map(estimate_message_tokens)
        .fold(0_u64, u64::saturating_add);
    ContextUsageEstimate {
        tokens,
        usage_tokens: 0,
        trailing_tokens: tokens,
        last_usage_index: None,
    }
}

fn safe_json_stringify(value: &impl Serialize) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "[unserializable]".to_string())
}

fn tokens_for_chars(chars: usize) -> u64 {
    u64::try_from(chars.div_ceil(CHARS_PER_TOKEN)).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn calculates_usage_total_or_component_sum() {
        assert_eq!(
            calculate_context_tokens(&Usage {
                total_tokens: 42,
                input: 1,
                output: 2,
                cache_read: 3,
                cache_write: 4,
            }),
            42
        );
        assert_eq!(
            calculate_context_tokens(&Usage {
                input: 1,
                output: 2,
                cache_read: 3,
                cache_write: 4,
                total_tokens: 0,
            }),
            10
        );
    }

    #[test]
    fn estimates_context_from_last_successful_assistant_usage() {
        let messages = vec![
            Message::Assistant(AssistantMessage {
                content: vec![],
                usage: Usage {
                    total_tokens: 10,
                    ..Usage::default()
                },
                stop_reason: StopReason::Stop,
            }),
            Message::User(UserMessage {
                content: TextAndImageContent::Text("hello".to_string()),
            }),
        ];

        assert_eq!(
            estimate_context_tokens(&messages),
            ContextUsageEstimate {
                tokens: 12,
                usage_tokens: 10,
                trailing_tokens: 2,
                last_usage_index: Some(0),
            }
        );
    }

    #[test]
    fn estimates_message_content_like_pi() {
        assert_eq!(estimate_text_tokens("hello"), 2);
        assert_eq!(
            estimate_text_and_image_content_tokens(&TextAndImageContent::Blocks(vec![
                TextAndImageContentBlock::Text(TextContent {
                    text: "hello".to_string(),
                }),
                TextAndImageContentBlock::Image(ImageContent {
                    data: String::new(),
                    mime_type: "image/png".to_string(),
                }),
            ])),
            1_202
        );

        let message = Message::Assistant(AssistantMessage {
            content: vec![AssistantContentBlock::ToolCall(ToolCall {
                name: "tool".to_string(),
                arguments: json!({"x":1}),
            })],
            usage: Usage::default(),
            stop_reason: StopReason::Stop,
        });
        assert_eq!(estimate_message_tokens(&message), 3);
    }
}
