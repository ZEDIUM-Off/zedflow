//! Shared simple stream option helpers ported from Pi.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

const CONTEXT_SAFETY_TOKENS: i64 = 4096;
const MIN_MAX_TOKENS: u32 = 1;
const CHARS_PER_TOKEN: usize = 4;
const ESTIMATED_IMAGE_CHARS: usize = 4800;
const MIN_OUTPUT_TOKENS: u32 = 1024;

/// Provider-scoped environment overrides.
pub type ProviderEnv = HashMap<String, String>;

/// Custom HTTP headers. `None` suppresses a provider/API default header.
pub type ProviderHeaders = HashMap<String, Option<String>>;

/// Optional metadata included in provider API requests.
pub type RequestMetadata = HashMap<String, Value>;

/// Callback that can inspect or replace provider payloads before sending.
pub type PayloadHook = Arc<dyn Fn(Value, &Model) -> Option<Value> + Send + Sync + 'static>;

/// Callback invoked after an HTTP response is received.
pub type ResponseHook = Arc<dyn Fn(&ProviderResponse, &Model) + Send + Sync + 'static>;

/// Prompt cache retention preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheRetention {
    /// Disable explicit prompt cache retention.
    None,
    /// Use the provider's short-lived cache retention.
    Short,
    /// Use long-lived cache retention when the provider supports it.
    Long,
}

/// Preferred transport for providers that support multiple transports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Transport {
    /// Server-sent events transport.
    Sse,
    /// WebSocket transport.
    Websocket,
    /// WebSocket transport with provider-side caching.
    WebsocketCached,
    /// Let the provider implementation select the transport.
    Auto,
}

/// Pi thinking level accepted by simple stream options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThinkingLevel {
    /// Minimal reasoning effort.
    Minimal,
    /// Low reasoning effort.
    Low,
    /// Medium reasoning effort.
    Medium,
    /// High reasoning effort.
    High,
    /// Extra-high reasoning effort.
    XHigh,
}

/// Thinking level after clamping to providers that do not accept `xhigh`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClampedThinkingLevel {
    /// Minimal reasoning effort.
    Minimal,
    /// Low reasoning effort.
    Low,
    /// Medium reasoning effort.
    Medium,
    /// High reasoning effort.
    High,
}

/// Token budgets for each thinking level.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThinkingBudgets {
    /// Minimal reasoning token budget override.
    pub minimal: Option<u32>,
    /// Low reasoning token budget override.
    pub low: Option<u32>,
    /// Medium reasoning token budget override.
    pub medium: Option<u32>,
    /// High reasoning token budget override.
    pub high: Option<u32>,
}

impl ThinkingBudgets {
    fn budget_for(&self, level: ClampedThinkingLevel) -> u32 {
        match level {
            ClampedThinkingLevel::Minimal => self.minimal.unwrap_or(1024),
            ClampedThinkingLevel::Low => self.low.unwrap_or(2048),
            ClampedThinkingLevel::Medium => self.medium.unwrap_or(8192),
            ClampedThinkingLevel::High => self.high.unwrap_or(16384),
        }
    }
}

/// HTTP response metadata passed to response hooks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderResponse {
    /// HTTP response status.
    pub status: u16,
    /// HTTP response headers.
    pub headers: HashMap<String, String>,
}

/// Minimal model shape consumed by Pi's simple option helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    /// Model context window in tokens.
    pub context_window: i64,
    /// Provider maximum output tokens.
    pub max_tokens: u32,
}

/// Conversation context used for context-token estimation.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Context {
    /// Optional system prompt prepended by providers.
    pub system_prompt: Option<String>,
    /// Conversation messages.
    pub messages: Vec<Message>,
    /// Available tool declarations.
    pub tools: Vec<Value>,
}

/// Conversation message used for token estimation.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// User-authored message.
    User {
        /// User content.
        content: UserMessageContent,
    },
    /// Assistant-authored message.
    Assistant(AssistantMessage),
    /// Tool-result message.
    ToolResult(ToolResultMessage),
}

/// Content accepted by Pi user and tool-result messages.
#[derive(Debug, Clone, PartialEq)]
pub enum UserMessageContent {
    /// Plain text content.
    Text(String),
    /// Structured text/image content parts.
    Parts(Vec<UserContentBlock>),
}

/// Text or image block in user-visible content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserContentBlock {
    /// Text content part.
    Text {
        /// Text payload.
        text: String,
    },
    /// Image content part.
    Image {
        /// Base64 image payload.
        data: String,
        /// Image MIME type.
        mime_type: String,
    },
}

/// Assistant message shape needed for replay usage estimates.
#[derive(Debug, Clone, PartialEq)]
pub struct AssistantMessage {
    /// Assistant content blocks.
    pub content: Vec<AssistantContentBlock>,
    /// Provider usage reported for this assistant message.
    pub usage: Usage,
    /// Why the assistant stopped.
    pub stop_reason: StopReason,
}

/// Assistant content block used for token estimation.
#[derive(Debug, Clone, PartialEq)]
pub enum AssistantContentBlock {
    /// Text content part.
    Text {
        /// Text payload.
        text: String,
    },
    /// Provider thinking content.
    Thinking {
        /// Thinking payload.
        thinking: String,
    },
    /// Tool call content.
    ToolCall(ToolCall),
}

/// Tool call content block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Tool/function name.
    pub name: String,
    /// JSON arguments supplied to the tool/function.
    pub arguments: Value,
}

/// Tool-result message shape needed for token estimation.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolResultMessage {
    /// Tool output content blocks.
    pub content: Vec<UserContentBlock>,
}

/// Token usage reported by a provider.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Usage {
    /// Input tokens.
    pub input: u32,
    /// Output tokens.
    pub output: u32,
    /// Prompt-cache read tokens.
    pub cache_read: u32,
    /// Prompt-cache write tokens.
    pub cache_write: u32,
    /// Total tokens when the provider reports it.
    pub total_tokens: u32,
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

/// Base options all providers share.
#[derive(Clone, Default)]
pub struct StreamOptions {
    /// Sampling temperature.
    pub temperature: Option<f64>,
    /// Maximum output tokens.
    pub max_tokens: Option<u32>,
    /// Cancellation signal handle.
    pub signal: Option<()>,
    /// API key override.
    pub api_key: Option<String>,
    /// Preferred transport.
    pub transport: Option<Transport>,
    /// Prompt cache retention preference.
    pub cache_retention: Option<CacheRetention>,
    /// Optional session identifier.
    pub session_id: Option<String>,
    /// Optional custom HTTP headers.
    pub headers: ProviderHeaders,
    /// Optional payload hook.
    pub on_payload: Option<PayloadHook>,
    /// Optional response hook.
    pub on_response: Option<ResponseHook>,
    /// Request timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// WebSocket connect timeout in milliseconds.
    pub websocket_connect_timeout_ms: Option<u64>,
    /// Maximum retry attempts.
    pub max_retries: Option<u32>,
    /// Maximum retry delay in milliseconds.
    pub max_retry_delay_ms: Option<u64>,
    /// Optional provider metadata.
    pub metadata: RequestMetadata,
    /// Provider-scoped environment values.
    pub env: ProviderEnv,
}

impl std::fmt::Debug for StreamOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamOptions")
            .field("temperature", &self.temperature)
            .field("max_tokens", &self.max_tokens)
            .field("signal", &self.signal)
            .field("api_key", &self.api_key)
            .field("transport", &self.transport)
            .field("cache_retention", &self.cache_retention)
            .field("session_id", &self.session_id)
            .field("headers", &self.headers)
            .field("on_payload", &self.on_payload.as_ref().map(|_| "<hook>"))
            .field("on_response", &self.on_response.as_ref().map(|_| "<hook>"))
            .field("timeout_ms", &self.timeout_ms)
            .field(
                "websocket_connect_timeout_ms",
                &self.websocket_connect_timeout_ms,
            )
            .field("max_retries", &self.max_retries)
            .field("max_retry_delay_ms", &self.max_retry_delay_ms)
            .field("metadata", &self.metadata)
            .field("env", &self.env)
            .finish()
    }
}

/// Unified options with reasoning passed to simple stream helpers.
#[derive(Debug, Clone, Default)]
pub struct SimpleStreamOptions {
    /// Base stream options.
    pub stream: StreamOptions,
    /// Reasoning effort.
    pub reasoning: Option<ThinkingLevel>,
    /// Custom token budgets for thinking levels.
    pub thinking_budgets: Option<ThinkingBudgets>,
}

/// Maximum-output-token and thinking-budget pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThinkingTokenOptions {
    /// Maximum output tokens to request.
    pub max_tokens: u32,
    /// Token budget reserved for thinking.
    pub thinking_budget: u32,
}

/// Clamps requested max tokens so the output fits in the model context window.
#[must_use]
pub fn clamp_max_tokens_to_context(model: &Model, context: &Context, max_tokens: u32) -> u32 {
    if model.context_window <= 0 {
        return max_tokens.max(MIN_MAX_TOKENS);
    }

    let available = model.context_window
        - i64::try_from(estimate_context_tokens(context).tokens).unwrap_or(i64::MAX)
        - CONTEXT_SAFETY_TOKENS;
    max_tokens.min(u32::try_from(available.max(i64::from(MIN_MAX_TOKENS))).unwrap_or(u32::MAX))
}

/// Builds provider stream options from simple stream options.
#[must_use]
pub fn build_base_options(
    model: &Model,
    context: &Context,
    options: Option<&SimpleStreamOptions>,
    api_key: Option<&str>,
) -> StreamOptions {
    let mut stream = options.map_or_else(StreamOptions::default, |options| options.stream.clone());
    let requested_max_tokens = stream.max_tokens.unwrap_or(model.max_tokens);
    stream.max_tokens = Some(clamp_max_tokens_to_context(
        model,
        context,
        requested_max_tokens,
    ));
    stream.api_key = api_key
        .filter(|key| !key.is_empty())
        .map(str::to_owned)
        .or(stream.api_key);
    stream
}

/// Clamps `xhigh` reasoning to `high` for providers that only accept lower levels.
#[must_use]
pub const fn clamp_reasoning(effort: Option<ThinkingLevel>) -> Option<ClampedThinkingLevel> {
    match effort {
        Some(ThinkingLevel::Minimal) => Some(ClampedThinkingLevel::Minimal),
        Some(ThinkingLevel::Low) => Some(ClampedThinkingLevel::Low),
        Some(ThinkingLevel::Medium) => Some(ClampedThinkingLevel::Medium),
        Some(ThinkingLevel::High | ThinkingLevel::XHigh) => Some(ClampedThinkingLevel::High),
        None => None,
    }
}

/// Adjusts output tokens so provider thinking budgets fit inside the model cap.
#[must_use]
pub fn adjust_max_tokens_for_thinking(
    base_max_tokens: Option<u32>,
    model_max_tokens: u32,
    reasoning_level: ThinkingLevel,
    custom_budgets: Option<&ThinkingBudgets>,
) -> ThinkingTokenOptions {
    let default_budgets = ThinkingBudgets::default();
    let budgets = custom_budgets.unwrap_or(&default_budgets);
    let level = match reasoning_level {
        ThinkingLevel::Minimal => ClampedThinkingLevel::Minimal,
        ThinkingLevel::Low => ClampedThinkingLevel::Low,
        ThinkingLevel::Medium => ClampedThinkingLevel::Medium,
        ThinkingLevel::High | ThinkingLevel::XHigh => ClampedThinkingLevel::High,
    };
    let mut thinking_budget = budgets.budget_for(level);
    let max_tokens = base_max_tokens.map_or(model_max_tokens, |base| {
        base.saturating_add(thinking_budget).min(model_max_tokens)
    });

    if max_tokens <= thinking_budget {
        thinking_budget = max_tokens.saturating_sub(MIN_OUTPUT_TOKENS);
    }

    ThinkingTokenOptions {
        max_tokens,
        thinking_budget,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ContextUsageEstimate {
    tokens: u32,
    usage_tokens: u32,
    trailing_tokens: u32,
    last_usage_index: Option<usize>,
}

fn calculate_context_tokens(usage: &Usage) -> u32 {
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

fn estimate_text_tokens(text: &str) -> u32 {
    tokens_for_chars(text.len())
}

fn estimate_text_and_image_content_tokens(content: &[UserContentBlock]) -> u32 {
    tokens_for_chars(
        content
            .iter()
            .map(|block| match block {
                UserContentBlock::Text { text } => text.len(),
                UserContentBlock::Image { .. } => ESTIMATED_IMAGE_CHARS,
            })
            .sum(),
    )
}

fn estimate_user_content_tokens(content: &UserMessageContent) -> u32 {
    match content {
        UserMessageContent::Text(text) => estimate_text_tokens(text),
        UserMessageContent::Parts(parts) => estimate_text_and_image_content_tokens(parts),
    }
}

fn estimate_message_tokens(message: &Message) -> u32 {
    match message {
        Message::User { content } => estimate_user_content_tokens(content),
        Message::ToolResult(message) => estimate_text_and_image_content_tokens(&message.content),
        Message::Assistant(message) => tokens_for_chars(
            message
                .content
                .iter()
                .map(|block| match block {
                    AssistantContentBlock::Text { text } => text.len(),
                    AssistantContentBlock::Thinking { thinking } => thinking.len(),
                    AssistantContentBlock::ToolCall(call) => {
                        call.name.len() + safe_json_stringify(&call.arguments).len()
                    }
                })
                .sum(),
        ),
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
            .fold(0_u32, u32::saturating_add);
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
        .fold(0_u32, u32::saturating_add);
    ContextUsageEstimate {
        tokens,
        usage_tokens: 0,
        trailing_tokens: tokens,
        last_usage_index: None,
    }
}

fn estimate_context_tokens(context: &Context) -> ContextUsageEstimate {
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

fn safe_json_stringify(value: &impl Serialize) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "[unserializable]".to_string())
}

fn tokens_for_chars(chars: usize) -> u32 {
    let tokens = chars.div_ceil(CHARS_PER_TOKEN);
    u32::try_from(tokens).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(context_window: i64, max_tokens: u32) -> Model {
        Model {
            context_window,
            max_tokens,
        }
    }

    #[test]
    fn clamps_max_tokens_to_context_with_safety_margin() {
        let context = Context {
            system_prompt: Some("a".repeat(400)),
            messages: vec![Message::User {
                content: UserMessageContent::Text("b".repeat(400)),
            }],
            tools: Vec::new(),
        };

        assert_eq!(
            clamp_max_tokens_to_context(&model(5000, 4096), &context, 4096),
            704
        );
        assert_eq!(clamp_max_tokens_to_context(&model(0, 0), &context, 0), 1);
    }

    #[test]
    fn build_base_options_prefers_non_empty_explicit_api_key() {
        let options = SimpleStreamOptions {
            stream: StreamOptions {
                temperature: Some(0.7),
                max_tokens: Some(2048),
                api_key: Some("from-options".to_string()),
                ..StreamOptions::default()
            },
            reasoning: None,
            thinking_budgets: None,
        };

        let base = build_base_options(
            &model(10_000, 4096),
            &Context::default(),
            Some(&options),
            Some(""),
        );
        assert_eq!(base.temperature, Some(0.7));
        assert_eq!(base.max_tokens, Some(2048));
        assert_eq!(base.api_key.as_deref(), Some("from-options"));

        let base = build_base_options(
            &model(10_000, 4096),
            &Context::default(),
            Some(&options),
            Some("explicit"),
        );
        assert_eq!(base.api_key.as_deref(), Some("explicit"));
    }

    #[test]
    fn adjusts_thinking_budget_inside_model_cap() {
        assert_eq!(
            clamp_reasoning(Some(ThinkingLevel::XHigh)),
            Some(ClampedThinkingLevel::High)
        );

        let adjusted = adjust_max_tokens_for_thinking(Some(1000), 2000, ThinkingLevel::High, None);
        assert_eq!(
            adjusted,
            ThinkingTokenOptions {
                max_tokens: 2000,
                thinking_budget: 976,
            }
        );
    }
}
