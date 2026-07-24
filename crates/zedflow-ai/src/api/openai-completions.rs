//! OpenAI-compatible Chat Completions API ported from Pi.

use std::collections::{HashMap, HashSet};
use std::error::Error as StdError;
use std::fmt;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::github_copilot_headers::{
    CopilotDynamicHeadersParams, Message as CopilotMessage, MessageContent as CopilotContent,
    UserMessageContent as CopilotUserContent, build_copilot_dynamic_headers,
    has_copilot_vision_input,
};

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
#[derive(Default)]
pub enum CacheRetention {
    /// Disable prompt caching.
    None,
    /// Use provider short retention.
    #[default]
    Short,
    /// Use provider long retention.
    Long,
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
    /// Whether z.ai's `tool_stream` flag should be enabled when tools are present.
    pub zai_tool_stream: Option<bool>,
    /// Static chat-template kwargs merged into provider-specific thinking kwargs.
    pub chat_template_kwargs: Option<Value>,
    /// Chat-template kwargs key used for reasoning effort values.
    pub chat_template_effort_key: Option<String>,
    /// Chat-template kwargs key used for boolean thinking toggles.
    pub chat_template_bool_key: Option<String>,
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
    /// Whether z.ai's `tool_stream` flag should be enabled when tools are present.
    pub zai_tool_stream: bool,
    /// Static chat-template kwargs merged into provider-specific thinking kwargs.
    pub chat_template_kwargs: Option<Value>,
    /// Chat-template kwargs key used for reasoning effort values.
    pub chat_template_effort_key: Option<String>,
    /// Chat-template kwargs key used for boolean thinking toggles.
    pub chat_template_bool_key: String,
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
    /// Default output-token cap used by Pi simple options.
    pub max_tokens: u32,
    /// Model context window used to clamp output tokens.
    pub context_window: Option<u32>,
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

/// Provider HTTP response metadata exposed to response hooks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderResponse {
    /// HTTP status code.
    pub status: u16,
    /// HTTP response headers.
    pub headers: HashMap<String, String>,
}

/// Payload hook used by the OpenAI-compatible transport.
pub type OpenAICompletionsPayloadHook = Arc<
    dyn Fn(
            Value,
            Model,
        ) -> futures::future::BoxFuture<
            'static,
            std::result::Result<Option<Value>, crate::types::ProviderHookError>,
        > + Send
        + Sync,
>;

/// Response hook used by the OpenAI-compatible transport.
pub type OpenAICompletionsResponseHook = Arc<
    dyn Fn(
            ProviderResponse,
            Model,
        ) -> futures::future::BoxFuture<
            'static,
            std::result::Result<(), crate::types::ProviderHookError>,
        > + Send
        + Sync,
>;

/// Options specific to Pi's OpenAI Completions stream implementation.
#[derive(Clone, Default)]
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
    /// Cancellation signal.
    pub signal: Option<crate::types::AbortSignal>,
    /// Optional custom HTTP headers.
    pub headers: ProviderHeaders,
    /// Provider-scoped environment values.
    pub env: ProviderEnv,
    /// Optional callback for inspecting or replacing the JSON payload before it is sent.
    pub on_payload: Option<OpenAICompletionsPayloadHook>,
    /// Optional callback invoked after the HTTP response is received.
    pub on_response: Option<OpenAICompletionsResponseHook>,
    /// Tool choice behavior.
    pub tool_choice: Option<OpenAIToolChoice>,
    /// Reasoning effort requested by the caller.
    pub reasoning_effort: Option<ReasoningEffort>,
}

impl fmt::Debug for OpenAICompletionsOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenAICompletionsOptions")
            .field("temperature", &self.temperature)
            .field("max_tokens", &self.max_tokens)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("cache_retention", &self.cache_retention)
            .field("session_id", &self.session_id)
            .field("timeout_ms", &self.timeout_ms)
            .field("max_retries", &self.max_retries)
            .field("signal", &self.signal)
            .field("headers", &self.headers)
            .field("env", &self.env)
            .field("on_payload", &self.on_payload.as_ref().map(|_| "<hook>"))
            .field("on_response", &self.on_response.as_ref().map(|_| "<hook>"))
            .field("tool_choice", &self.tool_choice)
            .field("reasoning_effort", &self.reasoning_effort)
            .finish()
    }
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
    /// Pi OpenAI SDK constructor options captured for tests.
    pub client_options: Value,
}

/// Pi's event-stream handle for OpenAI-compatible completions.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OpenAICompletionsStream {
    /// Request captured before provider I/O starts; deterministic tests assert Pi parity here.
    pub request: OpenAICompletionsRequest,
}

/// Usage accumulated from OpenAI-compatible stream chunks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenAICompletionsStreamUsage {
    /// Non-cached/non-write input tokens.
    pub input: u64,
    /// Output tokens, including reasoning tokens when providers report them in completion tokens.
    pub output: u64,
    /// Prompt-cache read tokens.
    pub cache_read: u64,
    /// Prompt-cache write tokens.
    pub cache_write: u64,
    /// Reasoning tokens included in output tokens.
    pub reasoning: u64,
    /// Pi-computed total tokens.
    pub total_tokens: u64,
}

/// Assistant message reconstructed from deterministic OpenAI-compatible stream chunks.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenAICompletionsStreamMessage {
    /// Requested model id.
    pub model: String,
    /// Concrete routed model surfaced by providers such as OpenRouter.
    pub response_model: Option<String>,
    /// Provider id.
    pub provider: String,
    /// Provider response id.
    pub response_id: Option<String>,
    /// Final content blocks.
    pub content: Vec<ContentBlock>,
    /// Final stop reason.
    pub stop_reason: StopReason,
    /// Error text for error terminal results.
    pub error_message: Option<String>,
    /// Token usage.
    pub usage: OpenAICompletionsStreamUsage,
}

/// Event emitted while reconstructing OpenAI-compatible stream chunks.
#[derive(Debug, Clone, PartialEq)]
pub enum OpenAICompletionsStreamEvent {
    /// Text block started.
    TextStart { content_index: usize },
    /// Text delta.
    TextDelta { content_index: usize, delta: String },
    /// Text block ended.
    TextEnd {
        content_index: usize,
        content: String,
    },
    /// Thinking block started.
    ThinkingStart { content_index: usize },
    /// Thinking delta.
    ThinkingDelta { content_index: usize, delta: String },
    /// Thinking block ended.
    ThinkingEnd {
        content_index: usize,
        content: String,
    },
    /// Tool-call block started.
    ToolCallStart { content_index: usize },
    /// Tool-call argument delta.
    ToolCallDelta { content_index: usize, delta: String },
    /// Tool-call block ended.
    ToolCallEnd {
        content_index: usize,
        tool_call: ToolCall,
    },
    /// Terminal event.
    Done {
        message: OpenAICompletionsStreamMessage,
    },
}

/// Deterministic result of processing OpenAI-compatible stream chunks.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenAICompletionsStreamResult {
    /// Events in Pi emission order.
    pub events: Vec<OpenAICompletionsStreamEvent>,
    /// Final assistant message.
    pub message: OpenAICompletionsStreamMessage,
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
    let headers = build_client_headers(model, context, options, &compat, cache_retention);
    let base_url = resolve_base_url(model, options);
    let request_options = build_request_options(options);
    Ok(OpenAICompletionsRequest {
        client_options: build_client_options(
            &base_url,
            &headers,
            options,
            request_options.max_retries,
        ),
        base_url,
        headers,
        body: build_params_value(model, context, options, &compat, cache_retention)?,
        timeout_ms: options.and_then(|options| options.timeout_ms),
        max_retries: request_options.max_retries,
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

/// Starts a live OpenAI-compatible Chat Completions stream over HTTP/SSE.
pub fn stream_live(
    model: &Model,
    context: &Context,
    options: Option<&OpenAICompletionsOptions>,
) -> Result<crate::types::AssistantMessageEventStream> {
    let stream = crate::types::AssistantMessageEventStream::new();
    let worker_stream = stream.clone();
    let model = model.clone();
    let context = context.clone();
    let options = options.cloned().unwrap_or_default();
    let identity = crate::utils::runtime::StreamIdentity::new(
        model.api.clone(),
        model.provider.clone(),
        model.id.clone(),
    );
    crate::utils::runtime::spawn_stream_worker(stream.clone(), identity, async move {
        run_openai_completions_live_worker(worker_stream, model, context, options, None).await;
    });
    Ok(stream)
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

#[derive(Debug)]
struct OpenAICompletionsLiveError {
    message: String,
    partial: Option<OpenAICompletionsStreamMessage>,
}

impl OpenAICompletionsLiveError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            partial: None,
        }
    }

    fn with_partial(message: impl Into<String>, partial: &OpenAICompletionsStreamState) -> Self {
        Self {
            message: message.into(),
            partial: Some(partial.message.clone()),
        }
    }
}

async fn run_openai_completions_live_worker(
    stream: crate::types::AssistantMessageEventStream,
    model: Model,
    context: Context,
    options: OpenAICompletionsOptions,
    cost: Option<crate::types::ModelCost>,
) {
    let result = async {
        let api_key = get_client_api_key(
            &model.provider,
            options.api_key.as_deref(),
            Some(&options.headers),
        )
        .map_err(|error| OpenAICompletionsLiveError::new(error.to_string()))?;
        let mut request = build_request(&model, &context, Some(&options))
            .map_err(|error| OpenAICompletionsLiveError::new(error.to_string()))?;
        if let Some(on_payload) = options.on_payload.as_ref()
            && let Some(next_payload) = on_payload(request.body.clone(), model.clone())
                .await
                .map_err(|error| OpenAICompletionsLiveError::new(error.to_string()))?
        {
            request.body = next_payload;
        }
        execute_openai_completions_live(
            &stream,
            &model,
            &request,
            &api_key,
            &options,
            cost.as_ref(),
        )
        .await
    }
    .await;
    if let Err(error) = result {
        let aborted = is_openai_abort_error(&error.message);
        let mut output = error.partial.as_ref().map_or_else(
            || empty_canonical_message_for_completions(&model),
            |partial| canonical_message_from_completions(partial, &model, cost.as_ref()),
        );
        output.stop_reason = if aborted {
            crate::types::StopReason::Aborted
        } else {
            crate::types::StopReason::Error
        };
        output.error_message = Some(error.message);
        stream.push(crate::types::AssistantMessageEvent::Error {
            reason: if aborted {
                crate::types::ErrorStopReason::Aborted
            } else {
                crate::types::ErrorStopReason::Error
            },
            error: output,
        });
    }
}

async fn execute_openai_completions_live(
    stream: &crate::types::AssistantMessageEventStream,
    model: &Model,
    request: &OpenAICompletionsRequest,
    api_key: &str,
    options: &OpenAICompletionsOptions,
    cost: Option<&crate::types::ModelCost>,
) -> std::result::Result<(), OpenAICompletionsLiveError> {
    check_openai_abort(options.signal.as_ref()).map_err(OpenAICompletionsLiveError::new)?;
    let client =
        build_openai_http_client(request.timeout_ms).map_err(OpenAICompletionsLiveError::new)?;
    let headers = openai_completions_headers(api_key, &request.headers)
        .map_err(OpenAICompletionsLiveError::new)?;
    let body = serde_json::to_vec(&request.body)
        .map_err(|error| OpenAICompletionsLiveError::new(error.to_string()))?;
    let mut attempts = 0;
    let response = loop {
        let response = await_openai_or_abort(
            client
                .post(openai_completions_url(&request.base_url))
                .headers(headers.clone())
                .body(body.clone())
                .send(),
            options.signal.clone(),
        )
        .await;
        match response {
            Ok(response) => {
                if let Some(on_response) = options.on_response.as_ref() {
                    on_response(
                        provider_response_from_headers(
                            response.status().as_u16(),
                            response.headers(),
                        ),
                        model.clone(),
                    )
                    .await
                    .map_err(|error| OpenAICompletionsLiveError::new(error.to_string()))?;
                }
                if is_retryable_openai_status(response.status().as_u16())
                    && attempts < request.max_retries
                {
                    let delay = crate::utils::retry::retry_after_delay(
                        response.headers(),
                        std::time::SystemTime::now(),
                    )
                    .unwrap_or_else(|| {
                        crate::utils::retry::retry_delay(
                            Duration::from_millis(500),
                            attempts,
                            Some(Duration::from_secs(8)),
                        )
                    });
                    if !crate::utils::retry::wait_or_abort(delay, options.signal.as_ref()).await {
                        return Err(OpenAICompletionsLiveError::new("Request was aborted"));
                    }
                    attempts += 1;
                    continue;
                }
                break response;
            }
            Err(error) if attempts < request.max_retries && !is_openai_abort_error(&error) => {
                let delay = crate::utils::retry::retry_delay(
                    Duration::from_millis(500),
                    attempts,
                    Some(Duration::from_secs(8)),
                );
                if !crate::utils::retry::wait_or_abort(delay, options.signal.as_ref()).await {
                    return Err(OpenAICompletionsLiveError::new("Request was aborted"));
                }
                attempts += 1;
            }
            Err(error) => return Err(OpenAICompletionsLiveError::new(error)),
        }
    };
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = await_openai_or_abort(response.text(), options.signal.clone())
            .await
            .map_err(OpenAICompletionsLiveError::new)?;
        return Err(OpenAICompletionsLiveError::new(format_openai_http_error(
            status, &body, None,
        )));
    }

    let mut state = OpenAICompletionsStreamState::new(model);
    stream.push(crate::types::AssistantMessageEvent::Start {
        partial: canonical_message_from_completions(&state.message, model, cost).into(),
    });
    let mut decoder = OpenAICompletionsSseDecoder::default();
    let mut bytes = response.bytes_stream();
    loop {
        let next = next_openai_bytes_or_abort(&mut bytes, options.signal.clone())
            .await
            .map_err(|error| OpenAICompletionsLiveError::with_partial(error, &state))?;
        let Some(bytes) = next else {
            return Err(OpenAICompletionsLiveError::with_partial(
                "OpenAI stream ended before [DONE]",
                &state,
            ));
        };
        let frames = decoder
            .push(&bytes)
            .map_err(|error| OpenAICompletionsLiveError::with_partial(error, &state))?;
        for frame in frames {
            check_openai_abort(options.signal.as_ref())
                .map_err(|error| OpenAICompletionsLiveError::with_partial(error, &state))?;
            if frame == "[DONE]" {
                let result = state.finish();
                push_canonical_completions_events(stream, &result, model, cost);
                return Ok(());
            }
            let chunk = serde_json::from_str::<Value>(&frame).map_err(|error| {
                OpenAICompletionsLiveError::with_partial(
                    format!("OpenAI stream JSON error: {error}"),
                    &state,
                )
            })?;
            state.apply_chunk(model, &chunk);
            push_pending_canonical_completions_events(stream, &mut state, model, cost);
            if state
                .finish_reason
                .as_deref()
                .is_some_and(|reason| map_openai_completions_finish_reason(reason).is_err())
            {
                let result = state.finish();
                push_canonical_completions_events(stream, &result, model, cost);
                return Ok(());
            }
            tokio::task::yield_now().await;
            check_openai_abort(options.signal.as_ref())
                .map_err(|error| OpenAICompletionsLiveError::with_partial(error, &state))?;
        }
    }
}

fn build_openai_http_client(
    timeout_ms: Option<u64>,
) -> std::result::Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder();
    if let Some(timeout_ms) = timeout_ms {
        builder = builder.timeout(Duration::from_millis(timeout_ms));
    }
    builder.build().map_err(|error| error.to_string())
}

async fn await_openai_or_abort<T>(
    future: impl std::future::Future<Output = std::result::Result<T, reqwest::Error>>,
    signal: Option<crate::types::AbortSignal>,
) -> std::result::Result<T, String> {
    if let Some(signal) = signal {
        match futures::future::select(Box::pin(future), Box::pin(wait_openai_abort(signal))).await {
            futures::future::Either::Left((result, _)) => result.map_err(|error| error.to_string()),
            futures::future::Either::Right(((), _)) => Err("Request was aborted".to_owned()),
        }
    } else {
        future.await.map_err(|error| error.to_string())
    }
}

async fn wait_openai_abort(signal: crate::types::AbortSignal) {
    signal.cancelled().await;
}

fn check_openai_abort(
    signal: Option<&crate::types::AbortSignal>,
) -> std::result::Result<(), String> {
    if signal.is_some_and(crate::types::AbortSignal::aborted) {
        Err("Request was aborted".to_owned())
    } else {
        Ok(())
    }
}

fn is_openai_abort_error(error: &str) -> bool {
    error == "Request was aborted"
}

fn is_retryable_openai_status(status: u16) -> bool {
    matches!(status, 408 | 409 | 429 | 500..=599)
}

fn openai_completions_headers(
    api_key: &str,
    headers: &ProviderHeaders,
) -> std::result::Result<HeaderMap, String> {
    let mut map = HeaderMap::new();
    map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    if !has_header(Some(headers), "authorization")
        && !has_header(Some(headers), "cf-aig-authorization")
    {
        map.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {api_key}"))
                .map_err(|error| error.to_string())?,
        );
    }
    for (name, value) in headers {
        map.insert(
            HeaderName::from_bytes(name.as_bytes()).map_err(|error| error.to_string())?,
            HeaderValue::from_str(value).map_err(|error| error.to_string())?,
        );
    }
    Ok(map)
}

fn openai_completions_url(base_url: &str) -> String {
    format!("{}/chat/completions", base_url.trim_end_matches('/'))
}

async fn next_openai_bytes_or_abort<S, B>(
    stream: &mut S,
    signal: Option<crate::types::AbortSignal>,
) -> std::result::Result<Option<B>, String>
where
    S: futures::Stream<Item = std::result::Result<B, reqwest::Error>> + Unpin,
{
    if let Some(signal) = signal {
        match futures::future::select(Box::pin(stream.next()), Box::pin(wait_openai_abort(signal)))
            .await
        {
            futures::future::Either::Left((Some(Ok(bytes)), _)) => Ok(Some(bytes)),
            futures::future::Either::Left((Some(Err(error)), _)) => Err(error.to_string()),
            futures::future::Either::Left((None, _)) => Ok(None),
            futures::future::Either::Right(((), _)) => Err("Request was aborted".to_owned()),
        }
    } else {
        stream
            .next()
            .await
            .transpose()
            .map_err(|error| error.to_string())
    }
}

#[derive(Default)]
struct OpenAICompletionsSseDecoder {
    pending: Vec<u8>,
}

impl OpenAICompletionsSseDecoder {
    fn push(&mut self, bytes: &[u8]) -> std::result::Result<Vec<String>, String> {
        self.pending.extend_from_slice(bytes);
        let mut frames = Vec::new();
        while let Some((end, delimiter)) = openai_sse_delimiter(&self.pending) {
            let event = self.pending.drain(..end).collect::<Vec<_>>();
            self.pending.drain(..delimiter);
            if let Some(data) = openai_sse_data(&event)? {
                frames.push(data);
            }
        }
        Ok(frames)
    }
}

fn openai_sse_delimiter(bytes: &[u8]) -> Option<(usize, usize)> {
    bytes
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|index| (index, 2))
        .into_iter()
        .chain(
            bytes
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|index| (index, 4)),
        )
        .min_by_key(|(index, _)| *index)
}

fn openai_sse_data(event: &[u8]) -> std::result::Result<Option<String>, String> {
    let event = std::str::from_utf8(event)
        .map_err(|error| format!("invalid UTF-8 in OpenAI SSE: {error}"))?;
    let data = event
        .lines()
        .filter_map(|line| line.strip_prefix("data:").map(str::trim))
        .collect::<Vec<_>>();
    Ok((!data.is_empty()).then(|| data.join("\n")))
}

fn provider_response_from_headers(
    status: u16,
    headers: &reqwest::header::HeaderMap,
) -> ProviderResponse {
    ProviderResponse {
        status,
        headers: headers
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.to_string(), value.to_string()))
            })
            .collect(),
    }
}

fn format_openai_http_error(status: u16, body: &str, prefix: Option<&str>) -> String {
    match prefix {
        Some(prefix) => format!("{prefix} ({status}): {body}"),
        None => format!("OpenAI API error ({status}): {body}"),
    }
}

fn push_pending_canonical_completions_events(
    stream: &crate::types::AssistantMessageEventStream,
    state: &mut OpenAICompletionsStreamState,
    model: &Model,
    cost: Option<&crate::types::ModelCost>,
) {
    if state.events.is_empty() {
        return;
    }
    let result = OpenAICompletionsStreamResult {
        events: std::mem::take(&mut state.events),
        message: state.message.clone(),
    };
    push_canonical_completions_events(stream, &result, model, cost);
}

fn push_canonical_completions_events(
    stream: &crate::types::AssistantMessageEventStream,
    result: &OpenAICompletionsStreamResult,
    model: &Model,
    cost: Option<&crate::types::ModelCost>,
) {
    let message = canonical_message_from_completions(&result.message, model, cost);
    for event in &result.events {
        match event {
            OpenAICompletionsStreamEvent::TextStart { content_index } => {
                stream.push(crate::types::AssistantMessageEvent::TextStart {
                    content_index: *content_index,
                    partial: message.clone().into(),
                });
            }
            OpenAICompletionsStreamEvent::TextDelta {
                content_index,
                delta,
            } => {
                stream.push(crate::types::AssistantMessageEvent::TextDelta {
                    content_index: *content_index,
                    delta: delta.clone(),
                    partial: message.clone().into(),
                });
            }
            OpenAICompletionsStreamEvent::TextEnd {
                content_index,
                content,
            } => {
                stream.push(crate::types::AssistantMessageEvent::TextEnd {
                    content_index: *content_index,
                    content: content.clone(),
                    partial: message.clone().into(),
                });
            }
            OpenAICompletionsStreamEvent::ThinkingStart { content_index } => {
                stream.push(crate::types::AssistantMessageEvent::ThinkingStart {
                    content_index: *content_index,
                    partial: message.clone().into(),
                });
            }
            OpenAICompletionsStreamEvent::ThinkingDelta {
                content_index,
                delta,
            } => {
                stream.push(crate::types::AssistantMessageEvent::ThinkingDelta {
                    content_index: *content_index,
                    delta: delta.clone(),
                    partial: message.clone().into(),
                });
            }
            OpenAICompletionsStreamEvent::ThinkingEnd {
                content_index,
                content,
            } => {
                stream.push(crate::types::AssistantMessageEvent::ThinkingEnd {
                    content_index: *content_index,
                    content: content.clone(),
                    partial: message.clone().into(),
                });
            }
            OpenAICompletionsStreamEvent::ToolCallStart { content_index } => {
                stream.push(crate::types::AssistantMessageEvent::ToolcallStart {
                    content_index: *content_index,
                    partial: message.clone().into(),
                });
            }
            OpenAICompletionsStreamEvent::ToolCallDelta {
                content_index,
                delta,
            } => {
                stream.push(crate::types::AssistantMessageEvent::ToolcallDelta {
                    content_index: *content_index,
                    delta: delta.clone(),
                    partial: message.clone().into(),
                });
            }
            OpenAICompletionsStreamEvent::ToolCallEnd {
                content_index,
                tool_call,
            } => {
                stream.push(crate::types::AssistantMessageEvent::ToolcallEnd {
                    content_index: *content_index,
                    tool_call: canonical_tool_call_from_completions(tool_call),
                    partial: message.clone().into(),
                });
            }
            OpenAICompletionsStreamEvent::Done { message: done } => {
                let done = canonical_message_from_completions(done, model, cost);
                if done.stop_reason == crate::types::StopReason::Error {
                    stream.push(crate::types::AssistantMessageEvent::Error {
                        reason: crate::types::ErrorStopReason::Error,
                        error: done,
                    });
                } else {
                    stream.push(crate::types::AssistantMessageEvent::Done {
                        reason: canonical_done_reason(done.stop_reason),
                        message: done,
                    });
                }
            }
        }
    }
}

fn canonical_message_from_completions(
    message: &OpenAICompletionsStreamMessage,
    model: &Model,
    cost: Option<&crate::types::ModelCost>,
) -> crate::types::AssistantMessage {
    let mut usage = crate::types::Usage {
        input: message.usage.input,
        output: message.usage.output,
        cache_read: message.usage.cache_read,
        cache_write: message.usage.cache_write,
        reasoning: Some(message.usage.reasoning),
        total_tokens: message.usage.total_tokens,
        ..crate::types::Usage::default()
    };
    if let Some(cost) = cost {
        usage.cost = openai_completions_cost(cost, &usage);
    }
    crate::types::AssistantMessage {
        role: crate::types::AssistantMessageRole::Assistant,
        content: message
            .content
            .iter()
            .map(canonical_content_from_completions)
            .collect(),
        api: model.api.clone(),
        provider: message.provider.clone(),
        model: message.model.clone(),
        response_model: message.response_model.clone(),
        response_id: message.response_id.clone(),
        diagnostics: None,
        usage,
        stop_reason: canonical_stop_reason(message.stop_reason),
        error_message: message.error_message.clone(),
        timestamp: unix_timestamp_ms(),
    }
}

fn openai_completions_cost(
    cost: &crate::types::ModelCost,
    usage: &crate::types::Usage,
) -> crate::types::UsageCost {
    let input = cost.input * usage.input as f64 / 1_000_000.0;
    let output = cost.output * usage.output as f64 / 1_000_000.0;
    let cache_read = cost.cache_read * usage.cache_read as f64 / 1_000_000.0;
    let cache_write = cost.cache_write * usage.cache_write as f64 / 1_000_000.0;
    crate::types::UsageCost {
        input,
        output,
        cache_read,
        cache_write,
        total: input + output + cache_read + cache_write,
    }
}

fn empty_canonical_message_for_completions(model: &Model) -> crate::types::AssistantMessage {
    crate::types::AssistantMessage {
        role: crate::types::AssistantMessageRole::Assistant,
        content: Vec::new(),
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: crate::types::Usage::default(),
        stop_reason: crate::types::StopReason::Stop,
        error_message: None,
        timestamp: unix_timestamp_ms(),
    }
}

fn canonical_content_from_completions(block: &ContentBlock) -> crate::types::AssistantContentBlock {
    match block {
        ContentBlock::Text { text } => {
            crate::types::AssistantContentBlock::Text(crate::types::TextContent {
                content_type: crate::types::TextContentType::Text,
                text: text.clone(),
                text_signature: None,
            })
        }
        ContentBlock::Thinking {
            thinking,
            thinking_signature,
            redacted,
        } => crate::types::AssistantContentBlock::Thinking(crate::types::ThinkingContent {
            content_type: crate::types::ThinkingContentType::Thinking,
            thinking: thinking.clone(),
            thinking_signature: thinking_signature.clone(),
            redacted: Some(*redacted),
        }),
        ContentBlock::ToolCall(tool_call) => crate::types::AssistantContentBlock::ToolCall(
            canonical_tool_call_from_completions(tool_call),
        ),
        ContentBlock::Image { .. } => {
            crate::types::AssistantContentBlock::Text(crate::types::TextContent {
                content_type: crate::types::TextContentType::Text,
                text: String::new(),
                text_signature: None,
            })
        }
    }
}

fn canonical_tool_call_from_completions(tool_call: &ToolCall) -> crate::types::ToolCall {
    crate::types::ToolCall {
        content_type: crate::types::ToolCallType::ToolCall,
        id: tool_call.id.clone(),
        name: tool_call.name.clone(),
        arguments: tool_call
            .arguments
            .as_object()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect(),
        thought_signature: tool_call.thought_signature.clone(),
    }
}

fn canonical_stop_reason(reason: StopReason) -> crate::types::StopReason {
    match reason {
        StopReason::Stop => crate::types::StopReason::Stop,
        StopReason::Length => crate::types::StopReason::Length,
        StopReason::ToolUse => crate::types::StopReason::ToolUse,
        StopReason::Aborted => crate::types::StopReason::Aborted,
        StopReason::Error => crate::types::StopReason::Error,
    }
}

fn canonical_done_reason(reason: crate::types::StopReason) -> crate::types::DoneStopReason {
    match reason {
        crate::types::StopReason::Length => crate::types::DoneStopReason::Length,
        crate::types::StopReason::ToolUse => crate::types::DoneStopReason::ToolUse,
        _ => crate::types::DoneStopReason::Stop,
    }
}

fn unix_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Processes OpenAI-compatible streaming chunks into Pi-ordered assistant events.
///
/// This is the deterministic core used by fixture tests and mirrors the observable chunk handling
/// from Pi's OpenAI SDK stream loop: null chunks are ignored, content/reasoning/tool deltas are
/// independent, tool-call identity is pinned to the first stable stream index/id, and a terminal
/// `finish_reason` is required.
#[must_use]
pub fn process_openai_completions_stream_chunks<I>(
    model: &Model,
    chunks: I,
) -> OpenAICompletionsStreamResult
where
    I: IntoIterator<Item = Option<Value>>,
{
    let mut state = OpenAICompletionsStreamState::new(model);
    for chunk in chunks.into_iter().flatten() {
        state.apply_chunk(model, &chunk);
    }
    state.finish()
}

#[derive(Debug, Clone)]
struct OpenAICompletionsStreamToolCall {
    content_index: usize,
    tool_call: ToolCall,
    partial_arguments: String,
}

#[derive(Debug, Clone)]
struct OpenAICompletionsStreamState {
    message: OpenAICompletionsStreamMessage,
    events: Vec<OpenAICompletionsStreamEvent>,
    text_index: Option<usize>,
    thinking_index: Option<usize>,
    tool_calls: HashMap<String, OpenAICompletionsStreamToolCall>,
    tool_order: Vec<String>,
    pending_reasoning_details: HashMap<String, String>,
    finish_reason: Option<String>,
}

impl OpenAICompletionsStreamState {
    fn new(model: &Model) -> Self {
        Self {
            message: OpenAICompletionsStreamMessage {
                model: model.id.clone(),
                response_model: None,
                provider: model.provider.clone(),
                response_id: None,
                content: Vec::new(),
                stop_reason: StopReason::Stop,
                error_message: None,
                usage: OpenAICompletionsStreamUsage::default(),
            },
            events: Vec::new(),
            text_index: None,
            thinking_index: None,
            tool_calls: HashMap::new(),
            tool_order: Vec::new(),
            pending_reasoning_details: HashMap::new(),
            finish_reason: None,
        }
    }

    fn apply_chunk(&mut self, model: &Model, chunk: &Value) {
        if self.message.response_id.is_none() {
            self.message.response_id = chunk.get("id").and_then(Value::as_str).map(str::to_owned);
        }
        if self.message.response_model.is_none()
            && let Some(response_model) = chunk.get("model").and_then(Value::as_str)
            && !response_model.is_empty()
            && response_model != model.id
        {
            self.message.response_model = Some(response_model.to_owned());
        }
        if let Some(usage) = chunk.get("usage") {
            self.message.usage = parse_openai_completions_usage(usage);
        }
        for choice in chunk
            .get("choices")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(usage) = choice.get("usage") {
                self.message.usage = parse_openai_completions_usage(usage);
            }
            let delta = choice.get("delta").unwrap_or(&Value::Null);
            self.apply_text_delta(
                delta
                    .get("content")
                    .and_then(Value::as_str)
                    .filter(|text| !text.is_empty()),
            );
            for field in ["reasoning_content", "reasoning", "reasoning_text"] {
                if delta
                    .get(field)
                    .and_then(Value::as_str)
                    .is_some_and(|text| !text.is_empty())
                {
                    self.apply_reasoning_delta(model, delta, field);
                    break;
                }
            }
            if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
                for tool_call in tool_calls {
                    self.apply_tool_call_delta(tool_call);
                }
            }
            if let Some(details) = delta.get("reasoning_details").and_then(Value::as_array) {
                for detail in details {
                    self.apply_reasoning_detail(detail);
                }
            }
            if let Some(reason) = choice.get("finish_reason").and_then(Value::as_str) {
                self.finish_reason = Some(reason.to_owned());
            }
        }
    }

    fn apply_text_delta(&mut self, delta: Option<&str>) {
        let Some(delta) = delta else { return };
        let content_index = match self.text_index {
            Some(index) => index,
            None => {
                self.message.content.push(ContentBlock::Text {
                    text: String::new(),
                });
                let index = self.message.content.len() - 1;
                self.text_index = Some(index);
                self.events.push(OpenAICompletionsStreamEvent::TextStart {
                    content_index: index,
                });
                index
            }
        };
        if let Some(ContentBlock::Text { text }) = self.message.content.get_mut(content_index) {
            text.push_str(delta);
        }
        self.events.push(OpenAICompletionsStreamEvent::TextDelta {
            content_index,
            delta: delta.to_owned(),
        });
    }

    fn apply_reasoning_delta(&mut self, model: &Model, delta: &Value, field: &str) {
        let Some(reasoning_delta) = delta.get(field).and_then(Value::as_str) else {
            return;
        };
        let signature = if model.provider == "opencode-go" && field == "reasoning" {
            "reasoning_content"
        } else {
            field
        };
        let content_index = match self.thinking_index {
            Some(index) => index,
            None => {
                self.message.content.push(ContentBlock::Thinking {
                    thinking: String::new(),
                    thinking_signature: Some(signature.to_owned()),
                    redacted: false,
                });
                let index = self.message.content.len() - 1;
                self.thinking_index = Some(index);
                self.events
                    .push(OpenAICompletionsStreamEvent::ThinkingStart {
                        content_index: index,
                    });
                index
            }
        };
        if let Some(ContentBlock::Thinking { thinking, .. }) =
            self.message.content.get_mut(content_index)
        {
            thinking.push_str(reasoning_delta);
        }
        self.events
            .push(OpenAICompletionsStreamEvent::ThinkingDelta {
                content_index,
                delta: reasoning_delta.to_owned(),
            });
    }

    fn apply_tool_call_delta(&mut self, delta: &Value) {
        let key = delta
            .get("index")
            .and_then(Value::as_u64)
            .map(|index| format!("index:{index}"))
            .or_else(|| {
                delta
                    .get("id")
                    .and_then(Value::as_str)
                    .map(|id| format!("id:{id}"))
            })
            .unwrap_or_else(|| format!("ordinal:{}", self.tool_order.len()));
        if !self.tool_calls.contains_key(&key) {
            let function = delta.get("function").unwrap_or(&Value::Null);
            let tool_call = ToolCall {
                id: delta
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                name: function
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                arguments: json!({}),
                thought_signature: None,
            };
            self.message
                .content
                .push(ContentBlock::ToolCall(tool_call.clone()));
            let content_index = self.message.content.len() - 1;
            self.events
                .push(OpenAICompletionsStreamEvent::ToolCallStart { content_index });
            self.tool_calls.insert(
                key.clone(),
                OpenAICompletionsStreamToolCall {
                    content_index,
                    tool_call,
                    partial_arguments: String::new(),
                },
            );
            self.tool_order.push(key.clone());
        }
        let Some(call) = self.tool_calls.get_mut(&key) else {
            return;
        };
        let function = delta.get("function").unwrap_or(&Value::Null);
        if call.tool_call.id.is_empty()
            && let Some(id) = delta.get("id").and_then(Value::as_str)
        {
            call.tool_call.id = id.to_owned();
        }
        if call.tool_call.name.is_empty()
            && let Some(name) = function.get("name").and_then(Value::as_str)
        {
            call.tool_call.name = name.to_owned();
        }
        if let Some(signature) = self.pending_reasoning_details.remove(&call.tool_call.id) {
            call.tool_call.thought_signature = Some(signature);
        }
        if let Some(arguments) = function.get("arguments").and_then(Value::as_str) {
            call.partial_arguments.push_str(arguments);
            call.tool_call.arguments = parse_partial_json_object(&call.partial_arguments);
            if let Some(ContentBlock::ToolCall(block)) =
                self.message.content.get_mut(call.content_index)
            {
                *block = call.tool_call.clone();
            }
            self.events
                .push(OpenAICompletionsStreamEvent::ToolCallDelta {
                    content_index: call.content_index,
                    delta: arguments.to_owned(),
                });
        }
    }

    fn apply_reasoning_detail(&mut self, detail: &Value) {
        if detail.get("type").and_then(Value::as_str) != Some("reasoning.encrypted") {
            return;
        }
        let Some(id) = detail
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
        else {
            return;
        };
        if detail
            .get("data")
            .and_then(Value::as_str)
            .is_none_or(|data| data.is_empty())
        {
            return;
        }
        let serialized = detail.to_string();
        if let Some(call) = self
            .tool_calls
            .values_mut()
            .find(|call| call.tool_call.id == id)
        {
            call.tool_call.thought_signature = Some(serialized);
            if let Some(ContentBlock::ToolCall(block)) =
                self.message.content.get_mut(call.content_index)
            {
                block.thought_signature = call.tool_call.thought_signature.clone();
            }
        } else {
            self.pending_reasoning_details
                .insert(id.to_owned(), serialized);
        }
    }

    fn finish(mut self) -> OpenAICompletionsStreamResult {
        if let Some(index) = self.text_index
            && let Some(ContentBlock::Text { text }) = self.message.content.get(index)
        {
            self.events.push(OpenAICompletionsStreamEvent::TextEnd {
                content_index: index,
                content: text.clone(),
            });
        }
        if let Some(index) = self.thinking_index
            && let Some(ContentBlock::Thinking { thinking, .. }) = self.message.content.get(index)
        {
            self.events.push(OpenAICompletionsStreamEvent::ThinkingEnd {
                content_index: index,
                content: thinking.clone(),
            });
        }
        for key in &self.tool_order {
            if let Some(call) = self.tool_calls.get(key) {
                self.events.push(OpenAICompletionsStreamEvent::ToolCallEnd {
                    content_index: call.content_index,
                    tool_call: call.tool_call.clone(),
                });
            }
        }
        if let Some(reason) = self.finish_reason.as_deref() {
            match map_openai_completions_finish_reason(reason) {
                Ok(stop_reason) => self.message.stop_reason = stop_reason,
                Err(message) => {
                    self.message.stop_reason = StopReason::Error;
                    self.message.error_message = Some(message);
                }
            }
        } else {
            self.message.stop_reason = StopReason::Error;
            self.message.error_message = Some("Stream ended without finish_reason".to_owned());
        }
        if self.message.stop_reason == StopReason::Stop
            && self
                .message
                .content
                .iter()
                .any(|block| matches!(block, ContentBlock::ToolCall(_)))
        {
            self.message.stop_reason = StopReason::ToolUse;
        }
        self.events.push(OpenAICompletionsStreamEvent::Done {
            message: self.message.clone(),
        });
        OpenAICompletionsStreamResult {
            events: self.events,
            message: self.message,
        }
    }
}

fn parse_partial_json_object(text: &str) -> Value {
    if text.trim().is_empty() {
        return json!({});
    }
    serde_json::from_str(text).unwrap_or_else(|_| json!({}))
}

fn parse_openai_completions_usage(value: &Value) -> OpenAICompletionsStreamUsage {
    let prompt_tokens = value
        .get("prompt_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let completion_tokens = value
        .get("completion_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let prompt_details = value.get("prompt_tokens_details").unwrap_or(&Value::Null);
    let cache_read = prompt_details
        .get("cached_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let cache_write = prompt_details
        .get("cache_write_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let reasoning = value
        .pointer("/completion_tokens_details/reasoning_tokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let input = prompt_tokens.saturating_sub(cache_read + cache_write);
    OpenAICompletionsStreamUsage {
        input,
        output: completion_tokens,
        cache_read,
        cache_write,
        reasoning,
        total_tokens: input
            .saturating_add(completion_tokens)
            .saturating_add(cache_read)
            .saturating_add(cache_write),
    }
}

fn map_openai_completions_finish_reason(reason: &str) -> std::result::Result<StopReason, String> {
    match reason {
        "stop" | "end" => Ok(StopReason::Stop),
        "length" => Ok(StopReason::Length),
        "tool_calls" | "function_call" => Ok(StopReason::ToolUse),
        other => Err(format!("Provider finish_reason: {other}")),
    }
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
    super::openai_prompt_cache::clamp_openai_prompt_cache_key(key)
}

fn build_client_headers(
    model: &Model,
    context: &Context,
    options: Option<&OpenAICompletionsOptions>,
    compat: &ResolvedOpenAICompletionsCompat,
    cache_retention: CacheRetention,
) -> ProviderHeaders {
    let mut headers = model.headers.clone();
    if let Some(cf_token) = options
        .and_then(|options| options.env.get("CLOUDFLARE_API_KEY"))
        .filter(|value| !value.is_empty())
        && model.provider == "cloudflare-ai-gateway"
    {
        headers.insert(
            "cf-aig-authorization".to_string(),
            format!("Bearer {cf_token}"),
        );
    }
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

fn resolve_base_url(model: &Model, options: Option<&OpenAICompletionsOptions>) -> String {
    if model.provider == "cloudflare-ai-gateway"
        && let Some(env) = options.map(|options| &options.env)
        && let (Some(account_id), Some(gateway_id)) = (
            env.get("CLOUDFLARE_ACCOUNT_ID")
                .filter(|value| !value.is_empty()),
            env.get("CLOUDFLARE_GATEWAY_ID")
                .filter(|value| !value.is_empty()),
        )
    {
        return format!("https://gateway.ai.cloudflare.com/v1/{account_id}/{gateway_id}/compat");
    }
    model.base_url.clone()
}

fn build_client_options(
    base_url: &str,
    headers: &ProviderHeaders,
    options: Option<&OpenAICompletionsOptions>,
    max_retries: u32,
) -> Value {
    let mut default_headers = serde_json::Map::new();
    for (name, value) in headers {
        default_headers.insert(name.clone(), Value::String(value.clone()));
    }
    if headers.contains_key("cf-aig-authorization") && !has_header(Some(headers), "Authorization") {
        default_headers.insert("Authorization".to_string(), Value::Null);
    }
    json!({
        "baseURL": base_url,
        "defaultHeaders": default_headers,
        "timeout": options.and_then(|options| options.timeout_ms),
        "maxRetries": max_retries,
    })
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
    let max_tokens = options
        .and_then(|options| options.max_tokens)
        .unwrap_or(model.max_tokens);
    if max_tokens > 0 {
        let field = match compat.max_tokens_field {
            MaxTokensField::MaxCompletionTokens => "max_completion_tokens",
            MaxTokensField::MaxTokens => "max_tokens",
        };
        object.insert(
            field.to_string(),
            json!(clamp_max_tokens(model, context, max_tokens)),
        );
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
        if compat.zai_tool_stream && !tools.is_empty() {
            object.insert("tool_stream".to_string(), Value::Bool(true));
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

fn clamp_max_tokens(model: &Model, context: &Context, max_tokens: u32) -> u32 {
    const CONTEXT_SAFETY_TOKENS: i64 = 4096;
    const CHARS_PER_TOKEN: usize = 4;
    let Some(context_window) = model.context_window else {
        return max_tokens;
    };
    let input_chars: usize = context.messages.iter().map(message_char_len).sum::<usize>()
        + context.system_prompt.as_ref().map_or(0, String::len);
    let input_tokens = input_chars.div_ceil(CHARS_PER_TOKEN);
    let available = i64::from(context_window)
        - i64::try_from(input_tokens).unwrap_or(i64::MAX)
        - CONTEXT_SAFETY_TOKENS;
    max_tokens.min(u32::try_from(available.max(1)).unwrap_or(u32::MAX))
}

fn message_char_len(message: &Message) -> usize {
    match message {
        Message::User { content } => match content {
            UserMessageContent::Text(text) => text.len(),
            UserMessageContent::Parts(parts) => parts.iter().map(content_block_char_len).sum(),
        },
        Message::Assistant(assistant) => assistant.content.iter().map(content_block_char_len).sum(),
        Message::ToolResult(tool_result) => {
            tool_result.content.iter().map(content_block_char_len).sum()
        }
    }
}

fn content_block_char_len(block: &ContentBlock) -> usize {
    match block {
        ContentBlock::Text { text } => text.len(),
        ContentBlock::Thinking { thinking, .. } => thinking.len(),
        ContentBlock::ToolCall(tool_call) => {
            serde_json::to_string(tool_call).map_or(0, |text| text.len())
        }
        ContentBlock::Image { .. } => 4800,
    }
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
        ThinkingFormat::Zai => {
            if let Some(effort) = effort {
                object.insert(
                    "thinking".to_string(),
                    json!({ "type": "enabled", "clear_thinking": false }),
                );
                if compat.supports_reasoning_effort {
                    object.insert("reasoning_effort".to_string(), Value::String(effort));
                }
            } else {
                object.insert("thinking".to_string(), json!({ "type": "disabled" }));
            }
        }
        ThinkingFormat::ChatTemplate | ThinkingFormat::QwenChatTemplate => {
            if let Some(effort) = effort {
                let mut kwargs = compat
                    .chat_template_kwargs
                    .as_ref()
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                if let Some(key) = &compat.chat_template_effort_key {
                    kwargs.insert(key.clone(), Value::String(effort));
                } else {
                    kwargs.insert(compat.chat_template_bool_key.clone(), Value::Bool(true));
                }
                object.insert("chat_template_kwargs".to_string(), Value::Object(kwargs));
            }
        }
        ThinkingFormat::AntLing => {
            if let Some(effort) = effort {
                object.insert(
                    "reasoning".to_string(),
                    json!({ "enable": true, "effort": effort }),
                );
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
        || provider == "opencode"
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
        send_session_affinity_headers: is_cloudflare_ai_gateway,
        supports_long_cache_retention: !(is_together
            || is_cloudflare_workers_ai
            || is_cloudflare_ai_gateway
            || is_nvidia
            || is_ant_ling),
        zai_tool_stream: is_zai
            && matches!(model.id.as_str(), "glm-5.1" | "glm-4.7" | "glm-5-turbo"),
        chat_template_kwargs: None,
        chat_template_effort_key: None,
        chat_template_bool_key: "enable_thinking".to_owned(),
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
        zai_tool_stream: compat.zai_tool_stream.unwrap_or(detected.zai_tool_stream),
        chat_template_kwargs: compat
            .chat_template_kwargs
            .clone()
            .or(detected.chat_template_kwargs),
        chat_template_effort_key: compat
            .chat_template_effort_key
            .clone()
            .or(detected.chat_template_effort_key),
        chat_template_bool_key: compat
            .chat_template_bool_key
            .clone()
            .unwrap_or(detected.chat_template_bool_key),
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

/// Returns the canonical OpenAI Completions production streams.
#[must_use]
pub fn provider_streams() -> crate::types::ProviderStreams {
    crate::types::ProviderStreams {
        stream: Arc::new(stream_registered),
        stream_simple: Arc::new(stream_simple_registered),
    }
}

/// Starts the canonical OpenAI Completions production stream.
#[must_use]
pub fn stream_registered(
    model: &crate::types::Model,
    context: &crate::types::Context,
    options: Option<&crate::types::StreamOptions>,
) -> crate::types::AssistantMessageEventStream {
    let stream = crate::types::AssistantMessageEventStream::new();
    let worker_stream = stream.clone();
    let context = crate::api::transform_messages::transform_context(context, model, None);
    let local_model = registered_model(model);
    let local_context = registered_context(&context);
    let local_options = registered_options(model, options);
    let cost = model.cost.clone();
    let identity = crate::utils::runtime::StreamIdentity::new(
        model.api.clone(),
        model.provider.clone(),
        model.id.clone(),
    );
    crate::utils::runtime::spawn_stream_worker(stream.clone(), identity, async move {
        run_openai_completions_live_worker(
            worker_stream,
            local_model,
            local_context,
            local_options,
            Some(cost),
        )
        .await;
    });
    stream
}

/// Starts the canonical simple OpenAI Completions stream.
#[must_use]
pub fn stream_simple_registered(
    model: &crate::types::Model,
    context: &crate::types::Context,
    options: Option<&crate::types::SimpleStreamOptions>,
) -> crate::types::AssistantMessageEventStream {
    let mut stream_options = options
        .map(|options| options.stream.clone())
        .unwrap_or_default();
    if let Some(reasoning) = options.and_then(|options| options.reasoning) {
        stream_options.extra.insert(
            "reasoning".to_owned(),
            Value::String(
                match reasoning {
                    crate::types::ThinkingLevel::Minimal => "minimal",
                    crate::types::ThinkingLevel::Low => "low",
                    crate::types::ThinkingLevel::Medium => "medium",
                    crate::types::ThinkingLevel::High => "high",
                    crate::types::ThinkingLevel::XHigh => "xhigh",
                }
                .to_owned(),
            ),
        );
    }
    stream_registered(model, context, Some(&stream_options))
}

fn registered_model(model: &crate::types::Model) -> Model {
    Model {
        id: model.id.clone(),
        api: model.api.clone(),
        provider: model.provider.clone(),
        base_url: model.base_url.clone(),
        input: model
            .input
            .iter()
            .map(|input| match input {
                crate::types::ModelInput::Text => ModelInput::Text,
                crate::types::ModelInput::Image => ModelInput::Image,
            })
            .collect(),
        reasoning: model.reasoning,
        thinking_level_map: model
            .thinking_level_map
            .as_ref()
            .map(|map| {
                map.iter()
                    .map(|(level, value)| {
                        (
                            match level {
                                crate::types::ModelThinkingLevel::Off => ModelThinkingLevel::Off,
                                crate::types::ModelThinkingLevel::Minimal => {
                                    ModelThinkingLevel::Minimal
                                }
                                crate::types::ModelThinkingLevel::Low => ModelThinkingLevel::Low,
                                crate::types::ModelThinkingLevel::Medium => {
                                    ModelThinkingLevel::Medium
                                }
                                crate::types::ModelThinkingLevel::High => ModelThinkingLevel::High,
                                crate::types::ModelThinkingLevel::XHigh => {
                                    ModelThinkingLevel::XHigh
                                }
                            },
                            value.clone(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        headers: model.headers.clone().unwrap_or_default(),
        max_tokens: u32::try_from(model.max_tokens).unwrap_or(u32::MAX),
        context_window: (model.context_window > 0)
            .then(|| u32::try_from(model.context_window).unwrap_or(u32::MAX)),
        compat: model.compat.as_ref().and_then(|compat| match compat {
            crate::types::ModelCompat::OpenAICompletions(compat) => Some(registered_compat(compat)),
            _ => None,
        }),
    }
}

fn registered_compat(compat: &crate::types::OpenAICompletionsCompat) -> OpenAICompletionsCompat {
    OpenAICompletionsCompat {
        supports_store: compat.supports_store,
        supports_developer_role: compat.supports_developer_role,
        supports_reasoning_effort: compat.supports_reasoning_effort,
        supports_usage_in_streaming: compat.supports_usage_in_streaming,
        max_tokens_field: compat.max_tokens_field.map(|field| match field {
            crate::types::MaxTokensField::MaxCompletionTokens => {
                MaxTokensField::MaxCompletionTokens
            }
            crate::types::MaxTokensField::MaxTokens => MaxTokensField::MaxTokens,
        }),
        requires_tool_result_name: compat.requires_tool_result_name,
        requires_assistant_after_tool_result: compat.requires_assistant_after_tool_result,
        requires_thinking_as_text: compat.requires_thinking_as_text,
        requires_reasoning_content_on_assistant_messages: compat
            .requires_reasoning_content_on_assistant_messages,
        thinking_format: compat.thinking_format.map(|format| match format {
            crate::types::ThinkingFormat::OpenAI => ThinkingFormat::OpenAI,
            crate::types::ThinkingFormat::OpenRouter => ThinkingFormat::OpenRouter,
            crate::types::ThinkingFormat::Deepseek => ThinkingFormat::DeepSeek,
            crate::types::ThinkingFormat::Together => ThinkingFormat::Together,
            crate::types::ThinkingFormat::Zai => ThinkingFormat::Zai,
            crate::types::ThinkingFormat::Qwen => ThinkingFormat::Qwen,
            crate::types::ThinkingFormat::ChatTemplate => ThinkingFormat::ChatTemplate,
            crate::types::ThinkingFormat::QwenChatTemplate => ThinkingFormat::QwenChatTemplate,
            crate::types::ThinkingFormat::StringThinking => ThinkingFormat::StringThinking,
            crate::types::ThinkingFormat::AntLing => ThinkingFormat::AntLing,
        }),
        supports_strict_mode: compat.supports_strict_mode,
        cache_control_format: compat.cache_control_format.map(|format| match format {
            crate::types::CacheControlFormat::Anthropic => CacheControlFormat::Anthropic,
        }),
        send_session_affinity_headers: compat.send_session_affinity_headers,
        supports_long_cache_retention: compat.supports_long_cache_retention,
        zai_tool_stream: compat.zai_tool_stream,
        chat_template_kwargs: compat
            .chat_template_kwargs
            .as_ref()
            .and_then(|kwargs| serde_json::to_value(kwargs).ok()),
        chat_template_effort_key: None,
        chat_template_bool_key: None,
    }
}

fn registered_context(context: &crate::types::Context) -> Context {
    Context {
        messages: context.messages.iter().map(registered_message).collect(),
        system_prompt: context.system_prompt.clone(),
        tools: context
            .tools
            .as_ref()
            .into_iter()
            .flatten()
            .map(|tool| Tool {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            })
            .collect(),
    }
}

fn registered_message(message: &crate::types::Message) -> Message {
    match message {
        crate::types::Message::User(message) => Message::User {
            content: match &message.content {
                crate::types::UserMessageContent::Text(text) => {
                    UserMessageContent::Text(text.clone())
                }
                crate::types::UserMessageContent::Blocks(blocks) => UserMessageContent::Parts(
                    blocks
                        .iter()
                        .map(|block| match block {
                            crate::types::UserContentBlock::Text(text) => ContentBlock::Text {
                                text: text.text.clone(),
                            },
                            crate::types::UserContentBlock::Image(image) => ContentBlock::Image {
                                data: image.data.clone(),
                                mime_type: image.mime_type.clone(),
                            },
                        })
                        .collect(),
                ),
            },
        },
        crate::types::Message::Assistant(message) => Message::Assistant(AssistantMessage {
            api: message.api.clone(),
            provider: message.provider.clone(),
            model: message.model.clone(),
            content: message
                .content
                .iter()
                .map(|block| match block {
                    crate::types::AssistantContentBlock::Text(text) => ContentBlock::Text {
                        text: text.text.clone(),
                    },
                    crate::types::AssistantContentBlock::Thinking(thinking) => {
                        ContentBlock::Thinking {
                            thinking: thinking.thinking.clone(),
                            thinking_signature: thinking.thinking_signature.clone(),
                            redacted: thinking.redacted.unwrap_or(false),
                        }
                    }
                    crate::types::AssistantContentBlock::ToolCall(call) => {
                        ContentBlock::ToolCall(ToolCall {
                            id: call.id.clone(),
                            name: call.name.clone(),
                            arguments: Value::Object(call.arguments.clone().into_iter().collect()),
                            thought_signature: call.thought_signature.clone(),
                        })
                    }
                })
                .collect(),
            stop_reason: registered_stop_reason(message.stop_reason),
        }),
        crate::types::Message::ToolResult(message) => Message::ToolResult(ToolResultMessage {
            tool_call_id: message.tool_call_id.clone(),
            tool_name: Some(message.tool_name.clone()),
            content: message
                .content
                .iter()
                .map(|block| match block {
                    crate::types::ToolResultContentBlock::Text(text) => ContentBlock::Text {
                        text: text.text.clone(),
                    },
                    crate::types::ToolResultContentBlock::Image(image) => ContentBlock::Image {
                        data: image.data.clone(),
                        mime_type: image.mime_type.clone(),
                    },
                })
                .collect(),
        }),
    }
}

fn registered_stop_reason(reason: crate::types::StopReason) -> StopReason {
    match reason {
        crate::types::StopReason::Stop => StopReason::Stop,
        crate::types::StopReason::Length => StopReason::Length,
        crate::types::StopReason::ToolUse => StopReason::ToolUse,
        crate::types::StopReason::Aborted => StopReason::Aborted,
        crate::types::StopReason::Error => StopReason::Error,
    }
}

fn registered_options(
    model: &crate::types::Model,
    options: Option<&crate::types::StreamOptions>,
) -> OpenAICompletionsOptions {
    let options = options.cloned().unwrap_or_default();
    let payload_model = model.clone();
    let response_model = model.clone();
    OpenAICompletionsOptions {
        temperature: options.temperature,
        max_tokens: options.max_tokens,
        api_key: options.api_key,
        cache_retention: options.cache_retention.map(|retention| match retention {
            crate::types::CacheRetention::None => CacheRetention::None,
            crate::types::CacheRetention::Short => CacheRetention::Short,
            crate::types::CacheRetention::Long => CacheRetention::Long,
        }),
        session_id: options.session_id,
        timeout_ms: options.timeout_ms,
        max_retries: options.max_retries,
        signal: options.signal,
        headers: options
            .headers
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(name, value)| value.map(|value| (name, value)))
            .collect(),
        env: options.env.unwrap_or_default(),
        on_payload: options.on_payload.map(|hook| {
            Arc::new(move |payload, _| hook(payload, payload_model.clone()))
                as OpenAICompletionsPayloadHook
        }),
        on_response: options.on_response.map(|hook| {
            Arc::new(move |response: ProviderResponse, _: Model| {
                hook(
                    crate::types::ProviderResponse {
                        status: response.status,
                        headers: response.headers,
                    },
                    response_model.clone(),
                )
            }) as OpenAICompletionsResponseHook
        }),
        tool_choice: options
            .extra
            .get("toolChoice")
            .and_then(|choice| serde_json::from_value(choice.clone()).ok()),
        reasoning_effort: options.extra.get("reasoning").and_then(|reasoning| {
            match reasoning.as_str()? {
                "minimal" => Some(ReasoningEffort::Minimal),
                "low" => Some(ReasoningEffort::Low),
                "medium" => Some(ReasoningEffort::Medium),
                "high" => Some(ReasoningEffort::High),
                "xhigh" => Some(ReasoningEffort::XHigh),
                _ => None,
            }
        }),
    }
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
            max_tokens: 4096,
            context_window: None,
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
            zai_tool_stream: false,
            chat_template_kwargs: None,
            chat_template_effort_key: None,
            chat_template_bool_key: "enable_thinking".to_owned(),
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
            zai_tool_stream: false,
            chat_template_kwargs: None,
            chat_template_effort_key: None,
            chat_template_bool_key: "enable_thinking".to_owned(),
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
            max_tokens: 4096,
            context_window: None,
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
        assert_eq!(
            stream.request.body["messages"][1]["content"],
            "visible answer"
        );
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
