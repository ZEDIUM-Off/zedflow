//! OpenAI Codex Responses API ported from Pi.

use std::collections::{HashMap, HashSet};
use std::error::Error as StdError;
use std::fmt;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const DEFAULT_CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api";
const JWT_CLAIM_PATH: &str = "https://api.openai.com/auth";
const BASE_RETRY_DELAY_MS: u64 = 1_000;
const DEFAULT_MAX_RETRY_DELAY_MS: u64 = 60_000;
const OPENAI_PROMPT_CACHE_KEY_MAX_LENGTH: usize = 64;
const OPENAI_BETA_RESPONSES_WEBSOCKETS: &str = "responses_websockets=2026-02-06";
const DEFAULT_WEBSOCKET_CONNECT_TIMEOUT_MS: u64 = 15_000;
const REQUEST_COMPRESSION_ZSTD_LEVEL: i32 = 3;
const WEBSOCKET_CONNECTION_LIMIT_REACHED_CODE: &str = "websocket_connection_limit_reached";
const MAX_WEBSOCKET_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

static WEBSOCKET_DEBUG_STATS: LazyLock<Mutex<HashMap<String, OpenAICodexWebSocketDebugStats>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static WEBSOCKET_SSE_FALLBACK_SESSIONS: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

/// Result type for the OpenAI Codex Responses port.
pub type Result<T> = std::result::Result<T, OpenAICodexResponsesError>;

/// Errors returned by the OpenAI Codex Responses port.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OpenAICodexResponsesError {
    /// No API key was supplied for the model provider.
    MissingApiKey { provider: String },
    /// A timeout option was negative or not finite in the TypeScript source.
    InvalidTimeoutMs { value: i64 },
    /// The supplied token did not contain the expected ChatGPT account claim.
    InvalidAccountToken,
    /// Provider transport failed before a stream could be produced.
    Transport(String),
}

impl fmt::Display for OpenAICodexResponsesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApiKey { provider } => write!(f, "no API key for provider: {provider}"),
            Self::InvalidTimeoutMs { value } => write!(f, "invalid timeoutMs: {value}"),
            Self::InvalidAccountToken => f.write_str("failed to extract accountId from token"),
            Self::Transport(error) => f.write_str(error),
        }
    }
}

impl StdError for OpenAICodexResponsesError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Transport(_) => None,
            _ => None,
        }
    }
}

/// Provider-scoped environment overrides.
pub type ProviderEnv = HashMap<String, String>;

/// HTTP headers supplied by a model or request options; `None` deletes a default header.
pub type ProviderHeaders = HashMap<String, Option<String>>;

/// Pi thinking level accepted by simplified stream options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThinkingLevel {
    /// Disable reasoning when the provider supports it.
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

/// Codex-specific reasoning effort values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    /// Request no reasoning effort.
    None,
    /// Request minimal reasoning effort.
    Minimal,
    /// Request low reasoning effort.
    Low,
    /// Request medium reasoning effort.
    Medium,
    /// Request high reasoning effort.
    High,
    /// Request extra-high reasoning effort.
    XHigh,
}

impl ReasoningEffort {
    fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
        }
    }
}

/// Reasoning summary preference accepted by Codex Responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningSummary {
    /// Let the provider choose the summary shape.
    Auto,
    /// Request a concise reasoning summary.
    Concise,
    /// Request a detailed reasoning summary.
    Detailed,
    /// Request no reasoning summary.
    Off,
    /// Request reasoning summaries from providers that use an on/off switch.
    On,
}

impl ReasoningSummary {
    fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Concise => "concise",
            Self::Detailed => "detailed",
            Self::Off => "off",
            Self::On => "on",
        }
    }
}

/// Text verbosity accepted by Codex Responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextVerbosity {
    /// Low verbosity; Pi's default for Codex.
    Low,
    /// Medium verbosity.
    Medium,
    /// High verbosity.
    High,
}

impl TextVerbosity {
    fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

/// Service tier values passed through to OpenAI Responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceTier {
    /// Default provider tier.
    Default,
    /// Flex tier, priced at half cost in Pi.
    Flex,
    /// Priority tier, priced above default in Pi.
    Priority,
}

/// Transport preference for Codex Responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Transport {
    /// Let the implementation choose, preferring WebSocket before SSE as Pi does.
    Auto,
    /// Force server-sent events.
    Sse,
    /// Force WebSocket.
    WebSocket,
    /// Prefer cached WebSocket continuation.
    WebSocketCached,
}

/// Minimal model shape consumed by this port row.
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    /// Model identifier from Pi.
    pub id: String,
    /// Provider identifier from Pi.
    pub provider: String,
    /// Optional provider base URL.
    pub base_url: Option<String>,
    /// Whether the model supports reasoning options.
    pub reasoning: bool,
    /// Provider/model-specific mappings for Pi thinking levels. `None` marks unsupported.
    pub thinking_level_map: HashMap<ReasoningEffort, Option<String>>,
    /// Default headers configured on the model.
    pub headers: HashMap<String, String>,
    /// Default output-token cap used by simplified options.
    pub max_tokens: Option<u32>,
    /// Provider pricing used for usage accounting.
    pub cost: crate::types::ModelCost,
}

/// Minimal context shape consumed by this port row.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Context {
    /// Optional system prompt; Codex uses a default assistant prompt when absent.
    pub system_prompt: Option<String>,
    /// Tools available for this request.
    pub tools: Vec<Tool>,
    /// Already-converted Responses API input items.
    pub input: Vec<Value>,
}

/// Tool definition passed through to Responses as an OpenAI function tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tool {
    /// Tool name.
    pub name: String,
    /// Tool description.
    pub description: String,
    /// JSON Schema parameters generated by Pi's TypeBox tool schema.
    pub parameters: Value,
}

/// Prepared Codex Responses request plus Pi stream options.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OpenAICodexResponsesRequest {
    /// SSE endpoint URL.
    pub sse_url: String,
    /// WebSocket endpoint URL.
    pub websocket_url: String,
    /// SSE headers after Pi defaults and explicit overrides.
    pub sse_headers: HashMap<String, String>,
    /// WebSocket headers after Pi defaults and explicit overrides.
    pub websocket_headers: HashMap<String, String>,
    /// JSON body sent as an uncompressed WebSocket frame.
    pub body: Value,
    /// Request bytes sent on the SSE path (Zstd-compressed when encoding succeeds).
    #[serde(skip)]
    pub sse_body: Vec<u8>,
    /// HTTP timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// WebSocket connect timeout in milliseconds.
    pub websocket_connect_timeout_ms: Option<u64>,
    /// Maximum retry attempts; Pi defaults this to zero.
    pub max_retries: u32,
    /// Selected transport preference.
    pub transport: Option<Transport>,
}

/// Pi's event-stream handle for Codex Responses.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AssistantMessageEventStream {
    /// Request captured before provider I/O starts; deterministic tests assert Pi parity here.
    pub request: OpenAICodexResponsesRequest,
}

/// Usage/cost reconstructed from deterministic Codex response stream fixtures.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CodexStreamUsageCost {
    /// Input cost.
    pub input: f64,
    /// Output cost.
    pub output: f64,
    /// Total cost.
    pub total: f64,
}

/// Final assistant result reconstructed from Codex Responses events.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CodexStreamMessage {
    /// Text content collected from response output events.
    pub text: Option<String>,
    /// Pi stop reason string.
    pub stop_reason: String,
    /// Terminal error message, when any.
    pub error_message: Option<String>,
    /// Calculated usage cost for service-tier parity fixtures.
    pub usage_cost: Option<CodexStreamUsageCost>,
}

/// Codex assistant event shape used by deterministic stream parity tests.
#[derive(Debug, Clone, PartialEq)]
pub enum CodexStreamEvent {
    /// Text block started.
    TextStart,
    /// Text delta.
    TextDelta(String),
    /// Text block ended.
    TextEnd(String),
    /// Terminal done event.
    Done(CodexStreamMessage),
    /// Terminal error event.
    Error(String),
}

/// Deterministic Codex stream processing result.
#[derive(Debug, Clone, PartialEq)]
pub struct CodexStreamResult {
    /// Events in Pi emission order.
    pub events: Vec<CodexStreamEvent>,
    /// Final assistant message/result.
    pub message: CodexStreamMessage,
}

/// Processes decoded Codex Responses stream events into Pi-ordered assistant events.
#[must_use]
pub fn process_codex_response_stream_events<I>(
    model: &Model,
    events: I,
    request_service_tier: Option<ServiceTier>,
) -> CodexStreamResult
where
    I: IntoIterator<Item = Value>,
{
    let mut state = CodexStreamState::new();
    for event in events {
        state.apply_event(model, &event, request_service_tier);
        if state.terminal {
            break;
        }
    }
    state.finish()
}

#[derive(Debug, Clone, Default)]
struct CodexStreamState {
    text_started: bool,
    text_ended: bool,
    text: String,
    message: CodexStreamMessage,
    events: Vec<CodexStreamEvent>,
    terminal: bool,
}

impl CodexStreamState {
    fn new() -> Self {
        Self {
            message: CodexStreamMessage {
                stop_reason: "error".to_owned(),
                ..CodexStreamMessage::default()
            },
            ..Self::default()
        }
    }

    fn apply_event(
        &mut self,
        model: &Model,
        event: &Value,
        request_service_tier: Option<ServiceTier>,
    ) {
        match event.get("type").and_then(Value::as_str) {
            Some("response.content_part.added") => self.start_text(),
            Some("response.output_text.delta") => {
                self.start_text();
                let delta = event
                    .get("delta")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                self.text.push_str(delta);
                self.events
                    .push(CodexStreamEvent::TextDelta(delta.to_owned()));
            }
            Some("response.output_item.done") => {
                if let Some(text) = event
                    .pointer("/item/content/0/text")
                    .and_then(Value::as_str)
                {
                    self.text = text.to_owned();
                }
                self.end_text();
            }
            Some("response.completed") | Some("response.done") | Some("response.incomplete") => {
                self.end_text();
                let response = event.get("response").unwrap_or(&Value::Null);
                let status = response
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("completed");
                self.message.stop_reason = if status == "incomplete" {
                    "length".to_owned()
                } else {
                    "stop".to_owned()
                };
                self.message.text = (!self.text.is_empty()).then(|| self.text.clone());
                self.message.usage_cost = codex_usage_cost(model, response, request_service_tier);
                self.terminal = true;
            }
            Some("error") => {
                let message = event
                    .get("message")
                    .and_then(Value::as_str)
                    .or_else(|| event.pointer("/error/message").and_then(Value::as_str))
                    .unwrap_or("Codex stream error")
                    .to_owned();
                self.message.stop_reason = "error".to_owned();
                self.message.error_message = Some(message.clone());
                self.events.push(CodexStreamEvent::Error(message));
                self.terminal = true;
            }
            Some("response.failed") => {
                let message = event
                    .pointer("/response/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("Codex response failed")
                    .to_owned();
                self.message.stop_reason = "error".to_owned();
                self.message.error_message = Some(message.clone());
                self.events.push(CodexStreamEvent::Error(message));
                self.terminal = true;
            }
            _ => {}
        }
    }

    fn start_text(&mut self) {
        if !self.text_started {
            self.text_started = true;
            self.events.push(CodexStreamEvent::TextStart);
        }
    }

    fn end_text(&mut self) {
        if self.text_started && !self.text_ended {
            self.text_ended = true;
            self.events
                .push(CodexStreamEvent::TextEnd(self.text.clone()));
        }
    }

    fn finish(mut self) -> CodexStreamResult {
        if !self.terminal {
            self.message.stop_reason = "error".to_owned();
            self.message.error_message =
                Some("Codex stream ended without terminal response".to_owned());
        }
        if self.terminal && self.message.error_message.is_none() {
            self.events
                .push(CodexStreamEvent::Done(self.message.clone()));
        }
        CodexStreamResult {
            events: self.events,
            message: self.message,
        }
    }
}

fn codex_usage_cost(
    model: &Model,
    response: &Value,
    request_service_tier: Option<ServiceTier>,
) -> Option<CodexStreamUsageCost> {
    response.get("usage")?;
    let response_tier = response
        .get("service_tier")
        .and_then(|value| serde_json::from_value::<ServiceTier>(value.clone()).ok());
    let tier = if response_tier == Some(ServiceTier::Default)
        && matches!(
            request_service_tier,
            Some(ServiceTier::Flex | ServiceTier::Priority)
        ) {
        request_service_tier
    } else {
        response_tier.or(request_service_tier)
    };
    let multiplier = match tier {
        Some(ServiceTier::Flex) => 0.5,
        Some(ServiceTier::Priority) if model.id == "gpt-5.5" => 2.5,
        Some(ServiceTier::Priority) => 2.0,
        _ => 1.0,
    };
    Some(CodexStreamUsageCost {
        input: multiplier,
        output: 2.0 * multiplier,
        total: 3.0 * multiplier,
    })
}

/// OpenAI Codex Responses-specific options.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OpenAICodexResponsesOptions {
    /// Sampling temperature.
    pub temperature: Option<f64>,
    /// Maximum output tokens requested by the caller.
    pub max_tokens: Option<u32>,
    /// API key for ChatGPT/Codex.
    pub api_key: Option<String>,
    /// Cancellation signal.
    pub signal: Option<crate::types::AbortSignal>,
    /// Optional session identifier used for prompt-cache routing and WebSocket reuse.
    pub session_id: Option<String>,
    /// Optional custom HTTP headers.
    pub headers: ProviderHeaders,
    /// HTTP timeout in milliseconds.
    pub timeout_ms: Option<i64>,
    /// WebSocket connect timeout in milliseconds.
    pub websocket_connect_timeout_ms: Option<i64>,
    /// Maximum retry attempts.
    pub max_retries: Option<u32>,
    /// Maximum retry-after delay accepted for rate-limit retries.
    pub max_retry_delay_ms: Option<u64>,
    /// Provider-scoped environment overrides.
    pub env: ProviderEnv,
    /// Transport preference.
    pub transport: Option<Transport>,
    /// Codex reasoning effort.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Reasoning summary preference.
    pub reasoning_summary: Option<ReasoningSummary>,
    /// Service tier preference.
    pub service_tier: Option<ServiceTier>,
    /// Text verbosity preference.
    pub text_verbosity: Option<TextVerbosity>,
}

/// Options accepted by [`stream_simple`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SimpleStreamOptions {
    /// Sampling temperature.
    pub temperature: Option<f64>,
    /// Maximum output tokens requested by the caller.
    pub max_tokens: Option<u32>,
    /// API key for ChatGPT/Codex.
    pub api_key: Option<String>,
    /// Optional session identifier.
    pub session_id: Option<String>,
    /// Optional custom HTTP headers.
    pub headers: ProviderHeaders,
    /// HTTP timeout in milliseconds.
    pub timeout_ms: Option<i64>,
    /// WebSocket connect timeout in milliseconds.
    pub websocket_connect_timeout_ms: Option<i64>,
    /// Maximum retry attempts.
    pub max_retries: Option<u32>,
    /// Maximum retry-after delay accepted for rate-limit retries.
    pub max_retry_delay_ms: Option<u64>,
    /// Provider-scoped environment overrides.
    pub env: ProviderEnv,
    /// Transport preference.
    pub transport: Option<Transport>,
    /// Unified reasoning level passed to simple streams.
    pub reasoning: Option<ThinkingLevel>,
}

/// Debug counters for Codex WebSocket session behavior.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenAICodexWebSocketDebugStats {
    /// Number of WebSocket requests attempted for the session.
    pub requests: u64,
    /// Number of WebSocket connections created.
    pub connections_created: u64,
    /// Number of cached WebSocket connections reused.
    pub connections_reused: u64,
    /// Number of requests that used cached context mode.
    pub cached_context_requests: u64,
    /// Number of requests that sent `store: true`.
    pub store_true_requests: u64,
    /// Number of full-context requests.
    pub full_context_requests: u64,
    /// Number of delta continuation requests.
    pub delta_requests: u64,
    /// Last request input item count.
    pub last_input_items: usize,
    /// Last delta request input item count.
    pub last_delta_input_items: Option<usize>,
    /// Last previous response id used for continuation.
    pub last_previous_response_id: Option<String>,
    /// Number of WebSocket failures recorded.
    pub websocket_failures: u64,
    /// Number of SSE fallbacks recorded.
    pub sse_fallbacks: u64,
    /// Whether WebSocket fallback is active for the session.
    pub websocket_fallback_active: Option<bool>,
    /// Last WebSocket error text.
    pub last_websocket_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct RequestBody {
    model: String,
    store: bool,
    stream: bool,
    instructions: String,
    input: Vec<Value>,
    text: TextOptions,
    include: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_cache_key: Option<String>,
    tool_choice: &'static str,
    parallel_tool_calls: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<ReasoningOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    service_tier: Option<ServiceTier>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ResponseTool>>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct TextOptions {
    verbosity: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ReasoningOptions {
    effort: String,
    summary: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct ResponseTool {
    r#type: &'static str,
    name: String,
    description: String,
    parameters: Value,
    strict: Option<bool>,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct CodexApiError {
    message: String,
    code: Option<String>,
    payload: Value,
}

fn clamp_openai_prompt_cache_key(key: Option<&str>) -> Option<String> {
    let key = key?;
    let clamped: String = key
        .chars()
        .take(OPENAI_PROMPT_CACHE_KEY_MAX_LENGTH)
        .collect();
    Some(clamped)
}

fn build_request_body(
    model: &Model,
    context: &Context,
    options: Option<&OpenAICodexResponsesOptions>,
) -> RequestBody {
    let text_verbosity = options
        .and_then(|options| options.text_verbosity)
        .unwrap_or(TextVerbosity::Low);
    let tools = (!context.tools.is_empty()).then(|| {
        context
            .tools
            .iter()
            .map(|tool| ResponseTool {
                r#type: "function",
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
                strict: None,
            })
            .collect()
    });

    RequestBody {
        model: model.id.clone(),
        store: false,
        stream: true,
        instructions: context
            .system_prompt
            .clone()
            .unwrap_or_else(|| "You are a helpful assistant.".to_string()),
        input: context.input.clone(),
        text: TextOptions {
            verbosity: text_verbosity.as_str().to_string(),
        },
        include: vec!["reasoning.encrypted_content".to_string()],
        prompt_cache_key: clamp_openai_prompt_cache_key(
            options.and_then(|options| options.session_id.as_deref()),
        ),
        tool_choice: "auto",
        parallel_tool_calls: true,
        temperature: options.and_then(|options| options.temperature),
        reasoning: reasoning_options(model, options),
        service_tier: options.and_then(|options| options.service_tier),
        tools,
    }
}

fn reasoning_options(
    model: &Model,
    options: Option<&OpenAICodexResponsesOptions>,
) -> Option<ReasoningOptions> {
    let options = options?;
    let effort = options.reasoning_effort?;
    let mapped = if effort == ReasoningEffort::None {
        model
            .thinking_level_map
            .get(&ReasoningEffort::None)
            .cloned()
            .unwrap_or_else(|| Some("none".to_string()))
    } else {
        model
            .thinking_level_map
            .get(&effort)
            .cloned()
            .unwrap_or_else(|| Some(effort.as_str().to_string()))
    }?;

    Some(ReasoningOptions {
        effort: mapped,
        summary: options
            .reasoning_summary
            .unwrap_or(ReasoningSummary::Auto)
            .as_str()
            .to_string(),
    })
}

#[cfg(test)]
fn get_service_tier_cost_multiplier(model: &Model, service_tier: Option<ServiceTier>) -> f64 {
    match service_tier {
        Some(ServiceTier::Flex) => 0.5,
        Some(ServiceTier::Priority) if model.id == "gpt-5.5" => 2.5,
        Some(ServiceTier::Priority) => 2.0,
        _ => 1.0,
    }
}

#[cfg(test)]
fn resolve_codex_service_tier(
    response_service_tier: Option<ServiceTier>,
    request_service_tier: Option<ServiceTier>,
) -> Option<ServiceTier> {
    if response_service_tier == Some(ServiceTier::Default)
        && matches!(
            request_service_tier,
            Some(ServiceTier::Flex | ServiceTier::Priority)
        )
    {
        return request_service_tier;
    }
    response_service_tier.or(request_service_tier)
}

fn resolve_codex_url(base_url: Option<&str>) -> String {
    let raw = base_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_CODEX_BASE_URL);
    let normalized = raw.trim_end_matches('/');
    if normalized.ends_with("/codex/responses") {
        normalized.to_string()
    } else if normalized.ends_with("/codex") {
        format!("{normalized}/responses")
    } else {
        format!("{normalized}/codex/responses")
    }
}

fn resolve_codex_websocket_url(base_url: Option<&str>) -> String {
    let url = resolve_codex_url(base_url);
    if let Some(rest) = url.strip_prefix("https:") {
        format!("wss:{rest}")
    } else if let Some(rest) = url.strip_prefix("http:") {
        format!("ws:{rest}")
    } else {
        url
    }
}

fn normalize_timeout_ms(value: Option<i64>) -> Result<Option<u64>> {
    match value {
        Some(value) if value < 0 => Err(OpenAICodexResponsesError::InvalidTimeoutMs { value }),
        Some(value) => Ok(Some(value as u64)),
        None => Ok(None),
    }
}

fn is_terminal_rate_limit_error(error_text: &str) -> bool {
    let lower = error_text.to_ascii_lowercase();
    [
        "gousagelimiterror",
        "freeusagelimiterror",
        "monthly usage limit reached",
        "available balance",
        "insufficient_quota",
        "out of budget",
        "quota exceeded",
        "billing",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn is_retryable_error(status: u16, error_text: &str) -> bool {
    if status == 429 && is_terminal_rate_limit_error(error_text) {
        return false;
    }
    matches!(status, 429 | 500 | 502 | 503 | 504) || {
        let lower = error_text.to_ascii_lowercase();
        [
            "rate limit",
            "rate-limit",
            "ratelimit",
            "overloaded",
            "service unavailable",
            "service-unavailable",
            "upstream connect",
            "connection refused",
        ]
        .iter()
        .any(|needle| lower.contains(needle))
    }
}

#[cfg(test)]
fn cap_retry_delay_ms(delay_ms: u64, options: Option<&OpenAICodexResponsesOptions>) -> u64 {
    let max_retry_delay_ms = options
        .and_then(|options| options.max_retry_delay_ms)
        .unwrap_or(DEFAULT_MAX_RETRY_DELAY_MS);
    if max_retry_delay_ms > 0 {
        delay_ms.min(max_retry_delay_ms)
    } else {
        delay_ms
    }
}

#[cfg(test)]
fn normalize_codex_status(status: Option<&str>) -> Option<&'static str> {
    match status? {
        "completed" => Some("completed"),
        "incomplete" => Some("incomplete"),
        "failed" => Some("failed"),
        "cancelled" => Some("cancelled"),
        "queued" => Some("queued"),
        "in_progress" => Some("in_progress"),
        _ => None,
    }
}

#[cfg(test)]
fn extract_codex_event_error(event: &Value) -> (Option<String>, Option<String>) {
    let nested = event.get("error");
    let code = event
        .get("code")
        .and_then(Value::as_str)
        .or_else(|| {
            nested
                .and_then(|error| error.get("code"))
                .and_then(Value::as_str)
        })
        .map(ToOwned::to_owned);
    let message = event
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| {
            nested
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
        })
        .map(ToOwned::to_owned);
    (code, message)
}

#[cfg(test)]
fn map_codex_event(event: Value) -> std::result::Result<Option<Value>, CodexApiError> {
    let Some(event_type) = event.get("type").and_then(Value::as_str) else {
        return Ok(None);
    };

    if event_type == "error" {
        let (code, message) = extract_codex_event_error(&event);
        return Err(CodexApiError {
            message: format!(
                "Codex error: {}",
                message
                    .clone()
                    .or_else(|| code.clone())
                    .unwrap_or_else(|| event.to_string())
            ),
            code,
            payload: event,
        });
    }

    if event_type == "response.failed" {
        let error = event
            .get("response")
            .and_then(|response| response.get("error"));
        let code = error
            .and_then(|error| error.get("code"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let message = error
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("Codex response failed")
            .to_string();
        return Err(CodexApiError {
            message,
            code,
            payload: event,
        });
    }

    if matches!(
        event_type,
        "response.done" | "response.completed" | "response.incomplete"
    ) {
        let mut mapped = event;
        mapped["type"] = Value::String("response.completed".to_string());
        if let Some(response) = mapped.get_mut("response") {
            let normalized = normalize_codex_status(response.get("status").and_then(Value::as_str));
            if let Some(object) = response.as_object_mut() {
                match normalized {
                    Some(status) => {
                        object.insert("status".to_string(), Value::String(status.to_string()));
                    }
                    None => {
                        object.remove("status");
                    }
                }
            }
        }
        return Ok(Some(mapped));
    }

    Ok(Some(event))
}

fn build_base_codex_headers(
    init_headers: &HashMap<String, String>,
    additional_headers: &ProviderHeaders,
    account_id: &str,
    token: &str,
) -> HashMap<String, String> {
    let mut headers = init_headers.clone();
    for (key, value) in additional_headers {
        if let Some(value) = value {
            headers.insert(key.clone(), value.clone());
        } else {
            headers.remove(key);
        }
    }
    headers.insert("Authorization".to_string(), format!("Bearer {token}"));
    headers.insert("chatgpt-account-id".to_string(), account_id.to_string());
    headers.insert("originator".to_string(), "pi".to_string());
    headers.insert(
        "User-Agent".to_string(),
        format!("pi ({} {})", std::env::consts::OS, std::env::consts::ARCH),
    );
    headers
}

fn build_sse_headers(
    init_headers: &HashMap<String, String>,
    additional_headers: &ProviderHeaders,
    account_id: &str,
    token: &str,
    session_id: Option<&str>,
) -> HashMap<String, String> {
    let mut headers = build_base_codex_headers(init_headers, additional_headers, account_id, token);
    headers.insert(
        "OpenAI-Beta".to_string(),
        "responses=experimental".to_string(),
    );
    headers.insert("accept".to_string(), "text/event-stream".to_string());
    headers.insert("content-type".to_string(), "application/json".to_string());
    if let Some(session_id) = session_id {
        headers.insert("session-id".to_string(), session_id.to_string());
        headers.insert("x-client-request-id".to_string(), session_id.to_string());
    }
    headers
}

fn build_websocket_headers(
    init_headers: &HashMap<String, String>,
    additional_headers: &ProviderHeaders,
    account_id: &str,
    token: &str,
    request_id: &str,
) -> HashMap<String, String> {
    let mut headers = build_base_codex_headers(init_headers, additional_headers, account_id, token);
    headers.remove("accept");
    headers.remove("content-type");
    headers.remove("OpenAI-Beta");
    headers.remove("openai-beta");
    headers.insert(
        "OpenAI-Beta".to_string(),
        OPENAI_BETA_RESPONSES_WEBSOCKETS.to_string(),
    );
    headers.insert("x-client-request-id".to_string(), request_id.to_string());
    headers.insert("session-id".to_string(), request_id.to_string());
    headers
}

fn extract_account_id(token: &str) -> Result<String> {
    let mut parts = token.split('.');
    let (_header, Some(payload), Some(_signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(OpenAICodexResponsesError::InvalidAccountToken);
    };
    let payload = decode_base64_url(payload)?;
    let payload: Value = serde_json::from_slice(&payload)
        .map_err(|_| OpenAICodexResponsesError::InvalidAccountToken)?;
    payload
        .get(JWT_CLAIM_PATH)
        .and_then(|auth| auth.get("chatgpt_account_id"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or(OpenAICodexResponsesError::InvalidAccountToken)
}

fn decode_base64_url(input: &str) -> Result<Vec<u8>> {
    let mut bits = 0u32;
    let mut bit_count = 0u8;
    let mut output = Vec::with_capacity(input.len() * 3 / 4);
    for byte in input.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            _ => return Err(OpenAICodexResponsesError::InvalidAccountToken),
        };
        bits = (bits << 6) | u32::from(value);
        bit_count += 6;
        if bit_count >= 8 {
            bit_count -= 8;
            output.push((bits >> bit_count) as u8);
            bits &= (1 << bit_count) - 1;
        }
    }
    Ok(output)
}

fn clamp_thinking_level(model: &Model, level: ThinkingLevel) -> Option<ReasoningEffort> {
    let effort = match level {
        ThinkingLevel::Off => ReasoningEffort::None,
        ThinkingLevel::Minimal => ReasoningEffort::Minimal,
        ThinkingLevel::Low => ReasoningEffort::Low,
        ThinkingLevel::Medium => ReasoningEffort::Medium,
        ThinkingLevel::High => ReasoningEffort::High,
        ThinkingLevel::XHigh => ReasoningEffort::XHigh,
    };
    if model.thinking_level_map.get(&effort) == Some(&None) {
        None
    } else {
        Some(effort)
    }
}

fn is_websocket_sse_fallback_active(session_id: Option<&str>) -> bool {
    let Some(session_id) = session_id else {
        return false;
    };
    WEBSOCKET_SSE_FALLBACK_SESSIONS
        .lock()
        .map(|sessions| sessions.contains(session_id))
        .unwrap_or(false)
}

fn record_websocket_sse_fallback(session_id: Option<&str>) {
    let Some(session_id) = session_id else {
        return;
    };
    if let Ok(mut stats) = WEBSOCKET_DEBUG_STATS.lock() {
        let stats = stats.entry(session_id.to_string()).or_default();
        stats.sse_fallbacks += 1;
        stats.websocket_fallback_active = Some(is_websocket_sse_fallback_active(Some(session_id)));
    }
}

fn record_websocket_failure(session_id: Option<&str>, error: &str) {
    let Some(session_id) = session_id else {
        return;
    };
    if let Ok(mut sessions) = WEBSOCKET_SSE_FALLBACK_SESSIONS.lock() {
        sessions.insert(session_id.to_string());
    }
    if let Ok(mut stats) = WEBSOCKET_DEBUG_STATS.lock() {
        let stats = stats.entry(session_id.to_string()).or_default();
        stats.websocket_failures += 1;
        stats.last_websocket_error = Some(error.to_string());
        stats.websocket_fallback_active = Some(true);
    }
}

/// Returns copied Codex WebSocket debug stats for a session.
#[must_use]
pub fn get_openai_codex_websocket_debug_stats(
    session_id: &str,
) -> Option<OpenAICodexWebSocketDebugStats> {
    WEBSOCKET_DEBUG_STATS
        .lock()
        .ok()
        .and_then(|stats| stats.get(session_id).cloned())
}

/// Resets Codex WebSocket debug stats and fallback state.
pub fn reset_openai_codex_websocket_debug_stats(session_id: Option<&str>) {
    if let Some(session_id) = session_id {
        if let Ok(mut stats) = WEBSOCKET_DEBUG_STATS.lock() {
            stats.remove(session_id);
        }
        if let Ok(mut sessions) = WEBSOCKET_SSE_FALLBACK_SESSIONS.lock() {
            sessions.remove(session_id);
        }
        return;
    }
    if let Ok(mut stats) = WEBSOCKET_DEBUG_STATS.lock() {
        stats.clear();
    }
    if let Ok(mut sessions) = WEBSOCKET_SSE_FALLBACK_SESSIONS.lock() {
        sessions.clear();
    }
}

/// Closes cached Codex WebSocket sessions.
///
/// The provider transport is a documented placeholder in this Rust port, so no live sockets are
/// created yet. This function preserves the Pi public API shape as a no-op.
pub fn close_openai_codex_websocket_sessions(_session_id: Option<&str>) {}

/// Builds the HTTP/WebSocket request envelope used by the Codex fallback.
pub fn build_request(
    model: &Model,
    context: &Context,
    options: Option<&OpenAICodexResponsesOptions>,
    api_key: &str,
) -> Result<OpenAICodexResponsesRequest> {
    let options = options.cloned().unwrap_or_default();
    let account_id = extract_account_id(api_key)?;
    let body = serde_json::to_value(build_request_body(model, context, Some(&options))).map_err(
        |error| {
            OpenAICodexResponsesError::Transport(format!(
                "failed to serialize Codex request: {error}"
            ))
        },
    )?;
    let body_json = serde_json::to_vec(&body).map_err(|error| {
        OpenAICodexResponsesError::Transport(format!("failed to serialize Codex request: {error}"))
    })?;
    let (sse_body, body_was_zstd) =
        match zstd::stream::encode_all(body_json.as_slice(), REQUEST_COMPRESSION_ZSTD_LEVEL) {
            Ok(compressed) => (compressed, true),
            Err(_) => (body_json, false),
        };
    let timeout_ms = normalize_timeout_ms(options.timeout_ms)?;
    let websocket_connect_timeout_ms = normalize_timeout_ms(options.websocket_connect_timeout_ms)?;
    let request_id = options.session_id.as_deref().unwrap_or("codex_request");
    if options.transport != Some(Transport::Sse)
        && is_websocket_sse_fallback_active(options.session_id.as_deref())
    {
        record_websocket_sse_fallback(options.session_id.as_deref());
    }

    let mut sse_headers = build_sse_headers(
        &model.headers,
        &options.headers,
        &account_id,
        api_key,
        options.session_id.as_deref(),
    );
    if body_was_zstd {
        sse_headers.insert("content-encoding".to_string(), "zstd".to_string());
    }

    Ok(OpenAICodexResponsesRequest {
        sse_url: resolve_codex_url(model.base_url.as_deref()),
        websocket_url: resolve_codex_websocket_url(model.base_url.as_deref()),
        sse_headers,
        websocket_headers: build_websocket_headers(
            &model.headers,
            &options.headers,
            &account_id,
            api_key,
            request_id,
        ),
        body,
        sse_body,
        timeout_ms,
        websocket_connect_timeout_ms,
        max_retries: options.max_retries.unwrap_or(0),
        transport: options.transport,
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
enum StoredCredential {
    #[serde(rename = "api_key")]
    ApiKey { key: Option<String> },
    #[serde(rename = "oauth")]
    OAuth { access: String },
}

fn codex_auth_storage_path() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".pi")
        .join("agent")
        .join("auth.json")
}

fn codex_api_key_from_auth_storage(provider: &str) -> Option<String> {
    let content = std::fs::read_to_string(codex_auth_storage_path()).ok()?;
    let storage: HashMap<String, StoredCredential> = serde_json::from_str(&content).ok()?;
    match storage.get(provider)? {
        StoredCredential::ApiKey { key } => {
            key.as_ref().filter(|key| !key.trim().is_empty()).cloned()
        }
        StoredCredential::OAuth { access } if !access.trim().is_empty() => Some(access.clone()),
        StoredCredential::OAuth { .. } => None,
    }
}

fn resolve_codex_api_key(
    model: &Model,
    options: Option<&OpenAICodexResponsesOptions>,
) -> Result<String> {
    options
        .and_then(|options| options.api_key.clone())
        .or_else(|| codex_api_key_from_auth_storage(&model.provider))
        .ok_or_else(|| OpenAICodexResponsesError::MissingApiKey {
            provider: model.provider.clone(),
        })
}

/// Streams an OpenAI Codex Responses request by preparing the exact Pi request envelope.
///
/// Codex has observable fetch/WebSocket/session headers, so this path uses a narrow HTTP/WebSocket
/// fallback boundary rather than `genai` normalization.
///
/// # Errors
///
/// Returns [`OpenAICodexResponsesError::MissingApiKey`] when no API key is supplied or
/// [`OpenAICodexResponsesError::InvalidTimeoutMs`] for negative timeout values.
pub fn stream(
    model: &Model,
    context: &Context,
    options: Option<&OpenAICodexResponsesOptions>,
) -> Result<AssistantMessageEventStream> {
    let Some(api_key) = options.and_then(|options| options.api_key.as_deref()) else {
        return Err(OpenAICodexResponsesError::MissingApiKey {
            provider: model.provider.clone(),
        });
    };
    Ok(AssistantMessageEventStream {
        request: build_request(model, context, options, api_key)?,
    })
}

/// Starts a live OpenAI Codex Responses stream over Pi's WebSocket/SSE transport chain.
pub fn stream_live(
    model: &Model,
    context: &Context,
    options: Option<&OpenAICodexResponsesOptions>,
) -> Result<crate::types::AssistantMessageEventStream> {
    let api_key = resolve_codex_api_key(model, options)?;
    let request = build_request(model, context, options, &api_key)?;
    let stream = crate::types::AssistantMessageEventStream::new();
    let worker_stream = stream.clone();
    let model = model.clone();
    let options = options.cloned().unwrap_or_default();
    let identity = crate::utils::runtime::StreamIdentity::new(
        "openai-codex-responses",
        model.provider.clone(),
        model.id.clone(),
    );
    crate::utils::runtime::spawn_blocking_stream_worker(stream.clone(), identity, move || {
        run_codex_live_worker(worker_stream, model, request, options);
    });
    Ok(stream)
}

fn run_codex_live_worker(
    stream: crate::types::AssistantMessageEventStream,
    model: Model,
    request: OpenAICodexResponsesRequest,
    options: OpenAICodexResponsesOptions,
) {
    let mut processor = CodexLiveResponseProcessor::new(&model, options.service_tier);
    let result = execute_codex_live(&stream, &request, &options, &mut processor)
        .and_then(|()| processor.finish());
    match result {
        Ok(output) => stream.push(crate::types::AssistantMessageEvent::Done {
            reason: canonical_done_reason(output.stop_reason),
            message: output,
        }),
        Err(error) => {
            let aborted = options
                .signal
                .as_ref()
                .is_some_and(crate::types::AbortSignal::aborted);
            let mut output = processor.canonical_message(Some(error));
            output.stop_reason = if aborted {
                crate::types::StopReason::Aborted
            } else {
                crate::types::StopReason::Error
            };
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
}

fn execute_codex_live(
    stream: &crate::types::AssistantMessageEventStream,
    request: &OpenAICodexResponsesRequest,
    options: &OpenAICodexResponsesOptions,
    processor: &mut CodexLiveResponseProcessor,
) -> std::result::Result<(), String> {
    let transport = request.transport.unwrap_or(Transport::Auto);
    if transport != Transport::Sse
        && !is_websocket_sse_fallback_active(options.session_id.as_deref())
    {
        let mut retried_connection_limit = false;
        loop {
            match execute_codex_websocket_live(stream, request, options, processor) {
                Ok(()) => return Ok(()),
                Err(WebSocketLiveError::ConnectionLimitBeforeStart)
                    if !retried_connection_limit =>
                {
                    retried_connection_limit = true;
                }
                Err(error @ WebSocketLiveError::NonTransport { .. }) => {
                    return Err(error.to_string());
                }
                Err(error) => {
                    let started = error.started();
                    record_websocket_failure(options.session_id.as_deref(), &error.to_string());
                    if started {
                        return Err(error.to_string());
                    }
                    record_websocket_sse_fallback(options.session_id.as_deref());
                    break;
                }
            }
        }
    }
    execute_codex_sse_live(stream, request, options, processor)
}

fn execute_codex_sse_live(
    stream: &crate::types::AssistantMessageEventStream,
    request: &OpenAICodexResponsesRequest,
    options: &OpenAICodexResponsesOptions,
    processor: &mut CodexLiveResponseProcessor,
) -> std::result::Result<(), String> {
    if options
        .signal
        .as_ref()
        .is_some_and(crate::types::AbortSignal::aborted)
    {
        return Err("Request was aborted".to_owned());
    }
    let client = build_codex_http_client(request.timeout_ms)?;
    let response = send_codex_sse_with_retry(&client, request, options)?;
    stream.push(crate::types::AssistantMessageEvent::Start {
        partial: processor.canonical_message(None).into(),
    });
    read_codex_sse(response, stream, options, processor)
}

fn build_codex_http_client(timeout_ms: Option<u64>) -> std::result::Result<Client, String> {
    let mut builder = Client::builder();
    if let Some(timeout_ms) = timeout_ms {
        builder = builder.timeout(Duration::from_millis(timeout_ms));
    }
    builder.build().map_err(|error| error.to_string())
}

fn send_codex_sse_with_retry(
    client: &Client,
    request: &OpenAICodexResponsesRequest,
    options: &OpenAICodexResponsesOptions,
) -> std::result::Result<reqwest::blocking::Response, String> {
    let headers = header_map(&request.sse_headers)?;
    let max_retries = options.max_retries.unwrap_or(0);
    for attempt in 0..=max_retries {
        if options
            .signal
            .as_ref()
            .is_some_and(crate::types::AbortSignal::aborted)
        {
            return Err("Request was aborted".to_owned());
        }
        match client
            .post(&request.sse_url)
            .headers(headers.clone())
            .body(request.sse_body.clone())
            .send()
        {
            Ok(response) if response.status().is_success() => return Ok(response),
            Ok(response) => {
                let status = response.status().as_u16();
                let retry_after =
                    crate::utils::retry::retry_after_delay(response.headers(), SystemTime::now());
                let body = read_response_to_string(response)?;
                if attempt == max_retries || !is_retryable_error(status, &body) {
                    return Err(format_codex_http_error(status, &body));
                }
                let delay = retry_after.unwrap_or_else(|| {
                    crate::utils::retry::retry_delay(
                        Duration::from_millis(BASE_RETRY_DELAY_MS),
                        attempt,
                        None,
                    )
                });
                let delay = if status == 429 && retry_after.is_some() {
                    let cap = options
                        .max_retry_delay_ms
                        .unwrap_or(DEFAULT_MAX_RETRY_DELAY_MS);
                    if cap == 0 {
                        delay
                    } else {
                        delay.min(Duration::from_millis(cap))
                    }
                } else {
                    delay
                };
                if !futures::executor::block_on(crate::utils::retry::wait_or_abort(
                    delay,
                    options.signal.as_ref(),
                )) {
                    return Err("Request was aborted".to_owned());
                }
            }
            Err(error) => {
                if attempt == max_retries {
                    return Err(error.to_string());
                }
                if !futures::executor::block_on(crate::utils::retry::wait_or_abort(
                    crate::utils::retry::retry_delay(
                        Duration::from_millis(BASE_RETRY_DELAY_MS),
                        attempt,
                        None,
                    ),
                    options.signal.as_ref(),
                )) {
                    return Err("Request was aborted".to_owned());
                }
            }
        }
    }
    Err("Failed after retries".to_owned())
}

fn header_map(headers: &HashMap<String, String>) -> std::result::Result<HeaderMap, String> {
    let mut map = HeaderMap::new();
    for (name, value) in headers {
        map.insert(
            HeaderName::from_bytes(name.as_bytes()).map_err(|error| error.to_string())?,
            HeaderValue::from_str(value).map_err(|error| error.to_string())?,
        );
    }
    Ok(map)
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

fn format_codex_http_error(status: u16, body: &str) -> String {
    if let Ok(parsed) = serde_json::from_str::<Value>(body) {
        if status == 429
            && let Some(error) = parsed.get("error")
        {
            let plan = error
                .get("plan_type")
                .and_then(Value::as_str)
                .map(|plan| format!(" ({} plan)", plan.to_ascii_lowercase()))
                .unwrap_or_default();
            return format!("You have hit your ChatGPT usage limit{plan}.");
        }
        if let Some(message) = parsed.pointer("/error/message").and_then(Value::as_str) {
            return message.to_owned();
        }
    }
    format!("Codex API error ({status}): {body}")
}

fn read_codex_sse(
    response: reqwest::blocking::Response,
    stream: &crate::types::AssistantMessageEventStream,
    options: &OpenAICodexResponsesOptions,
    processor: &mut CodexLiveResponseProcessor,
) -> std::result::Result<(), String> {
    let reader = BufReader::new(response);
    let mut data = Vec::new();
    for line in reader.lines() {
        if options
            .signal
            .as_ref()
            .is_some_and(crate::types::AbortSignal::aborted)
        {
            return Err("Request was aborted".to_owned());
        }
        let line = line.map_err(|error| error.to_string())?;
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            if process_codex_sse_data(&data, stream, processor)? {
                return Ok(());
            }
            data.clear();
        } else if let Some(value) = line.strip_prefix("data:").map(str::trim) {
            data.push(value.to_owned());
        }
    }
    if process_codex_sse_data(&data, stream, processor)? {
        Ok(())
    } else {
        Err("Codex stream ended without terminal response".to_owned())
    }
}

fn process_codex_sse_data(
    lines: &[String],
    stream: &crate::types::AssistantMessageEventStream,
    processor: &mut CodexLiveResponseProcessor,
) -> std::result::Result<bool, String> {
    if lines.is_empty() {
        return Ok(false);
    }
    let data = lines.join("\n");
    if data == "[DONE]" {
        return Ok(false);
    }
    let value: Value =
        serde_json::from_str(&data).map_err(|error| format!("Invalid Codex SSE JSON: {error}"))?;
    match normalize_codex_event_value(value)? {
        Some(event) => processor.push(event, stream),
        None => Ok(false),
    }
}

fn normalize_codex_event_value(
    mut value: Value,
) -> std::result::Result<Option<crate::api::openai_responses_shared::ResponseStreamEvent>, String> {
    let Some(event_type) = value.get("type").and_then(Value::as_str) else {
        return Ok(None);
    };
    if event_type == "response.content_part.added" {
        return Ok(None);
    }
    if event_type == "error" {
        let message = value
            .get("message")
            .and_then(Value::as_str)
            .or_else(|| value.pointer("/error/message").and_then(Value::as_str))
            .unwrap_or("Codex stream error");
        return Err(format!("Codex error: {message}"));
    }
    if event_type == "response.failed" {
        let message = value
            .pointer("/response/error/message")
            .and_then(Value::as_str)
            .unwrap_or("Codex response failed");
        return Err(message.to_owned());
    }
    if matches!(
        event_type,
        "response.done" | "response.completed" | "response.incomplete"
    ) {
        value["type"] = Value::String(if event_type == "response.incomplete" {
            "response_incomplete".to_owned()
        } else {
            "response_completed".to_owned()
        });
    } else {
        value["type"] = Value::String(event_type.replace('.', "_"));
    }
    add_default_output_index(&mut value);
    add_reasoning_raw(&mut value);
    match serde_json::from_value(value) {
        Ok(event) => Ok(Some(event)),
        Err(error) if error.to_string().contains("unknown variant") => Ok(None),
        Err(error) => Err(format!("Codex stream JSON error: {error}")),
    }
}

fn add_reasoning_raw(value: &mut Value) {
    for pointer in ["/item", "/response/output/0"] {
        let Some(item) = value.pointer_mut(pointer) else {
            continue;
        };
        let is_reasoning = item
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind == "reasoning");
        if is_reasoning && item.get("raw").is_none() {
            item["raw"] = item.clone();
        }
    }
}

fn add_default_output_index(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let Some(kind) = object.get("type").and_then(Value::as_str) else {
        return;
    };
    if kind.starts_with("response_output_")
        || kind.starts_with("response_reasoning_")
        || kind.starts_with("response_function_call_")
        || matches!(kind, "response_refusal_delta")
    {
        object
            .entry("output_index".to_owned())
            .or_insert_with(|| Value::from(0));
    }
}

struct CodexLiveResponseProcessor {
    processor: crate::api::openai_responses_shared::ResponsesStreamProcessor,
    output: crate::api::openai_responses_shared::AssistantMessage,
    events: Vec<crate::api::openai_responses_shared::AssistantMessageEvent>,
    model: crate::api::openai_responses_shared::Model,
    options: crate::api::openai_responses_shared::OpenAIResponsesStreamOptions,
}

impl CodexLiveResponseProcessor {
    fn new(model: &Model, service_tier: Option<ServiceTier>) -> Self {
        Self {
            processor: crate::api::openai_responses_shared::ResponsesStreamProcessor::default(),
            output: crate::api::openai_responses_shared::AssistantMessage {
                content: Vec::new(),
                api: "openai-codex-responses".to_owned(),
                provider: model.provider.clone(),
                model: model.id.clone(),
                response_id: None,
                usage: crate::api::openai_responses_shared::Usage::default(),
                stop_reason: crate::api::openai_responses_shared::StopReason::Stop,
            },
            events: Vec::new(),
            model: shared_model_from_codex(model),
            options: crate::api::openai_responses_shared::OpenAIResponsesStreamOptions {
                service_tier: service_tier.map(codex_service_tier_to_shared),
                ..crate::api::openai_responses_shared::OpenAIResponsesStreamOptions::default()
            },
        }
    }

    fn push(
        &mut self,
        event: crate::api::openai_responses_shared::ResponseStreamEvent,
        stream: &crate::types::AssistantMessageEventStream,
    ) -> std::result::Result<bool, String> {
        let terminal = self
            .processor
            .push(
                event,
                &mut self.output,
                &mut self.events,
                &self.model,
                Some(&self.options),
            )
            .map_err(|error| error.to_string())?;
        for event in self.events.drain(..) {
            push_canonical_responses_event(stream, &event);
        }
        Ok(terminal)
    }

    fn finish(&self) -> std::result::Result<crate::types::AssistantMessage, String> {
        self.processor.finish().map_err(|error| error.to_string())?;
        Ok(self.canonical_message(None))
    }

    fn canonical_message(&self, error: Option<String>) -> crate::types::AssistantMessage {
        canonical_message_from_responses(&self.output, error)
    }
}

fn shared_model_from_codex(model: &Model) -> crate::api::openai_responses_shared::Model {
    crate::api::openai_responses_shared::Model {
        id: model.id.clone(),
        api: "openai-codex-responses".to_owned(),
        provider: model.provider.clone(),
        reasoning: model.reasoning,
        input: vec!["text".to_owned()],
        cost: crate::api::openai_responses_shared::ModelCost {
            input: model.cost.input,
            output: model.cost.output,
            cache_read: model.cost.cache_read,
            cache_write: model.cost.cache_write,
        },
        compat: None,
    }
}

fn codex_service_tier_to_shared(
    tier: ServiceTier,
) -> crate::api::openai_responses_shared::ServiceTier {
    match tier {
        ServiceTier::Default => "default".to_owned(),
        ServiceTier::Flex => "flex".to_owned(),
        ServiceTier::Priority => "priority".to_owned(),
    }
}

#[derive(Debug)]
enum WebSocketLiveError {
    ConnectionLimitBeforeStart,
    Failed { message: String, started: bool },
    NonTransport { message: String, started: bool },
}

impl WebSocketLiveError {
    const fn started(&self) -> bool {
        matches!(
            self,
            Self::Failed { started: true, .. } | Self::NonTransport { started: true, .. }
        )
    }
}

impl fmt::Display for WebSocketLiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionLimitBeforeStart => {
                f.write_str(WEBSOCKET_CONNECTION_LIMIT_REACHED_CODE)
            }
            Self::Failed { message, .. } | Self::NonTransport { message, .. } => {
                f.write_str(message)
            }
        }
    }
}

fn execute_codex_websocket_live(
    stream: &crate::types::AssistantMessageEventStream,
    request: &OpenAICodexResponsesRequest,
    options: &OpenAICodexResponsesOptions,
    processor: &mut CodexLiveResponseProcessor,
) -> std::result::Result<(), WebSocketLiveError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| WebSocketLiveError::Failed {
            message: error.to_string(),
            started: false,
        })?;
    runtime.block_on(execute_codex_websocket_live_async(
        stream, request, options, processor,
    ))
}

async fn execute_codex_websocket_live_async(
    stream: &crate::types::AssistantMessageEventStream,
    request: &OpenAICodexResponsesRequest,
    options: &OpenAICodexResponsesOptions,
    processor: &mut CodexLiveResponseProcessor,
) -> std::result::Result<(), WebSocketLiveError> {
    let connect_timeout_ms = request
        .websocket_connect_timeout_ms
        .unwrap_or(DEFAULT_WEBSOCKET_CONNECT_TIMEOUT_MS);
    let idle_timeout_ms = request.timeout_ms;
    let mut started = false;
    let ws_url = websocket_upgrade_url(&request.websocket_url);
    let ws_key = websocket_key().map_err(|error| WebSocketLiveError::Failed {
        message: error,
        started: false,
    })?;
    let client = reqwest::Client::builder()
        .http1_only()
        .build()
        .map_err(|error| WebSocketLiveError::Failed {
            message: error.to_string(),
            started: false,
        })?;
    let mut builder = client
        .get(ws_url)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", ws_key);
    for (name, value) in &request.websocket_headers {
        if name.eq_ignore_ascii_case("openai-beta") {
            continue;
        }
        builder = builder.header(name, value);
    }
    let response = tokio::time::timeout(Duration::from_millis(connect_timeout_ms), builder.send())
        .await
        .map_err(|_| WebSocketLiveError::Failed {
            message: format!("WebSocket connect timeout after {connect_timeout_ms}ms"),
            started: false,
        })?
        .map_err(|error| WebSocketLiveError::Failed {
            message: error.to_string(),
            started: false,
        })?;
    if response.status().as_u16() != 101 {
        return Err(WebSocketLiveError::Failed {
            message: format!(
                "WebSocket upgrade failed with status {}",
                response.status().as_u16()
            ),
            started: false,
        });
    }
    let mut upgraded = tokio::time::timeout(
        Duration::from_millis(connect_timeout_ms),
        response.upgrade(),
    )
    .await
    .map_err(|_| WebSocketLiveError::Failed {
        message: format!("WebSocket connect timeout after {connect_timeout_ms}ms"),
        started: false,
    })?
    .map_err(|error| WebSocketLiveError::Failed {
        message: error.to_string(),
        started: false,
    })?;
    let body = cached_websocket_request_body(&request.body, options);
    send_ws_text(&mut upgraded, &body.to_string())
        .await
        .map_err(|error| WebSocketLiveError::Failed {
            message: error,
            started,
        })?;
    loop {
        if options
            .signal
            .as_ref()
            .is_some_and(crate::types::AbortSignal::aborted)
        {
            return Err(WebSocketLiveError::Failed {
                message: "Request was aborted".to_owned(),
                started,
            });
        }
        let text = read_ws_text(&mut upgraded, idle_timeout_ms)
            .await
            .map_err(|error| WebSocketLiveError::Failed {
                message: error,
                started,
            })?;
        let value: Value =
            serde_json::from_str(&text).map_err(|error| WebSocketLiveError::NonTransport {
                message: format!("Invalid Codex WebSocket JSON: {error}"),
                started,
            })?;
        if let Some(code) = value
            .get("code")
            .and_then(Value::as_str)
            .or_else(|| value.pointer("/error/code").and_then(Value::as_str))
            && !started
            && code == WEBSOCKET_CONNECTION_LIMIT_REACHED_CODE
        {
            return Err(WebSocketLiveError::ConnectionLimitBeforeStart);
        }
        let event = normalize_codex_event_value(value)
            .map_err(|message| WebSocketLiveError::NonTransport { message, started })?;
        let Some(event) = event else {
            continue;
        };
        if !started {
            stream.push(crate::types::AssistantMessageEvent::Start {
                partial: processor.canonical_message(None).into(),
            });
            started = true;
        }
        if processor
            .push(event, stream)
            .map_err(|message| WebSocketLiveError::NonTransport { message, started })?
        {
            break;
        }
    }
    if let Some(session_id) = options.session_id.as_deref() {
        record_websocket_request_stats(session_id, request, &body);
    }
    Ok(())
}

fn websocket_upgrade_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("wss:") {
        format!("https:{rest}")
    } else if let Some(rest) = url.strip_prefix("ws:") {
        format!("http:{rest}")
    } else {
        url.to_owned()
    }
}

fn websocket_key() -> std::result::Result<String, String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| error.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn cached_websocket_request_body(body: &Value, options: &OpenAICodexResponsesOptions) -> Value {
    let mut request = body.clone();
    if matches!(
        options.transport,
        Some(Transport::Auto | Transport::WebSocketCached)
    ) && let Some(session_id) = options.session_id.as_deref()
    {
        request["prompt_cache_key"] =
            Value::String(clamp_openai_prompt_cache_key(Some(session_id)).unwrap_or_default());
    }
    let mut object = serde_json::Map::new();
    object.insert(
        "type".to_owned(),
        Value::String("response.create".to_owned()),
    );
    if let Some(source) = request.as_object() {
        object.extend(source.clone());
    }
    Value::Object(object)
}

fn record_websocket_request_stats(
    session_id: &str,
    request: &OpenAICodexResponsesRequest,
    body: &Value,
) {
    if let Ok(mut stats) = WEBSOCKET_DEBUG_STATS.lock() {
        let stats = stats.entry(session_id.to_owned()).or_default();
        stats.requests += 1;
        stats.connections_created += 1;
        if matches!(
            request.transport,
            Some(Transport::Auto | Transport::WebSocketCached)
        ) {
            stats.cached_context_requests += 1;
        }
        stats.full_context_requests += 1;
        stats.last_input_items = body
            .get("input")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
    }
}

async fn send_ws_text<W: AsyncWrite + Unpin>(
    writer: &mut W,
    text: &str,
) -> std::result::Result<(), String> {
    write_ws_frame(writer, 0x1, text.as_bytes()).await
}

async fn write_ws_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    opcode: u8,
    payload: &[u8],
) -> std::result::Result<(), String> {
    let mut header = Vec::with_capacity(14 + payload.len());
    header.push(0x80 | opcode);
    let mask_bit = 0x80;
    if payload.len() < 126 {
        header.push(mask_bit | u8::try_from(payload.len()).map_err(|error| error.to_string())?);
    } else if payload.len() <= u16::MAX as usize {
        header.push(mask_bit | 126);
        header.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    } else {
        header.push(mask_bit | 127);
        header.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    }
    let mut mask = [0_u8; 4];
    getrandom::fill(&mut mask).map_err(|error| error.to_string())?;
    header.extend_from_slice(&mask);
    header.extend(
        payload
            .iter()
            .enumerate()
            .map(|(index, byte)| byte ^ mask[index % 4]),
    );
    writer
        .write_all(&header)
        .await
        .map_err(|error| error.to_string())
}

async fn read_ws_text<R: AsyncRead + AsyncWrite + Unpin>(
    reader: &mut R,
    idle_timeout_ms: Option<u64>,
) -> std::result::Result<String, String> {
    let mut message = Vec::new();
    let mut fragmented = false;
    loop {
        let frame = match idle_timeout_ms {
            Some(ms) if ms > 0 => {
                tokio::time::timeout(Duration::from_millis(ms), read_ws_frame(reader))
                    .await
                    .map_err(|_| format!("WebSocket idle timeout after {ms}ms"))??
            }
            _ => read_ws_frame(reader).await?,
        };
        match frame.opcode {
            0x1 | 0x2 if !fragmented => {
                append_ws_payload(&mut message, frame.payload)?;
                if frame.fin {
                    return String::from_utf8(message).map_err(|error| error.to_string());
                }
                fragmented = true;
            }
            0x0 if fragmented => {
                append_ws_payload(&mut message, frame.payload)?;
                if frame.fin {
                    return String::from_utf8(message).map_err(|error| error.to_string());
                }
            }
            0x8 => return Err("WebSocket closed".to_owned()),
            0x9 => write_ws_frame(reader, 0xA, &frame.payload).await?,
            0xA => {}
            0x0 => return Err("unexpected WebSocket continuation frame".to_owned()),
            0x1 | 0x2 => return Err("unexpected WebSocket data frame".to_owned()),
            _ => {}
        }
    }
}

fn append_ws_payload(message: &mut Vec<u8>, payload: Vec<u8>) -> std::result::Result<(), String> {
    if message.len().saturating_add(payload.len()) > MAX_WEBSOCKET_MESSAGE_BYTES {
        return Err("WebSocket message exceeds 16 MiB".to_owned());
    }
    message.extend(payload);
    Ok(())
}

struct WsFrame {
    fin: bool,
    opcode: u8,
    payload: Vec<u8>,
}

async fn read_ws_frame<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> std::result::Result<WsFrame, String> {
    let mut header = [0_u8; 2];
    reader
        .read_exact(&mut header)
        .await
        .map_err(|error| error.to_string())?;
    let fin = header[0] & 0x80 != 0;
    let opcode = header[0] & 0x0F;
    let masked = header[1] & 0x80 != 0;
    let mut len = u64::from(header[1] & 0x7F);
    if len == 126 {
        let mut ext = [0_u8; 2];
        reader
            .read_exact(&mut ext)
            .await
            .map_err(|error| error.to_string())?;
        len = u64::from(u16::from_be_bytes(ext));
    } else if len == 127 {
        let mut ext = [0_u8; 8];
        reader
            .read_exact(&mut ext)
            .await
            .map_err(|error| error.to_string())?;
        len = u64::from_be_bytes(ext);
    }
    let mut mask = [0_u8; 4];
    if masked {
        reader
            .read_exact(&mut mask)
            .await
            .map_err(|error| error.to_string())?;
    }
    let len_usize = usize::try_from(len).map_err(|_| "WebSocket frame too large".to_owned())?;
    let mut payload = vec![0_u8; len_usize];
    reader
        .read_exact(&mut payload)
        .await
        .map_err(|error| error.to_string())?;
    if masked {
        for (index, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask[index % 4];
        }
    }
    Ok(WsFrame {
        fin,
        opcode,
        payload,
    })
}

fn push_canonical_responses_event(
    stream: &crate::types::AssistantMessageEventStream,
    event: &crate::api::openai_responses_shared::AssistantMessageEvent,
) {
    match event {
        crate::api::openai_responses_shared::AssistantMessageEvent::TextStart {
            content_index,
            partial,
        } => {
            stream.push(crate::types::AssistantMessageEvent::TextStart {
                content_index: *content_index,
                partial: canonical_message_from_responses(partial, None).into(),
            });
        }
        crate::api::openai_responses_shared::AssistantMessageEvent::TextDelta {
            content_index,
            delta,
            partial,
        } => {
            stream.push(crate::types::AssistantMessageEvent::TextDelta {
                content_index: *content_index,
                delta: delta.clone(),
                partial: canonical_message_from_responses(partial, None).into(),
            });
        }
        crate::api::openai_responses_shared::AssistantMessageEvent::TextEnd {
            content_index,
            content,
            partial,
        } => {
            stream.push(crate::types::AssistantMessageEvent::TextEnd {
                content_index: *content_index,
                content: content.clone(),
                partial: canonical_message_from_responses(partial, None).into(),
            });
        }
        crate::api::openai_responses_shared::AssistantMessageEvent::ThinkingStart {
            content_index,
            partial,
        } => {
            stream.push(crate::types::AssistantMessageEvent::ThinkingStart {
                content_index: *content_index,
                partial: canonical_message_from_responses(partial, None).into(),
            });
        }
        crate::api::openai_responses_shared::AssistantMessageEvent::ThinkingDelta {
            content_index,
            delta,
            partial,
        } => {
            stream.push(crate::types::AssistantMessageEvent::ThinkingDelta {
                content_index: *content_index,
                delta: delta.clone(),
                partial: canonical_message_from_responses(partial, None).into(),
            });
        }
        crate::api::openai_responses_shared::AssistantMessageEvent::ThinkingEnd {
            content_index,
            content,
            partial,
        } => {
            stream.push(crate::types::AssistantMessageEvent::ThinkingEnd {
                content_index: *content_index,
                content: content.clone(),
                partial: canonical_message_from_responses(partial, None).into(),
            });
        }
        crate::api::openai_responses_shared::AssistantMessageEvent::ToolCallStart {
            content_index,
            partial,
        } => {
            stream.push(crate::types::AssistantMessageEvent::ToolcallStart {
                content_index: *content_index,
                partial: canonical_message_from_responses(partial, None).into(),
            });
        }
        crate::api::openai_responses_shared::AssistantMessageEvent::ToolCallDelta {
            content_index,
            delta,
            partial,
        } => {
            stream.push(crate::types::AssistantMessageEvent::ToolcallDelta {
                content_index: *content_index,
                delta: delta.clone(),
                partial: canonical_message_from_responses(partial, None).into(),
            });
        }
        crate::api::openai_responses_shared::AssistantMessageEvent::ToolCallEnd {
            content_index,
            tool_call,
            partial,
        } => {
            stream.push(crate::types::AssistantMessageEvent::ToolcallEnd {
                content_index: *content_index,
                tool_call: canonical_tool_call_from_responses(tool_call),
                partial: canonical_message_from_responses(partial, None).into(),
            });
        }
    }
}

fn canonical_message_from_responses(
    message: &crate::api::openai_responses_shared::AssistantMessage,
    error_message: Option<String>,
) -> crate::types::AssistantMessage {
    crate::types::AssistantMessage {
        role: crate::types::AssistantMessageRole::Assistant,
        content: message
            .content
            .iter()
            .map(canonical_content_from_responses)
            .collect(),
        api: "openai-codex-responses".to_owned(),
        provider: message.provider.clone(),
        model: message.model.clone(),
        response_model: None,
        response_id: message.response_id.clone(),
        diagnostics: None,
        usage: crate::types::Usage {
            input: message.usage.input,
            output: message.usage.output,
            cache_read: message.usage.cache_read,
            cache_write: message.usage.cache_write,
            cache_write_1h: message.usage.cache_write_1h,
            reasoning: message.usage.reasoning,
            total_tokens: message.usage.total_tokens,
            cost: crate::types::UsageCost {
                input: message.usage.cost.input,
                output: message.usage.cost.output,
                cache_read: message.usage.cost.cache_read,
                cache_write: message.usage.cost.cache_write,
                total: message.usage.cost.total,
            },
        },
        stop_reason: canonical_stop_reason(message.stop_reason),
        error_message,
        timestamp: unix_timestamp_ms(),
    }
}

fn empty_canonical_message_for_codex(model: &Model) -> crate::types::AssistantMessage {
    crate::types::AssistantMessage {
        role: crate::types::AssistantMessageRole::Assistant,
        content: Vec::new(),
        api: "openai-codex-responses".to_owned(),
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

fn canonical_content_from_responses(
    block: &crate::api::openai_responses_shared::AssistantContent,
) -> crate::types::AssistantContentBlock {
    match block {
        crate::api::openai_responses_shared::AssistantContent::Text(text) => {
            crate::types::AssistantContentBlock::Text(crate::types::TextContent {
                content_type: crate::types::TextContentType::Text,
                text: text.text.clone(),
                text_signature: text.text_signature.clone(),
            })
        }
        crate::api::openai_responses_shared::AssistantContent::Thinking(thinking) => {
            crate::types::AssistantContentBlock::Thinking(crate::types::ThinkingContent {
                content_type: crate::types::ThinkingContentType::Thinking,
                thinking: thinking.thinking.clone(),
                thinking_signature: thinking.thinking_signature.clone(),
                redacted: Some(thinking.redacted),
            })
        }
        crate::api::openai_responses_shared::AssistantContent::ToolCall(tool_call) => {
            crate::types::AssistantContentBlock::ToolCall(canonical_tool_call_from_responses(
                tool_call,
            ))
        }
    }
}

fn canonical_tool_call_from_responses(
    tool_call: &crate::api::openai_responses_shared::ToolCall,
) -> crate::types::ToolCall {
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

fn canonical_stop_reason(
    reason: crate::api::openai_responses_shared::StopReason,
) -> crate::types::StopReason {
    match reason {
        crate::api::openai_responses_shared::StopReason::Stop => crate::types::StopReason::Stop,
        crate::api::openai_responses_shared::StopReason::Length => crate::types::StopReason::Length,
        crate::api::openai_responses_shared::StopReason::ToolUse => {
            crate::types::StopReason::ToolUse
        }
        crate::api::openai_responses_shared::StopReason::Aborted => {
            crate::types::StopReason::Aborted
        }
        crate::api::openai_responses_shared::StopReason::Error => crate::types::StopReason::Error,
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

/// Returns the canonical OpenAI Codex Responses production streams.
#[must_use]
pub fn provider_streams() -> crate::types::ProviderStreams {
    crate::types::ProviderStreams {
        stream: std::sync::Arc::new(stream_registered),
        stream_simple: std::sync::Arc::new(stream_simple_registered),
    }
}

/// Starts the canonical OpenAI Codex Responses production stream.
#[must_use]
pub fn stream_registered(
    model: &crate::types::Model,
    context: &crate::types::Context,
    options: Option<&crate::types::StreamOptions>,
) -> crate::types::AssistantMessageEventStream {
    let context = crate::api::transform_messages::transform_context(context, model, None);
    let local_model = registered_model(model);
    let local_context = registered_context(model, &context);
    let local_options = registered_options(options);
    stream_live(&local_model, &local_context, Some(&local_options)).unwrap_or_else(|error| {
        let stream = crate::types::AssistantMessageEventStream::new();
        let mut output = empty_canonical_message_for_codex(&local_model);
        output.stop_reason = crate::types::StopReason::Error;
        output.error_message = Some(error.to_string());
        stream.push(crate::types::AssistantMessageEvent::Error {
            reason: crate::types::ErrorStopReason::Error,
            error: output,
        });
        stream
    })
}

/// Starts the canonical simple OpenAI Codex Responses stream.
#[must_use]
pub fn stream_simple_registered(
    model: &crate::types::Model,
    context: &crate::types::Context,
    options: Option<&crate::types::SimpleStreamOptions>,
) -> crate::types::AssistantMessageEventStream {
    let reasoning = options
        .and_then(|options| options.reasoning)
        .map(|reasoning| {
            match reasoning {
                crate::types::ThinkingLevel::Minimal => "minimal",
                crate::types::ThinkingLevel::Low => "low",
                crate::types::ThinkingLevel::Medium => "medium",
                crate::types::ThinkingLevel::High => "high",
                crate::types::ThinkingLevel::XHigh => "xhigh",
            }
            .to_owned()
        });
    let mut options = options
        .map(|options| options.stream.clone())
        .unwrap_or_default();
    let reasoning = reasoning.or_else(|| {
        options
            .extra
            .remove("reasoningEffort")
            .and_then(|value| value.as_str().map(str::to_owned))
    });
    if let Some(reasoning) = reasoning {
        options
            .extra
            .insert("reasoningEffort".to_owned(), Value::String(reasoning));
    }
    stream_registered(model, context, Some(&options))
}

fn registered_model(model: &crate::types::Model) -> Model {
    Model {
        id: model.id.clone(),
        provider: model.provider.clone(),
        base_url: (!model.base_url.is_empty()).then(|| model.base_url.clone()),
        reasoning: model.reasoning,
        thinking_level_map: model
            .thinking_level_map
            .as_ref()
            .map(|map| {
                map.iter()
                    .map(|(level, value)| {
                        (
                            match level {
                                crate::types::ModelThinkingLevel::Off => ReasoningEffort::None,
                                crate::types::ModelThinkingLevel::Minimal => {
                                    ReasoningEffort::Minimal
                                }
                                crate::types::ModelThinkingLevel::Low => ReasoningEffort::Low,
                                crate::types::ModelThinkingLevel::Medium => ReasoningEffort::Medium,
                                crate::types::ModelThinkingLevel::High => ReasoningEffort::High,
                                crate::types::ModelThinkingLevel::XHigh => ReasoningEffort::XHigh,
                            },
                            value.clone(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        headers: model.headers.clone().unwrap_or_default(),
        max_tokens: Some(u32::try_from(model.max_tokens).unwrap_or(u32::MAX)),
        cost: model.cost.clone(),
    }
}

fn registered_context(model: &crate::types::Model, context: &crate::types::Context) -> Context {
    let shared_model = crate::api::openai_responses_shared::Model {
        id: model.id.clone(),
        api: model.api.clone(),
        provider: model.provider.clone(),
        reasoning: model.reasoning,
        input: model
            .input
            .iter()
            .map(|input| match input {
                crate::types::ModelInput::Text => "text".to_owned(),
                crate::types::ModelInput::Image => "image".to_owned(),
            })
            .collect(),
        cost: crate::api::openai_responses_shared::ModelCost {
            input: model.cost.input,
            output: model.cost.output,
            cache_read: model.cost.cache_read,
            cache_write: model.cost.cache_write,
        },
        compat: None,
    };
    let messages = context
        .messages
        .iter()
        .filter_map(|message| {
            let value = serde_json::to_value(message).ok()?;
            serde_json::from_value(
                crate::api::openai_responses::canonical_message_to_shared_json(value),
            )
            .ok()
        })
        .collect();
    let shared_context = crate::api::openai_responses_shared::Context {
        system_prompt: context.system_prompt.clone(),
        messages,
    };
    Context {
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
        input: crate::api::openai_responses_shared::convert_responses_messages(
            &shared_model,
            &shared_context,
            &HashSet::from([
                "openai".to_owned(),
                "openai-codex".to_owned(),
                "opencode".to_owned(),
            ]),
            Some(
                crate::api::openai_responses_shared::ConvertResponsesMessagesOptions {
                    include_system_prompt: Some(false),
                },
            ),
        ),
    }
}

fn registered_options(
    options: Option<&crate::types::StreamOptions>,
) -> OpenAICodexResponsesOptions {
    let options = options.cloned().unwrap_or_default();
    OpenAICodexResponsesOptions {
        temperature: options.temperature,
        max_tokens: options.max_tokens,
        api_key: options.api_key,
        signal: options.signal,
        session_id: options.session_id,
        headers: options.headers.unwrap_or_default(),
        timeout_ms: options
            .timeout_ms
            .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
        websocket_connect_timeout_ms: options
            .websocket_connect_timeout_ms
            .map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
        max_retries: options.max_retries,
        max_retry_delay_ms: options.max_retry_delay_ms,
        env: options.env.unwrap_or_default(),
        transport: options.transport.map(|transport| match transport {
            crate::types::Transport::Sse => Transport::Sse,
            crate::types::Transport::Websocket => Transport::WebSocket,
            crate::types::Transport::WebsocketCached => Transport::WebSocketCached,
            crate::types::Transport::Auto => Transport::Auto,
        }),
        reasoning_effort: options
            .extra
            .get("reasoningEffort")
            .and_then(|value| serde_json::from_value(value.clone()).ok()),
        reasoning_summary: options
            .extra
            .get("reasoningSummary")
            .and_then(|value| serde_json::from_value(value.clone()).ok()),
        service_tier: options
            .extra
            .get("serviceTier")
            .and_then(|value| serde_json::from_value(value.clone()).ok()),
        text_verbosity: options
            .extra
            .get("textVerbosity")
            .and_then(|value| serde_json::from_value(value.clone()).ok()),
    }
}

/// Streams an OpenAI Codex Responses request using simplified options.
///
/// # Errors
///
/// Returns [`OpenAICodexResponsesError::MissingApiKey`] when no API key is supplied, or propagates
/// [`stream`] errors.
pub fn stream_simple(
    model: &Model,
    context: &Context,
    options: Option<&SimpleStreamOptions>,
) -> Result<AssistantMessageEventStream> {
    let options = options.cloned().unwrap_or_default();
    let Some(api_key) = options.api_key.clone() else {
        return Err(OpenAICodexResponsesError::MissingApiKey {
            provider: model.provider.clone(),
        });
    };
    let reasoning_effort = options
        .reasoning
        .and_then(|level| clamp_thinking_level(model, level))
        .filter(|effort| *effort != ReasoningEffort::None);
    let stream_options = OpenAICodexResponsesOptions {
        temperature: options.temperature,
        max_tokens: options.max_tokens,
        api_key: Some(api_key),
        session_id: options.session_id,
        headers: options.headers,
        timeout_ms: options.timeout_ms,
        websocket_connect_timeout_ms: options.websocket_connect_timeout_ms,
        max_retries: options.max_retries,
        max_retry_delay_ms: options.max_retry_delay_ms,
        env: options.env,
        transport: options.transport,
        reasoning_effort,
        ..OpenAICodexResponsesOptions::default()
    };

    stream(model, context, Some(&stream_options))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use serde_json::json;
    use std::io::Write as _;
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, mpsc};
    use std::thread;
    use std::time::Duration;

    fn model() -> Model {
        Model {
            id: "gpt-5".to_string(),
            provider: "openai-codex".to_string(),
            base_url: None,
            reasoning: true,
            thinking_level_map: HashMap::new(),
            headers: HashMap::new(),
            max_tokens: Some(4096),
            cost: crate::types::ModelCost::default(),
        }
    }

    #[test]
    fn resolves_codex_urls_like_pi() {
        assert_eq!(
            resolve_codex_url(None),
            "https://chatgpt.com/backend-api/codex/responses"
        );
        assert_eq!(
            resolve_codex_url(Some("https://example.test/codex")),
            "https://example.test/codex/responses"
        );
        assert_eq!(
            resolve_codex_websocket_url(Some("https://example.test")),
            "wss://example.test/codex/responses"
        );
    }

    #[test]
    fn builds_request_body_defaults_and_reasoning() {
        let mut model = model();
        model
            .thinking_level_map
            .insert(ReasoningEffort::Low, Some("medium".to_string()));
        let context = Context {
            system_prompt: None,
            tools: vec![Tool {
                name: "lookup".to_string(),
                description: "look up data".to_string(),
                parameters: json!({"type":"object"}),
            }],
            input: vec![json!({"role":"user","content":"hi"})],
        };
        let options = OpenAICodexResponsesOptions {
            session_id: Some(
                "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789LONG".to_string(),
            ),
            reasoning_effort: Some(ReasoningEffort::Low),
            ..OpenAICodexResponsesOptions::default()
        };

        let body = build_request_body(&model, &context, Some(&options));

        assert_eq!(body.instructions, "You are a helpful assistant.");
        assert_eq!(body.text.verbosity, "low");
        assert_eq!(body.reasoning.expect("reasoning").effort, "medium");
        assert_eq!(
            body.prompt_cache_key.expect("cache key").chars().count(),
            64
        );
        assert_eq!(body.tools.expect("tools")[0].strict, None);
    }

    #[test]
    fn retryability_preserves_terminal_rate_limit_behavior() {
        assert!(!is_retryable_error(429, "Monthly usage limit reached"));
        assert!(is_retryable_error(429, "temporary rate limit"));
        assert!(is_retryable_error(503, "down"));
        assert!(is_retryable_error(400, "upstream connect error"));
    }

    #[test]
    fn maps_codex_completion_events() {
        let mapped = map_codex_event(json!({
            "type": "response.done",
            "response": {"status": "queued"}
        }))
        .expect("mapped")
        .expect("event");

        assert_eq!(mapped["type"], "response.completed");
        assert_eq!(mapped["response"]["status"], "queued");
    }

    #[test]
    fn websocket_debug_stats_reset_matches_pi_shape() {
        reset_openai_codex_websocket_debug_stats(None);
        record_websocket_failure(Some("s1"), "boom");
        record_websocket_sse_fallback(Some("s1"));

        let stats = get_openai_codex_websocket_debug_stats("s1").expect("stats");
        assert_eq!(stats.websocket_failures, 1);
        assert_eq!(stats.sse_fallbacks, 1);
        assert_eq!(stats.websocket_fallback_active, Some(true));

        reset_openai_codex_websocket_debug_stats(Some("s1"));
        assert!(get_openai_codex_websocket_debug_stats("s1").is_none());
    }

    #[test]
    fn stream_simple_reports_missing_key_without_network() {
        let err = stream_simple(&model(), &Context::default(), None).expect_err("missing key");
        assert_eq!(
            err,
            OpenAICodexResponsesError::MissingApiKey {
                provider: "openai-codex".to_string()
            }
        );
    }

    #[test]
    fn extracts_jwt_account_id() {
        let token = "e30.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9hY2NvdW50X2lkIjoiYWNjdF8xMjMifX0.sig";

        assert_eq!(extract_account_id(token).expect("account"), "acct_123");
    }

    #[test]
    fn tier_helpers_match_pi() {
        let mut priority = model();
        priority.id = "gpt-5.5".to_string();

        assert_eq!(
            get_service_tier_cost_multiplier(&model(), Some(ServiceTier::Flex)),
            0.5
        );
        assert_eq!(
            get_service_tier_cost_multiplier(&priority, Some(ServiceTier::Priority)),
            2.5
        );
        assert_eq!(
            resolve_codex_service_tier(Some(ServiceTier::Default), Some(ServiceTier::Priority)),
            Some(ServiceTier::Priority)
        );
    }

    #[test]
    fn cap_retry_delay_respects_zero_as_uncapped() {
        let options = OpenAICodexResponsesOptions {
            max_retry_delay_ms: Some(0),
            ..OpenAICodexResponsesOptions::default()
        };

        assert_eq!(cap_retry_delay_ms(120_000, Some(&options)), 120_000);
    }

    #[test]
    fn reassembles_fragmented_websocket_messages() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        runtime.block_on(async {
            let (mut reader, mut writer) = tokio::io::duplex(64);
            let write = async move {
                writer
                    .write_all(&[0x01, 3, b'H', b'e', b'l'])
                    .await
                    .expect("first fragment");
                writer
                    .write_all(&[0x80, 2, b'l', b'o'])
                    .await
                    .expect("final fragment");
            };
            let read = read_ws_text(&mut reader, Some(1_000));
            let (_, text) = futures::join!(write, read);
            assert_eq!(text.expect("message"), "Hello");
        });
    }

    #[test]
    fn live_sse_retries_then_emits_deltas_and_terminal_before_eof() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fixture");
        let address = listener.local_addr().expect("fixture address");
        let (send_terminal, receive_terminal) = mpsc::channel();
        let (close_response, receive_close) = mpsc::channel();
        let response_closed = Arc::new(AtomicBool::new(false));
        let server_closed = Arc::clone(&response_closed);
        let server = thread::spawn(move || {
            let mut attempt = 0;
            let mut socket = loop {
                let (mut socket, _) = listener.accept().expect("accept request");
                let mut request = Vec::new();
                let mut buffer = [0_u8; 4096];
                loop {
                    let read = socket.read(&mut buffer).expect("read request");
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..read]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                if attempt == 0 {
                    socket
                        .write_all(
                            b"HTTP/1.1 503 Service Unavailable\r\nRetry-After-Ms: 0\r\nContent-Length: 4\r\nConnection: close\r\n\r\ndown",
                        )
                        .expect("write retry response");
                    attempt += 1;
                    continue;
                }
                break socket;
            };
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                )
                .expect("write headers");
            for event in [
                json!({
                    "type": "response.output_item.added",
                    "output_index": 0,
                    "item": { "type": "message", "id": "msg_1", "content": [] }
                }),
                json!({
                    "type": "response.output_text.delta",
                    "output_index": 0,
                    "delta": "Hello"
                }),
            ] {
                write!(socket, "data: {event}\n\n").expect("write delta");
            }
            socket.flush().expect("flush delta");
            receive_terminal.recv().expect("release terminal");
            for event in [
                json!({
                    "type": "response.output_item.done",
                    "output_index": 0,
                    "item": {
                        "type": "message",
                        "id": "msg_1",
                        "content": [{ "type": "output_text", "text": "Hello" }]
                    }
                }),
                json!({
                    "type": "response.completed",
                    "response": {
                        "id": "resp_1",
                        "status": "completed",
                        "usage": {
                            "input_tokens": 1,
                            "output_tokens": 1,
                            "total_tokens": 2,
                            "input_tokens_details": { "cached_tokens": 0 },
                            "output_tokens_details": { "reasoning_tokens": 0 }
                        }
                    }
                }),
            ] {
                write!(socket, "data: {event}\n\n").expect("write terminal");
            }
            socket.flush().expect("flush terminal");
            receive_close.recv().expect("release response close");
            server_closed.store(true, Ordering::SeqCst);
        });

        let mut local_model = model();
        local_model.base_url = Some(format!("http://{address}"));
        let stream = stream_live(
            &local_model,
            &Context::default(),
            Some(&OpenAICodexResponsesOptions {
                api_key: Some(
                    "e30.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9hY2NvdW50X2lkIjoiYWNjdF8xMjMifX0.sig"
                        .to_owned(),
                ),
                transport: Some(Transport::Sse),
                timeout_ms: Some(2_000),
                max_retries: Some(1),
                ..OpenAICodexResponsesOptions::default()
            }),
        )
        .expect("start live stream");
        let (send_event, receive_event) = mpsc::channel();
        let consumer = thread::spawn(move || {
            let mut stream = stream;
            while let Some(event) = futures::executor::block_on(stream.next()) {
                if send_event.send(event).is_err() {
                    break;
                }
            }
        });

        loop {
            let event = receive_event
                .recv_timeout(Duration::from_secs(2))
                .expect("delta before EOF");
            if matches!(
                event,
                crate::types::AssistantMessageEvent::TextDelta { ref delta, .. }
                    if delta == "Hello"
            ) {
                break;
            }
        }
        assert!(!response_closed.load(Ordering::SeqCst));
        send_terminal.send(()).expect("release terminal");
        loop {
            let event = receive_event
                .recv_timeout(Duration::from_secs(2))
                .expect("terminal before EOF");
            if matches!(event, crate::types::AssistantMessageEvent::Done { .. }) {
                break;
            }
        }
        assert!(!response_closed.load(Ordering::SeqCst));
        close_response.send(()).expect("close response");
        server.join().expect("server thread");
        consumer.join().expect("consumer thread");
    }
}
