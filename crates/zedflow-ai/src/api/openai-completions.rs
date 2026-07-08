//! OpenAI-compatible Chat Completions API ported from Pi.

use std::collections::{HashMap, HashSet};
use std::error::Error as StdError;
use std::fmt;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::github_copilot_headers::{
    CopilotDynamicHeadersParams, Message as CopilotMessage, MessageContent as CopilotContent,
    UserMessageContent as CopilotUserContent, build_copilot_dynamic_headers,
    has_copilot_vision_input,
};

const OPENAI_PROMPT_CACHE_KEY_MAX_LENGTH: usize = 64;

static EMPTY_ENV: LazyLock<ProviderEnv> = LazyLock::new(ProviderEnv::new);

/// Result type for the OpenAI Completions port.
pub type Result<T> = std::result::Result<T, OpenAICompletionsError>;

/// Errors returned by the OpenAI Completions port.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OpenAICompletionsError {
    /// No API key or authorization header was supplied for the model provider.
    MissingApiKey {
        /// Provider identifier from Pi.
        provider: String,
    },
    /// Provider transport failed before a stream could be produced.
    Transport(String),
}

impl fmt::Display for OpenAICompletionsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApiKey { provider } => write!(f, "no API key for provider: {provider}"),
            Self::Transport(error) => f.write_str(error),
        }
    }
}

impl StdError for OpenAICompletionsError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Transport(_) => None,
            Self::MissingApiKey { .. } => None,
        }
    }
}

/// HTTP headers supplied by a model or request options.
pub type ProviderHeaders = HashMap<String, String>;

/// Provider-scoped environment overrides.
pub type ProviderEnv = HashMap<String, String>;

/// Prompt cache retention preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheRetention {
    /// Disable prompt caching.
    None,
    /// Use provider short retention.
    Short,
    /// Use provider long retention.
    Long,
}

impl Default for CacheRetention {
    fn default() -> Self {
        Self::Short
    }
}

/// OpenAI-compatible reasoning effort accepted by Pi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
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

/// Pi thinking-level map key, including the `off` sentinel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelThinkingLevel {
    /// Disable reasoning.
    Off,
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

impl From<ReasoningEffort> for ModelThinkingLevel {
    fn from(value: ReasoningEffort) -> Self {
        match value {
            ReasoningEffort::Minimal => Self::Minimal,
            ReasoningEffort::Low => Self::Low,
            ReasoningEffort::Medium => Self::Medium,
            ReasoningEffort::High => Self::High,
            ReasoningEffort::XHigh => Self::XHigh,
        }
    }
}

/// Model input modality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelInput {
    /// Text input.
    Text,
    /// Image input.
    Image,
}

/// Compatibility settings for OpenAI-compatible completions APIs.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OpenAICompletionsCompat {
    /// Whether the provider supports the `store` field.
    pub supports_store: Option<bool>,
    /// Whether the provider supports the `developer` role instead of `system`.
    pub supports_developer_role: Option<bool>,
    /// Whether the provider supports `reasoning_effort`.
    pub supports_reasoning_effort: Option<bool>,
    /// Whether streaming usage can be requested.
    pub supports_usage_in_streaming: Option<bool>,
    /// Which field to use for max tokens.
    pub max_tokens_field: Option<MaxTokensField>,
    /// Whether tool results require the `name` field.
    pub requires_tool_result_name: Option<bool>,
    /// Whether a user message after tool results requires an assistant bridge message.
    pub requires_assistant_after_tool_result: Option<bool>,
    /// Whether thinking blocks must be converted to text.
    pub requires_thinking_as_text: Option<bool>,
    /// Whether replayed assistant messages need `reasoning_content` when reasoning is enabled.
    pub requires_reasoning_content_on_assistant_messages: Option<bool>,
    /// Provider-specific thinking parameter shape.
    pub thinking_format: Option<ThinkingFormat>,
    /// Whether strict mode is accepted in tool definitions.
    pub supports_strict_mode: Option<bool>,
    /// Cache-control convention for prompt caching.
    pub cache_control_format: Option<CacheControlFormat>,
    /// Whether to send session affinity headers from the session id.
    pub send_session_affinity_headers: Option<bool>,
    /// Whether long prompt cache retention is supported.
    pub supports_long_cache_retention: Option<bool>,
}

/// Resolved OpenAI-compatible completions API settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedOpenAICompletionsCompat {
    /// Whether the provider supports the `store` field.
    pub supports_store: bool,
    /// Whether the provider supports the `developer` role instead of `system`.
    pub supports_developer_role: bool,
    /// Whether the provider supports `reasoning_effort`.
    pub supports_reasoning_effort: bool,
    /// Whether streaming usage can be requested.
    pub supports_usage_in_streaming: bool,
    /// Which field to use for max tokens.
    pub max_tokens_field: MaxTokensField,
    /// Whether tool results require the `name` field.
    pub requires_tool_result_name: bool,
    /// Whether a user message after tool results requires an assistant bridge message.
    pub requires_assistant_after_tool_result: bool,
    /// Whether thinking blocks must be converted to text.
    pub requires_thinking_as_text: bool,
    /// Whether replayed assistant messages need `reasoning_content` when reasoning is enabled.
    pub requires_reasoning_content_on_assistant_messages: bool,
    /// Provider-specific thinking parameter shape.
    pub thinking_format: ThinkingFormat,
    /// Whether strict mode is accepted in tool definitions.
    pub supports_strict_mode: bool,
    /// Cache-control convention for prompt caching.
    pub cache_control_format: Option<CacheControlFormat>,
    /// Whether to send session affinity headers from the session id.
    pub send_session_affinity_headers: bool,
    /// Whether long prompt cache retention is supported.
    pub supports_long_cache_retention: bool,
}

/// Max-token field accepted by a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaxTokensField {
    /// Use `max_completion_tokens`.
    MaxCompletionTokens,
    /// Use `max_tokens`.
    MaxTokens,
}

/// Provider-specific thinking parameter shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThinkingFormat {
    /// OpenAI-style `reasoning_effort`.
    OpenAI,
    /// OpenRouter nested reasoning object.
    OpenRouter,
    /// DeepSeek thinking object.
    DeepSeek,
    /// Together reasoning object.
    Together,
    /// z.ai thinking object.
    Zai,
    /// Qwen `enable_thinking` flag.
    Qwen,
    /// Configurable chat-template kwargs.
    ChatTemplate,
    /// Qwen chat-template kwargs.
    QwenChatTemplate,
    /// Top-level string thinking value.
    StringThinking,
    /// Ant Ling reasoning object.
    AntLing,
}

/// Cache-control convention for prompt caching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheControlFormat {
    /// Anthropic-style cache-control markers.
    Anthropic,
}

/// Minimal model shape consumed by this port row.
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    /// Model identifier sent to the provider.
    pub id: String,
    /// API identifier from Pi.
    pub api: String,
    /// Provider identifier from Pi.
    pub provider: String,
    /// Provider base URL.
    pub base_url: String,
    /// Input modalities supported by the model.
    pub input: Vec<ModelInput>,
    /// Whether the model supports reasoning options.
    pub reasoning: bool,
    /// Provider/model-specific mappings for Pi thinking levels.
    pub thinking_level_map: HashMap<ModelThinkingLevel, Option<String>>,
    /// Default headers configured on the model.
    pub headers: ProviderHeaders,
    /// Optional OpenAI-compatible provider overrides.
    pub compat: Option<OpenAICompletionsCompat>,
}

/// Minimal context shape consumed by this port row.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Context {
    /// Conversation messages.
    pub messages: Vec<Message>,
    /// Optional system prompt.
    pub system_prompt: Option<String>,
    /// Available tool declarations.
    pub tools: Vec<Tool>,
}

/// Conversation message ported from Pi.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// User-authored message.
    User {
        /// User content, either plain text or structured text/image content.
        content: UserMessageContent,
    },
    /// Assistant-authored message.
    Assistant(AssistantMessage),
    /// Tool-result message.
    ToolResult(ToolResultMessage),
}

impl Message {
    /// Returns the Pi role string for this message.
    #[must_use]
    pub const fn role(&self) -> &'static str {
        match self {
            Self::User { .. } => "user",
            Self::Assistant(_) => "assistant",
            Self::ToolResult(_) => "toolResult",
        }
    }
}

/// Content accepted by Pi user messages.
#[derive(Debug, Clone, PartialEq)]
pub enum UserMessageContent {
    /// Plain text content.
    Text(String),
    /// Structured text/image content parts.
    Parts(Vec<ContentBlock>),
}

/// Assistant message shape consumed by OpenAI Chat Completions conversion.
#[derive(Debug, Clone, PartialEq)]
pub struct AssistantMessage {
    /// API identifier that produced this message.
    pub api: String,
    /// Provider identifier that produced this message.
    pub provider: String,
    /// Model identifier that produced this message.
    pub model: String,
    /// Assistant content blocks.
    pub content: Vec<ContentBlock>,
    /// Stop reason for replay filtering.
    pub stop_reason: StopReason,
}

/// Tool-result message shape consumed by OpenAI Chat Completions conversion.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolResultMessage {
    /// Tool-call id this result answers.
    pub tool_call_id: String,
    /// Optional tool name.
    pub tool_name: Option<String>,
    /// Tool output content blocks.
    pub content: Vec<ContentBlock>,
}

/// Structured content block used by Pi messages.
#[derive(Debug, Clone, PartialEq)]
pub enum ContentBlock {
    /// Text content part.
    Text {
        /// Text payload.
        text: String,
    },
    /// Provider thinking/reasoning content.
    Thinking {
        /// Thinking payload.
        thinking: String,
        /// Optional provider-specific replay signature.
        thinking_signature: Option<String>,
        /// Whether this is opaque redacted thinking.
        redacted: bool,
    },
    /// Tool call content part.
    ToolCall(ToolCall),
    /// Image content part.
    Image {
        /// Base64 encoded image data.
        data: String,
        /// Image MIME type, such as `image/png`.
        mime_type: String,
    },
}

/// Tool call content block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Tool-call identifier.
    pub id: String,
    /// Function/tool name.
    pub name: String,
    /// JSON arguments supplied to the function/tool.
    pub arguments: Value,
    /// Optional provider-specific reasoning detail serialized as JSON.
    pub thought_signature: Option<String>,
}

/// Tool declaration shape consumed by OpenAI Chat Completions conversion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tool {
    /// Function/tool name.
    pub name: String,
    /// Function/tool description.
    pub description: String,
    /// JSON Schema parameters.
    pub parameters: Value,
}

/// Assistant stop reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StopReason {
    /// Normal stop.
    Stop,
    /// Output hit a length limit.
    Length,
    /// Model requested tool use.
    ToolUse,
    /// Request was aborted.
    Aborted,
    /// Provider returned an error.
    Error,
}

/// OpenAI tool choice behavior.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OpenAIToolChoice {
    /// String tool choice such as `auto`, `none`, or `required`.
    Mode(String),
    /// Force a specific function tool by name.
    Function {
        /// Tool-choice type, usually `function`.
        #[serde(rename = "type")]
        kind: String,
        /// Function selector.
        function: OpenAIToolChoiceFunction,
    },
}

/// Function selector for [`OpenAIToolChoice`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenAIToolChoiceFunction {
    /// Function name to force.
    pub name: String,
}

/// Options specific to Pi's OpenAI Completions stream implementation.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OpenAICompletionsOptions {
    /// Sampling temperature.
    pub temperature: Option<f64>,
    /// Maximum output tokens requested by the caller.
    pub max_tokens: Option<u32>,
    /// API key for OpenAI-compatible providers.
    pub api_key: Option<String>,
    /// Prompt cache retention preference.
    pub cache_retention: Option<CacheRetention>,
    /// Optional session identifier used for prompt caching.
    pub session_id: Option<String>,
    /// HTTP timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Maximum OpenAI SDK retry attempts.
    pub max_retries: Option<u32>,
    /// Optional custom HTTP headers.
    pub headers: ProviderHeaders,
    /// Provider-scoped environment values.
    pub env: ProviderEnv,
    /// Tool choice behavior.
    pub tool_choice: Option<OpenAIToolChoice>,
    /// Reasoning effort requested by the caller.
    pub reasoning_effort: Option<ReasoningEffort>,
}

/// Prepared OpenAI-compatible Chat Completions request plus Pi stream options.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OpenAICompletionsRequest {
    /// Provider base URL used by the OpenAI-compatible endpoint.
    pub base_url: String,
    /// Headers sent with the request, after Pi default/session/Copilot/explicit merge.
    pub headers: ProviderHeaders,
    /// JSON body sent to `/chat/completions`.
    pub body: Value,
    /// Request timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Maximum retry attempts; Pi defaults this to zero.
    pub max_retries: u32,
}

/// Pi's event-stream handle for OpenAI-compatible completions.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OpenAICompletionsStream {
    /// Request captured before provider I/O starts; deterministic tests assert Pi parity here.
    pub request: OpenAICompletionsRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpenAICompletionsRequestOptions {
    max_retries: u32,
}

fn build_request_options(
    options: Option<&OpenAICompletionsOptions>,
) -> OpenAICompletionsRequestOptions {
    OpenAICompletionsRequestOptions {
        max_retries: options.and_then(|options| options.max_retries).unwrap_or(0),
    }
}

/// Builds the HTTP request envelope used by the OpenAI-compatible fallback.
pub fn build_request(
    model: &Model,
    context: &Context,
    options: Option<&OpenAICompletionsOptions>,
) -> Result<OpenAICompletionsRequest> {
    let compat = get_compat(model);
    let cache_retention = resolve_cache_retention(
        options.and_then(|options| options.cache_retention),
        options.map(|options| &options.env).unwrap_or(&EMPTY_ENV),
    );
    Ok(OpenAICompletionsRequest {
        base_url: model.base_url.clone(),
        headers: build_client_headers(model, context, options, &compat, cache_retention),
        body: build_params_value(model, context, options, &compat, cache_retention)?,
        timeout_ms: options.and_then(|options| options.timeout_ms),
        max_retries: build_request_options(options).max_retries,
    })
}

/// Starts an OpenAI-compatible Chat Completions stream by preparing the exact Pi request envelope.
///
/// Hooks require a mutable raw payload and raw response headers, so the OpenAI-family path uses a
/// narrow HTTP fallback boundary rather than `genai` normalization.
///
/// # Errors
///
/// Returns [`OpenAICompletionsError::MissingApiKey`] when no API key or authorization header is
/// available.
pub fn stream(
    model: &Model,
    context: &Context,
    options: Option<&OpenAICompletionsOptions>,
) -> Result<OpenAICompletionsStream> {
    get_client_api_key(
        &model.provider,
        options.and_then(|options| options.api_key.as_deref()),
        options.map(|options| &options.headers),
    )?;

    Ok(OpenAICompletionsStream {
        request: build_request(model, context, options)?,
    })
}

/// Starts an OpenAI-compatible stream using Pi's simple stream option mapping.
///
/// # Errors
///
/// Returns [`OpenAICompletionsError::MissingApiKey`] when no API key or authorization
/// header is available. Otherwise returns a port placeholder error until the
/// streaming dependency is selected and implemented.
pub fn stream_simple(
    model: &Model,
    context: &Context,
    options: Option<&OpenAICompletionsOptions>,
) -> Result<OpenAICompletionsStream> {
    stream(model, context, options)
}

/// Returns the prompt-cache retention, defaulting from `PI_CACHE_RETENTION` when present.
#[must_use]
pub fn resolve_cache_retention(
    cache_retention: Option<CacheRetention>,
    env: &ProviderEnv,
) -> CacheRetention {
    if let Some(cache_retention) = cache_retention {
        return cache_retention;
    }
    if env.get("PI_CACHE_RETENTION").map(String::as_str) == Some("long") {
        CacheRetention::Long
    } else {
        CacheRetention::Short
    }
}

/// Returns the OpenAI-compatible prompt-cache key for a resolved retention preference.
#[must_use]
pub fn prompt_cache_key(
    model: &Model,
    compat: &ResolvedOpenAICompletionsCompat,
    cache_retention: CacheRetention,
    session_id: Option<&str>,
) -> Option<String> {
    if (model.base_url.contains("api.openai.com") && cache_retention != CacheRetention::None)
        || (cache_retention == CacheRetention::Long && compat.supports_long_cache_retention)
    {
        clamp_openai_prompt_cache_key(session_id)
    } else {
        None
    }
}

/// Returns the provider prompt-cache retention value for OpenAI-compatible completions.
#[must_use]
pub fn prompt_cache_retention(
    compat: &ResolvedOpenAICompletionsCompat,
    cache_retention: CacheRetention,
) -> Option<&'static str> {
    match (cache_retention, compat.supports_long_cache_retention) {
        (CacheRetention::Long, true) => Some("24h"),
        _ => None,
    }
}

fn clamp_openai_prompt_cache_key(key: Option<&str>) -> Option<String> {
    let key = key?;
    Some(
        key.chars()
            .take(OPENAI_PROMPT_CACHE_KEY_MAX_LENGTH)
            .collect(),
    )
}

fn build_client_headers(
    model: &Model,
    context: &Context,
    options: Option<&OpenAICompletionsOptions>,
    compat: &ResolvedOpenAICompletionsCompat,
    cache_retention: CacheRetention,
) -> ProviderHeaders {
    let mut headers = model.headers.clone();
    if model.provider == "github-copilot" {
        let copilot_messages = to_copilot_messages(&context.messages);
        headers.extend(build_copilot_dynamic_headers(CopilotDynamicHeadersParams {
            messages: &copilot_messages,
            has_images: has_copilot_vision_input(&copilot_messages),
        }));
    }
    if cache_retention != CacheRetention::None
        && compat.send_session_affinity_headers
        && let Some(session_id) = options.and_then(|options| options.session_id.as_deref())
    {
        headers.insert("session_id".to_string(), session_id.to_string());
        headers.insert("x-client-request-id".to_string(), session_id.to_string());
        headers.insert("x-session-affinity".to_string(), session_id.to_string());
    }
    if let Some(options) = options {
        headers.extend(options.headers.clone());
    }
    headers
}

fn build_params_value(
    model: &Model,
    context: &Context,
    options: Option<&OpenAICompletionsOptions>,
    compat: &ResolvedOpenAICompletionsCompat,
    cache_retention: CacheRetention,
) -> Result<Value> {
    let messages = convert_messages(model, context, compat);
    let mut body = json!({
        "model": model.id,
        "messages": messages.iter().map(message_to_value).collect::<Vec<_>>(),
        "stream": true,
    });
    let object = body.as_object_mut().expect("request body is object");
    if let Some(key) = prompt_cache_key(
        model,
        compat,
        cache_retention,
        options.and_then(|options| options.session_id.as_deref()),
    ) {
        object.insert("prompt_cache_key".to_string(), Value::String(key));
    }
    if let Some(retention) = prompt_cache_retention(compat, cache_retention) {
        object.insert(
            "prompt_cache_retention".to_string(),
            Value::String(retention.to_string()),
        );
    }
    if compat.supports_usage_in_streaming {
        object.insert(
            "stream_options".to_string(),
            json!({ "include_usage": true }),
        );
    }
    if compat.supports_store {
        object.insert("store".to_string(), Value::Bool(false));
    }
    if let Some(max_tokens) = options
        .and_then(|options| options.max_tokens)
        .filter(|tokens| *tokens > 0)
    {
        let field = match compat.max_tokens_field {
            MaxTokensField::MaxCompletionTokens => "max_completion_tokens",
            MaxTokensField::MaxTokens => "max_tokens",
        };
        object.insert(field.to_string(), json!(max_tokens));
    }
    if let Some(temperature) = options.and_then(|options| options.temperature) {
        object.insert("temperature".to_string(), json!(temperature));
    }
    if !context.tools.is_empty() || has_tool_history(&context.messages) {
        let mut tools = context
            .tools
            .iter()
            .map(|tool| tool_to_value(tool, compat))
            .collect::<Vec<_>>();
        if let Some(cache_control) = compat_cache_control(compat, cache_retention)
            && let Some(last) = tools.last_mut()
        {
            last["cache_control"] = cache_control.clone();
        }
        object.insert("tools".to_string(), Value::Array(tools));
    }
    if let Some(cache_control) = compat_cache_control(compat, cache_retention) {
        apply_cache_control_to_messages(object, cache_control);
    }
    if let Some(tool_choice) = options.and_then(|options| options.tool_choice.as_ref()) {
        object.insert(
            "tool_choice".to_string(),
            serde_json::to_value(tool_choice).map_err(|error| {
                OpenAICompletionsError::Transport(format!(
                    "failed to serialize tool choice: {error}"
                ))
            })?,
        );
    }
    apply_reasoning(model, options, compat, object);
    Ok(body)
}

fn has_tool_history(messages: &[Message]) -> bool {
    messages.iter().any(|message| {
        matches!(message, Message::Assistant(assistant) if assistant.content.iter().any(|block| matches!(block, ContentBlock::ToolCall(_))))
            || matches!(message, Message::ToolResult(_))
    })
}

fn tool_to_value(tool: &Tool, compat: &ResolvedOpenAICompletionsCompat) -> Value {
    let mut value = json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.parameters,
        }
    });
    if compat.supports_strict_mode {
        value["function"]["strict"] = Value::Bool(false);
    }
    value
}

fn compat_cache_control(
    compat: &ResolvedOpenAICompletionsCompat,
    cache_retention: CacheRetention,
) -> Option<Value> {
    if compat.cache_control_format != Some(CacheControlFormat::Anthropic)
        || cache_retention == CacheRetention::None
    {
        return None;
    }
    let mut value = json!({ "type": "ephemeral" });
    if cache_retention == CacheRetention::Long && compat.supports_long_cache_retention {
        value["ttl"] = Value::String("1h".to_string());
    }
    Some(value)
}

fn apply_cache_control_to_messages(
    object: &mut serde_json::Map<String, Value>,
    cache_control: Value,
) {
    let Some(messages) = object.get_mut("messages").and_then(Value::as_array_mut) else {
        return;
    };
    if let Some(message) = messages.iter_mut().find(|message| {
        matches!(
            message.get("role").and_then(Value::as_str),
            Some("system" | "developer")
        )
    }) {
        ensure_content_parts(message, cache_control.clone());
    }
    if let Some(message) = messages.iter_mut().rev().find(|message| {
        matches!(
            message.get("role").and_then(Value::as_str),
            Some("user" | "assistant")
        )
    }) {
        ensure_content_parts(message, cache_control);
    }
}

fn ensure_content_parts(message: &mut Value, cache_control: Value) {
    let content = message.get_mut("content");
    match content {
        Some(Value::Array(parts)) => {
            if let Some(first) = parts.first_mut() {
                first["cache_control"] = cache_control;
            }
        }
        Some(Value::String(text)) => {
            message["content"] = Value::Array(vec![json!({
                "type": "text",
                "text": text.clone(),
                "cache_control": cache_control,
            })]);
        }
        _ => {}
    }
}

fn apply_reasoning(
    model: &Model,
    options: Option<&OpenAICompletionsOptions>,
    compat: &ResolvedOpenAICompletionsCompat,
    object: &mut serde_json::Map<String, Value>,
) {
    if !model.reasoning {
        return;
    }
    let effort = options
        .and_then(|options| options.reasoning_effort)
        .and_then(|effort| mapped_reasoning_effort(model, effort));
    match compat.thinking_format {
        ThinkingFormat::OpenRouter => {
            if let Some(effort) = effort {
                object.insert("reasoning".to_string(), json!({ "effort": effort }));
            } else if let Some(off) = off_reasoning_effort(model) {
                object.insert("reasoning".to_string(), json!({ "effort": off }));
            }
        }
        ThinkingFormat::DeepSeek => {
            object.insert(
                "thinking".to_string(),
                json!({ "type": if effort.is_some() { "enabled" } else { "disabled" } }),
            );
            if let Some(effort) = effort.filter(|_| compat.supports_reasoning_effort) {
                object.insert("reasoning_effort".to_string(), Value::String(effort));
            }
        }
        ThinkingFormat::Together => {
            object.insert(
                "reasoning".to_string(),
                json!({ "enabled": effort.is_some() }),
            );
            if let Some(effort) = effort.filter(|_| compat.supports_reasoning_effort) {
                object.insert("reasoning_effort".to_string(), Value::String(effort));
            }
        }
        _ => {
            if let Some(effort) = effort.filter(|_| compat.supports_reasoning_effort) {
                object.insert("reasoning_effort".to_string(), Value::String(effort));
            } else if compat.supports_reasoning_effort
                && let Some(off) = off_reasoning_effort(model)
            {
                object.insert("reasoning_effort".to_string(), Value::String(off));
            }
        }
    }
}

fn mapped_reasoning_effort(model: &Model, effort: ReasoningEffort) -> Option<String> {
    model
        .thinking_level_map
        .get(&effort.into())
        .cloned()
        .unwrap_or_else(|| Some(reasoning_effort_str(effort).to_string()))
}

fn off_reasoning_effort(model: &Model) -> Option<String> {
    match model.thinking_level_map.get(&ModelThinkingLevel::Off) {
        Some(None) => None,
        Some(Some(value)) => Some(value.clone()),
        None => Some("none".to_string()),
    }
}

fn reasoning_effort_str(effort: ReasoningEffort) -> &'static str {
    match effort {
        ReasoningEffort::Minimal => "minimal",
        ReasoningEffort::Low => "low",
        ReasoningEffort::Medium => "medium",
        ReasoningEffort::High => "high",
        ReasoningEffort::XHigh => "xhigh",
    }
}

fn message_to_value(message: &ChatCompletionMessage) -> Value {
    match message {
        ChatCompletionMessage::Instruction { role, content } => json!({
            "role": match role {
                InstructionRole::System => "system",
                InstructionRole::Developer => "developer",
            },
            "content": content,
        }),
        ChatCompletionMessage::User { content } => json!({
            "role": "user",
            "content": user_content_to_value(content),
        }),
        ChatCompletionMessage::Assistant {
            content,
            tool_calls,
            extra,
        } => {
            let mut value = json!({ "role": "assistant" });
            if let Some(content) = content {
                value["content"] = assistant_content_to_value(content);
            }
            if let Some(tool_calls) = tool_calls {
                value["tool_calls"] = serde_json::to_value(tool_calls).unwrap_or(Value::Null);
            }
            if let Some(object) = value.as_object_mut() {
                object.extend(extra.clone());
            }
            value
        }
        ChatCompletionMessage::Tool {
            content,
            tool_call_id,
            name,
        } => {
            let mut value = json!({
                "role": "tool",
                "content": content,
                "tool_call_id": tool_call_id,
            });
            if let Some(name) = name {
                value["name"] = Value::String(name.clone());
            }
            value
        }
    }
}

fn user_content_to_value(content: &UserChatContent) -> Value {
    match content {
        UserChatContent::Text(text) => Value::String(text.clone()),
        UserChatContent::Parts(parts) => serde_json::to_value(parts).unwrap_or(Value::Null),
    }
}

fn assistant_content_to_value(content: &AssistantChatContent) -> Value {
    match content {
        AssistantChatContent::Text(text) => Value::String(text.clone()),
        AssistantChatContent::Parts(parts) => serde_json::to_value(parts).unwrap_or(Value::Null),
    }
}

fn to_copilot_messages(messages: &[Message]) -> Vec<CopilotMessage> {
    messages.iter().map(to_copilot_message).collect()
}

fn to_copilot_message(message: &Message) -> CopilotMessage {
    match message {
        Message::User { content } => CopilotMessage::User {
            content: match content {
                UserMessageContent::Text(text) => CopilotUserContent::Text(text.clone()),
                UserMessageContent::Parts(parts) => {
                    CopilotUserContent::Parts(parts.iter().filter_map(to_copilot_content).collect())
                }
            },
        },
        Message::Assistant(_) => CopilotMessage::Assistant,
        Message::ToolResult(tool_result) => CopilotMessage::ToolResult {
            content: tool_result
                .content
                .iter()
                .filter_map(to_copilot_content)
                .collect(),
        },
    }
}

fn to_copilot_content(content: &ContentBlock) -> Option<CopilotContent> {
    match content {
        ContentBlock::Text { text } => Some(CopilotContent::Text { text: text.clone() }),
        ContentBlock::Image { data, mime_type } => Some(CopilotContent::Image {
            data: data.clone(),
            mime_type: mime_type.clone(),
        }),
        ContentBlock::Thinking { .. } | ContentBlock::ToolCall(_) => None,
    }
}

/// Converts Pi messages into OpenAI Chat Completions messages.
#[must_use]
pub fn convert_messages(
    model: &Model,
    context: &Context,
    compat: &ResolvedOpenAICompletionsCompat,
) -> Vec<ChatCompletionMessage> {
    let mut params = Vec::new();

    if let Some(system_prompt) = &context.system_prompt {
        let role = if model.reasoning && compat.supports_developer_role {
            InstructionRole::Developer
        } else {
            InstructionRole::System
        };
        params.push(ChatCompletionMessage::Instruction {
            role,
            content: sanitize_surrogates(system_prompt),
        });
    }

    let transformed_messages = transform_messages(&context.messages, model);
    let mut last_role: Option<&'static str> = None;
    let mut i = 0;

    while i < transformed_messages.len() {
        let msg = &transformed_messages[i];
        if compat.requires_assistant_after_tool_result
            && last_role == Some("toolResult")
            && msg.role() == "user"
        {
            params.push(processed_tool_results_assistant_message());
        }

        match msg {
            Message::User { content } => match content {
                UserMessageContent::Text(content) => {
                    params.push(ChatCompletionMessage::User {
                        content: UserChatContent::Text(sanitize_surrogates(content)),
                    });
                }
                UserMessageContent::Parts(content) => {
                    let content: Vec<ChatCompletionContentPart> = content
                        .iter()
                        .filter_map(|item| match item {
                            ContentBlock::Text { text } => Some(ChatCompletionContentPart::Text {
                                text: sanitize_surrogates(text),
                            }),
                            ContentBlock::Image { data, mime_type } => {
                                Some(ChatCompletionContentPart::ImageUrl {
                                    image_url: ChatCompletionImageUrl {
                                        url: format!("data:{mime_type};base64,{data}"),
                                    },
                                })
                            }
                            ContentBlock::Thinking { .. } | ContentBlock::ToolCall(_) => None,
                        })
                        .collect();
                    if !content.is_empty() {
                        params.push(ChatCompletionMessage::User {
                            content: UserChatContent::Parts(content),
                        });
                    }
                }
            },
            Message::Assistant(message) => {
                if let Some(assistant) = convert_assistant_message(model, compat, message) {
                    params.push(assistant);
                }
            }
            Message::ToolResult(_) => {
                let mut image_blocks = Vec::new();
                let mut j = i;

                while let Some(Message::ToolResult(tool_msg)) = transformed_messages.get(j) {
                    let text_result = tool_msg
                        .content
                        .iter()
                        .filter_map(|block| match block {
                            ContentBlock::Text { text } => Some(text.as_str()),
                            ContentBlock::Thinking { .. }
                            | ContentBlock::ToolCall(_)
                            | ContentBlock::Image { .. } => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    let has_images = tool_msg
                        .content
                        .iter()
                        .any(|block| matches!(block, ContentBlock::Image { .. }));
                    let tool_result_text = if !text_result.is_empty() {
                        text_result
                    } else if has_images {
                        "(see attached image)".to_string()
                    } else {
                        "(no tool output)".to_string()
                    };

                    params.push(ChatCompletionMessage::Tool {
                        content: sanitize_surrogates(&tool_result_text),
                        tool_call_id: tool_msg.tool_call_id.clone(),
                        name: compat
                            .requires_tool_result_name
                            .then(|| tool_msg.tool_name.clone())
                            .flatten(),
                    });

                    if has_images && model.input.contains(&ModelInput::Image) {
                        for block in &tool_msg.content {
                            if let ContentBlock::Image { data, mime_type } = block {
                                image_blocks.push(ChatCompletionContentPart::ImageUrl {
                                    image_url: ChatCompletionImageUrl {
                                        url: format!("data:{mime_type};base64,{data}"),
                                    },
                                });
                            }
                        }
                    }
                    j += 1;
                }

                i = j - 1;
                if !image_blocks.is_empty() {
                    if compat.requires_assistant_after_tool_result {
                        params.push(processed_tool_results_assistant_message());
                    }
                    let mut content = Vec::with_capacity(image_blocks.len() + 1);
                    content.push(ChatCompletionContentPart::Text {
                        text: "Attached image(s) from tool result:".to_string(),
                    });
                    content.extend(image_blocks);
                    params.push(ChatCompletionMessage::User {
                        content: UserChatContent::Parts(content),
                    });
                    last_role = Some("user");
                    i += 1;
                    continue;
                }
                last_role = Some("toolResult");
                i += 1;
                continue;
            }
        }

        last_role = Some(msg.role());
        i += 1;
    }

    params
}

/// OpenAI Chat Completions message parameter.
#[derive(Debug, Clone, PartialEq)]
pub enum ChatCompletionMessage {
    /// System or developer instruction message.
    Instruction {
        /// Instruction role.
        role: InstructionRole,
        /// Instruction text.
        content: String,
    },
    /// User message.
    User {
        /// User content.
        content: UserChatContent,
    },
    /// Assistant message.
    Assistant {
        /// Assistant text content, content parts, or null.
        content: Option<AssistantChatContent>,
        /// Tool calls requested by the assistant.
        tool_calls: Option<Vec<OpenAIToolCall>>,
        /// Provider-specific assistant fields such as reasoning signatures.
        extra: HashMap<String, Value>,
    },
    /// Tool-result message.
    Tool {
        /// Tool-result text.
        content: String,
        /// Tool-call id this result answers.
        tool_call_id: String,
        /// Optional tool name.
        name: Option<String>,
    },
}

impl ChatCompletionMessage {
    /// Returns the role emitted for OpenAI Chat Completions.
    #[must_use]
    pub const fn role(&self) -> &'static str {
        match self {
            Self::Instruction {
                role: InstructionRole::System,
                ..
            } => "system",
            Self::Instruction {
                role: InstructionRole::Developer,
                ..
            } => "developer",
            Self::User { .. } => "user",
            Self::Assistant { .. } => "assistant",
            Self::Tool { .. } => "tool",
        }
    }
}

/// Instruction role for OpenAI-compatible providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstructionRole {
    /// System prompt role.
    System,
    /// Developer prompt role.
    Developer,
}

/// User content accepted by OpenAI Chat Completions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UserChatContent {
    /// Plain text content.
    Text(String),
    /// Structured text/image content parts.
    Parts(Vec<ChatCompletionContentPart>),
}

/// Assistant content accepted by OpenAI Chat Completions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AssistantChatContent {
    /// Plain text content.
    Text(String),
    /// Structured text content parts.
    Parts(Vec<ChatCompletionTextPart>),
}

/// Structured content part accepted by OpenAI Chat Completions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ChatCompletionContentPart {
    /// Text content part.
    #[serde(rename = "text")]
    Text {
        /// Text payload.
        text: String,
    },
    /// Image URL content part.
    #[serde(rename = "image_url")]
    ImageUrl {
        /// Image URL payload.
        image_url: ChatCompletionImageUrl,
    },
}

/// Image URL payload accepted by OpenAI Chat Completions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatCompletionImageUrl {
    /// Data URL or remote URL for the image.
    pub url: String,
}

/// Structured assistant text content part.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatCompletionTextPart {
    /// Part type, always `text`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Text payload.
    pub text: String,
}

/// OpenAI tool call parameter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenAIToolCall {
    /// Tool-call id.
    pub id: String,
    /// Tool-call type, always `function`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Function call payload.
    pub function: OpenAIFunctionCall,
}

/// OpenAI function call payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenAIFunctionCall {
    /// Function/tool name.
    pub name: String,
    /// JSON-serialized function/tool arguments.
    pub arguments: String,
}

/// Auto-detects OpenAI-compatible provider settings from provider name and base URL.
#[must_use]
pub fn detect_compat(model: &Model) -> ResolvedOpenAICompletionsCompat {
    let provider = model.provider.as_str();
    let base_url = model.base_url.as_str();

    let is_zai = matches!(provider, "zai" | "zai-coding-cn")
        || base_url.contains("api.z.ai")
        || base_url.contains("open.bigmodel.cn");
    let is_together = provider == "together"
        || base_url.contains("api.together.ai")
        || base_url.contains("api.together.xyz");
    let is_moonshot =
        matches!(provider, "moonshotai" | "moonshotai-cn") || base_url.contains("api.moonshot.");
    let is_openrouter = provider == "openrouter" || base_url.contains("openrouter.ai");
    let is_cloudflare_workers_ai =
        provider == "cloudflare-workers-ai" || base_url.contains("api.cloudflare.com");
    let is_cloudflare_ai_gateway =
        provider == "cloudflare-ai-gateway" || base_url.contains("gateway.ai.cloudflare.com");
    let is_nvidia = provider == "nvidia" || base_url.contains("integrate.api.nvidia.com");
    let is_ant_ling = provider == "ant-ling" || base_url.contains("api.ant-ling.com");

    let is_non_standard = is_nvidia
        || provider == "cerebras"
        || base_url.contains("cerebras.ai")
        || provider == "xai"
        || base_url.contains("api.x.ai")
        || is_together
        || base_url.contains("chutes.ai")
        || base_url.contains("deepseek.com")
        || is_zai
        || is_moonshot
        || provider == "opencode"
        || base_url.contains("opencode.ai")
        || is_cloudflare_workers_ai
        || is_cloudflare_ai_gateway
        || is_ant_ling;

    let use_max_tokens = base_url.contains("chutes.ai")
        || is_moonshot
        || is_cloudflare_ai_gateway
        || is_together
        || is_nvidia
        || is_ant_ling;
    let is_grok = provider == "xai" || base_url.contains("api.x.ai");
    let is_deepseek = provider == "deepseek" || base_url.contains("deepseek.com");
    let is_openrouter_developer_role_model =
        is_openrouter && (model.id.starts_with("anthropic/") || model.id.starts_with("openai/"));
    let cache_control_format = (provider == "openrouter" && model.id.starts_with("anthropic/"))
        .then_some(CacheControlFormat::Anthropic);

    ResolvedOpenAICompletionsCompat {
        supports_store: !is_non_standard,
        supports_developer_role: is_openrouter_developer_role_model
            || (!is_non_standard && !is_openrouter),
        supports_reasoning_effort: !is_grok
            && !is_zai
            && !is_moonshot
            && !is_together
            && !is_cloudflare_ai_gateway
            && !is_nvidia
            && !is_ant_ling,
        supports_usage_in_streaming: true,
        max_tokens_field: if use_max_tokens {
            MaxTokensField::MaxTokens
        } else {
            MaxTokensField::MaxCompletionTokens
        },
        requires_tool_result_name: false,
        requires_assistant_after_tool_result: false,
        requires_thinking_as_text: false,
        requires_reasoning_content_on_assistant_messages: is_deepseek,
        thinking_format: if is_deepseek {
            ThinkingFormat::DeepSeek
        } else if is_zai {
            ThinkingFormat::Zai
        } else if is_together {
            ThinkingFormat::Together
        } else if is_ant_ling {
            ThinkingFormat::AntLing
        } else if is_openrouter {
            ThinkingFormat::OpenRouter
        } else {
            ThinkingFormat::OpenAI
        },
        supports_strict_mode: !is_moonshot
            && !is_together
            && !is_cloudflare_ai_gateway
            && !is_nvidia,
        cache_control_format,
        send_session_affinity_headers: false,
        supports_long_cache_retention: !(is_together
            || is_cloudflare_workers_ai
            || is_cloudflare_ai_gateway
            || is_nvidia
            || is_ant_ling),
    }
}

/// Resolves explicit model compatibility overrides over auto-detected settings.
#[must_use]
pub fn get_compat(model: &Model) -> ResolvedOpenAICompletionsCompat {
    let detected = detect_compat(model);
    let Some(compat) = &model.compat else {
        return detected;
    };

    ResolvedOpenAICompletionsCompat {
        supports_store: compat.supports_store.unwrap_or(detected.supports_store),
        supports_developer_role: compat
            .supports_developer_role
            .unwrap_or(detected.supports_developer_role),
        supports_reasoning_effort: compat
            .supports_reasoning_effort
            .unwrap_or(detected.supports_reasoning_effort),
        supports_usage_in_streaming: compat
            .supports_usage_in_streaming
            .unwrap_or(detected.supports_usage_in_streaming),
        max_tokens_field: compat.max_tokens_field.unwrap_or(detected.max_tokens_field),
        requires_tool_result_name: compat
            .requires_tool_result_name
            .unwrap_or(detected.requires_tool_result_name),
        requires_assistant_after_tool_result: compat
            .requires_assistant_after_tool_result
            .unwrap_or(detected.requires_assistant_after_tool_result),
        requires_thinking_as_text: compat
            .requires_thinking_as_text
            .unwrap_or(detected.requires_thinking_as_text),
        requires_reasoning_content_on_assistant_messages: compat
            .requires_reasoning_content_on_assistant_messages
            .unwrap_or(detected.requires_reasoning_content_on_assistant_messages),
        thinking_format: compat.thinking_format.unwrap_or(detected.thinking_format),
        supports_strict_mode: compat
            .supports_strict_mode
            .unwrap_or(detected.supports_strict_mode),
        cache_control_format: compat
            .cache_control_format
            .or(detected.cache_control_format),
        send_session_affinity_headers: compat
            .send_session_affinity_headers
            .unwrap_or(detected.send_session_affinity_headers),
        supports_long_cache_retention: compat
            .supports_long_cache_retention
            .unwrap_or(detected.supports_long_cache_retention),
    }
}

fn has_header(headers: Option<&ProviderHeaders>, name: &str) -> bool {
    let Some(headers) = headers else {
        return false;
    };
    headers
        .iter()
        .any(|(key, value)| key.eq_ignore_ascii_case(name) && !value.trim().is_empty())
}

fn get_client_api_key(
    provider: &str,
    api_key: Option<&str>,
    headers: Option<&ProviderHeaders>,
) -> Result<String> {
    if let Some(api_key) = api_key.filter(|value| !value.is_empty()) {
        return Ok(api_key.to_string());
    }
    if has_header(headers, "authorization") || has_header(headers, "cf-aig-authorization") {
        return Ok("unused".to_string());
    }
    Err(OpenAICompletionsError::MissingApiKey {
        provider: provider.to_string(),
    })
}

fn convert_assistant_message(
    model: &Model,
    compat: &ResolvedOpenAICompletionsCompat,
    message: &AssistantMessage,
) -> Option<ChatCompletionMessage> {
    let assistant_text_parts: Vec<ChatCompletionTextPart> = message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } if !text.trim().is_empty() => {
                Some(ChatCompletionTextPart {
                    kind: "text".to_string(),
                    text: sanitize_surrogates(text),
                })
            }
            ContentBlock::Text { .. }
            | ContentBlock::Thinking { .. }
            | ContentBlock::ToolCall(_)
            | ContentBlock::Image { .. } => None,
        })
        .collect();
    let assistant_text = assistant_text_parts
        .iter()
        .map(|part| part.text.as_str())
        .collect::<String>();
    let non_empty_thinking_blocks: Vec<_> = message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Thinking {
                thinking,
                thinking_signature,
                ..
            } if !thinking.trim().is_empty() => Some((thinking, thinking_signature)),
            ContentBlock::Thinking { .. }
            | ContentBlock::Text { .. }
            | ContentBlock::ToolCall(_)
            | ContentBlock::Image { .. } => None,
        })
        .collect();

    let mut content = None;
    let mut extra = HashMap::new();

    if !non_empty_thinking_blocks.is_empty() {
        if compat.requires_thinking_as_text {
            let thinking_text = non_empty_thinking_blocks
                .iter()
                .map(|(thinking, _)| sanitize_surrogates(thinking))
                .collect::<Vec<_>>()
                .join("\n\n");
            let mut parts = Vec::with_capacity(assistant_text_parts.len() + 1);
            parts.push(ChatCompletionTextPart {
                kind: "text".to_string(),
                text: thinking_text,
            });
            parts.extend(assistant_text_parts);
            content = Some(AssistantChatContent::Parts(parts));
        } else {
            if !assistant_text.is_empty() {
                content = Some(AssistantChatContent::Text(assistant_text));
            }
            let mut signature = non_empty_thinking_blocks
                .first()
                .and_then(|(_, signature)| signature.as_deref());
            if model.provider == "opencode-go" && signature == Some("reasoning") {
                signature = Some("reasoning_content");
            }
            if let Some(signature) = signature.filter(|signature| !signature.is_empty()) {
                let thinking = non_empty_thinking_blocks
                    .iter()
                    .map(|(thinking, _)| thinking.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                extra.insert(signature.to_string(), Value::String(thinking));
            }
        }
    } else if !assistant_text.is_empty() {
        content = Some(AssistantChatContent::Text(assistant_text));
    }

    let tool_calls: Vec<OpenAIToolCall> = message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::ToolCall(tool_call) => Some(OpenAIToolCall {
                id: tool_call.id.clone(),
                kind: "function".to_string(),
                function: OpenAIFunctionCall {
                    name: tool_call.name.clone(),
                    arguments: serde_json::to_string(&tool_call.arguments)
                        .unwrap_or_else(|_| "null".to_string()),
                },
            }),
            ContentBlock::Text { .. }
            | ContentBlock::Thinking { .. }
            | ContentBlock::Image { .. } => None,
        })
        .collect();

    let reasoning_details: Vec<Value> = message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::ToolCall(ToolCall {
                thought_signature: Some(signature),
                ..
            }) => serde_json::from_str(signature).ok(),
            ContentBlock::ToolCall(_)
            | ContentBlock::Text { .. }
            | ContentBlock::Thinking { .. }
            | ContentBlock::Image { .. } => None,
        })
        .collect();
    if !reasoning_details.is_empty() {
        extra.insert(
            "reasoning_details".to_string(),
            Value::Array(reasoning_details),
        );
    }

    if compat.requires_reasoning_content_on_assistant_messages
        && model.reasoning
        && !extra.contains_key("reasoning_content")
    {
        extra.insert(
            "reasoning_content".to_string(),
            Value::String(String::new()),
        );
    }

    let tool_calls = (!tool_calls.is_empty()).then_some(tool_calls);
    let has_content = match &content {
        Some(AssistantChatContent::Text(text)) => !text.is_empty(),
        Some(AssistantChatContent::Parts(parts)) => !parts.is_empty(),
        None => false,
    };

    if !has_content && tool_calls.is_none() {
        return None;
    }

    Some(ChatCompletionMessage::Assistant {
        content,
        tool_calls,
        extra,
    })
}

fn processed_tool_results_assistant_message() -> ChatCompletionMessage {
    ChatCompletionMessage::Assistant {
        content: Some(AssistantChatContent::Text(
            "I have processed the tool results.".to_string(),
        )),
        tool_calls: None,
        extra: HashMap::new(),
    }
}

fn transform_messages(messages: &[Message], model: &Model) -> Vec<Message> {
    let normalized: Vec<Message> = downgrade_unsupported_images(messages, model);
    let mut tool_call_id_map: HashMap<String, String> = HashMap::new();

    let transformed: Vec<Message> = normalized
        .into_iter()
        .map(|msg| match msg {
            Message::User { .. } => msg,
            Message::ToolResult(mut tool_result) => {
                if let Some(normalized_id) = tool_call_id_map.get(&tool_result.tool_call_id) {
                    tool_result.tool_call_id = normalized_id.clone();
                }
                Message::ToolResult(tool_result)
            }
            Message::Assistant(mut assistant) => {
                let is_same_model = assistant.provider == model.provider
                    && assistant.api == model.api
                    && assistant.model == model.id;
                assistant.content = assistant
                    .content
                    .into_iter()
                    .filter_map(|block| match block {
                        ContentBlock::Thinking {
                            thinking,
                            thinking_signature,
                            redacted,
                        } => {
                            if redacted {
                                return is_same_model.then_some(ContentBlock::Thinking {
                                    thinking,
                                    thinking_signature,
                                    redacted,
                                });
                            }
                            if is_same_model && thinking_signature.is_some() {
                                return Some(ContentBlock::Thinking {
                                    thinking,
                                    thinking_signature,
                                    redacted,
                                });
                            }
                            if thinking.trim().is_empty() {
                                return None;
                            }
                            if is_same_model {
                                Some(ContentBlock::Thinking {
                                    thinking,
                                    thinking_signature,
                                    redacted,
                                })
                            } else {
                                Some(ContentBlock::Text { text: thinking })
                            }
                        }
                        ContentBlock::ToolCall(mut tool_call) => {
                            if !is_same_model {
                                tool_call.thought_signature = None;
                                let normalized_id = normalize_tool_call_id(&tool_call.id, model);
                                if normalized_id != tool_call.id {
                                    tool_call_id_map
                                        .insert(tool_call.id.clone(), normalized_id.clone());
                                    tool_call.id = normalized_id;
                                }
                            }
                            Some(ContentBlock::ToolCall(tool_call))
                        }
                        ContentBlock::Text { text } => Some(ContentBlock::Text { text }),
                        ContentBlock::Image { data, mime_type } => {
                            Some(ContentBlock::Image { data, mime_type })
                        }
                    })
                    .collect();
                Message::Assistant(assistant)
            }
        })
        .collect();

    insert_synthetic_tool_results(transformed)
}

fn downgrade_unsupported_images(messages: &[Message], model: &Model) -> Vec<Message> {
    if model.input.contains(&ModelInput::Image) {
        return messages.to_vec();
    }

    messages
        .iter()
        .cloned()
        .map(|msg| match msg {
            Message::User {
                content: UserMessageContent::Parts(content),
            } => Message::User {
                content: UserMessageContent::Parts(replace_images_with_placeholder(
                    content,
                    "(image omitted: model does not support images)",
                )),
            },
            Message::ToolResult(mut tool_result) => {
                tool_result.content = replace_images_with_placeholder(
                    tool_result.content,
                    "(tool image omitted: model does not support images)",
                );
                Message::ToolResult(tool_result)
            }
            other => other,
        })
        .collect()
}

fn replace_images_with_placeholder(
    content: Vec<ContentBlock>,
    placeholder: &str,
) -> Vec<ContentBlock> {
    let mut result = Vec::with_capacity(content.len());
    let mut previous_was_placeholder = false;

    for block in content {
        match block {
            ContentBlock::Image { .. } => {
                if !previous_was_placeholder {
                    result.push(ContentBlock::Text {
                        text: placeholder.to_string(),
                    });
                }
                previous_was_placeholder = true;
            }
            ContentBlock::Text { text } => {
                previous_was_placeholder = text == placeholder;
                result.push(ContentBlock::Text { text });
            }
            other => {
                previous_was_placeholder = false;
                result.push(other);
            }
        }
    }

    result
}

fn insert_synthetic_tool_results(transformed: Vec<Message>) -> Vec<Message> {
    let mut result = Vec::with_capacity(transformed.len());
    let mut pending_tool_calls: Vec<ToolCall> = Vec::new();
    let mut existing_tool_result_ids: HashSet<String> = HashSet::new();

    for msg in transformed {
        match &msg {
            Message::Assistant(assistant) => {
                flush_synthetic_tool_results(
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
                        ContentBlock::ToolCall(tool_call) => Some(tool_call.clone()),
                        ContentBlock::Text { .. }
                        | ContentBlock::Thinking { .. }
                        | ContentBlock::Image { .. } => None,
                    })
                    .collect();
                if !tool_calls.is_empty() {
                    pending_tool_calls = tool_calls;
                    existing_tool_result_ids.clear();
                }
                result.push(msg);
            }
            Message::ToolResult(tool_result) => {
                existing_tool_result_ids.insert(tool_result.tool_call_id.clone());
                result.push(msg);
            }
            Message::User { .. } => {
                flush_synthetic_tool_results(
                    &mut result,
                    &mut pending_tool_calls,
                    &mut existing_tool_result_ids,
                );
                result.push(msg);
            }
        }
    }

    flush_synthetic_tool_results(
        &mut result,
        &mut pending_tool_calls,
        &mut existing_tool_result_ids,
    );
    result
}

fn flush_synthetic_tool_results(
    result: &mut Vec<Message>,
    pending_tool_calls: &mut Vec<ToolCall>,
    existing_tool_result_ids: &mut HashSet<String>,
) {
    for tool_call in pending_tool_calls.drain(..) {
        if !existing_tool_result_ids.contains(&tool_call.id) {
            result.push(Message::ToolResult(ToolResultMessage {
                tool_call_id: tool_call.id,
                tool_name: Some(tool_call.name),
                content: vec![ContentBlock::Text {
                    text: "No result provided".to_string(),
                }],
            }));
        }
    }
    existing_tool_result_ids.clear();
}

fn normalize_tool_call_id(id: &str, model: &Model) -> String {
    if let Some((call_id, _)) = id.split_once('|') {
        return call_id
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                    ch
                } else {
                    '_'
                }
            })
            .take(40)
            .collect();
    }

    if model.provider == "openai" {
        id.chars().take(40).collect()
    } else {
        id.to_string()
    }
}

fn sanitize_surrogates(text: &str) -> String {
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Model {
        Model {
            id: "gpt-4".to_string(),
            api: "openai-completions".to_string(),
            provider: "openai".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            input: vec![ModelInput::Text],
            reasoning: false,
            thinking_level_map: HashMap::new(),
            headers: ProviderHeaders::new(),
            compat: None,
        }
    }

    fn image_model() -> Model {
        Model {
            id: "gpt-4o-mini".to_string(),
            input: vec![ModelInput::Text, ModelInput::Image],
            ..model()
        }
    }

    fn default_completions_compat() -> ResolvedOpenAICompletionsCompat {
        ResolvedOpenAICompletionsCompat {
            supports_store: true,
            supports_developer_role: true,
            supports_reasoning_effort: true,
            supports_usage_in_streaming: true,
            max_tokens_field: MaxTokensField::MaxCompletionTokens,
            requires_tool_result_name: false,
            requires_assistant_after_tool_result: false,
            requires_thinking_as_text: false,
            requires_reasoning_content_on_assistant_messages: false,
            thinking_format: ThinkingFormat::OpenAI,
            supports_strict_mode: true,
            cache_control_format: Some(CacheControlFormat::Anthropic),
            send_session_affinity_headers: false,
            supports_long_cache_retention: true,
        }
    }

    fn tool_call(id: &str, name: &str, arguments: Value) -> ContentBlock {
        ContentBlock::ToolCall(ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            arguments,
            thought_signature: None,
        })
    }

    fn tool_result(tool_call_id: &str, tool_name: &str, content: Vec<ContentBlock>) -> Message {
        Message::ToolResult(ToolResultMessage {
            tool_call_id: tool_call_id.to_string(),
            tool_name: Some(tool_name.to_string()),
            content,
        })
    }

    fn read_image_tool_result(tool_call_id: &str) -> Message {
        tool_result(
            tool_call_id,
            "read",
            vec![
                ContentBlock::Text {
                    text: "Read image file [image/png]".to_string(),
                },
                ContentBlock::Image {
                    data: "ZmFrZQ==".to_string(),
                    mime_type: "image/png".to_string(),
                },
            ],
        )
    }

    #[test]
    fn batches_tool_result_images_after_consecutive_tool_results() {
        let model = image_model();
        let context = Context {
            messages: vec![
                Message::User {
                    content: UserMessageContent::Text("Read the images".to_string()),
                },
                Message::Assistant(AssistantMessage {
                    api: model.api.clone(),
                    provider: model.provider.clone(),
                    model: model.id.clone(),
                    content: vec![
                        tool_call("tool-1", "read", serde_json::json!({ "path": "img-1.png" })),
                        tool_call("tool-2", "read", serde_json::json!({ "path": "img-2.png" })),
                    ],
                    stop_reason: StopReason::ToolUse,
                }),
                read_image_tool_result("tool-1"),
                read_image_tool_result("tool-2"),
            ],
            ..Context::default()
        };

        let messages = convert_messages(&model, &context, &default_completions_compat());
        let roles: Vec<&str> = messages.iter().map(ChatCompletionMessage::role).collect();
        assert_eq!(roles, vec!["user", "assistant", "tool", "tool", "user"]);

        let image_message = messages.last().expect("image user message");
        assert_eq!(image_message.role(), "user");
        let ChatCompletionMessage::User {
            content: UserChatContent::Parts(parts),
        } = image_message
        else {
            panic!("expected structured user image message: {image_message:?}");
        };
        let image_part_count = parts
            .iter()
            .filter(|part| matches!(part, ChatCompletionContentPart::ImageUrl { .. }))
            .count();
        assert_eq!(image_part_count, 2);
    }

    #[test]
    fn uses_no_tool_output_placeholder_for_empty_tool_results_without_images() {
        let model = image_model();
        let context = Context {
            messages: vec![
                Message::User {
                    content: UserMessageContent::Text("Run the command".to_string()),
                },
                Message::Assistant(AssistantMessage {
                    api: model.api.clone(),
                    provider: model.provider.clone(),
                    model: model.id.clone(),
                    content: vec![tool_call(
                        "tool-1",
                        "bash",
                        serde_json::json!({ "command": "true" }),
                    )],
                    stop_reason: StopReason::ToolUse,
                }),
                tool_result(
                    "tool-1",
                    "bash",
                    vec![ContentBlock::Text {
                        text: String::new(),
                    }],
                ),
            ],
            ..Context::default()
        };

        let messages = convert_messages(&model, &context, &default_completions_compat());
        let tool_message = messages.iter().find_map(|message| match message {
            ChatCompletionMessage::Tool { content, .. } => Some(content),
            _ => None,
        });
        assert_eq!(tool_message.map(String::as_str), Some("(no tool output)"));
        assert!(
            !tool_message
                .expect("tool message")
                .contains("see attached image")
        );
    }

    #[test]
    fn stream_reports_missing_key_before_placeholder() {
        let err = stream(&model(), &Context::default(), None).expect_err("missing key");
        assert!(matches!(err, OpenAICompletionsError::MissingApiKey { .. }));
    }

    #[test]
    fn stream_accepts_authorization_header_without_api_key() {
        let mut options = OpenAICompletionsOptions::default();
        options
            .headers
            .insert("Authorization".to_string(), "Bearer token".to_string());
        let stream = stream(&model(), &Context::default(), Some(&options))
            .expect("authorization header should satisfy key check");

        assert_eq!(
            stream
                .request
                .headers
                .get("Authorization")
                .map(String::as_str),
            Some("Bearer token")
        );
    }

    #[test]
    fn disables_sdk_retries_by_default() {
        let options = OpenAICompletionsOptions::default();

        assert_eq!(build_request_options(Some(&options)).max_retries, 0);
    }

    #[test]
    fn honors_explicit_provider_retry_settings() {
        let options = OpenAICompletionsOptions {
            max_retries: Some(2),
            ..OpenAICompletionsOptions::default()
        };

        assert_eq!(build_request_options(Some(&options)).max_retries, 2);
    }

    #[derive(Default)]
    struct PromptCacheCaptureOptions {
        cache_retention: Option<CacheRetention>,
        session_id: Option<String>,
        env: ProviderEnv,
    }

    #[derive(Debug, PartialEq, Eq)]
    struct CapturedCompletionsPayload {
        prompt_cache_key: Option<String>,
        prompt_cache_retention: Option<&'static str>,
    }

    fn capture_prompt_cache_request(
        model: &Model,
        options: &PromptCacheCaptureOptions,
    ) -> CapturedCompletionsPayload {
        let compat = get_compat(model);
        let cache_retention = resolve_cache_retention(options.cache_retention, &options.env);

        CapturedCompletionsPayload {
            prompt_cache_key: prompt_cache_key(
                model,
                &compat,
                cache_retention,
                options.session_id.as_deref(),
            ),
            prompt_cache_retention: prompt_cache_retention(&compat, cache_retention),
        }
    }

    #[test]
    fn openai_completions_prompt_cache_sets_key_for_direct_openai_when_enabled() {
        let payload = capture_prompt_cache_request(
            &model(),
            &PromptCacheCaptureOptions {
                session_id: Some("session-123".to_string()),
                ..PromptCacheCaptureOptions::default()
            },
        );

        assert_eq!(payload.prompt_cache_key.as_deref(), Some("session-123"));
        assert_eq!(payload.prompt_cache_retention, None);
    }

    #[test]
    fn openai_completions_prompt_cache_sets_retention_to_24h_when_long() {
        let payload = capture_prompt_cache_request(
            &model(),
            &PromptCacheCaptureOptions {
                cache_retention: Some(CacheRetention::Long),
                session_id: Some("session-456".to_string()),
                ..PromptCacheCaptureOptions::default()
            },
        );

        assert_eq!(payload.prompt_cache_key.as_deref(), Some("session-456"));
        assert_eq!(payload.prompt_cache_retention, Some("24h"));
    }

    #[test]
    fn openai_completions_prompt_cache_clamps_key_to_openai_limit() {
        let session_id = "x".repeat(67);
        let payload = capture_prompt_cache_request(
            &model(),
            &PromptCacheCaptureOptions {
                session_id: Some(session_id),
                ..PromptCacheCaptureOptions::default()
            },
        );

        assert_eq!(payload.prompt_cache_key, Some("x".repeat(64)));
    }

    #[test]
    fn openai_completions_prompt_cache_omits_fields_when_retention_is_none() {
        let payload = capture_prompt_cache_request(
            &model(),
            &PromptCacheCaptureOptions {
                cache_retention: Some(CacheRetention::None),
                session_id: Some("session-789".to_string()),
                ..PromptCacheCaptureOptions::default()
            },
        );

        assert_eq!(payload.prompt_cache_key, None);
        assert_eq!(payload.prompt_cache_retention, None);
    }

    #[test]
    fn openai_completions_prompt_cache_omits_fields_for_incompatible_proxy() {
        let mut model = model();
        model.base_url = "https://proxy.example.com/v1".to_string();
        model.compat = Some(OpenAICompletionsCompat {
            supports_long_cache_retention: Some(false),
            ..OpenAICompletionsCompat::default()
        });

        let payload = capture_prompt_cache_request(
            &model,
            &PromptCacheCaptureOptions {
                cache_retention: Some(CacheRetention::Long),
                session_id: Some("session-proxy".to_string()),
                ..PromptCacheCaptureOptions::default()
            },
        );

        assert_eq!(payload.prompt_cache_key, None);
        assert_eq!(payload.prompt_cache_retention, None);
    }

    #[test]
    fn openai_completions_prompt_cache_uses_pi_cache_retention_for_direct_openai_requests() {
        let mut env = ProviderEnv::new();
        env.insert("PI_CACHE_RETENTION".to_string(), "long".to_string());
        let payload = capture_prompt_cache_request(
            &model(),
            &PromptCacheCaptureOptions {
                session_id: Some("session-env".to_string()),
                env,
                ..PromptCacheCaptureOptions::default()
            },
        );

        assert_eq!(payload.prompt_cache_key.as_deref(), Some("session-env"));
        assert_eq!(payload.prompt_cache_retention, Some("24h"));
    }

    #[test]
    fn openai_completions_prompt_cache_sends_session_affinity_headers_when_enabled() {
        let mut model = model();
        model.compat = Some(OpenAICompletionsCompat {
            send_session_affinity_headers: Some(true),
            ..OpenAICompletionsCompat::default()
        });
        let request = build_request(
            &model,
            &Context::default(),
            Some(&OpenAICompletionsOptions {
                api_key: Some("test".to_string()),
                session_id: Some("session-affinity".to_string()),
                ..OpenAICompletionsOptions::default()
            }),
        )
        .expect("request should build");

        assert_eq!(
            request.headers.get("session_id").map(String::as_str),
            Some("session-affinity")
        );
        assert_eq!(
            request
                .headers
                .get("x-client-request-id")
                .map(String::as_str),
            Some("session-affinity")
        );
        assert_eq!(
            request
                .headers
                .get("x-session-affinity")
                .map(String::as_str),
            Some("session-affinity")
        );
    }

    #[test]
    fn openai_completions_prompt_cache_omits_session_affinity_headers_when_retention_is_none() {
        let mut model = model();
        model.compat = Some(OpenAICompletionsCompat {
            send_session_affinity_headers: Some(true),
            ..OpenAICompletionsCompat::default()
        });
        let request = build_request(
            &model,
            &Context::default(),
            Some(&OpenAICompletionsOptions {
                api_key: Some("test".to_string()),
                session_id: Some("session-affinity".to_string()),
                cache_retention: Some(CacheRetention::None),
                ..OpenAICompletionsOptions::default()
            }),
        )
        .expect("request should build");

        assert!(!request.headers.contains_key("session_id"));
        assert!(!request.headers.contains_key("x-client-request-id"));
        assert!(!request.headers.contains_key("x-session-affinity"));
    }

    #[test]
    fn openai_completions_prompt_cache_lets_explicit_headers_override_generated_affinity_headers() {
        let mut model = model();
        model.compat = Some(OpenAICompletionsCompat {
            send_session_affinity_headers: Some(true),
            ..OpenAICompletionsCompat::default()
        });
        let mut headers = ProviderHeaders::new();
        headers.insert("session_id".to_string(), "override-session".to_string());
        headers.insert(
            "x-client-request-id".to_string(),
            "override-request".to_string(),
        );
        headers.insert(
            "x-session-affinity".to_string(),
            "override-affinity".to_string(),
        );
        let request = build_request(
            &model,
            &Context::default(),
            Some(&OpenAICompletionsOptions {
                api_key: Some("test".to_string()),
                session_id: Some("session-affinity".to_string()),
                headers,
                ..OpenAICompletionsOptions::default()
            }),
        )
        .expect("request should build");

        assert_eq!(
            request.headers.get("session_id").map(String::as_str),
            Some("override-session")
        );
        assert_eq!(
            request
                .headers
                .get("x-client-request-id")
                .map(String::as_str),
            Some("override-request")
        );
        assert_eq!(
            request
                .headers
                .get("x-session-affinity")
                .map(String::as_str),
            Some("override-affinity")
        );
    }

    fn thinking_as_text_compat() -> ResolvedOpenAICompletionsCompat {
        ResolvedOpenAICompletionsCompat {
            supports_store: true,
            supports_developer_role: true,
            supports_reasoning_effort: true,
            supports_usage_in_streaming: true,
            max_tokens_field: MaxTokensField::MaxCompletionTokens,
            requires_tool_result_name: false,
            requires_assistant_after_tool_result: false,
            requires_thinking_as_text: true,
            requires_reasoning_content_on_assistant_messages: false,
            thinking_format: ThinkingFormat::OpenAI,
            supports_strict_mode: true,
            cache_control_format: None,
            send_session_affinity_headers: false,
            supports_long_cache_retention: true,
        }
    }

    fn repro_model(base_url: &str) -> Model {
        Model {
            id: "repro-model".to_string(),
            api: "openai-completions".to_string(),
            provider: "repro-provider".to_string(),
            base_url: base_url.to_string(),
            input: vec![ModelInput::Text],
            reasoning: true,
            thinking_level_map: HashMap::new(),
            headers: ProviderHeaders::new(),
            compat: None,
        }
    }

    fn repro_assistant(content: Vec<ContentBlock>) -> AssistantMessage {
        AssistantMessage {
            api: "openai-completions".to_string(),
            provider: "repro-provider".to_string(),
            model: "repro-model".to_string(),
            content,
            stop_reason: StopReason::Stop,
        }
    }

    fn repro_context(assistant: AssistantMessage) -> Context {
        Context {
            messages: vec![
                Message::User {
                    content: UserMessageContent::Text("hello".to_string()),
                },
                Message::Assistant(assistant),
                Message::User {
                    content: UserMessageContent::Text("continue".to_string()),
                },
            ],
            system_prompt: None,
            tools: Vec::new(),
        }
    }

    #[test]
    fn serializes_same_model_thinking_plus_text_replay_as_assistant_text_parts() {
        let messages = convert_messages(
            &repro_model("http://127.0.0.1:1"),
            &repro_context(repro_assistant(vec![
                ContentBlock::Thinking {
                    thinking: "internal reasoning".to_string(),
                    thinking_signature: None,
                    redacted: false,
                },
                ContentBlock::Text {
                    text: "visible answer".to_string(),
                },
            ])),
            &thinking_as_text_compat(),
        );

        match &messages[1] {
            ChatCompletionMessage::Assistant {
                content: Some(AssistantChatContent::Parts(parts)),
                tool_calls: None,
                extra,
            } => {
                assert_eq!(
                    parts,
                    &vec![
                        ChatCompletionTextPart {
                            kind: "text".to_string(),
                            text: "internal reasoning".to_string(),
                        },
                        ChatCompletionTextPart {
                            kind: "text".to_string(),
                            text: "visible answer".to_string(),
                        },
                    ]
                );
                assert!(extra.is_empty());
            }
            other => panic!("unexpected assistant replay: {other:?}"),
        }
    }

    #[test]
    fn serializes_same_model_thinking_only_replay_as_assistant_text_parts() {
        let messages = convert_messages(
            &repro_model("http://127.0.0.1:1"),
            &repro_context(repro_assistant(vec![ContentBlock::Thinking {
                thinking: "internal reasoning".to_string(),
                thinking_signature: None,
                redacted: false,
            }])),
            &thinking_as_text_compat(),
        );

        match &messages[1] {
            ChatCompletionMessage::Assistant {
                content: Some(AssistantChatContent::Parts(parts)),
                tool_calls: None,
                extra,
            } => {
                assert_eq!(
                    parts,
                    &vec![ChatCompletionTextPart {
                        kind: "text".to_string(),
                        text: "internal reasoning".to_string(),
                    }]
                );
                assert!(extra.is_empty());
            }
            other => panic!("unexpected assistant replay: {other:?}"),
        }
    }

    #[test]
    fn reaches_endpoint_when_replay_contains_both_thinking_and_text() {
        let stream = stream(
            &repro_model("http://127.0.0.1:1"),
            &repro_context(repro_assistant(vec![
                ContentBlock::Thinking {
                    thinking: "internal reasoning".to_string(),
                    thinking_signature: None,
                    redacted: false,
                },
                ContentBlock::Text {
                    text: "visible answer".to_string(),
                },
            ])),
            Some(&OpenAICompletionsOptions {
                api_key: Some("test".to_string()),
                ..OpenAICompletionsOptions::default()
            }),
        )
        .expect("request should build");

        assert_eq!(stream.request.base_url, "http://127.0.0.1:1");
        assert!(stream.request.body["messages"][1]["content"].is_array());
    }

    #[test]
    fn convert_messages_adds_developer_system_and_normalizes_tool_ids() {
        let mut model = model();
        model.reasoning = true;
        let compat = ResolvedOpenAICompletionsCompat {
            supports_developer_role: true,
            ..detect_compat(&model)
        };
        let context = Context {
            system_prompt: Some("rules".to_string()),
            messages: vec![Message::Assistant(AssistantMessage {
                api: "other".to_string(),
                provider: "other".to_string(),
                model: "other".to_string(),
                content: vec![ContentBlock::ToolCall(ToolCall {
                    id: "call.id|huge-response-id".to_string(),
                    name: "lookup".to_string(),
                    arguments: serde_json::json!({"q":"x"}),
                    thought_signature: Some("secret".to_string()),
                })],
                stop_reason: StopReason::ToolUse,
            })],
            tools: Vec::new(),
        };

        let messages = convert_messages(&model, &context, &compat);
        assert_eq!(messages[0].role(), "developer");
        match &messages[1] {
            ChatCompletionMessage::Assistant {
                tool_calls: Some(tool_calls),
                ..
            } => {
                assert_eq!(tool_calls[0].id, "call_id");
                assert_eq!(tool_calls[0].function.name, "lookup");
            }
            other => panic!("unexpected message: {other:?}"),
        }
        match &messages[2] {
            ChatCompletionMessage::Tool {
                content,
                tool_call_id,
                ..
            } => {
                assert_eq!(content, "No result provided");
                assert_eq!(tool_call_id, "call_id");
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[test]
    fn convert_messages_replays_tool_call_thought_signatures_as_reasoning_details() {
        let mut model = model();
        model.id = "google/gemini-test".to_string();
        model.provider = "openrouter".to_string();
        model.base_url = "https://openrouter.ai/api/v1".to_string();
        model.reasoning = true;
        let reasoning_detail = serde_json::json!({"type":"reasoning.encrypted","id":"call_1","data":"encrypted-signature"});
        let context = Context {
            messages: vec![Message::Assistant(AssistantMessage {
                api: "openai-completions".to_string(),
                provider: "openrouter".to_string(),
                model: "google/gemini-test".to_string(),
                content: vec![ContentBlock::ToolCall(ToolCall {
                    id: "call_1".to_string(),
                    name: "read".to_string(),
                    arguments: serde_json::json!({"path":"README.md"}),
                    thought_signature: Some(reasoning_detail.to_string()),
                })],
                stop_reason: StopReason::ToolUse,
            })],
            ..Context::default()
        };

        let messages = convert_messages(&model, &context, &get_compat(&model));

        match &messages[0] {
            ChatCompletionMessage::Assistant {
                tool_calls: Some(tool_calls),
                extra,
                ..
            } => {
                assert_eq!(tool_calls[0].id, "call_1");
                assert_eq!(tool_calls[0].function.name, "read");
                assert_eq!(tool_calls[0].function.arguments, r#"{"path":"README.md"}"#);
                assert_eq!(
                    extra.get("reasoning_details"),
                    Some(&Value::Array(vec![reasoning_detail]))
                );
            }
            other => panic!("unexpected message: {other:?}"),
        }
    }

    #[test]
    fn preserves_reasoning_details_on_tool_call_replay_request() {
        let mut model = model();
        model.provider = "openrouter".to_string();
        model.base_url = "https://openrouter.ai/api/v1".to_string();
        let reasoning_detail = serde_json::json!({"type":"reasoning.encrypted","id":"call_1","data":"encrypted-signature"});
        let context = Context {
            messages: vec![Message::Assistant(AssistantMessage {
                api: "openai-completions".to_string(),
                provider: "openrouter".to_string(),
                model: model.id.clone(),
                content: vec![ContentBlock::ToolCall(ToolCall {
                    id: "call_1".to_string(),
                    name: "read".to_string(),
                    arguments: serde_json::json!({"path":"README.md"}),
                    thought_signature: Some(reasoning_detail.to_string()),
                })],
                stop_reason: StopReason::ToolUse,
            })],
            ..Context::default()
        };
        let request = build_request(
            &model,
            &context,
            Some(&OpenAICompletionsOptions {
                api_key: Some("test".to_string()),
                ..OpenAICompletionsOptions::default()
            }),
        )
        .expect("request should build");

        assert_eq!(
            request.body["messages"][0]["reasoning_details"],
            Value::Array(vec![reasoning_detail])
        );
    }
}
