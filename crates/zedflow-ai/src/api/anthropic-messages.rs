use std::borrow::Cow;
use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::types::{
    Api, AssistantContentBlock, AssistantMessage, AssistantMessageEvent,
    AssistantMessageEventStream, AssistantMessageRole, CacheRetention, Context, DoneStopReason,
    ErrorStopReason, Message, Model, ModelCompat, ModelThinkingLevel, ProviderEnv, ProviderHeaders,
    ProviderResponse, ProviderStreams, SimpleStreamOptions, StopReason, StreamOptions, TextContent,
    TextContentType, ThinkingContent, ThinkingContentType, ThinkingLevel, Tool, ToolCall,
    ToolCallType, ToolResultContentBlock, Usage, UsageCost, UserContentBlock, UserMessageContent,
};
use crate::utils::headers::provider_headers_to_record;
use crate::utils::json_parse::{parse_json_with_repair, parse_streaming_json_value};
use crate::utils::node_http_proxy::resolve_reqwest_proxy_for_target;

const CLAUDE_CODE_VERSION: &str = "2.1.75";
const FINE_GRAINED_TOOL_STREAMING_BETA: &str = "fine-grained-tool-streaming-2025-05-14";
const INTERLEAVED_THINKING_BETA: &str = "interleaved-thinking-2025-05-14";
const ANTHROPIC_MESSAGE_EVENTS: &[&str] = &[
    "message_start",
    "message_delta",
    "message_stop",
    "content_block_start",
    "content_block_delta",
    "content_block_stop",
];

/// Errors returned by the Anthropic Messages port.
#[derive(Debug)]
pub enum AnthropicError {
    /// No API key or authorization-style header was supplied.
    MissingApiKey { provider: String },
    /// A local request cannot be represented by the synchronous compatibility wrapper.
    Unsupported(String),
    /// HTTP client construction or request failure.
    Http(reqwest::Error),
    /// Invalid HTTP header name or value.
    InvalidHeader(String),
    /// Proxy configuration error.
    Proxy(String),
    /// The request was cancelled through its abort signal.
    Aborted,
    /// Provider returned a non-success HTTP status.
    HttpStatus { status: u16, body: String },
    /// Server-sent-event parsing failed.
    Sse(String),
    /// JSON parsing failed.
    Json(String),
    /// A payload or response hook rejected the request.
    Hook(crate::types::ProviderHookError),
}

impl fmt::Display for AnthropicError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApiKey { provider } => {
                write!(formatter, "no API key for provider: {provider}")
            }
            Self::Unsupported(message)
            | Self::InvalidHeader(message)
            | Self::Proxy(message)
            | Self::Sse(message)
            | Self::Json(message) => formatter.write_str(message),
            Self::Aborted => formatter.write_str("Request was aborted"),
            Self::Http(error) => error.fmt(formatter),
            Self::Hook(error) => error.fmt(formatter),
            Self::HttpStatus { status, body } => write!(
                formatter,
                "Anthropic request failed with status {status}: {body}"
            ),
        }
    }
}

impl StdError for AnthropicError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Http(error) => Some(error),
            Self::Hook(error) => Some(error),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for AnthropicError {
    fn from(value: reqwest::Error) -> Self {
        Self::Http(value)
    }
}

impl From<crate::types::ProviderHookError> for AnthropicError {
    fn from(value: crate::types::ProviderHookError) -> Self {
        Self::Hook(value)
    }
}

/// Result type for Anthropic Messages helpers.
pub type Result<T> = std::result::Result<T, AnthropicError>;

/// Anthropic adaptive thinking effort levels accepted by Pi's Anthropic Messages API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnthropicEffort {
    /// Minimal thinking; skips reasoning for simple tasks.
    Low,
    /// Moderate thinking; may skip reasoning for simple tasks.
    Medium,
    /// Deep reasoning.
    High,
    /// Highest reasoning level for newer adaptive thinking models.
    XHigh,
    /// Unconstrained reasoning for models that support it.
    Max,
}

impl AnthropicEffort {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
        }
    }
}

/// Controls how Anthropic thinking blocks are returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnthropicThinkingDisplay {
    /// Return summarized thinking text.
    Summarized,
    /// Omit thinking text while preserving provider continuity metadata.
    Omitted,
}

impl AnthropicThinkingDisplay {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Summarized => "summarized",
            Self::Omitted => "omitted",
        }
    }
}

/// Anthropic tool choice behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnthropicToolChoice {
    /// Let Anthropic choose whether to call a tool.
    Auto,
    /// Force some tool use.
    Any,
    /// Disable tool use.
    None,
    /// Force a specific tool by name.
    Tool {
        /// Tool name to force.
        name: String,
    },
}

impl AnthropicToolChoice {
    fn to_json(&self) -> Value {
        match self {
            Self::Auto => json!({ "type": "auto" }),
            Self::Any => json!({ "type": "any" }),
            Self::None => json!({ "type": "none" }),
            Self::Tool { name } => json!({ "type": "tool", "name": name }),
        }
    }
}

/// Raw Anthropic response fixture for deterministic tests or alternate transports.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnthropicClientConfig {
    /// Raw SSE body to parse instead of performing network I/O.
    pub raw_sse: Option<String>,
    /// HTTP status exposed to `on_response` when `raw_sse` is used.
    pub status: u16,
    /// HTTP response headers exposed to `on_response` when `raw_sse` is used.
    pub response_headers: HashMap<String, String>,
}

/// Anthropic ephemeral cache-control marker used by Pi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheControlEphemeral {
    /// Cache-control type; Pi always sends `ephemeral`.
    pub r#type: &'static str,
    /// Optional long-retention TTL.
    pub ttl: Option<&'static str>,
}

/// Returns the prompt-cache retention, defaulting from `PI_CACHE_RETENTION` when present.
#[must_use]
pub fn resolve_cache_retention(
    cache_retention: Option<CacheRetention>,
    env: Option<&ProviderEnv>,
) -> CacheRetention {
    if let Some(cache_retention) = cache_retention {
        return cache_retention;
    }
    if env
        .and_then(|env| env.get("PI_CACHE_RETENTION"))
        .map(String::as_str)
        == Some("long")
    {
        CacheRetention::Long
    } else {
        CacheRetention::Short
    }
}

/// Returns Pi's Anthropic cache-control marker for a resolved retention preference.
#[must_use]
pub const fn cache_control(
    cache_retention: CacheRetention,
    supports_long_cache_retention: bool,
) -> Option<CacheControlEphemeral> {
    match cache_retention {
        CacheRetention::None => None,
        CacheRetention::Short => Some(CacheControlEphemeral {
            r#type: "ephemeral",
            ttl: None,
        }),
        CacheRetention::Long if supports_long_cache_retention => Some(CacheControlEphemeral {
            r#type: "ephemeral",
            ttl: Some("1h"),
        }),
        CacheRetention::Long => Some(CacheControlEphemeral {
            r#type: "ephemeral",
            ttl: None,
        }),
    }
}

const CLAUDE_CODE_TOOLS: &[&str] = &[
    "Read",
    "Write",
    "Edit",
    "Bash",
    "Grep",
    "Glob",
    "AskUserQuestion",
    "EnterPlanMode",
    "ExitPlanMode",
    "KillShell",
    "NotebookEdit",
    "Skill",
    "Task",
    "TaskOutput",
    "TodoWrite",
    "WebFetch",
    "WebSearch",
];

/// Converts a tool name to Claude Code's canonical casing when it matches a
/// Claude Code tool name case-insensitively.
#[must_use]
pub fn to_claude_code_tool_name(name: &str) -> Cow<'_, str> {
    CLAUDE_CODE_TOOLS
        .iter()
        .copied()
        .find(|tool| tool.eq_ignore_ascii_case(name))
        .map_or(Cow::Borrowed(name), Cow::Borrowed)
}

/// Converts a Claude Code-cased tool name back to the matching original tool
/// name from the request context.
#[must_use]
pub fn from_claude_code_tool_name<'a>(name: &'a str, tools: Option<&'a [Tool]>) -> Cow<'a, str> {
    tools
        .into_iter()
        .flatten()
        .find(|tool| tool.name.eq_ignore_ascii_case(name))
        .map_or(Cow::Borrowed(name), |tool| {
            Cow::Borrowed(tool.name.as_str())
        })
}

/// Options specific to Pi's Anthropic Messages stream implementation.
#[derive(Clone, Default)]
pub struct AnthropicOptions {
    /// Base Pi stream options.
    pub stream: StreamOptions<Api>,
    /// Enable extended thinking.
    pub thinking_enabled: Option<bool>,
    /// Token budget for budget-based thinking models.
    pub thinking_budget_tokens: Option<u32>,
    /// Adaptive thinking effort level.
    pub effort: Option<AnthropicEffort>,
    /// Thinking content display mode.
    pub thinking_display: Option<AnthropicThinkingDisplay>,
    /// Whether to request interleaved thinking for non-adaptive thinking models.
    pub interleaved_thinking: Option<bool>,
    /// Anthropic tool choice behavior.
    pub tool_choice: Option<AnthropicToolChoice>,
    /// Injected Anthropic-compatible fixture/config.
    pub client: Option<AnthropicClientConfig>,
}

/// Final result returned by the synchronous deterministic Anthropic stream wrapper.
#[derive(Debug, Clone, PartialEq)]
pub struct AnthropicMessageStream {
    /// Final assistant message.
    pub message: AssistantMessage,
}

/// Decoded server-sent event preserving Pi's raw line data for parse errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerSentEvent {
    /// SSE `event` field. Missing field is `None`.
    pub event: Option<String>,
    /// Joined SSE `data` lines.
    pub data: String,
    /// Raw non-blank lines for diagnostics.
    pub raw: Vec<String>,
}

#[derive(Debug, Default)]
struct SseDecoderState {
    event: Option<String>,
    data: Vec<String>,
    raw: Vec<String>,
}

fn flush_sse_event(state: &mut SseDecoderState) -> Option<ServerSentEvent> {
    if state.event.is_none() && state.data.is_empty() {
        return None;
    }

    Some(ServerSentEvent {
        event: state.event.take(),
        data: std::mem::take(&mut state.data).join("\n"),
        raw: std::mem::take(&mut state.raw),
    })
}

fn decode_sse_line(line: &str, state: &mut SseDecoderState) -> Option<ServerSentEvent> {
    if line.is_empty() {
        return flush_sse_event(state);
    }

    state.raw.push(line.to_owned());
    if line.starts_with(':') {
        return None;
    }

    let (field_name, value) = line.split_once(':').map_or((line, ""), |(name, value)| {
        (name, value.strip_prefix(' ').unwrap_or(value))
    });

    match field_name {
        "event" => state.event = Some(value.to_owned()),
        "data" => state.data.push(value.to_owned()),
        _ => {}
    }

    None
}

/// Decodes raw SSE text exactly enough for Anthropic's Messages stream.
#[must_use]
pub fn decode_sse_messages(text: &str) -> Vec<ServerSentEvent> {
    let mut state = SseDecoderState::default();
    let mut events = Vec::new();
    let mut rest = text;

    while let Some(line_break_index) = next_line_break_index(rest) {
        let line = &rest[..line_break_index];
        let mut next_index = line_break_index + 1;
        if rest.as_bytes()[line_break_index] == b'\r'
            && rest.as_bytes().get(next_index) == Some(&b'\n')
        {
            next_index += 1;
        }
        if let Some(event) = decode_sse_line(line, &mut state) {
            events.push(event);
        }
        rest = &rest[next_index..];
    }

    if !rest.is_empty()
        && let Some(event) = decode_sse_line(rest, &mut state)
    {
        events.push(event);
    }
    if let Some(event) = flush_sse_event(&mut state) {
        events.push(event);
    }

    events
}

fn next_line_break_index(text: &str) -> Option<usize> {
    match (text.find('\r'), text.find('\n')) {
        (Some(carriage_return), Some(newline)) => Some(carriage_return.min(newline)),
        (Some(carriage_return), None) => Some(carriage_return),
        (None, Some(newline)) => Some(newline),
        (None, None) => None,
    }
}

/// Parses Anthropic raw message SSE events, ignoring unknown event names and validating `message_stop`.
pub fn parse_anthropic_sse_events(text: &str) -> Result<Vec<Value>> {
    let mut events = Vec::new();
    let mut saw_message_start = false;
    let mut saw_message_stop = false;

    for sse in decode_sse_messages(text) {
        if sse.event.as_deref() == Some("error") {
            return Err(AnthropicError::Sse(sse.data));
        }
        if !ANTHROPIC_MESSAGE_EVENTS.contains(&sse.event.as_deref().unwrap_or_default()) {
            continue;
        }

        let event = parse_json_with_repair::<Value>(&sse.data).map_err(|error| {
            AnthropicError::Json(format!(
                "Could not parse Anthropic SSE event {}: {error}; data={}; raw={}",
                sse.event.as_deref().unwrap_or_default(),
                sse.data,
                sse.raw.join("\\n")
            ))
        })?;
        match event.get("type").and_then(Value::as_str) {
            Some("message_start") => saw_message_start = true,
            Some("message_stop") => saw_message_stop = true,
            _ => {}
        }
        events.push(event);
    }

    if saw_message_start && !saw_message_stop {
        return Err(AnthropicError::Sse(
            "Anthropic stream ended before message_stop".to_owned(),
        ));
    }

    Ok(events)
}

#[derive(Debug, Clone, Copy)]
struct AnthropicCompatResolved {
    supports_eager_tool_input_streaming: bool,
    supports_long_cache_retention: bool,
    send_session_affinity_headers: bool,
    supports_cache_control_on_tools: bool,
    supports_temperature: bool,
    force_adaptive_thinking: Option<bool>,
    allow_empty_signature: bool,
}

fn get_anthropic_compat(model: &Model) -> AnthropicCompatResolved {
    let compat = match &model.compat {
        Some(ModelCompat::AnthropicMessages(compat)) => Some(compat),
        _ => None,
    };
    AnthropicCompatResolved {
        supports_eager_tool_input_streaming: compat
            .and_then(|compat| compat.supports_eager_tool_input_streaming)
            .unwrap_or(true),
        supports_long_cache_retention: compat
            .and_then(|compat| compat.supports_long_cache_retention)
            .unwrap_or(true),
        send_session_affinity_headers: compat
            .and_then(|compat| compat.send_session_affinity_headers)
            .unwrap_or(false),
        supports_cache_control_on_tools: compat
            .and_then(|compat| compat.supports_cache_control_on_tools)
            .unwrap_or(true),
        supports_temperature: compat
            .and_then(|compat| compat.supports_temperature)
            .unwrap_or(true),
        force_adaptive_thinking: compat.and_then(|compat| compat.force_adaptive_thinking),
        allow_empty_signature: compat
            .and_then(|compat| compat.allow_empty_signature)
            .unwrap_or(false),
    }
}

fn is_oauth_token(api_key: &str) -> bool {
    api_key.contains("sk-ant-oat")
}

fn has_header(headers: &ProviderHeaders, name: &str) -> bool {
    headers.iter().any(|(key, value)| {
        key.eq_ignore_ascii_case(name)
            && value
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
    })
}

fn assert_request_auth(model: &Model, options: Option<&AnthropicOptions>) -> Result<()> {
    if options
        .and_then(|options| options.stream.api_key.as_deref())
        .is_some()
    {
        return Ok(());
    }
    if options
        .and_then(|options| options.stream.headers.as_ref())
        .is_some_and(|headers| {
            has_header(headers, "authorization")
                || has_header(headers, "x-api-key")
                || has_header(headers, "cf-aig-authorization")
        })
    {
        return Ok(());
    }
    Err(AnthropicError::MissingApiKey {
        provider: model.provider.clone(),
    })
}

fn cache_control_value(cache_control: CacheControlEphemeral) -> Value {
    let mut value = json!({ "type": cache_control.r#type });
    if let Some(ttl) = cache_control.ttl {
        value["ttl"] = Value::String(ttl.to_owned());
    }
    value
}

fn text_block(text: String, cache_control: Option<CacheControlEphemeral>) -> Value {
    let mut block = json!({ "type": "text", "text": text });
    if let Some(cache_control) = cache_control {
        block["cache_control"] = cache_control_value(cache_control);
    }
    block
}

fn convert_user_content(content: &UserMessageContent) -> Option<Value> {
    match content {
        UserMessageContent::Text(text) if !text.trim().is_empty() => {
            Some(Value::String(text.clone()))
        }
        UserMessageContent::Text(_) => None,
        UserMessageContent::Blocks(blocks) => {
            let converted = blocks
                .iter()
                .filter_map(|block| match block {
                    UserContentBlock::Text(text) if !text.text.trim().is_empty() => {
                        Some(json!({ "type": "text", "text": text.text }))
                    }
                    UserContentBlock::Text(_) => None,
                    UserContentBlock::Image(image) => Some(json!({
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": image.mime_type,
                            "data": image.data,
                        }
                    })),
                })
                .collect::<Vec<_>>();
            (!converted.is_empty()).then_some(Value::Array(converted))
        }
    }
}

fn convert_tool_result_content(content: &[ToolResultContentBlock]) -> Value {
    let has_images = content
        .iter()
        .any(|block| matches!(block, ToolResultContentBlock::Image(_)));
    if !has_images {
        return Value::String(
            content
                .iter()
                .filter_map(|block| match block {
                    ToolResultContentBlock::Text(text) => Some(text.text.as_str()),
                    ToolResultContentBlock::Image(_) => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }

    let mut blocks = content
        .iter()
        .map(|block| match block {
            ToolResultContentBlock::Text(text) => json!({ "type": "text", "text": text.text }),
            ToolResultContentBlock::Image(image) => json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": image.mime_type,
                    "data": image.data,
                }
            }),
        })
        .collect::<Vec<_>>();
    if !blocks
        .iter()
        .any(|block| block.get("type") == Some(&json!("text")))
    {
        blocks.insert(0, json!({ "type": "text", "text": "(see attached image)" }));
    }
    Value::Array(blocks)
}

fn convert_messages(
    messages: &[Message],
    is_oauth: bool,
    cache_control: Option<CacheControlEphemeral>,
    allow_empty_signature: bool,
) -> Vec<Value> {
    let mut params = Vec::new();
    let mut index = 0;
    while index < messages.len() {
        match &messages[index] {
            Message::User(message) => {
                if let Some(content) = convert_user_content(&message.content) {
                    params.push(json!({ "role": "user", "content": content }));
                }
            }
            Message::Assistant(message) => {
                let blocks = message
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        AssistantContentBlock::Text(text) if !text.text.trim().is_empty() => {
                            Some(json!({ "type": "text", "text": text.text }))
                        }
                        AssistantContentBlock::Text(_) => None,
                        AssistantContentBlock::Thinking(thinking) if thinking.redacted == Some(true) => {
                            Some(json!({ "type": "redacted_thinking", "data": thinking.thinking_signature.clone().unwrap_or_default() }))
                        }
                        AssistantContentBlock::Thinking(thinking) if !thinking.thinking.trim().is_empty() => {
                            match thinking.thinking_signature.as_deref().map(str::trim) {
                                Some(signature) if !signature.is_empty() => Some(json!({
                                    "type": "thinking",
                                    "thinking": thinking.thinking,
                                    "signature": signature,
                                })),
                                _ if allow_empty_signature => Some(json!({
                                    "type": "thinking",
                                    "thinking": thinking.thinking,
                                    "signature": "",
                                })),
                                _ => Some(json!({ "type": "text", "text": thinking.thinking })),
                            }
                        }
                        AssistantContentBlock::Thinking(_) => None,
                        AssistantContentBlock::ToolCall(tool_call) => Some(json!({
                            "type": "tool_use",
                            "id": normalize_tool_call_id(&tool_call.id),
                            "name": if is_oauth { to_claude_code_tool_name(&tool_call.name).into_owned() } else { tool_call.name.clone() },
                            "input": tool_call.arguments,
                        })),
                    })
                    .collect::<Vec<_>>();
                if !blocks.is_empty() {
                    params.push(json!({ "role": "assistant", "content": blocks }));
                }
            }
            Message::ToolResult(_) => {
                let mut tool_results = Vec::new();
                while let Some(Message::ToolResult(tool_result)) = messages.get(index) {
                    tool_results.push(json!({
                        "type": "tool_result",
                        "tool_use_id": normalize_tool_call_id(&tool_result.tool_call_id),
                        "content": convert_tool_result_content(&tool_result.content),
                        "is_error": tool_result.is_error,
                    }));
                    index += 1;
                }
                index = index.saturating_sub(1);
                params.push(json!({ "role": "user", "content": tool_results }));
            }
        }
        index += 1;
    }

    if let Some(cache_control) = cache_control
        && let Some(last_message) = params.last_mut()
        && last_message.get("role") == Some(&json!("user"))
    {
        match last_message.get_mut("content") {
            Some(Value::Array(blocks)) => {
                if let Some(block) = blocks.last_mut()
                    && matches!(
                        block.get("type").and_then(Value::as_str),
                        Some("text" | "image" | "tool_result")
                    )
                {
                    block["cache_control"] = cache_control_value(cache_control);
                }
            }
            Some(Value::String(text)) => {
                let text = std::mem::take(text);
                last_message["content"] = Value::Array(vec![text_block(text, Some(cache_control))]);
            }
            _ => {}
        }
    }

    params
}

/// Builds Anthropic request JSON, preserving Pi-visible cache, thinking, tool, and metadata fields.
#[must_use]
pub fn build_request_payload(
    model: &Model,
    context: &Context,
    is_oauth: bool,
    options: Option<&AnthropicOptions>,
) -> Value {
    let compat = get_anthropic_compat(model);
    let cache = cache_control(
        resolve_cache_retention(
            options.and_then(|options| options.stream.cache_retention),
            options.and_then(|options| options.stream.env.as_ref()),
        ),
        compat.supports_long_cache_retention,
    );
    let mut params = json!({
        "model": model.id,
        "messages": convert_messages(&context.messages, is_oauth, cache, compat.allow_empty_signature),
        "max_tokens": options
            .and_then(|options| options.stream.max_tokens)
            .map_or(model.max_tokens, u64::from),
        "stream": true,
    });

    if is_oauth {
        let mut system = vec![text_block(
            "You are Claude Code, Anthropic's official CLI for Claude.".to_owned(),
            cache,
        )];
        if let Some(prompt) = &context.system_prompt {
            system.push(text_block(prompt.clone(), cache));
        }
        params["system"] = Value::Array(system);
    } else if let Some(prompt) = &context.system_prompt {
        params["system"] = Value::Array(vec![text_block(prompt.clone(), cache)]);
    }

    if let Some(temperature) = options.and_then(|options| options.stream.temperature)
        && options.and_then(|options| options.thinking_enabled) != Some(true)
        && compat.supports_temperature
    {
        params["temperature"] = json!(temperature);
    }

    if let Some(tools) = &context.tools
        && !tools.is_empty()
    {
        let tool_cache = compat
            .supports_cache_control_on_tools
            .then_some(cache)
            .flatten();
        params["tools"] = Value::Array(convert_tools(
            tools,
            is_oauth,
            compat.supports_eager_tool_input_streaming,
            tool_cache,
        ));
    }

    if model.reasoning {
        match options.and_then(|options| options.thinking_enabled) {
            Some(true) if compat.force_adaptive_thinking == Some(true) => {
                let display = options
                    .and_then(|options| options.thinking_display)
                    .unwrap_or(AnthropicThinkingDisplay::Summarized)
                    .as_str();
                params["thinking"] = json!({ "type": "adaptive", "display": display });
                if let Some(effort) = options.and_then(|options| options.effort) {
                    params["output_config"] = json!({ "effort": effort.as_str() });
                }
            }
            Some(true) => {
                let display = options
                    .and_then(|options| options.thinking_display)
                    .unwrap_or(AnthropicThinkingDisplay::Summarized)
                    .as_str();
                params["thinking"] = json!({
                    "type": "enabled",
                    "budget_tokens": options
                        .and_then(|options| options.thinking_budget_tokens)
                        .unwrap_or(1024),
                    "display": display,
                });
            }
            Some(false)
                if model.thinking_level_map.as_ref().is_none_or(|map| {
                    map.get(&crate::types::ModelThinkingLevel::Off) != Some(&None)
                }) =>
            {
                params["thinking"] = json!({ "type": "disabled" });
            }
            _ => {}
        }
    }

    if let Some(user_id) = options
        .and_then(|options| options.stream.metadata.as_ref())
        .and_then(|metadata| metadata.get("user_id"))
        .and_then(Value::as_str)
    {
        params["metadata"] = json!({ "user_id": user_id });
    }

    if let Some(tool_choice) = options.and_then(|options| options.tool_choice.as_ref()) {
        params["tool_choice"] = tool_choice.to_json();
    }

    params
}

fn convert_tools(
    tools: &[Tool],
    is_oauth: bool,
    supports_eager_tool_input_streaming: bool,
    cache_control: Option<CacheControlEphemeral>,
) -> Vec<Value> {
    let last_index = tools.len().saturating_sub(1);
    tools
        .iter()
        .enumerate()
        .map(|(index, tool)| {
            let mut value = json!({
                "name": if is_oauth { to_claude_code_tool_name(&tool.name).into_owned() } else { tool.name.clone() },
                "description": tool.description,
                "input_schema": {
                    "type": "object",
                    "properties": tool.parameters.get("properties").cloned().unwrap_or_else(|| json!({})),
                    "required": tool.parameters.get("required").cloned().unwrap_or_else(|| json!([])),
                }
            });
            if supports_eager_tool_input_streaming {
                value["eager_input_streaming"] = Value::Bool(true);
            }
            if index == last_index
                && let Some(cache_control) = cache_control
            {
                value["cache_control"] = cache_control_value(cache_control);
            }
            value
        })
        .collect()
}

/// Builds request headers for the raw reqwest fallback.
pub fn build_request_headers(
    model: &Model,
    api_key: Option<&str>,
    options: Option<&AnthropicOptions>,
    is_oauth: bool,
    use_fine_grained_tool_streaming_beta: bool,
    dynamic_headers: Option<&HashMap<String, String>>,
) -> ProviderHeaders {
    let compat = get_anthropic_compat(model);
    let needs_interleaved_beta = options
        .and_then(|options| options.interleaved_thinking)
        .unwrap_or(true)
        && compat.force_adaptive_thinking != Some(true);
    let mut beta_features = Vec::new();
    if use_fine_grained_tool_streaming_beta {
        beta_features.push(FINE_GRAINED_TOOL_STREAMING_BETA);
    }
    if needs_interleaved_beta {
        beta_features.push(INTERLEAVED_THINKING_BETA);
    }

    let mut headers = ProviderHeaders::new();
    headers.insert("accept".to_owned(), Some("application/json".to_owned()));
    headers.insert(
        "content-type".to_owned(),
        Some("application/json".to_owned()),
    );
    headers.insert(
        "anthropic-version".to_owned(),
        Some("2023-06-01".to_owned()),
    );
    headers.insert(
        "anthropic-dangerous-direct-browser-access".to_owned(),
        Some("true".to_owned()),
    );

    if model.provider == "github-copilot" {
        if !beta_features.is_empty() {
            headers.insert("anthropic-beta".to_owned(), Some(beta_features.join(",")));
        }
    } else if is_oauth {
        let mut oauth_betas = vec!["claude-code-20250219", "oauth-2025-04-20"];
        oauth_betas.extend(beta_features);
        headers.insert("anthropic-beta".to_owned(), Some(oauth_betas.join(",")));
        headers.insert(
            "user-agent".to_owned(),
            Some(format!("claude-cli/{CLAUDE_CODE_VERSION}")),
        );
        headers.insert("x-app".to_owned(), Some("cli".to_owned()));
    } else if !beta_features.is_empty() {
        headers.insert("anthropic-beta".to_owned(), Some(beta_features.join(",")));
    }

    if let Some(api_key) = api_key {
        if is_oauth || model.provider == "github-copilot" {
            headers.insert(
                "authorization".to_owned(),
                Some(format!("Bearer {api_key}")),
            );
        } else {
            headers.insert("x-api-key".to_owned(), Some(api_key.to_owned()));
        }
    }

    if let Some(session_id) = options.and_then(|options| options.stream.session_id.as_deref())
        && compat.send_session_affinity_headers
        && resolve_cache_retention(
            options.and_then(|options| options.stream.cache_retention),
            options.and_then(|options| options.stream.env.as_ref()),
        ) != CacheRetention::None
    {
        headers.insert("x-session-affinity".to_owned(), Some(session_id.to_owned()));
    }

    merge_headers(&mut headers, model.headers.as_ref());
    merge_headers(&mut headers, dynamic_headers);
    if let Some(option_headers) = options.and_then(|options| options.stream.headers.as_ref()) {
        for (key, value) in option_headers {
            headers.insert(key.clone(), value.clone());
        }
    }

    headers
}

fn merge_headers(target: &mut ProviderHeaders, source: Option<&HashMap<String, String>>) {
    if let Some(source) = source {
        for (key, value) in source {
            target.insert(key.clone(), Some(value.clone()));
        }
    }
}

fn provider_headers_to_headermap(headers: &ProviderHeaders) -> Result<HeaderMap> {
    let mut map = HeaderMap::new();
    for (name, value) in provider_headers_to_record(Some(headers)).unwrap_or_default() {
        let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
            AnthropicError::InvalidHeader(format!("invalid header name {name:?}: {error}"))
        })?;
        let header_value = HeaderValue::from_str(&value).map_err(|error| {
            AnthropicError::InvalidHeader(format!("invalid header value for {name:?}: {error}"))
        })?;
        map.insert(header_name, header_value);
    }
    Ok(map)
}

fn messages_url(base_url: &str) -> String {
    let trimmed = base_url.trim_end_matches('/');
    if trimmed.ends_with("/messages") {
        trimmed.to_owned()
    } else if trimmed.ends_with("/v1") {
        format!("{trimmed}/messages")
    } else {
        format!("{trimmed}/v1/messages")
    }
}

async fn wait_for_abort(signal: crate::types::AbortSignal) {
    while !signal.aborted() {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

/// Sends an Anthropic Messages request via raw reqwest and returns the raw SSE body.
///
/// The function has no implicit retries; callers must opt into any retry loop outside this API.
pub async fn request_raw_sse(
    model: &Model,
    context: &Context,
    options: Option<&AnthropicOptions>,
) -> Result<String> {
    assert_request_auth(model, options)?;
    let signal = options.and_then(|options| options.stream.signal.clone());
    if signal
        .as_ref()
        .is_some_and(crate::types::AbortSignal::aborted)
    {
        return Err(AnthropicError::Aborted);
    }
    let api_key = options.and_then(|options| options.stream.api_key.as_deref());
    let is_oauth = api_key.is_some_and(is_oauth_token);
    let mut payload = build_request_payload(model, context, is_oauth, options);
    if let Some(hook) = options.and_then(|options| options.stream.on_payload.as_ref())
        && let Some(next_payload) = hook(payload.clone(), model.clone()).await?
    {
        payload = next_payload;
    }

    let use_fine_grained_tool_streaming_beta = context
        .tools
        .as_ref()
        .is_some_and(|tools| !tools.is_empty())
        && !get_anthropic_compat(model).supports_eager_tool_input_streaming;
    let headers = build_request_headers(
        model,
        api_key,
        options,
        is_oauth,
        use_fine_grained_tool_streaming_beta,
        None,
    );
    let mut builder = reqwest::Client::builder();
    if let Some(timeout_ms) = options.and_then(|options| options.stream.timeout_ms) {
        builder = builder.timeout(Duration::from_millis(timeout_ms));
    }
    if let Some(proxy) = resolve_reqwest_proxy_for_target(
        &model.base_url,
        options.and_then(|options| options.stream.env.as_ref()),
    )
    .map_err(|error| AnthropicError::Proxy(error.to_string()))?
    {
        builder = builder.proxy(proxy);
    }
    let client = builder.build()?;
    let request = client
        .post(messages_url(&model.base_url))
        .headers(provider_headers_to_headermap(&headers)?)
        .body(payload.to_string())
        .send();
    let response = if let Some(signal) = signal.clone() {
        match futures::future::select(Box::pin(request), Box::pin(wait_for_abort(signal))).await {
            futures::future::Either::Left((response, _)) => response?,
            futures::future::Either::Right(((), _)) => return Err(AnthropicError::Aborted),
        }
    } else {
        request.await?
    };

    let status = response.status().as_u16();
    let response_headers = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.to_string(), value.to_owned()))
        })
        .collect::<HashMap<_, _>>();
    if let Some(hook) = options.and_then(|options| options.stream.on_response.as_ref()) {
        hook(
            ProviderResponse {
                status,
                headers: response_headers,
            },
            model.clone(),
        )
        .await?;
    }
    let response_body = response.text();
    let body = if let Some(signal) = signal {
        match futures::future::select(Box::pin(response_body), Box::pin(wait_for_abort(signal)))
            .await
        {
            futures::future::Either::Left((body, _)) => body?,
            futures::future::Either::Right(((), _)) => return Err(AnthropicError::Aborted),
        }
    } else {
        response_body.await?
    };
    if !(200..300).contains(&status) {
        return Err(AnthropicError::HttpStatus { status, body });
    }
    Ok(body)
}

/// Returns canonical registered Anthropic provider streams.
#[must_use]
pub fn provider_streams() -> ProviderStreams {
    ProviderStreams {
        stream: std::sync::Arc::new(|model, context, options| {
            stream_registered(model, context, options)
        }),
        stream_simple: std::sync::Arc::new(|model, context, options| {
            stream_simple_registered(model, context, options)
        }),
    }
}

/// Starts the canonical Anthropic stream and returns before HTTP work begins.
#[must_use]
pub fn stream_registered(
    model: &Model,
    context: &Context,
    options: Option<&StreamOptions>,
) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    let worker_stream = stream.clone();
    let model = model.clone();
    let context = context.clone();
    let options = anthropic_options_from_stream(options.cloned().unwrap_or_default());
    crate::utils::runtime::spawn_worker(async move {
        run_registered_worker(worker_stream, model, context, options).await;
    });
    stream
}

/// Starts Anthropic through Pi's simple reasoning option mapping.
#[must_use]
pub fn stream_simple_registered(
    model: &Model,
    context: &Context,
    options: Option<&SimpleStreamOptions>,
) -> AssistantMessageEventStream {
    let mut mapped = options
        .map(|options| options.stream.clone())
        .unwrap_or_default();
    let model_max = u32::try_from(model.max_tokens).unwrap_or(u32::MAX);
    mapped.max_tokens = Some(mapped.max_tokens.unwrap_or(model_max).min(model_max));
    let reasoning = options.and_then(|options| options.reasoning);
    let mut anthropic = AnthropicOptions {
        stream: mapped.clone(),
        thinking_enabled: Some(reasoning.is_some()),
        ..AnthropicOptions::default()
    };

    if let Some(reasoning) = reasoning {
        if get_anthropic_compat(model).force_adaptive_thinking == Some(true) {
            anthropic.effort = Some(map_simple_effort(model, reasoning));
        } else {
            let budgets = options.and_then(|options| options.thinking_budgets.as_ref());
            let mut thinking_budget = match reasoning {
                ThinkingLevel::Minimal => {
                    budgets.and_then(|budgets| budgets.minimal).unwrap_or(1024)
                }
                ThinkingLevel::Low => budgets.and_then(|budgets| budgets.low).unwrap_or(2048),
                ThinkingLevel::Medium => budgets.and_then(|budgets| budgets.medium).unwrap_or(8192),
                ThinkingLevel::High | ThinkingLevel::XHigh => {
                    budgets.and_then(|budgets| budgets.high).unwrap_or(16_384)
                }
            };
            let max_tokens = options
                .and_then(|options| options.stream.max_tokens)
                .map_or(model_max, |base| {
                    base.saturating_add(thinking_budget).min(model_max)
                });
            if max_tokens <= thinking_budget {
                thinking_budget = max_tokens.saturating_sub(1024);
            }
            mapped.max_tokens = Some(max_tokens);
            anthropic.stream = mapped;
            anthropic.thinking_budget_tokens =
                Some(thinking_budget.min(max_tokens.saturating_sub(1024)));
        }
    }

    stream_registered_with_options(model, context, anthropic)
}

fn stream_registered_with_options(
    model: &Model,
    context: &Context,
    options: AnthropicOptions,
) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    let worker_stream = stream.clone();
    let model = model.clone();
    let context = context.clone();
    crate::utils::runtime::spawn_worker(async move {
        run_registered_worker(worker_stream, model, context, options).await;
    });
    stream
}

fn anthropic_options_from_stream(stream: StreamOptions) -> AnthropicOptions {
    let bool_extra = |name: &str| stream.extra.get(name).and_then(Value::as_bool);
    let effort = stream
        .extra
        .get("effort")
        .and_then(Value::as_str)
        .and_then(parse_effort);
    let thinking_display = stream
        .extra
        .get("thinkingDisplay")
        .and_then(Value::as_str)
        .and_then(|display| match display {
            "summarized" => Some(AnthropicThinkingDisplay::Summarized),
            "omitted" => Some(AnthropicThinkingDisplay::Omitted),
            _ => None,
        });
    AnthropicOptions {
        thinking_enabled: bool_extra("thinkingEnabled"),
        thinking_budget_tokens: stream
            .extra
            .get("thinkingBudgetTokens")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok()),
        effort,
        thinking_display,
        interleaved_thinking: bool_extra("interleavedThinking"),
        stream,
        ..AnthropicOptions::default()
    }
}

fn parse_effort(effort: &str) -> Option<AnthropicEffort> {
    match effort {
        "low" => Some(AnthropicEffort::Low),
        "medium" => Some(AnthropicEffort::Medium),
        "high" => Some(AnthropicEffort::High),
        "xhigh" => Some(AnthropicEffort::XHigh),
        "max" => Some(AnthropicEffort::Max),
        _ => None,
    }
}

fn map_simple_effort(model: &Model, level: ThinkingLevel) -> AnthropicEffort {
    let model_level = match level {
        ThinkingLevel::Minimal => ModelThinkingLevel::Minimal,
        ThinkingLevel::Low => ModelThinkingLevel::Low,
        ThinkingLevel::Medium => ModelThinkingLevel::Medium,
        ThinkingLevel::High => ModelThinkingLevel::High,
        ThinkingLevel::XHigh => ModelThinkingLevel::XHigh,
    };
    model
        .thinking_level_map
        .as_ref()
        .and_then(|map| map.get(&model_level))
        .and_then(|effort| effort.as_deref())
        .and_then(parse_effort)
        .unwrap_or(match level {
            ThinkingLevel::Minimal | ThinkingLevel::Low => AnthropicEffort::Low,
            ThinkingLevel::Medium => AnthropicEffort::Medium,
            ThinkingLevel::High => AnthropicEffort::High,
            ThinkingLevel::XHigh => AnthropicEffort::XHigh,
        })
}

async fn run_registered_worker(
    stream: AssistantMessageEventStream,
    model: Model,
    context: Context,
    options: AnthropicOptions,
) {
    let mut output = empty_assistant_message(&model);
    match execute_registered_stream(&stream, &model, &context, &options, &mut output).await {
        Ok(()) if output.stop_reason == StopReason::Error => {
            if output.error_message.is_none() {
                output.error_message = Some("An unknown error occurred".to_owned());
            }
            stream.push(AssistantMessageEvent::Error {
                reason: ErrorStopReason::Error,
                error: output,
            });
        }
        Ok(()) => stream.push(AssistantMessageEvent::Done {
            reason: done_reason(output.stop_reason).unwrap_or(DoneStopReason::Stop),
            message: output,
        }),
        Err(error) => {
            let aborted = matches!(error, AnthropicError::Aborted)
                || options
                    .stream
                    .signal
                    .as_ref()
                    .is_some_and(crate::types::AbortSignal::aborted);
            output.stop_reason = if aborted {
                StopReason::Aborted
            } else {
                StopReason::Error
            };
            output.error_message = Some(error.to_string());
            stream.push(AssistantMessageEvent::Error {
                reason: if aborted {
                    ErrorStopReason::Aborted
                } else {
                    ErrorStopReason::Error
                },
                error: output,
            });
        }
    }
}

async fn execute_registered_stream(
    stream: &AssistantMessageEventStream,
    model: &Model,
    context: &Context,
    options: &AnthropicOptions,
    output: &mut AssistantMessage,
) -> Result<()> {
    assert_request_auth(model, Some(options))?;
    let signal = options.stream.signal.clone();
    if signal
        .as_ref()
        .is_some_and(crate::types::AbortSignal::aborted)
    {
        return Err(AnthropicError::Aborted);
    }
    let api_key = options.stream.api_key.as_deref();
    let is_oauth = api_key.is_some_and(is_oauth_token);
    let mut payload = build_request_payload(model, context, is_oauth, Some(options));
    if let Some(hook) = options.stream.on_payload.as_ref()
        && let Some(next_payload) = hook(payload.clone(), model.clone()).await?
    {
        payload = next_payload;
    }
    let use_legacy_beta = context
        .tools
        .as_ref()
        .is_some_and(|tools| !tools.is_empty())
        && !get_anthropic_compat(model).supports_eager_tool_input_streaming;
    let headers = build_request_headers(
        model,
        api_key,
        Some(options),
        is_oauth,
        use_legacy_beta,
        None,
    );
    let mut builder = reqwest::Client::builder();
    if let Some(timeout_ms) = options.stream.timeout_ms {
        builder = builder.timeout(Duration::from_millis(timeout_ms));
    }
    if let Some(proxy) =
        resolve_reqwest_proxy_for_target(&model.base_url, options.stream.env.as_ref())
            .map_err(|error| AnthropicError::Proxy(error.to_string()))?
    {
        builder = builder.proxy(proxy);
    }
    let request = builder
        .build()?
        .post(messages_url(&model.base_url))
        .headers(provider_headers_to_headermap(&headers)?)
        .body(payload.to_string())
        .send();
    let mut response = await_or_abort(request, signal.clone()).await??;
    let status = response.status().as_u16();
    let response_headers = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.to_string(), value.to_owned()))
        })
        .collect();
    if let Some(hook) = options.stream.on_response.as_ref() {
        hook(
            ProviderResponse {
                status,
                headers: response_headers,
            },
            model.clone(),
        )
        .await?;
    }
    if !(200..300).contains(&status) {
        let mut body = Vec::new();
        while let Some(chunk) = await_or_abort(response.chunk(), signal.clone()).await?? {
            body.extend_from_slice(&chunk);
        }
        return Err(AnthropicError::HttpStatus {
            status,
            body: String::from_utf8_lossy(&body).into_owned(),
        });
    }

    abort_if_requested(signal.as_ref())?;
    stream.push(AssistantMessageEvent::Start {
        partial: output.clone(),
    });
    let mut parser = IncrementalAnthropicSse {
        signal: signal.clone(),
        ..IncrementalAnthropicSse::default()
    };
    let mut blocks = Vec::new();
    while let Some(chunk) = await_or_abort(response.chunk(), signal.clone()).await?? {
        abort_if_requested(signal.as_ref())?;
        parser.push_chunk(
            &chunk,
            model,
            context,
            is_oauth,
            output,
            &mut blocks,
            Some(stream),
        )?;
    }
    parser.finish(model, context, is_oauth, output, &mut blocks, Some(stream))?;
    Ok(())
}

async fn await_or_abort<F, T>(future: F, signal: Option<crate::types::AbortSignal>) -> Result<T>
where
    F: std::future::Future<Output = T>,
{
    abort_if_requested(signal.as_ref())?;
    if let Some(signal) = signal {
        match futures::future::select(Box::pin(future), Box::pin(wait_for_abort(signal.clone())))
            .await
        {
            futures::future::Either::Left((value, _)) => {
                abort_if_requested(Some(&signal))?;
                Ok(value)
            }
            futures::future::Either::Right(((), _)) => Err(AnthropicError::Aborted),
        }
    } else {
        Ok(future.await)
    }
}

fn abort_if_requested(signal: Option<&crate::types::AbortSignal>) -> Result<()> {
    if signal.is_some_and(crate::types::AbortSignal::aborted) {
        Err(AnthropicError::Aborted)
    } else {
        Ok(())
    }
}

/// Starts an Anthropic Messages stream from an injected raw-SSE fixture.
///
/// Live provider I/O is available through [`request_raw_sse`], which is async and preserves raw
/// response hooks before parsing.
pub fn stream(
    model: &Model,
    context: &Context,
    options: Option<&AnthropicOptions>,
) -> Result<AnthropicMessageStream> {
    if let Some(raw_sse) = options
        .and_then(|options| options.client.as_ref())
        .and_then(|client| client.raw_sse.as_deref())
    {
        return Ok(AnthropicMessageStream {
            message: assistant_message_from_sse(model, context, raw_sse, false)?,
        });
    }

    Err(AnthropicError::Unsupported(
        "anthropic_messages::stream requires an injected raw SSE body in sync tests; use request_raw_sse for provider I/O".to_owned(),
    ))
}

/// Starts an Anthropic Messages stream using Pi's simple stream options mapping.
pub fn stream_simple(
    model: &Model,
    context: &Context,
    options: Option<&AnthropicOptions>,
) -> Result<AnthropicMessageStream> {
    stream(model, context, options)
}

/// Applies parsed Anthropic raw SSE events to a final assistant message.
pub fn assistant_message_from_sse(
    model: &Model,
    context: &Context,
    raw_sse: &str,
    is_oauth: bool,
) -> Result<AssistantMessage> {
    let mut output = empty_assistant_message(model);
    let mut blocks = Vec::<InProgressBlock>::new();

    for event in parse_anthropic_sse_events(raw_sse)? {
        match event.get("type").and_then(Value::as_str) {
            Some("message_start") => {
                output.response_id = event
                    .pointer("/message/id")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                update_usage(&mut output.usage, event.pointer("/message/usage"));
                calculate_cost(model, &mut output.usage);
            }
            Some("content_block_start") => {
                let provider_index =
                    event.pointer("/index").and_then(Value::as_u64).unwrap_or(0) as usize;
                match event.pointer("/content_block/type").and_then(Value::as_str) {
                    Some("text") => {
                        let index = output.content.len();
                        output
                            .content
                            .push(AssistantContentBlock::Text(TextContent {
                                content_type: TextContentType::Text,
                                text: String::new(),
                                text_signature: None,
                            }));
                        blocks.push(InProgressBlock {
                            provider_index,
                            content_index: index,
                            partial_json: String::new(),
                        });
                    }
                    Some("thinking") => {
                        let index = output.content.len();
                        output
                            .content
                            .push(AssistantContentBlock::Thinking(ThinkingContent {
                                content_type: ThinkingContentType::Thinking,
                                thinking: String::new(),
                                thinking_signature: Some(String::new()),
                                redacted: None,
                            }));
                        blocks.push(InProgressBlock {
                            provider_index,
                            content_index: index,
                            partial_json: String::new(),
                        });
                    }
                    Some("redacted_thinking") => {
                        let index = output.content.len();
                        output
                            .content
                            .push(AssistantContentBlock::Thinking(ThinkingContent {
                                content_type: ThinkingContentType::Thinking,
                                thinking: "[Reasoning redacted]".to_owned(),
                                thinking_signature: event
                                    .pointer("/content_block/data")
                                    .and_then(Value::as_str)
                                    .map(str::to_owned),
                                redacted: Some(true),
                            }));
                        blocks.push(InProgressBlock {
                            provider_index,
                            content_index: index,
                            partial_json: String::new(),
                        });
                    }
                    Some("tool_use") => {
                        let index = output.content.len();
                        let name = event
                            .pointer("/content_block/name")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        output
                            .content
                            .push(AssistantContentBlock::ToolCall(ToolCall {
                                content_type: ToolCallType::ToolCall,
                                id: event
                                    .pointer("/content_block/id")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default()
                                    .to_owned(),
                                name: if is_oauth {
                                    from_claude_code_tool_name(name, context.tools.as_deref())
                                        .into_owned()
                                } else {
                                    name.to_owned()
                                },
                                arguments: value_object_to_hashmap(
                                    event
                                        .pointer("/content_block/input")
                                        .cloned()
                                        .unwrap_or_else(|| json!({})),
                                ),
                                thought_signature: None,
                            }));
                        blocks.push(InProgressBlock {
                            provider_index,
                            content_index: index,
                            partial_json: String::new(),
                        });
                    }
                    _ => {}
                }
            }
            Some("content_block_delta") => {
                let provider_index =
                    event.pointer("/index").and_then(Value::as_u64).unwrap_or(0) as usize;
                let Some(block) = blocks
                    .iter_mut()
                    .find(|block| block.provider_index == provider_index)
                else {
                    continue;
                };
                match event.pointer("/delta/type").and_then(Value::as_str) {
                    Some("text_delta") => {
                        if let Some(AssistantContentBlock::Text(text)) =
                            output.content.get_mut(block.content_index)
                            && let Some(delta) =
                                event.pointer("/delta/text").and_then(Value::as_str)
                        {
                            text.text.push_str(delta);
                        }
                    }
                    Some("thinking_delta") => {
                        if let Some(AssistantContentBlock::Thinking(thinking)) =
                            output.content.get_mut(block.content_index)
                            && let Some(delta) =
                                event.pointer("/delta/thinking").and_then(Value::as_str)
                        {
                            thinking.thinking.push_str(delta);
                        }
                    }
                    Some("signature_delta") => {
                        if let Some(AssistantContentBlock::Thinking(thinking)) =
                            output.content.get_mut(block.content_index)
                            && let Some(delta) =
                                event.pointer("/delta/signature").and_then(Value::as_str)
                        {
                            thinking
                                .thinking_signature
                                .get_or_insert_with(String::new)
                                .push_str(delta);
                        }
                    }
                    Some("input_json_delta") => {
                        if let Some(delta) =
                            event.pointer("/delta/partial_json").and_then(Value::as_str)
                        {
                            block.partial_json.push_str(delta);
                            if let Some(AssistantContentBlock::ToolCall(tool_call)) =
                                output.content.get_mut(block.content_index)
                            {
                                tool_call.arguments = value_object_to_hashmap(
                                    parse_streaming_json_value(Some(&block.partial_json)),
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
            Some("content_block_stop") => {
                let provider_index =
                    event.pointer("/index").and_then(Value::as_u64).unwrap_or(0) as usize;
                if let Some(block) = blocks
                    .iter()
                    .find(|block| block.provider_index == provider_index)
                    && let Some(AssistantContentBlock::ToolCall(tool_call)) =
                        output.content.get_mut(block.content_index)
                {
                    tool_call.arguments = value_object_to_hashmap(parse_streaming_json_value(
                        Some(&block.partial_json),
                    ));
                }
            }
            Some("message_delta") => {
                if let Some(reason) = event.pointer("/delta/stop_reason").and_then(Value::as_str) {
                    let mapped = map_stop_reason(reason, event.pointer("/delta/stop_details"))?;
                    output.stop_reason = mapped.0;
                    output.error_message = mapped.1;
                }
                update_usage(&mut output.usage, event.get("usage"));
                calculate_cost(model, &mut output.usage);
            }
            Some("message_stop") | None | Some(_) => {}
        }
    }

    Ok(output)
}

#[derive(Debug, Clone)]
struct InProgressBlock {
    provider_index: usize,
    content_index: usize,
    partial_json: String,
}

#[derive(Debug, Default)]
struct IncrementalAnthropicSse {
    decoder: SseDecoderState,
    text: String,
    pending_utf8: Vec<u8>,
    saw_message_start: bool,
    saw_message_stop: bool,
    signal: Option<crate::types::AbortSignal>,
}

impl IncrementalAnthropicSse {
    #[allow(
        clippy::too_many_arguments,
        reason = "stream parser state is passed explicitly"
    )]
    fn push_chunk(
        &mut self,
        chunk: &[u8],
        model: &Model,
        context: &Context,
        is_oauth: bool,
        output: &mut AssistantMessage,
        blocks: &mut Vec<InProgressBlock>,
        stream: Option<&AssistantMessageEventStream>,
    ) -> Result<()> {
        self.pending_utf8.extend_from_slice(chunk);
        loop {
            match std::str::from_utf8(&self.pending_utf8) {
                Ok(text) => {
                    let text = text.to_owned();
                    self.pending_utf8.clear();
                    self.push_text(&text, model, context, is_oauth, output, blocks, stream)?;
                    return Ok(());
                }
                Err(error) if error.error_len().is_none() => {
                    let valid = error.valid_up_to();
                    if valid == 0 {
                        return Ok(());
                    }
                    let text = std::str::from_utf8(&self.pending_utf8[..valid])
                        .map_err(|error| AnthropicError::Sse(error.to_string()))?
                        .to_owned();
                    self.pending_utf8.drain(..valid);
                    self.push_text(&text, model, context, is_oauth, output, blocks, stream)?;
                }
                Err(error) => {
                    return Err(AnthropicError::Sse(format!(
                        "Anthropic SSE was not valid UTF-8: {error}"
                    )));
                }
            }
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "stream parser state is passed explicitly"
    )]
    fn push_text(
        &mut self,
        text: &str,
        model: &Model,
        context: &Context,
        is_oauth: bool,
        output: &mut AssistantMessage,
        blocks: &mut Vec<InProgressBlock>,
        stream: Option<&AssistantMessageEventStream>,
    ) -> Result<()> {
        self.text.push_str(text);
        while let Some(line_break) = next_line_break_index(&self.text) {
            let line = self.text[..line_break].to_owned();
            let mut next = line_break + 1;
            if self.text.as_bytes()[line_break] == b'\r'
                && self.text.as_bytes().get(next) == Some(&b'\n')
            {
                next += 1;
            }
            self.text.drain(..next);
            if let Some(event) = decode_sse_line(&line, &mut self.decoder) {
                self.handle_sse(event, model, context, is_oauth, output, blocks, stream)?;
            }
        }
        Ok(())
    }

    fn finish(
        &mut self,
        model: &Model,
        context: &Context,
        is_oauth: bool,
        output: &mut AssistantMessage,
        blocks: &mut Vec<InProgressBlock>,
        stream: Option<&AssistantMessageEventStream>,
    ) -> Result<()> {
        if !self.pending_utf8.is_empty() {
            let text = std::str::from_utf8(&self.pending_utf8)
                .map_err(|error| {
                    AnthropicError::Sse(format!("Anthropic SSE was not valid UTF-8: {error}"))
                })?
                .to_owned();
            self.pending_utf8.clear();
            self.push_text(&text, model, context, is_oauth, output, blocks, stream)?;
        }
        if !self.text.is_empty() {
            let line = std::mem::take(&mut self.text);
            if let Some(event) = decode_sse_line(&line, &mut self.decoder) {
                self.handle_sse(event, model, context, is_oauth, output, blocks, stream)?;
            }
        }
        if let Some(event) = flush_sse_event(&mut self.decoder) {
            self.handle_sse(event, model, context, is_oauth, output, blocks, stream)?;
        }
        if self.saw_message_start && !self.saw_message_stop {
            return Err(AnthropicError::Sse(
                "Anthropic stream ended before message_stop".to_owned(),
            ));
        }
        Ok(())
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "stream parser state is passed explicitly"
    )]
    fn handle_sse(
        &mut self,
        sse: ServerSentEvent,
        model: &Model,
        context: &Context,
        is_oauth: bool,
        output: &mut AssistantMessage,
        blocks: &mut Vec<InProgressBlock>,
        stream: Option<&AssistantMessageEventStream>,
    ) -> Result<()> {
        abort_if_requested(self.signal.as_ref())?;
        if sse.event.as_deref() == Some("error") {
            return Err(AnthropicError::Sse(sse.data));
        }
        if !ANTHROPIC_MESSAGE_EVENTS.contains(&sse.event.as_deref().unwrap_or_default()) {
            return Ok(());
        }
        let event = parse_json_with_repair::<Value>(&sse.data).map_err(|error| {
            AnthropicError::Json(format!(
                "Could not parse Anthropic SSE event {}: {error}; data={}; raw={}",
                sse.event.as_deref().unwrap_or_default(),
                sse.data,
                sse.raw.join("\\n")
            ))
        })?;
        match event.get("type").and_then(Value::as_str) {
            Some("message_start") => self.saw_message_start = true,
            Some("message_stop") => self.saw_message_stop = true,
            _ => {}
        }
        apply_anthropic_event(model, context, is_oauth, output, blocks, &event, stream)
    }
}

fn apply_anthropic_event(
    model: &Model,
    context: &Context,
    is_oauth: bool,
    output: &mut AssistantMessage,
    blocks: &mut Vec<InProgressBlock>,
    event: &Value,
    stream: Option<&AssistantMessageEventStream>,
) -> Result<()> {
    match event.get("type").and_then(Value::as_str) {
        Some("message_start") => {
            output.response_id = event
                .pointer("/message/id")
                .and_then(Value::as_str)
                .map(str::to_owned);
            update_usage(&mut output.usage, event.pointer("/message/usage"));
            calculate_cost(model, &mut output.usage);
        }
        Some("content_block_start") => {
            let provider_index =
                event.pointer("/index").and_then(Value::as_u64).unwrap_or(0) as usize;
            let content_index = output.content.len();
            let event_to_push = match event.pointer("/content_block/type").and_then(Value::as_str) {
                Some("text") => {
                    output
                        .content
                        .push(AssistantContentBlock::Text(TextContent {
                            content_type: TextContentType::Text,
                            text: String::new(),
                            text_signature: None,
                        }));
                    Some(AssistantMessageEvent::TextStart {
                        content_index,
                        partial: output.clone(),
                    })
                }
                Some("thinking") => {
                    output
                        .content
                        .push(AssistantContentBlock::Thinking(ThinkingContent {
                            content_type: ThinkingContentType::Thinking,
                            thinking: String::new(),
                            thinking_signature: Some(String::new()),
                            redacted: None,
                        }));
                    Some(AssistantMessageEvent::ThinkingStart {
                        content_index,
                        partial: output.clone(),
                    })
                }
                Some("redacted_thinking") => {
                    output
                        .content
                        .push(AssistantContentBlock::Thinking(ThinkingContent {
                            content_type: ThinkingContentType::Thinking,
                            thinking: "[Reasoning redacted]".to_owned(),
                            thinking_signature: event
                                .pointer("/content_block/data")
                                .and_then(Value::as_str)
                                .map(str::to_owned),
                            redacted: Some(true),
                        }));
                    Some(AssistantMessageEvent::ThinkingStart {
                        content_index,
                        partial: output.clone(),
                    })
                }
                Some("tool_use") => {
                    let name = event
                        .pointer("/content_block/name")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    output
                        .content
                        .push(AssistantContentBlock::ToolCall(ToolCall {
                            content_type: ToolCallType::ToolCall,
                            id: event
                                .pointer("/content_block/id")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_owned(),
                            name: if is_oauth {
                                from_claude_code_tool_name(name, context.tools.as_deref())
                                    .into_owned()
                            } else {
                                name.to_owned()
                            },
                            arguments: value_object_to_hashmap(
                                event
                                    .pointer("/content_block/input")
                                    .cloned()
                                    .unwrap_or_else(|| json!({})),
                            ),
                            thought_signature: None,
                        }));
                    Some(AssistantMessageEvent::ToolcallStart {
                        content_index,
                        partial: output.clone(),
                    })
                }
                _ => None,
            };
            if event_to_push.is_some() {
                blocks.push(InProgressBlock {
                    provider_index,
                    content_index,
                    partial_json: String::new(),
                });
            }
            if let (Some(stream), Some(event)) = (stream, event_to_push) {
                stream.push(event);
            }
        }
        Some("content_block_delta") => {
            let provider_index =
                event.pointer("/index").and_then(Value::as_u64).unwrap_or(0) as usize;
            let Some(block) = blocks
                .iter_mut()
                .find(|block| block.provider_index == provider_index)
            else {
                return Ok(());
            };
            let event_to_push = match event.pointer("/delta/type").and_then(Value::as_str) {
                Some("text_delta") => {
                    let delta = event
                        .pointer("/delta/text")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned();
                    if let Some(AssistantContentBlock::Text(text)) =
                        output.content.get_mut(block.content_index)
                    {
                        text.text.push_str(&delta);
                    }
                    Some(AssistantMessageEvent::TextDelta {
                        content_index: block.content_index,
                        delta,
                        partial: output.clone(),
                    })
                }
                Some("thinking_delta") => {
                    let delta = event
                        .pointer("/delta/thinking")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned();
                    if let Some(AssistantContentBlock::Thinking(thinking)) =
                        output.content.get_mut(block.content_index)
                    {
                        thinking.thinking.push_str(&delta);
                    }
                    Some(AssistantMessageEvent::ThinkingDelta {
                        content_index: block.content_index,
                        delta,
                        partial: output.clone(),
                    })
                }
                Some("signature_delta") => {
                    if let Some(delta) = event.pointer("/delta/signature").and_then(Value::as_str)
                        && let Some(AssistantContentBlock::Thinking(thinking)) =
                            output.content.get_mut(block.content_index)
                    {
                        thinking
                            .thinking_signature
                            .get_or_insert_with(String::new)
                            .push_str(delta);
                    }
                    None
                }
                Some("input_json_delta") => {
                    let delta = event
                        .pointer("/delta/partial_json")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned();
                    block.partial_json.push_str(&delta);
                    if let Some(AssistantContentBlock::ToolCall(tool_call)) =
                        output.content.get_mut(block.content_index)
                    {
                        tool_call.arguments = value_object_to_hashmap(parse_streaming_json_value(
                            Some(&block.partial_json),
                        ));
                    }
                    Some(AssistantMessageEvent::ToolcallDelta {
                        content_index: block.content_index,
                        delta,
                        partial: output.clone(),
                    })
                }
                _ => None,
            };
            if let (Some(stream), Some(event)) = (stream, event_to_push) {
                stream.push(event);
            }
        }
        Some("content_block_stop") => {
            let provider_index =
                event.pointer("/index").and_then(Value::as_u64).unwrap_or(0) as usize;
            let Some(block) = blocks
                .iter()
                .find(|block| block.provider_index == provider_index)
            else {
                return Ok(());
            };
            let event_to_push = match output.content.get_mut(block.content_index) {
                Some(AssistantContentBlock::Text(text)) => Some(AssistantMessageEvent::TextEnd {
                    content_index: block.content_index,
                    content: text.text.clone(),
                    partial: output.clone(),
                }),
                Some(AssistantContentBlock::Thinking(thinking)) => {
                    Some(AssistantMessageEvent::ThinkingEnd {
                        content_index: block.content_index,
                        content: thinking.thinking.clone(),
                        partial: output.clone(),
                    })
                }
                Some(AssistantContentBlock::ToolCall(tool_call)) => {
                    tool_call.arguments = value_object_to_hashmap(parse_streaming_json_value(
                        Some(&block.partial_json),
                    ));
                    Some(AssistantMessageEvent::ToolcallEnd {
                        content_index: block.content_index,
                        tool_call: tool_call.clone(),
                        partial: output.clone(),
                    })
                }
                None => None,
            };
            if let (Some(stream), Some(event)) = (stream, event_to_push) {
                stream.push(event);
            }
        }
        Some("message_delta") => {
            if let Some(reason) = event.pointer("/delta/stop_reason").and_then(Value::as_str) {
                let mapped = map_stop_reason(reason, event.pointer("/delta/stop_details"))?;
                output.stop_reason = mapped.0;
                output.error_message = mapped.1;
            }
            update_usage(&mut output.usage, event.get("usage"));
            calculate_cost(model, &mut output.usage);
        }
        Some("message_stop") | None | Some(_) => {}
    }
    Ok(())
}

fn empty_assistant_message(model: &Model) -> AssistantMessage {
    AssistantMessage {
        role: AssistantMessageRole::Assistant,
        content: Vec::new(),
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage {
            input: 0,
            output: 0,
            cache_read: 0,
            cache_write: 0,
            cache_write_1h: None,
            reasoning: None,
            total_tokens: 0,
            cost: UsageCost {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
                total: 0.0,
            },
        },
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp: now_ms(),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

fn update_usage(usage: &mut Usage, source: Option<&Value>) {
    let Some(source) = source else {
        return;
    };
    if let Some(input) = source.get("input_tokens").and_then(Value::as_u64) {
        usage.input = input;
    }
    if let Some(output) = source.get("output_tokens").and_then(Value::as_u64) {
        usage.output = output;
    }
    if let Some(cache_read) = source
        .get("cache_read_input_tokens")
        .and_then(Value::as_u64)
    {
        usage.cache_read = cache_read;
    }
    if let Some(cache_write) = source
        .get("cache_creation_input_tokens")
        .and_then(Value::as_u64)
    {
        usage.cache_write = cache_write;
    }
    if let Some(cache_write_1h) = source
        .pointer("/cache_creation/ephemeral_1h_input_tokens")
        .and_then(Value::as_u64)
    {
        usage.cache_write_1h = Some(cache_write_1h);
    }
    if let Some(thinking_tokens) = source
        .pointer("/output_tokens_details/thinking_tokens")
        .and_then(Value::as_u64)
    {
        usage.reasoning = Some(thinking_tokens);
    }
    usage.total_tokens = usage.input + usage.output + usage.cache_read + usage.cache_write;
}

fn calculate_cost(model: &Model, usage: &mut Usage) {
    let long_write = usage.cache_write_1h.unwrap_or(0);
    let short_write = usage.cache_write.saturating_sub(long_write);
    usage.cost.input = model.cost.input * usage.input as f64 / 1_000_000.0;
    usage.cost.output = model.cost.output * usage.output as f64 / 1_000_000.0;
    usage.cost.cache_read = model.cost.cache_read * usage.cache_read as f64 / 1_000_000.0;
    usage.cost.cache_write = (model.cost.cache_write * short_write as f64
        + model.cost.input * 2.0 * long_write as f64)
        / 1_000_000.0;
    usage.cost.total =
        usage.cost.input + usage.cost.output + usage.cost.cache_read + usage.cost.cache_write;
}

fn map_stop_reason(
    reason: &str,
    stop_details: Option<&Value>,
) -> Result<(StopReason, Option<String>)> {
    match reason {
        "end_turn" | "pause_turn" | "stop_sequence" => Ok((StopReason::Stop, None)),
        "max_tokens" => Ok((StopReason::Length, None)),
        "tool_use" => Ok((StopReason::ToolUse, None)),
        "refusal" => Ok((
            StopReason::Error,
            Some(
                stop_details
                    .and_then(|details| details.get("explanation"))
                    .and_then(Value::as_str)
                    .unwrap_or("The model refused to complete the request")
                    .to_owned(),
            ),
        )),
        "sensitive" => Ok((StopReason::Error, None)),
        other => Err(AnthropicError::Sse(format!(
            "Unhandled stop reason: {other}"
        ))),
    }
}

fn value_object_to_hashmap(value: Value) -> HashMap<String, Value> {
    match value {
        Value::Object(map) => map.into_iter().collect(),
        _ => HashMap::new(),
    }
}

fn normalize_tool_call_id(id: &str) -> String {
    id.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .take(64)
        .collect()
}

#[allow(dead_code)]
fn done_reason(reason: StopReason) -> Option<DoneStopReason> {
    match reason {
        StopReason::Stop => Some(DoneStopReason::Stop),
        StopReason::Length => Some(DoneStopReason::Length),
        StopReason::ToolUse => Some(DoneStopReason::ToolUse),
        StopReason::Error | StopReason::Aborted => None,
    }
}

#[allow(dead_code)]
fn error_reason(reason: StopReason) -> Option<ErrorStopReason> {
    match reason {
        StopReason::Error => Some(ErrorStopReason::Error),
        StopReason::Aborted => Some(ErrorStopReason::Aborted),
        StopReason::Stop | StopReason::Length | StopReason::ToolUse => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        AnthropicMessagesCompat, ModelCost, ModelInput, TextContent, ThinkingLevelMap, UserMessage,
        UserMessageRole,
    };

    fn model() -> Model {
        Model {
            id: "claude-opus-4-8".to_owned(),
            name: "Claude Opus 4.8".to_owned(),
            api: "anthropic-messages".to_owned(),
            provider: "anthropic".to_owned(),
            base_url: "https://api.anthropic.com".to_owned(),
            reasoning: true,
            thinking_level_map: Some(ThinkingLevelMap::new()),
            input: vec![ModelInput::Text],
            cost: ModelCost {
                input: 3.0,
                output: 15.0,
                cache_read: 0.3,
                cache_write: 3.75,
            },
            context_window: 200_000,
            max_tokens: 4096,
            headers: None,
            compat: Some(ModelCompat::AnthropicMessages(AnthropicMessagesCompat {
                force_adaptive_thinking: Some(true),
                ..AnthropicMessagesCompat::default()
            })),
        }
    }

    fn context() -> Context {
        Context {
            system_prompt: Some("system".to_owned()),
            messages: vec![Message::User(UserMessage {
                role: UserMessageRole::User,
                content: UserMessageContent::Text("Hello".to_owned()),
                timestamp: 0,
            })],
            tools: None,
        }
    }

    fn sse(event: &str, data: Value) -> String {
        format!("event: {event}\ndata: {data}\n\n")
    }

    #[test]
    fn parses_raw_sse_and_validates_message_stop() {
        let raw = sse(
            "message_start",
            json!({ "type": "message_start", "message": { "id": "msg", "usage": { "input_tokens": 1 } } }),
        );

        let error = parse_anthropic_sse_events(&raw).expect_err("message_stop is required");
        assert_eq!(
            error.to_string(),
            "Anthropic stream ended before message_stop"
        );
    }

    #[test]
    fn ignores_unknown_sse_events_after_message_stop() {
        let raw = format!(
            "{}{}{}{}{}{}{}",
            sse(
                "message_start",
                json!({ "type": "message_start", "message": { "id": "msg", "usage": { "input_tokens": 12 } } })
            ),
            sse(
                "content_block_start",
                json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" } })
            ),
            sse(
                "content_block_delta",
                json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": "Hello" } })
            ),
            sse(
                "content_block_stop",
                json!({ "type": "content_block_stop", "index": 0 })
            ),
            sse(
                "message_delta",
                json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn" }, "usage": { "output_tokens": 5 } })
            ),
            sse("message_stop", json!({ "type": "message_stop" })),
            "event: proxy.stats\ndata: not json\n\n",
        );

        let message =
            assistant_message_from_sse(&model(), &context(), &raw, false).expect("valid fixture");
        assert_eq!(message.stop_reason, StopReason::Stop);
        assert_eq!(message.usage.input, 12);
        assert_eq!(message.usage.output, 5);
        assert_eq!(message.usage.total_tokens, 17);
        assert_eq!(
            message.content,
            vec![AssistantContentBlock::Text(TextContent {
                content_type: TextContentType::Text,
                text: "Hello".to_owned(),
                text_signature: None,
            })]
        );
    }

    #[test]
    fn preserves_usage_cache_details_and_thinking_signatures() {
        let raw = format!(
            "{}{}{}{}{}{}{}",
            sse(
                "message_start",
                json!({
                    "type": "message_start",
                    "message": {
                        "id": "msg",
                        "usage": {
                            "input_tokens": 10,
                            "output_tokens": 0,
                            "cache_read_input_tokens": 2,
                            "cache_creation_input_tokens": 8,
                            "cache_creation": { "ephemeral_1h_input_tokens": 3 }
                        }
                    }
                })
            ),
            sse(
                "content_block_start",
                json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "thinking", "thinking": "" } })
            ),
            sse(
                "content_block_delta",
                json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "thinking_delta", "thinking": "secret" } })
            ),
            sse(
                "content_block_delta",
                json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "signature_delta", "signature": "sig" } })
            ),
            sse(
                "content_block_stop",
                json!({ "type": "content_block_stop", "index": 0 })
            ),
            sse(
                "message_delta",
                json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn" }, "usage": { "output_tokens": 4, "output_tokens_details": { "thinking_tokens": 4 } } })
            ),
            sse("message_stop", json!({ "type": "message_stop" })),
        );

        let message =
            assistant_message_from_sse(&model(), &context(), &raw, false).expect("valid fixture");
        assert_eq!(message.usage.cache_read, 2);
        assert_eq!(message.usage.cache_write, 8);
        assert_eq!(message.usage.cache_write_1h, Some(3));
        assert_eq!(message.usage.reasoning, Some(4));
        assert!((message.usage.cost.cache_write - 0.00003675).abs() < 1e-12);
        assert_eq!(
            message.content,
            vec![AssistantContentBlock::Thinking(ThinkingContent {
                content_type: ThinkingContentType::Thinking,
                thinking: "secret".to_owned(),
                thinking_signature: Some("sig".to_owned()),
                redacted: None,
            })]
        );
    }

    #[test]
    fn repairs_malformed_streamed_tool_json() {
        let raw = format!(
            "{}{}{}{}{}{}",
            sse(
                "message_start",
                json!({ "type": "message_start", "message": { "id": "msg", "usage": {} } })
            ),
            sse(
                "content_block_start",
                json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "tool_use", "id": "tool", "name": "edit", "input": {} } })
            ),
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\\\"A\\H\\\",\\\"text\\\":\\\"col1\tcol2\\\"}\"}}\n\n",
            sse(
                "content_block_stop",
                json!({ "type": "content_block_stop", "index": 0 })
            ),
            sse(
                "message_delta",
                json!({ "type": "message_delta", "delta": { "stop_reason": "tool_use" }, "usage": {} })
            ),
            sse("message_stop", json!({ "type": "message_stop" })),
        );

        let message =
            assistant_message_from_sse(&model(), &context(), &raw, false).expect("valid fixture");
        let AssistantContentBlock::ToolCall(tool_call) = &message.content[0] else {
            panic!("expected tool call");
        };
        assert_eq!(message.stop_reason, StopReason::ToolUse);
        assert_eq!(tool_call.name, "edit");
        assert_eq!(tool_call.arguments.get("path"), Some(&json!("A\\H")));
        assert_eq!(tool_call.arguments.get("text"), Some(&json!("col1\tcol2")));
    }

    #[test]
    fn builds_payload_headers_and_claude_code_tool_names() {
        let mut context = context();
        context.tools = Some(vec![Tool {
            name: "todowrite".to_owned(),
            description: "todo".to_owned(),
            parameters: json!({ "type": "object", "properties": { "task": { "type": "string" } }, "required": ["task"] }),
        }]);
        let options = AnthropicOptions {
            stream: StreamOptions {
                api_key: Some("sk-ant-oat-token".to_owned()),
                cache_retention: Some(CacheRetention::Long),
                session_id: Some("session".to_owned()),
                max_tokens: Some(128),
                ..StreamOptions::default()
            },
            thinking_enabled: Some(true),
            effort: Some(AnthropicEffort::XHigh),
            ..AnthropicOptions::default()
        };

        let payload = build_request_payload(&model(), &context, true, Some(&options));
        assert_eq!(payload["max_tokens"], json!(128));
        assert_eq!(payload["tools"][0]["name"], json!("TodoWrite"));
        assert_eq!(payload["tools"][0]["eager_input_streaming"], json!(true));
        assert_eq!(
            payload["thinking"],
            json!({ "type": "adaptive", "display": "summarized" })
        );
        assert_eq!(payload["output_config"], json!({ "effort": "xhigh" }));
        assert_eq!(
            payload["system"][0]["cache_control"],
            json!({ "type": "ephemeral", "ttl": "1h" })
        );

        let headers = build_request_headers(
            &model(),
            Some("sk-ant-oat-token"),
            Some(&options),
            true,
            false,
            None,
        );
        assert_eq!(
            headers
                .get("authorization")
                .and_then(|value| value.as_deref()),
            Some("Bearer sk-ant-oat-token")
        );
        assert!(
            headers
                .get("anthropic-beta")
                .and_then(|value| value.as_deref())
                .is_some_and(|value| value.contains("oauth-2025-04-20"))
        );
    }

    #[test]
    fn stream_uses_injected_raw_sse_fixture() {
        let raw = format!(
            "{}{}",
            sse(
                "message_start",
                json!({ "type": "message_start", "message": { "id": "msg", "usage": {} } })
            ),
            sse("message_stop", json!({ "type": "message_stop" })),
        );
        let options = AnthropicOptions {
            client: Some(AnthropicClientConfig {
                raw_sse: Some(raw),
                status: 200,
                response_headers: HashMap::new(),
            }),
            ..AnthropicOptions::default()
        };

        let stream = stream(&model(), &context(), Some(&options)).expect("fixture stream");
        assert_eq!(stream.message.response_id.as_deref(), Some("msg"));
    }
}
