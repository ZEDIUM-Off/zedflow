//! Mistral Conversations API ported from Pi.

use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;
use std::io::{BufRead, BufReader, Read};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;

use crate::utils::json_parse::parse_streaming_json_value;

const MISTRAL_TOOL_CALL_ID_LENGTH: usize = 9;
const MAX_MISTRAL_ERROR_BODY_CHARS: usize = 4000;

/// Result type for the Mistral Conversations port.
pub type Result<T> = std::result::Result<T, MistralConversationsError>;

/// Errors returned by the Mistral Conversations port.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MistralConversationsError {
    /// No API key was supplied for the model provider.
    MissingApiKey {
        /// Provider identifier from Pi.
        provider: String,
    },
    /// The HTTP request failed before a provider response was available.
    Http(String),
}

impl fmt::Display for MistralConversationsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApiKey { provider } => write!(f, "no API key for provider: {provider}"),
            Self::Http(message) => write!(f, "{message}"),
        }
    }
}

impl StdError for MistralConversationsError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::MissingApiKey { .. } | Self::Http(_) => None,
        }
    }
}

/// HTTP headers supplied by a model or request options.
pub type ProviderHeaders = HashMap<String, String>;

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

/// Mistral reasoning effort values accepted by Pi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MistralReasoningEffort {
    /// Disable reasoning effort.
    None,
    /// Request high reasoning effort.
    High,
}

impl MistralReasoningEffort {
    /// Returns the Mistral API string for this reasoning effort.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::High => "high",
        }
    }
}

/// Pi thinking level map key, including the `off` sentinel.
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

impl From<ThinkingLevel> for ModelThinkingLevel {
    fn from(value: ThinkingLevel) -> Self {
        match value {
            ThinkingLevel::Minimal => Self::Minimal,
            ThinkingLevel::Low => Self::Low,
            ThinkingLevel::Medium => Self::Medium,
            ThinkingLevel::High => Self::High,
            ThinkingLevel::XHigh => Self::XHigh,
        }
    }
}

/// Mistral tool choice behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MistralToolChoice {
    /// Let Mistral choose whether to call a tool.
    Auto,
    /// Disable tool use.
    None,
    /// Allow any tool use.
    Any,
    /// Require tool use.
    Required,
    /// Force a specific function tool by name.
    Function {
        /// Function tool selector.
        function: MistralToolChoiceFunction,
    },
}

impl Serialize for MistralToolChoice {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Auto => serializer.serialize_str("auto"),
            Self::None => serializer.serialize_str("none"),
            Self::Any => serializer.serialize_str("any"),
            Self::Required => serializer.serialize_str("required"),
            Self::Function { function } => {
                let mut state = serializer.serialize_struct("MistralToolChoice", 2)?;
                state.serialize_field("type", "function")?;
                state.serialize_field("function", function)?;
                state.end()
            }
        }
    }
}

/// Function selector for [`MistralToolChoice`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MistralToolChoiceFunction {
    /// Function name to force.
    pub name: String,
}

/// Prompt cache retention preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheRetention {
    /// Disable prompt caching.
    None,
    /// Use provider short retention.
    Short,
    /// Use provider long retention.
    Long,
}

/// Model input modality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelInput {
    /// Text input.
    Text,
    /// Image input.
    Image,
}

/// Minimal model shape consumed by this port row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    /// Model identifier sent to Mistral.
    pub id: String,
    /// API identifier from Pi.
    pub api: String,
    /// Provider identifier from Pi.
    pub provider: String,
    /// Optional provider base URL.
    pub base_url: Option<String>,
    /// Input modalities supported by the model.
    pub input: Vec<ModelInput>,
    /// Whether the model supports reasoning options.
    pub reasoning: bool,
    /// Provider/model-specific mappings for Pi thinking levels.
    pub thinking_level_map: HashMap<ModelThinkingLevel, MistralReasoningEffort>,
    /// Default headers configured on the model.
    pub headers: ProviderHeaders,
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

/// Pi assistant message produced by the Mistral stream.
#[derive(Debug, Clone, PartialEq)]
pub struct AssistantMessage {
    /// Assistant role.
    pub role: String,
    /// Assistant content blocks.
    pub content: Vec<AssistantContent>,
    /// API identifier.
    pub api: String,
    /// Provider identifier.
    pub provider: String,
    /// Requested model identifier.
    pub model: String,
    /// Provider response id.
    pub response_id: Option<String>,
    /// Token usage.
    pub usage: Usage,
    /// Stop reason.
    pub stop_reason: StopReason,
    /// Error message on failure.
    pub error_message: Option<String>,
    /// Unix timestamp in milliseconds.
    pub timestamp: u64,
}

/// Token usage and cost counters.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Usage {
    /// Billable input tokens after cache read subtraction.
    pub input: u64,
    /// Output tokens.
    pub output: u64,
    /// Cache-read prompt tokens.
    pub cache_read: u64,
    /// Cache-write tokens.
    pub cache_write: u64,
    /// Provider total tokens.
    pub total_tokens: u64,
}

/// Event emitted while decoding Mistral SSE chunks.
#[derive(Debug, Clone, PartialEq)]
pub enum AssistantMessageEvent {
    /// Stream started.
    Start { partial: AssistantMessage },
    /// Text block started.
    TextStart {
        content_index: usize,
        partial: AssistantMessage,
    },
    /// Text delta.
    TextDelta {
        content_index: usize,
        delta: String,
        partial: AssistantMessage,
    },
    /// Text block ended.
    TextEnd {
        content_index: usize,
        content: String,
        partial: AssistantMessage,
    },
    /// Thinking block started.
    ThinkingStart {
        content_index: usize,
        partial: AssistantMessage,
    },
    /// Thinking delta.
    ThinkingDelta {
        content_index: usize,
        delta: String,
        partial: AssistantMessage,
    },
    /// Thinking block ended.
    ThinkingEnd {
        content_index: usize,
        content: String,
        partial: AssistantMessage,
    },
    /// Tool call block started.
    ToolcallStart {
        content_index: usize,
        partial: AssistantMessage,
    },
    /// Tool call argument delta.
    ToolcallDelta {
        content_index: usize,
        delta: String,
        partial: AssistantMessage,
    },
    /// Tool call block ended.
    ToolcallEnd {
        content_index: usize,
        tool_call: AssistantContent,
        partial: AssistantMessage,
    },
    /// Stream completed successfully.
    Done {
        reason: StopReason,
        message: AssistantMessage,
    },
    /// Stream failed.
    Error {
        reason: StopReason,
        error: AssistantMessage,
    },
}

#[derive(Debug, Default)]
struct AssistantMessageEventStreamState {
    events: Vec<AssistantMessageEvent>,
    result: Option<AssistantMessage>,
    done: bool,
}

/// Pi-style assistant event stream backed by a narrow blocking reqwest transport.
#[derive(Debug, Clone, Default)]
pub struct AssistantMessageEventStream {
    inner: Arc<(Mutex<AssistantMessageEventStreamState>, Condvar)>,
}

impl AssistantMessageEventStream {
    /// Creates an empty stream.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Pushes an event into the stream.
    pub fn push(&self, event: AssistantMessageEvent) {
        let (state, condvar) = &*self.inner;
        let mut state = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.done {
            return;
        }
        if let AssistantMessageEvent::Done { message, .. } = &event {
            state.result = Some(message.clone());
            state.done = true;
        } else if let AssistantMessageEvent::Error { error, .. } = &event {
            state.result = Some(error.clone());
            state.done = true;
        }
        state.events.push(event);
        condvar.notify_all();
    }

    /// Returns all events collected so far.
    #[must_use]
    pub fn events(&self) -> Vec<AssistantMessageEvent> {
        let (state, _) = &*self.inner;
        state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .events
            .clone()
    }

    /// Blocks until a terminal event arrives or the timeout elapses.
    #[must_use]
    pub fn wait_result(&self, timeout: Duration) -> Option<AssistantMessage> {
        let (state, condvar) = &*self.inner;
        let state = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (state, _) = condvar
            .wait_timeout_while(state, timeout, |state| !state.done)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.result.clone()
    }
}

/// Options specific to Pi's Mistral Conversations stream implementation.
#[derive(Clone, Default)]
pub struct MistralOptions {
    /// Sampling temperature.
    pub temperature: Option<f64>,
    /// Maximum output tokens requested by the caller.
    pub max_tokens: Option<u32>,
    /// API key for Mistral.
    pub api_key: Option<String>,
    /// Optional session identifier used for prompt caching.
    pub session_id: Option<String>,
    /// Prompt cache retention preference.
    pub cache_retention: Option<CacheRetention>,
    /// Optional custom HTTP headers.
    pub headers: ProviderHeaders,
    /// Optional HTTP request timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Optional callback for inspecting or replacing the JSON payload before it is sent.
    pub on_payload: Option<MistralPayloadHook>,
    /// Mistral tool choice behavior.
    pub tool_choice: Option<MistralToolChoice>,
    /// Mistral prompt mode.
    pub prompt_mode: Option<String>,
    /// Mistral reasoning effort.
    pub reasoning_effort: Option<MistralReasoningEffort>,
}

/// Payload hook used by this narrow blocking transport.
pub type MistralPayloadHook = Arc<dyn Fn(Value, &Model) -> Option<Value> + Send + Sync>;

impl fmt::Debug for MistralOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MistralOptions")
            .field("temperature", &self.temperature)
            .field("max_tokens", &self.max_tokens)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("session_id", &self.session_id)
            .field("cache_retention", &self.cache_retention)
            .field("headers", &self.headers)
            .field("timeout_ms", &self.timeout_ms)
            .field("on_payload", &self.on_payload.as_ref().map(|_| "<hook>"))
            .field("tool_choice", &self.tool_choice)
            .field("prompt_mode", &self.prompt_mode)
            .field("reasoning_effort", &self.reasoning_effort)
            .finish()
    }
}

/// Options accepted by [`stream_simple`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SimpleStreamOptions {
    /// Sampling temperature.
    pub temperature: Option<f64>,
    /// Maximum output tokens requested by the caller.
    pub max_tokens: Option<u32>,
    /// API key for Mistral.
    pub api_key: Option<String>,
    /// Optional session identifier used for prompt caching.
    pub session_id: Option<String>,
    /// Prompt cache retention preference.
    pub cache_retention: Option<CacheRetention>,
    /// Optional custom HTTP headers.
    pub headers: ProviderHeaders,
    /// Optional HTTP request timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Unified reasoning level passed to simple streams.
    pub reasoning: Option<ThinkingLevel>,
}

/// Pi assistant message content block.
#[derive(Debug, Clone, PartialEq)]
pub enum AssistantContent {
    /// Text block.
    Text {
        /// Text payload.
        text: String,
    },
    /// Thinking block.
    Thinking {
        /// Thinking payload.
        thinking: String,
    },
    /// Tool call block.
    ToolCall {
        /// Tool call identifier.
        id: String,
        /// Function name.
        name: String,
        /// Function arguments.
        arguments: Value,
    },
}

/// Pi user message content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserContent {
    /// Plain text content.
    Text(String),
    /// Structured text/image content.
    Parts(Vec<UserContentPart>),
}

/// Pi text or image content part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserContentPart {
    /// Text content part.
    Text {
        /// Text payload.
        text: String,
    },
    /// Image content part.
    Image {
        /// Base64 image bytes.
        data: String,
        /// Image MIME type.
        mime_type: String,
    },
}

/// Minimal Pi message shape consumed by Mistral payload conversion.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// User-authored message.
    User {
        /// User content.
        content: UserContent,
    },
    /// Assistant-authored message.
    Assistant {
        /// Assistant content blocks.
        content: Vec<AssistantContent>,
    },
    /// Tool result message.
    ToolResult {
        /// Tool call identifier.
        tool_call_id: String,
        /// Tool name.
        tool_name: String,
        /// Tool result content.
        content: Vec<UserContentPart>,
        /// Whether this result represents a tool error.
        is_error: bool,
    },
}

/// Pi tool declaration consumed by Mistral payload conversion.
#[derive(Debug, Clone, PartialEq)]
pub struct Tool {
    /// Tool name.
    pub name: String,
    /// Tool description.
    pub description: String,
    /// Tool parameters schema.
    pub parameters: Value,
}

/// Mistral chat completion stream request payload.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatCompletionStreamRequest {
    /// Model identifier.
    pub model: String,
    /// Stream flag; Pi always sets this to true.
    pub stream: bool,
    /// Conversation messages.
    pub messages: Vec<ChatCompletionStreamRequestMessage>,
    /// Function tools available to the model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<FunctionTool>>,
    /// Sampling temperature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Maximum output tokens.
    #[serde(rename = "maxTokens", skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Tool choice behavior.
    #[serde(rename = "toolChoice", skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<MistralToolChoice>,
    /// Mistral prompt mode.
    #[serde(rename = "promptMode", skip_serializing_if = "Option::is_none")]
    pub prompt_mode: Option<String>,
    /// Mistral reasoning effort.
    #[serde(rename = "reasoningEffort", skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<MistralReasoningEffort>,
    /// Mistral prompt cache key.
    #[serde(rename = "promptCacheKey", skip_serializing_if = "Option::is_none")]
    pub prompt_cache_key: Option<String>,
}

/// Mistral chat message payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionStreamRequestMessage {
    /// Message role.
    pub role: String,
    /// Message content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<ChatMessageContent>,
    /// Assistant tool calls.
    #[serde(rename = "toolCalls", skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ChatToolCall>>,
    /// Tool call identifier for tool result messages.
    #[serde(rename = "toolCallId", skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Tool name for tool result messages.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Mistral chat message content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatMessageContent {
    /// Plain text content.
    Text(String),
    /// Structured content chunks.
    Chunks(Vec<ContentChunk>),
}

/// Mistral content chunk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentChunk {
    /// Chunk type.
    #[serde(rename = "type")]
    pub kind: String,
    /// Text payload for text chunks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Image URL payload for image chunks.
    #[serde(rename = "imageUrl", skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    /// Thinking payload chunks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<Vec<ThinkingPart>>,
}

/// Mistral thinking part.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThinkingPart {
    /// Thinking part type.
    #[serde(rename = "type")]
    pub kind: String,
    /// Thinking text.
    pub text: String,
}

/// Mistral function tool declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionTool {
    /// Tool type; Pi always sets `function`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Function declaration.
    pub function: FunctionToolFunction,
}

/// Mistral function declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionToolFunction {
    /// Function name.
    pub name: String,
    /// Function description.
    pub description: String,
    /// JSON schema parameters.
    pub parameters: Value,
    /// Pi sets strict mode to false.
    pub strict: bool,
}

/// Mistral assistant tool call payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatToolCall {
    /// Tool call identifier.
    pub id: String,
    /// Tool call type; Pi always sets `function`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Function call payload.
    pub function: ChatToolCallFunction,
}

/// Mistral function call payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatToolCallFunction {
    /// Function name.
    pub name: String,
    /// JSON-encoded arguments.
    pub arguments: String,
}

/// Request options passed to the Mistral SDK.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequestOptions {
    /// Merged custom headers.
    pub headers: ProviderHeaders,
    /// Pi disables SDK retries for Mistral streams.
    pub retries_none: bool,
}

/// Mistral API error shape used by [`format_mistral_error`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MistralApiError {
    /// Human-readable error message.
    pub message: String,
    /// Optional HTTP status code reported by the SDK.
    pub status_code: Option<u16>,
    /// Optional raw error body reported by the SDK.
    pub body: Option<String>,
}

/// Starts a Mistral Conversations stream using a per-request direct reqwest transport.
///
/// # Errors
///
/// Returns [`MistralConversationsError::MissingApiKey`] when no API key is supplied.
pub fn stream(
    model: &Model,
    context: &Context,
    options: Option<&MistralOptions>,
) -> Result<AssistantMessageEventStream> {
    if options
        .and_then(|options| options.api_key.as_deref())
        .is_none()
    {
        return Err(MistralConversationsError::MissingApiKey {
            provider: model.provider.clone(),
        });
    }

    let stream = AssistantMessageEventStream::new();
    let worker_stream = stream.clone();
    let model = model.clone();
    let context = context.clone();
    let options = options.cloned().unwrap_or_default();
    thread::spawn(move || run_stream_worker(worker_stream, model, context, options));
    Ok(stream)
}

/// Starts a Mistral Conversations stream using Pi's simple stream options mapping.
///
/// # Errors
///
/// Returns [`MistralConversationsError::MissingApiKey`] when no API key is supplied.
pub fn stream_simple(
    model: &Model,
    context: &Context,
    options: Option<&SimpleStreamOptions>,
) -> Result<AssistantMessageEventStream> {
    let Some(api_key) = options.and_then(|options| options.api_key.clone()) else {
        return Err(MistralConversationsError::MissingApiKey {
            provider: model.provider.clone(),
        });
    };
    let mapped = build_simple_options(model, options, api_key);
    stream(model, context, Some(&mapped))
}

/// Builds the Mistral chat payload used by Pi before SDK streaming.
#[must_use]
pub fn build_chat_payload(
    model: &Model,
    context: &Context,
    messages: &[Message],
    options: Option<&MistralOptions>,
) -> ChatCompletionStreamRequest {
    let mut payload = ChatCompletionStreamRequest {
        model: model.id.clone(),
        stream: true,
        messages: to_chat_messages(messages, model.input.contains(&ModelInput::Image)),
        tools: (!context.tools.is_empty()).then(|| to_function_tools(&context.tools)),
        temperature: options.and_then(|options| options.temperature),
        max_tokens: options.and_then(|options| options.max_tokens),
        tool_choice: options.and_then(|options| options.tool_choice.clone()),
        prompt_mode: options.and_then(|options| options.prompt_mode.clone()),
        reasoning_effort: options.and_then(|options| options.reasoning_effort),
        prompt_cache_key: options
            .filter(|options| should_use_prompt_caching(options))
            .and_then(|options| options.session_id.clone()),
    };

    if let Some(system_prompt) = &context.system_prompt {
        payload.messages.insert(
            0,
            ChatCompletionStreamRequestMessage {
                role: "system".to_string(),
                content: Some(ChatMessageContent::Text(sanitize_surrogates(system_prompt))),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
        );
    }

    payload
}

/// Builds request options passed to the Mistral SDK.
#[must_use]
pub fn build_request_options(model: &Model, options: Option<&MistralOptions>) -> RequestOptions {
    let mut headers = model.headers.clone();
    if let Some(options) = options {
        headers.extend(options.headers.clone());
        if should_use_prompt_caching(options) && !headers.contains_key("x-affinity") {
            if let Some(session_id) = &options.session_id {
                headers.insert("x-affinity".to_string(), session_id.clone());
            }
        }
    }

    RequestOptions {
        headers,
        retries_none: true,
    }
}

/// Returns true when Pi enables Mistral prompt caching.
#[must_use]
pub fn should_use_prompt_caching(options: &MistralOptions) -> bool {
    options.cache_retention != Some(CacheRetention::None)
        && options
            .session_id
            .as_deref()
            .is_some_and(|session_id| !session_id.is_empty())
}

/// Extracts cached prompt token counts from the Mistral usage variants Pi accepts.
#[must_use]
pub fn get_mistral_cached_prompt_tokens(usage: &Value, prompt_tokens: u64) -> u64 {
    let raw = usage
        .pointer("/promptTokensDetails/cachedTokens")
        .or_else(|| usage.pointer("/prompt_tokens_details/cached_tokens"))
        .or_else(|| usage.pointer("/promptTokenDetails/cachedTokens"))
        .or_else(|| usage.pointer("/prompt_token_details/cached_tokens"))
        .or_else(|| usage.get("numCachedTokens"))
        .or_else(|| usage.get("num_cached_tokens"));
    let cached_tokens = raw.and_then(Value::as_u64).unwrap_or(0);
    prompt_tokens.min(cached_tokens)
}

fn run_stream_worker(
    stream: AssistantMessageEventStream,
    model: Model,
    context: Context,
    options: MistralOptions,
) {
    let mut output = create_output(&model);
    stream.push(AssistantMessageEvent::Start {
        partial: output.clone(),
    });

    if let Err(error) = execute_mistral_stream(&model, &context, &options, &stream, &mut output) {
        output.stop_reason = StopReason::Error;
        output.error_message = Some(error);
        stream.push(AssistantMessageEvent::Error {
            reason: StopReason::Error,
            error: output,
        });
        return;
    }

    if output.stop_reason == StopReason::Error {
        output.error_message = Some("An unknown error occurred".to_string());
        stream.push(AssistantMessageEvent::Error {
            reason: StopReason::Error,
            error: output,
        });
        return;
    }

    stream.push(AssistantMessageEvent::Done {
        reason: output.stop_reason,
        message: output,
    });
}

fn execute_mistral_stream(
    model: &Model,
    context: &Context,
    options: &MistralOptions,
    stream: &AssistantMessageEventStream,
    output: &mut AssistantMessage,
) -> std::result::Result<(), String> {
    let api_key = options
        .api_key
        .as_deref()
        .ok_or_else(|| format!("No API key for provider: {}", model.provider))?;
    let messages = normalize_tool_call_ids(&context.messages);
    let payload = build_chat_payload(model, context, &messages, Some(options));
    let mut payload = serde_json::to_value(payload).map_err(|error| error.to_string())?;
    if let Some(on_payload) = options.on_payload.as_ref() {
        if let Some(next_payload) = on_payload(payload.clone(), model) {
            payload = next_payload;
        }
    }

    let request_options = build_request_options(model, Some(options));
    let client = build_http_client(options)?;
    let response = client
        .post(mistral_chat_url(model))
        .headers(request_headers(api_key, &request_options.headers)?)
        .body(serde_json::to_vec(&payload).map_err(|error| error.to_string())?)
        .send()
        .map_err(|error| {
            format_mistral_error(&MistralApiError {
                message: error.to_string(),
                status_code: error.status().map(|status| status.as_u16()),
                body: None,
            })
        })?;

    let status = response.status();
    if !status.is_success() {
        let body = read_response_to_string(response)?;
        return Err(format_mistral_error(&MistralApiError {
            message: status.to_string(),
            status_code: Some(status.as_u16()),
            body: Some(body),
        }));
    }

    consume_sse_response(model, output, stream, response)
}

fn build_http_client(options: &MistralOptions) -> std::result::Result<Client, String> {
    let mut builder = Client::builder();
    if let Some(timeout_ms) = options.timeout_ms {
        builder = builder.timeout(Duration::from_millis(timeout_ms));
    }
    builder.build().map_err(|error| error.to_string())
}

fn request_headers(
    api_key: &str,
    headers: &ProviderHeaders,
) -> std::result::Result<HeaderMap, String> {
    let mut map = HeaderMap::new();
    map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    map.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {api_key}")).map_err(|error| error.to_string())?,
    );
    for (name, value) in headers {
        map.insert(
            HeaderName::from_bytes(name.as_bytes()).map_err(|error| error.to_string())?,
            HeaderValue::from_str(value).map_err(|error| error.to_string())?,
        );
    }
    Ok(map)
}

fn mistral_chat_url(model: &Model) -> String {
    let base = model
        .base_url
        .as_deref()
        .unwrap_or("https://api.mistral.ai")
        .trim_end_matches('/');
    if base.ends_with("/v1") {
        format!("{base}/chat/completions")
    } else {
        format!("{base}/v1/chat/completions")
    }
}

fn read_response_to_string(
    mut response: reqwest::blocking::Response,
) -> std::result::Result<String, String> {
    let mut body = String::new();
    response
        .read_to_string(&mut body)
        .map_err(|error| error.to_string())?;
    Ok(body)
}

fn consume_sse_response(
    model: &Model,
    output: &mut AssistantMessage,
    stream: &AssistantMessageEventStream,
    response: reqwest::blocking::Response,
) -> std::result::Result<(), String> {
    let mut consumer = MistralStreamConsumer::default();
    let reader = BufReader::new(response);
    for line in reader.lines() {
        let line = line.map_err(|error| error.to_string())?;
        let line = line.trim();
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        let Some(data) = line.strip_prefix("data:").map(str::trim) else {
            continue;
        };
        if data == "[DONE]" {
            break;
        }
        let event = serde_json::from_str::<MistralStreamEnvelope>(data)
            .map_err(|error| format!("Mistral stream JSON error: {error}"))?;
        consumer.consume_chunk(model, output, stream, event.into_chunk());
    }
    consumer.finish(output, stream);
    Ok(())
}

fn create_output(model: &Model) -> AssistantMessage {
    AssistantMessage {
        role: "assistant".to_string(),
        content: Vec::new(),
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        response_id: None,
        usage: Usage::default(),
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp: unix_timestamp_ms(),
    }
}

fn unix_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn normalize_tool_call_ids(messages: &[Message]) -> Vec<Message> {
    let mut normalizer = MistralToolCallIdNormalizer::new();
    messages
        .iter()
        .cloned()
        .map(|message| normalize_message_tool_call_ids(message, &mut normalizer))
        .collect()
}

fn normalize_message_tool_call_ids(
    message: Message,
    normalizer: &mut MistralToolCallIdNormalizer,
) -> Message {
    match message {
        Message::Assistant { content } => Message::Assistant {
            content: content
                .into_iter()
                .map(|block| match block {
                    AssistantContent::ToolCall {
                        id,
                        name,
                        arguments,
                    } => AssistantContent::ToolCall {
                        id: normalizer.normalize(&id),
                        name,
                        arguments,
                    },
                    block => block,
                })
                .collect(),
        },
        Message::ToolResult {
            tool_call_id,
            tool_name,
            content,
            is_error,
        } => Message::ToolResult {
            tool_call_id: normalizer.normalize(&tool_call_id),
            tool_name,
            content,
            is_error,
        },
        message => message,
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MistralStreamEnvelope {
    Wrapped { data: MistralStreamChunk },
    Chunk(MistralStreamChunk),
}

impl MistralStreamEnvelope {
    fn into_chunk(self) -> MistralStreamChunk {
        match self {
            Self::Wrapped { data } | Self::Chunk(data) => data,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct MistralStreamChunk {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    choices: Vec<MistralChoice>,
    #[serde(default)]
    usage: Option<Value>,
}

#[derive(Debug, Default, Deserialize)]
struct MistralChoice {
    #[serde(default, alias = "finishReason", alias = "finish_reason")]
    finish_reason: Option<String>,
    #[serde(default)]
    delta: MistralDelta,
}

#[derive(Debug, Default, Deserialize)]
struct MistralDelta {
    #[serde(default)]
    content: Option<MistralDeltaContent>,
    #[serde(default, alias = "toolCalls", alias = "tool_calls")]
    tool_calls: Vec<MistralStreamToolCall>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MistralDeltaContent {
    Text(String),
    Chunks(Vec<ContentChunk>),
}

#[derive(Debug, Default, Deserialize)]
struct MistralStreamToolCall {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    index: Option<usize>,
    function: MistralStreamToolFunction,
}

#[derive(Debug, Default, Deserialize)]
struct MistralStreamToolFunction {
    #[serde(default)]
    name: String,
    #[serde(default)]
    arguments: Option<Value>,
}

#[derive(Debug, Default)]
struct MistralStreamConsumer {
    current_block: Option<CurrentBlock>,
    tool_blocks_by_key: HashMap<String, ToolBlockState>,
}

#[derive(Debug, Clone, Copy)]
enum CurrentBlock {
    Text(usize),
    Thinking(usize),
}

#[derive(Debug, Clone)]
struct ToolBlockState {
    content_index: usize,
    partial_args: String,
}

impl MistralStreamConsumer {
    fn consume_chunk(
        &mut self,
        model: &Model,
        output: &mut AssistantMessage,
        stream: &AssistantMessageEventStream,
        chunk: MistralStreamChunk,
    ) {
        if output.response_id.is_none() {
            output.response_id = chunk.id.filter(|id| !id.is_empty());
        }
        if let Some(usage) = chunk.usage {
            apply_usage(model, output, &usage);
        }

        let Some(choice) = chunk.choices.into_iter().next() else {
            return;
        };
        if let Some(finish_reason) = choice.finish_reason.as_deref() {
            output.stop_reason = map_chat_stop_reason(Some(finish_reason));
        }
        if let Some(content) = choice.delta.content {
            self.consume_content(output, stream, content);
        }
        for tool_call in choice.delta.tool_calls {
            self.consume_tool_call(output, stream, tool_call);
        }
    }

    fn consume_content(
        &mut self,
        output: &mut AssistantMessage,
        stream: &AssistantMessageEventStream,
        content: MistralDeltaContent,
    ) {
        match content {
            MistralDeltaContent::Text(text) => {
                self.push_text(output, stream, &sanitize_surrogates(&text))
            }
            MistralDeltaContent::Chunks(chunks) => {
                for chunk in chunks {
                    match chunk.kind.as_str() {
                        "thinking" => {
                            let delta = chunk
                                .thinking
                                .unwrap_or_default()
                                .into_iter()
                                .map(|part| part.text)
                                .collect::<String>();
                            let delta = sanitize_surrogates(&delta);
                            if !delta.is_empty() {
                                self.push_thinking(output, stream, &delta);
                            }
                        }
                        "text" => {
                            if let Some(text) = chunk.text {
                                self.push_text(output, stream, &sanitize_surrogates(&text));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn push_text(
        &mut self,
        output: &mut AssistantMessage,
        stream: &AssistantMessageEventStream,
        delta: &str,
    ) {
        let index = match self.current_block {
            Some(CurrentBlock::Text(index)) => index,
            current => {
                self.finish_current(output, stream, current);
                let index = output.content.len();
                output.content.push(AssistantContent::Text {
                    text: String::new(),
                });
                self.current_block = Some(CurrentBlock::Text(index));
                stream.push(AssistantMessageEvent::TextStart {
                    content_index: index,
                    partial: output.clone(),
                });
                index
            }
        };
        if let Some(AssistantContent::Text { text }) = output.content.get_mut(index) {
            text.push_str(delta);
        }
        stream.push(AssistantMessageEvent::TextDelta {
            content_index: index,
            delta: delta.to_string(),
            partial: output.clone(),
        });
    }

    fn push_thinking(
        &mut self,
        output: &mut AssistantMessage,
        stream: &AssistantMessageEventStream,
        delta: &str,
    ) {
        let index = match self.current_block {
            Some(CurrentBlock::Thinking(index)) => index,
            current => {
                self.finish_current(output, stream, current);
                let index = output.content.len();
                output.content.push(AssistantContent::Thinking {
                    thinking: String::new(),
                });
                self.current_block = Some(CurrentBlock::Thinking(index));
                stream.push(AssistantMessageEvent::ThinkingStart {
                    content_index: index,
                    partial: output.clone(),
                });
                index
            }
        };
        if let Some(AssistantContent::Thinking { thinking }) = output.content.get_mut(index) {
            thinking.push_str(delta);
        }
        stream.push(AssistantMessageEvent::ThinkingDelta {
            content_index: index,
            delta: delta.to_string(),
            partial: output.clone(),
        });
    }

    fn consume_tool_call(
        &mut self,
        output: &mut AssistantMessage,
        stream: &AssistantMessageEventStream,
        tool_call: MistralStreamToolCall,
    ) {
        let current = self.current_block.take();
        self.finish_current(output, stream, current);
        let call_id = tool_call.id.filter(|id| id != "null").unwrap_or_else(|| {
            derive_mistral_tool_call_id(&format!("toolcall:{}", tool_call.index.unwrap_or(0)), 0)
        });
        let key = format!("{}:{}", call_id, tool_call.index.unwrap_or(0));
        if !self.tool_blocks_by_key.contains_key(&key) {
            let content_index = output.content.len();
            output.content.push(AssistantContent::ToolCall {
                id: call_id,
                name: tool_call.function.name.clone(),
                arguments: Value::Object(Default::default()),
            });
            self.tool_blocks_by_key.insert(
                key.clone(),
                ToolBlockState {
                    content_index,
                    partial_args: String::new(),
                },
            );
            stream.push(AssistantMessageEvent::ToolcallStart {
                content_index,
                partial: output.clone(),
            });
        }

        let args_delta = match tool_call.function.arguments {
            Some(Value::String(arguments)) => arguments,
            Some(value) => value.to_string(),
            None => "{}".to_string(),
        };
        let state = self
            .tool_blocks_by_key
            .get_mut(&key)
            .expect("tool block exists");
        state.partial_args.push_str(&args_delta);
        if let Some(AssistantContent::ToolCall {
            name, arguments, ..
        }) = output.content.get_mut(state.content_index)
        {
            if !tool_call.function.name.is_empty() {
                *name = tool_call.function.name;
            }
            *arguments = parse_streaming_json_value(Some(&state.partial_args));
        }
        stream.push(AssistantMessageEvent::ToolcallDelta {
            content_index: state.content_index,
            delta: args_delta,
            partial: output.clone(),
        });
    }

    fn finish(&mut self, output: &mut AssistantMessage, stream: &AssistantMessageEventStream) {
        let current = self.current_block.take();
        self.finish_current(output, stream, current);
        for state in self.tool_blocks_by_key.values() {
            let Some(AssistantContent::ToolCall { arguments, .. }) =
                output.content.get_mut(state.content_index)
            else {
                continue;
            };
            *arguments = parse_streaming_json_value(Some(&state.partial_args));
            let tool_call = output.content[state.content_index].clone();
            stream.push(AssistantMessageEvent::ToolcallEnd {
                content_index: state.content_index,
                tool_call,
                partial: output.clone(),
            });
        }
    }

    fn finish_current(
        &self,
        output: &AssistantMessage,
        stream: &AssistantMessageEventStream,
        current: Option<CurrentBlock>,
    ) {
        match current {
            Some(CurrentBlock::Text(index)) => {
                let content = match &output.content[index] {
                    AssistantContent::Text { text } => text.clone(),
                    _ => String::new(),
                };
                stream.push(AssistantMessageEvent::TextEnd {
                    content_index: index,
                    content,
                    partial: output.clone(),
                });
            }
            Some(CurrentBlock::Thinking(index)) => {
                let content = match &output.content[index] {
                    AssistantContent::Thinking { thinking } => thinking.clone(),
                    _ => String::new(),
                };
                stream.push(AssistantMessageEvent::ThinkingEnd {
                    content_index: index,
                    content,
                    partial: output.clone(),
                });
            }
            None => {}
        }
    }
}

fn apply_usage(_model: &Model, output: &mut AssistantMessage, usage: &Value) {
    let prompt_tokens = usage_u64(usage, &["promptTokens", "prompt_tokens"]);
    let cached_prompt_tokens = get_mistral_cached_prompt_tokens(usage, prompt_tokens);
    output.usage.input = prompt_tokens.saturating_sub(cached_prompt_tokens);
    output.usage.output = usage_u64(usage, &["completionTokens", "completion_tokens"]);
    output.usage.cache_read = cached_prompt_tokens;
    output.usage.cache_write = 0;
    output.usage.total_tokens = usage_u64(usage, &["totalTokens", "total_tokens"]);
    if output.usage.total_tokens == 0 {
        output.usage.total_tokens =
            output.usage.input + output.usage.output + output.usage.cache_read;
    }
}

fn usage_u64(usage: &Value, keys: &[&str]) -> u64 {
    keys.iter()
        .find_map(|key| usage.get(*key).and_then(Value::as_u64))
        .unwrap_or(0)
}

/// Converts Pi tools to Mistral function declarations.
#[must_use]
pub fn to_function_tools(tools: &[Tool]) -> Vec<FunctionTool> {
    tools
        .iter()
        .map(|tool| FunctionTool {
            kind: "function".to_string(),
            function: FunctionToolFunction {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: strip_symbol_keys(&tool.parameters),
                strict: false,
            },
        })
        .collect()
}

/// Converts Pi messages to Mistral chat messages.
#[must_use]
pub fn to_chat_messages(
    messages: &[Message],
    supports_images: bool,
) -> Vec<ChatCompletionStreamRequestMessage> {
    let mut result = Vec::new();

    for msg in messages {
        match msg {
            Message::User { content } => match content {
                UserContent::Text(text) => result.push(ChatCompletionStreamRequestMessage {
                    role: "user".to_string(),
                    content: Some(ChatMessageContent::Text(sanitize_surrogates(text))),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                }),
                UserContent::Parts(parts) => {
                    let had_images = parts
                        .iter()
                        .any(|item| matches!(item, UserContentPart::Image { .. }));
                    let content = parts
                        .iter()
                        .filter_map(|item| user_part_to_content_chunk(item, supports_images))
                        .collect::<Vec<_>>();
                    if !content.is_empty() {
                        result.push(ChatCompletionStreamRequestMessage {
                            role: "user".to_string(),
                            content: Some(ChatMessageContent::Chunks(content)),
                            tool_calls: None,
                            tool_call_id: None,
                            name: None,
                        });
                    } else if had_images && !supports_images {
                        result.push(ChatCompletionStreamRequestMessage {
                            role: "user".to_string(),
                            content: Some(ChatMessageContent::Text(
                                "(image omitted: model does not support images)".to_string(),
                            )),
                            tool_calls: None,
                            tool_call_id: None,
                            name: None,
                        });
                    }
                }
            },
            Message::Assistant { content } => {
                let mut content_parts = Vec::new();
                let mut tool_calls = Vec::new();

                for block in content {
                    match block {
                        AssistantContent::Text { text } => {
                            if !text.trim().is_empty() {
                                content_parts.push(ContentChunk {
                                    kind: "text".to_string(),
                                    text: Some(sanitize_surrogates(text)),
                                    image_url: None,
                                    thinking: None,
                                });
                            }
                        }
                        AssistantContent::Thinking { thinking } => {
                            if !thinking.trim().is_empty() {
                                content_parts.push(ContentChunk {
                                    kind: "thinking".to_string(),
                                    text: None,
                                    image_url: None,
                                    thinking: Some(vec![ThinkingPart {
                                        kind: "text".to_string(),
                                        text: sanitize_surrogates(thinking),
                                    }]),
                                });
                            }
                        }
                        AssistantContent::ToolCall {
                            id,
                            name,
                            arguments,
                        } => tool_calls.push(ChatToolCall {
                            id: id.clone(),
                            kind: "function".to_string(),
                            function: ChatToolCallFunction {
                                name: name.clone(),
                                arguments: arguments.to_string(),
                            },
                        }),
                    }
                }

                if !content_parts.is_empty() || !tool_calls.is_empty() {
                    result.push(ChatCompletionStreamRequestMessage {
                        role: "assistant".to_string(),
                        content: (!content_parts.is_empty())
                            .then_some(ChatMessageContent::Chunks(content_parts)),
                        tool_calls: (!tool_calls.is_empty()).then_some(tool_calls),
                        tool_call_id: None,
                        name: None,
                    });
                }
            }
            Message::ToolResult {
                tool_call_id,
                tool_name,
                content,
                is_error,
            } => {
                let text_result = content
                    .iter()
                    .filter_map(|part| match part {
                        UserContentPart::Text { text } => Some(sanitize_surrogates(text)),
                        UserContentPart::Image { .. } => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let has_images = content
                    .iter()
                    .any(|part| matches!(part, UserContentPart::Image { .. }));
                let mut tool_content = vec![ContentChunk {
                    kind: "text".to_string(),
                    text: Some(build_tool_result_text(
                        &text_result,
                        has_images,
                        supports_images,
                        *is_error,
                    )),
                    image_url: None,
                    thinking: None,
                }];

                if supports_images {
                    tool_content.extend(content.iter().filter_map(|part| {
                        if let UserContentPart::Image { data, mime_type } = part {
                            Some(ContentChunk {
                                kind: "image_url".to_string(),
                                text: None,
                                image_url: Some(format!("data:{mime_type};base64,{data}")),
                                thinking: None,
                            })
                        } else {
                            None
                        }
                    }));
                }

                result.push(ChatCompletionStreamRequestMessage {
                    role: "tool".to_string(),
                    content: Some(ChatMessageContent::Chunks(tool_content)),
                    tool_calls: None,
                    tool_call_id: Some(tool_call_id.clone()),
                    name: Some(tool_name.clone()),
                });
            }
        }
    }

    result
}

/// Builds Pi's text wrapper for Mistral tool result content.
#[must_use]
pub fn build_tool_result_text(
    text: &str,
    has_images: bool,
    supports_images: bool,
    is_error: bool,
) -> String {
    let trimmed = text.trim();
    let error_prefix = if is_error { "[tool error] " } else { "" };

    if !trimmed.is_empty() {
        let image_suffix = if has_images && !supports_images {
            "\n[tool image omitted: model does not support images]"
        } else {
            ""
        };
        return format!("{error_prefix}{trimmed}{image_suffix}");
    }

    if has_images {
        if supports_images {
            return if is_error {
                "[tool error] (see attached image)"
            } else {
                "(see attached image)"
            }
            .to_string();
        }
        return if is_error {
            "[tool error] (image omitted: model does not support images)"
        } else {
            "(image omitted: model does not support images)"
        }
        .to_string();
    }

    if is_error {
        "[tool error] (no tool output)".to_string()
    } else {
        "(no tool output)".to_string()
    }
}

/// Returns true when Pi uses Mistral's `reasoningEffort` option for this model.
#[must_use]
pub fn uses_reasoning_effort(model: &Model) -> bool {
    matches!(
        model.id.as_str(),
        "mistral-small-2603" | "mistral-small-latest" | "mistral-medium-3.5"
    )
}

/// Returns true when Pi uses Mistral's `promptMode: reasoning` option for this model.
#[must_use]
pub fn uses_prompt_mode_reasoning(model: &Model) -> bool {
    model.reasoning && !uses_reasoning_effort(model)
}

/// Maps Pi reasoning levels to Mistral reasoning effort.
#[must_use]
pub fn map_reasoning_effort(model: &Model, level: ThinkingLevel) -> MistralReasoningEffort {
    model
        .thinking_level_map
        .get(&ModelThinkingLevel::from(level))
        .copied()
        .unwrap_or(MistralReasoningEffort::High)
}

/// Maps Mistral finish reasons to Pi stop reasons.
#[must_use]
pub fn map_chat_stop_reason(reason: Option<&str>) -> StopReason {
    match reason {
        None | Some("stop") => StopReason::Stop,
        Some("length" | "model_length") => StopReason::Length,
        Some("tool_calls") => StopReason::ToolUse,
        Some("error") => StopReason::Error,
        Some(_) => StopReason::Stop,
    }
}

/// Pi stop reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    /// Successful stop.
    Stop,
    /// Length-limited stop.
    Length,
    /// Tool-use stop.
    ToolUse,
    /// Provider or protocol error.
    Error,
    /// Aborted request.
    Aborted,
}

/// Formats Mistral SDK errors the way Pi reports them in assistant error messages.
#[must_use]
pub fn format_mistral_error(error: &MistralApiError) -> String {
    match (error.status_code, error.body.as_deref().map(str::trim)) {
        (Some(status_code), Some(body)) if !body.is_empty() => format!(
            "Mistral API error ({status_code}): {}",
            truncate_error_text(body, MAX_MISTRAL_ERROR_BODY_CHARS)
        ),
        (Some(status_code), _) => format!("Mistral API error ({status_code}): {}", error.message),
        (None, _) => error.message.clone(),
    }
}

/// Derives a Mistral-safe 9-character tool call ID from Pi's arbitrary IDs.
#[must_use]
pub fn derive_mistral_tool_call_id(id: &str, attempt: u32) -> String {
    let normalized = id
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>();
    if attempt == 0 && normalized.chars().count() == MISTRAL_TOOL_CALL_ID_LENGTH {
        return normalized;
    }

    let seed_base = if normalized.is_empty() {
        id.to_string()
    } else {
        normalized
    };
    let seed = if attempt == 0 {
        seed_base
    } else {
        format!("{seed_base}:{attempt}")
    };
    short_hash(&seed)
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(MISTRAL_TOOL_CALL_ID_LENGTH)
        .collect()
}

/// Stateful normalizer that preserves one derived Mistral tool call ID per original ID.
#[derive(Debug, Default, Clone)]
pub struct MistralToolCallIdNormalizer {
    id_map: HashMap<String, String>,
    reverse_map: HashMap<String, String>,
}

impl MistralToolCallIdNormalizer {
    /// Creates an empty Mistral tool call ID normalizer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the stable Mistral-safe ID for an original tool call ID.
    #[must_use]
    pub fn normalize(&mut self, id: &str) -> String {
        if let Some(existing) = self.id_map.get(id) {
            return existing.clone();
        }

        let mut attempt = 0;
        loop {
            let candidate = derive_mistral_tool_call_id(id, attempt);
            let owner = self.reverse_map.get(&candidate);
            if owner.is_none_or(|owner| owner == id) {
                self.id_map.insert(id.to_string(), candidate.clone());
                self.reverse_map.insert(candidate.clone(), id.to_string());
                return candidate;
            }
            attempt += 1;
        }
    }
}

fn build_simple_options(
    model: &Model,
    options: Option<&SimpleStreamOptions>,
    api_key: String,
) -> MistralOptions {
    let clamped_reasoning = options.and_then(|options| options.reasoning);
    let should_use_reasoning = model.reasoning && clamped_reasoning.is_some();
    let reasoning = clamped_reasoning.unwrap_or(ThinkingLevel::High);

    MistralOptions {
        temperature: options.and_then(|options| options.temperature),
        max_tokens: options.and_then(|options| options.max_tokens),
        api_key: Some(api_key),
        session_id: options.and_then(|options| options.session_id.clone()),
        cache_retention: options.and_then(|options| options.cache_retention),
        headers: options
            .map(|options| options.headers.clone())
            .unwrap_or_default(),
        timeout_ms: options.and_then(|options| options.timeout_ms),
        on_payload: None,
        tool_choice: None,
        prompt_mode: (should_use_reasoning && uses_prompt_mode_reasoning(model))
            .then(|| "reasoning".to_string()),
        reasoning_effort: (should_use_reasoning && uses_reasoning_effort(model))
            .then(|| map_reasoning_effort(model, reasoning)),
    }
}

fn user_part_to_content_chunk(
    item: &UserContentPart,
    supports_images: bool,
) -> Option<ContentChunk> {
    match item {
        UserContentPart::Text { text } => Some(ContentChunk {
            kind: "text".to_string(),
            text: Some(sanitize_surrogates(text)),
            image_url: None,
            thinking: None,
        }),
        UserContentPart::Image { data, mime_type } if supports_images => Some(ContentChunk {
            kind: "image_url".to_string(),
            text: None,
            image_url: Some(format!("data:{mime_type};base64,{data}")),
            thinking: None,
        }),
        UserContentPart::Image { .. } => None,
    }
}

fn strip_symbol_keys(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(strip_symbol_keys).collect()),
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| (key.clone(), strip_symbol_keys(value)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn truncate_error_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let truncated = text.chars().take(max_chars).collect::<String>();
    format!(
        "{truncated}... [truncated {} chars]",
        text.chars().count() - max_chars
    )
}

fn sanitize_surrogates(text: &str) -> String {
    text.to_string()
}

fn short_hash(str: &str) -> String {
    let mut h1 = 0xdead_beefu32;
    let mut h2 = 0x41c6_ce57u32;

    for ch in str.encode_utf16() {
        let ch = u32::from(ch);
        h1 = (h1 ^ ch).wrapping_mul(2_654_435_761);
        h2 = (h2 ^ ch).wrapping_mul(1_597_334_677);
    }

    h1 = (h1 ^ (h1 >> 16)).wrapping_mul(2_246_822_507)
        ^ (h2 ^ (h2 >> 13)).wrapping_mul(3_266_489_909);
    h2 = (h2 ^ (h2 >> 16)).wrapping_mul(2_246_822_507)
        ^ (h1 ^ (h1 >> 13)).wrapping_mul(3_266_489_909);

    format!("{}{}", to_base36(h2), to_base36(h1))
}

fn to_base36(mut value: u32) -> String {
    if value == 0 {
        return "0".to_string();
    }

    let mut digits = Vec::new();
    while value > 0 {
        let digit = value % 36;
        let byte = if digit < 10 {
            b'0' + u8::try_from(digit).unwrap_or(0)
        } else {
            b'a' + u8::try_from(digit - 10).unwrap_or(0)
        };
        digits.push(char::from(byte));
        value /= 36;
    }
    digits.iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::sync::mpsc::{self, Receiver};

    use super::*;
    use serde_json::json;

    fn model(id: &str) -> Model {
        Model {
            id: id.to_string(),
            api: "mistral-conversations".to_string(),
            provider: "mistral".to_string(),
            base_url: None,
            input: vec![ModelInput::Text],
            reasoning: true,
            thinking_level_map: HashMap::new(),
            headers: HashMap::new(),
        }
    }

    fn make_context() -> Context {
        Context {
            messages: vec![Message::User {
                content: UserContent::Text("Hello".to_string()),
            }],
            ..Context::default()
        }
    }

    fn capture_simple_payload(
        model: &Model,
        options: SimpleStreamOptions,
    ) -> ChatCompletionStreamRequest {
        let context = make_context();
        let mistral_options = build_simple_options(model, Some(&options), "fake-key".to_string());
        build_chat_payload(model, &context, &context.messages, Some(&mistral_options))
    }

    fn serve_once(body: &'static str, status: &str) -> (String, Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let url = format!("http://{}", listener.local_addr().expect("local addr"));
        let (sender, receiver) = mpsc::channel();
        let status = status.to_string();
        thread::spawn(move || {
            let (mut socket, _) = listener.accept().expect("accept request");
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1024];
            loop {
                let read = socket.read(&mut buffer).expect("read request");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    let request_text = String::from_utf8_lossy(&request).into_owned();
                    let content_length = request_text
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length: ")
                                .or_else(|| line.strip_prefix("Content-Length: "))
                        })
                        .and_then(|value| value.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    let header_len = request
                        .windows(4)
                        .position(|window| window == b"\r\n\r\n")
                        .map(|position| position + 4)
                        .unwrap_or(request.len());
                    while request.len().saturating_sub(header_len) < content_length {
                        let read = socket.read(&mut buffer).expect("read request body");
                        if read == 0 {
                            break;
                        }
                        request.extend_from_slice(&buffer[..read]);
                    }
                    break;
                }
            }
            sender
                .send(String::from_utf8_lossy(&request).into_owned())
                .expect("send request");
            let response = format!(
                "HTTP/1.1 {status}\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n{body}"
            );
            socket
                .write_all(response.as_bytes())
                .expect("write response");
        });
        (url, receiver)
    }

    #[test]
    fn mistral_reasoning_mode_uses_reasoning_effort_for_mistral_small_4() {
        let payload = capture_simple_payload(
            &model("mistral-small-2603"),
            SimpleStreamOptions {
                reasoning: Some(ThinkingLevel::Medium),
                ..SimpleStreamOptions::default()
            },
        );

        assert_eq!(payload.reasoning_effort, Some(MistralReasoningEffort::High));
        assert_eq!(payload.prompt_mode, None);
    }

    #[test]
    fn mistral_reasoning_mode_omits_controls_for_mistral_small_4_when_thinking_is_off() {
        let payload =
            capture_simple_payload(&model("mistral-small-2603"), SimpleStreamOptions::default());

        assert_eq!(payload.reasoning_effort, None);
        assert_eq!(payload.prompt_mode, None);
    }

    #[test]
    fn mistral_reasoning_mode_uses_prompt_mode_for_magistral_reasoning_models() {
        let payload = capture_simple_payload(
            &model("magistral-medium-latest"),
            SimpleStreamOptions {
                reasoning: Some(ThinkingLevel::Medium),
                ..SimpleStreamOptions::default()
            },
        );

        assert_eq!(payload.prompt_mode.as_deref(), Some("reasoning"));
        assert_eq!(payload.reasoning_effort, None);
    }

    #[test]
    fn mistral_reasoning_mode_uses_reasoning_effort_for_mistral_medium_3_5() {
        let payload = capture_simple_payload(
            &model("mistral-medium-3.5"),
            SimpleStreamOptions {
                reasoning: Some(ThinkingLevel::Medium),
                ..SimpleStreamOptions::default()
            },
        );

        assert_eq!(payload.reasoning_effort, Some(MistralReasoningEffort::High));
        assert_eq!(payload.prompt_mode, None);
    }

    #[test]
    fn mistral_reasoning_mode_omits_controls_for_mistral_medium_3_5_when_thinking_is_off() {
        let payload =
            capture_simple_payload(&model("mistral-medium-3.5"), SimpleStreamOptions::default());

        assert_eq!(payload.reasoning_effort, None);
        assert_eq!(payload.prompt_mode, None);
    }

    #[test]
    fn mistral_reasoning_mode_uses_the_session_id_as_prompt_cache_key() {
        let payload = capture_simple_payload(
            &model("mistral-large-latest"),
            SimpleStreamOptions {
                session_id: Some("session-123".to_string()),
                ..SimpleStreamOptions::default()
            },
        );

        assert_eq!(payload.prompt_cache_key.as_deref(), Some("session-123"));
    }

    #[test]
    fn mistral_reasoning_mode_omits_prompt_cache_key_when_cache_retention_is_disabled() {
        let payload = capture_simple_payload(
            &model("mistral-large-latest"),
            SimpleStreamOptions {
                session_id: Some("session-123".to_string()),
                cache_retention: Some(CacheRetention::None),
                ..SimpleStreamOptions::default()
            },
        );

        assert_eq!(payload.prompt_cache_key, None);
    }

    #[test]
    fn derives_mistral_tool_call_ids_like_pi_hash() {
        assert_eq!(derive_mistral_tool_call_id("abcDEF123", 0), "abcDEF123");
        assert_eq!(derive_mistral_tool_call_id("toolcall:0", 0), "toolcall0");
        assert_eq!(derive_mistral_tool_call_id("toolcall:0", 1), "1t6qern11");
        assert_eq!(derive_mistral_tool_call_id("abc-123-long", 0), "14mjz5m1g");
    }

    #[test]
    fn normalizer_reuses_ids() {
        let mut normalizer = MistralToolCallIdNormalizer::new();
        let first = normalizer.normalize("abc-123-long");
        assert_eq!(first, normalizer.normalize("abc-123-long"));
        assert_eq!(first.len(), MISTRAL_TOOL_CALL_ID_LENGTH);
    }

    #[test]
    fn tool_result_text_matches_pi_cases() {
        assert_eq!(
            build_tool_result_text(" result ", true, false, true),
            "[tool error] result\n[tool image omitted: model does not support images]"
        );
        assert_eq!(
            build_tool_result_text("", true, true, false),
            "(see attached image)"
        );
        assert_eq!(
            build_tool_result_text("", false, false, true),
            "[tool error] (no tool output)"
        );
    }

    #[test]
    fn builds_payload_with_system_prompt_tools_and_cache() {
        let mut context = Context {
            messages: vec![Message::User {
                content: UserContent::Text("hello".to_string()),
            }],
            system_prompt: Some("system".to_string()),
            tools: vec![Tool {
                name: "lookup".to_string(),
                description: "Lookup".to_string(),
                parameters: json!({"type":"object"}),
            }],
        };
        let options = MistralOptions {
            session_id: Some("session-1".to_string()),
            ..MistralOptions::default()
        };
        let payload = build_chat_payload(
            &model("mistral-large-latest"),
            &context,
            &context.messages,
            Some(&options),
        );
        assert_eq!(payload.messages[0].role, "system");
        assert_eq!(payload.prompt_cache_key.as_deref(), Some("session-1"));
        assert_eq!(payload.tools.as_ref().map(Vec::len), Some(1));
        context.tools.clear();
    }

    #[test]
    fn mistral_tool_schema_serialization_strips_typebox_symbol_keys() {
        let parameters = json!({
            "type": "object",
            "properties": {
                "nested": {
                    "type": "object",
                    "properties": {
                        "value": { "type": "string" }
                    }
                }
            }
        });
        let context = Context {
            messages: vec![Message::User {
                content: UserContent::Text("Hi".to_string()),
            }],
            tools: vec![Tool {
                name: "inspect_schema".to_string(),
                description: "Inspect the schema".to_string(),
                parameters: parameters.clone(),
            }],
            ..Context::default()
        };

        let payload = build_chat_payload(
            &model("devstral-medium-latest"),
            &context,
            &context.messages,
            None,
        );

        let tools = payload.tools.as_ref().expect("tools are serialized");
        assert_eq!(tools.len(), 1);
        let payload_parameters = &tools[0].function.parameters;
        assert_eq!(payload_parameters, &parameters);
        let properties = payload_parameters
            .get("properties")
            .and_then(Value::as_object)
            .expect("schema properties are present");
        let nested = properties
            .get("nested")
            .and_then(Value::as_object)
            .expect("nested schema is present");
        assert_eq!(
            nested
                .get("properties")
                .and_then(Value::as_object)
                .and_then(|properties| properties.get("value"))
                .and_then(|value| value.get("type"))
                .and_then(Value::as_str),
            Some("string")
        );
    }

    #[test]
    fn mistral_stream_error_uses_status_and_body_without_sdk_validation_text() {
        let (base_url, _request) = serve_once("too many requests", "429 Too Many Requests");
        let mut test_model = model("mistral-large-latest");
        test_model.base_url = Some(base_url);
        let stream = stream(
            &test_model,
            &make_context(),
            Some(&MistralOptions {
                api_key: Some("test-key".to_string()),
                timeout_ms: Some(2000),
                ..MistralOptions::default()
            }),
        )
        .expect("stream starts");

        let output = stream
            .wait_result(Duration::from_secs(2))
            .expect("terminal event");
        assert_eq!(output.stop_reason, StopReason::Error);
        let message = output.error_message.expect("error message");
        assert_eq!(message, "Mistral API error (429): too many requests");
        assert!(!message.contains("Input validation failed"));
    }

    #[test]
    fn tool_choice_serializes_like_mistral_payload() {
        assert_eq!(
            serde_json::to_value(MistralToolChoice::Auto).expect("serializes"),
            json!("auto")
        );
        assert_eq!(
            serde_json::to_value(MistralToolChoice::Function {
                function: MistralToolChoiceFunction {
                    name: "lookup".to_string(),
                },
            })
            .expect("serializes"),
            json!({"type":"function", "function":{"name":"lookup"}})
        );
    }

    #[test]
    fn request_options_add_affinity_without_overriding_header() {
        let mut options = MistralOptions {
            session_id: Some("session-1".to_string()),
            ..MistralOptions::default()
        };
        let request_options = build_request_options(&model("mistral-large-latest"), Some(&options));
        assert_eq!(
            request_options
                .headers
                .get("x-affinity")
                .map(String::as_str),
            Some("session-1")
        );

        options
            .headers
            .insert("x-affinity".to_string(), "caller".to_string());
        let request_options = build_request_options(&model("mistral-large-latest"), Some(&options));
        assert_eq!(
            request_options
                .headers
                .get("x-affinity")
                .map(String::as_str),
            Some("caller")
        );
    }

    #[test]
    fn cached_prompt_tokens_accepts_mistral_variants() {
        assert_eq!(
            get_mistral_cached_prompt_tokens(
                &json!({"prompt_tokens_details":{"cached_tokens": 7}}),
                5
            ),
            5
        );
        assert_eq!(
            get_mistral_cached_prompt_tokens(&json!({"numCachedTokens": 3}), 5),
            3
        );
    }

    fn replace_payload(mut payload: Value, _model: &Model) -> Option<Value> {
        payload["temperature"] = json!(0.42);
        Some(payload)
    }

    #[test]
    fn stream_uses_reqwest_transport_payload_hook_partial_json_and_usage() {
        let body = concat!(
            "data: {\"id\":\"cmpl-1\",\"choices\":[{\"delta\":{\"content\":\"hi \"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":[{\"type\":\"thinking\",\"thinking\":[{\"type\":\"text\",\"text\":\"why\"}]}]}}]}\n\n",
            r#"data: {"choices":[{"delta":{"toolCalls":[{"id":"abc123XYZ","index":0,"function":{"name":"lookup","arguments":"{\"city\":\"Par"}}]}}]}

"#,
            r#"data: {"choices":[{"delta":{"toolCalls":[{"id":"abc123XYZ","index":0,"function":{"arguments":"is\"}"}}]}}]}

"#,
            "data: {\"usage\":{\"promptTokens\":10,\"completionTokens\":4,\"totalTokens\":14,\"numCachedTokens\":3},\"choices\":[{\"finishReason\":\"tool_calls\",\"delta\":{}}]}\n\n",
            "data: [DONE]\n\n",
        );
        let (base_url, request) = serve_once(body, "200 OK");
        let mut test_model = model("mistral-large-latest");
        test_model.base_url = Some(base_url);
        let stream = stream(
            &test_model,
            &make_context(),
            Some(&MistralOptions {
                api_key: Some("test-key".to_string()),
                temperature: Some(1.0),
                timeout_ms: Some(2000),
                on_payload: Some(Arc::new(replace_payload)),
                ..MistralOptions::default()
            }),
        )
        .expect("stream starts");

        let output = stream
            .wait_result(Duration::from_secs(2))
            .expect("terminal event");
        assert_eq!(output.response_id.as_deref(), Some("cmpl-1"));
        assert_eq!(output.stop_reason, StopReason::ToolUse);
        assert_eq!(output.usage.input, 7);
        assert_eq!(output.usage.output, 4);
        assert_eq!(output.usage.cache_read, 3);
        assert!(matches!(&output.content[0], AssistantContent::Text { text } if text == "hi "));
        assert!(
            matches!(&output.content[1], AssistantContent::Thinking { thinking } if thinking == "why")
        );
        assert!(
            matches!(&output.content[2], AssistantContent::ToolCall { id, name, arguments }
                if id == "abc123XYZ" && name == "lookup" && arguments == &json!({"city":"Paris"})
            )
        );
        let request = request
            .recv_timeout(Duration::from_secs(1))
            .expect("request captured");
        assert!(
            request.contains("authorization: Bearer test-key")
                || request.contains("Authorization: Bearer test-key")
        );
        assert!(request.contains("/v1/chat/completions"));
        assert!(request.contains("\"temperature\":0.42"));
    }
}
