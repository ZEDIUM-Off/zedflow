//! OpenAI Codex Responses API ported from Pi.

use std::collections::{HashMap, HashSet};
use std::error::Error as StdError;
use std::fmt;
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::Value;

const DEFAULT_CODEX_BASE_URL: &str = "https://chatgpt.com/backend-api";
const JWT_CLAIM_PATH: &str = "https://api.openai.com/auth";
#[cfg(test)]
const DEFAULT_MAX_RETRY_DELAY_MS: u64 = 60_000;
const OPENAI_PROMPT_CACHE_KEY_MAX_LENGTH: usize = 64;
const OPENAI_BETA_RESPONSES_WEBSOCKETS: &str = "responses_websockets=2026-02-06";

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
    /// JSON body sent to Codex Responses.
    pub body: Value,
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

/// OpenAI Codex Responses-specific options.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OpenAICodexResponsesOptions {
    /// Sampling temperature.
    pub temperature: Option<f64>,
    /// Maximum output tokens requested by the caller.
    pub max_tokens: Option<u32>,
    /// API key for ChatGPT/Codex.
    pub api_key: Option<String>,
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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
    let timeout_ms = normalize_timeout_ms(options.timeout_ms)?;
    let websocket_connect_timeout_ms = normalize_timeout_ms(options.websocket_connect_timeout_ms)?;
    let request_id = options.session_id.as_deref().unwrap_or("codex_request");
    if options.transport != Some(Transport::Sse)
        && is_websocket_sse_fallback_active(options.session_id.as_deref())
    {
        record_websocket_sse_fallback(options.session_id.as_deref());
    }

    Ok(OpenAICodexResponsesRequest {
        sse_url: resolve_codex_url(model.base_url.as_deref()),
        websocket_url: resolve_codex_websocket_url(model.base_url.as_deref()),
        sse_headers: build_sse_headers(
            &model.headers,
            &options.headers,
            &account_id,
            api_key,
            options.session_id.as_deref(),
        ),
        websocket_headers: build_websocket_headers(
            &model.headers,
            &options.headers,
            &account_id,
            api_key,
            request_id,
        ),
        body,
        timeout_ms,
        websocket_connect_timeout_ms,
        max_retries: options.max_retries.unwrap_or(0),
        transport: options.transport,
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
    use serde_json::json;

    fn model() -> Model {
        Model {
            id: "gpt-5".to_string(),
            provider: "openai-codex".to_string(),
            base_url: None,
            reasoning: true,
            thinking_level_map: HashMap::new(),
            headers: HashMap::new(),
            max_tokens: Some(4096),
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
}
