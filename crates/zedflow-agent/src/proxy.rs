//! Pi-compatible proxy HTTP and server-sent event transport.

use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;
use std::io::{BufRead, BufReader};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use zedflow_ai::{
    AssistantContentBlock, AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream,
    AssistantMessageRole, CacheRetention, Context, DoneStopReason, ErrorStopReason, Model,
    ProviderHeaders, SharedAssistantMessage, SimpleStreamOptions, StopReason, TextContent,
    TextContentType, ThinkingBudgets, ThinkingContent, ThinkingContentType, ThinkingLevel,
    ToolCall, ToolCallType, Transport, Usage, parse_streaming_json,
};

/// Options for [`stream_proxy`].
#[derive(Clone)]
pub struct ProxyStreamOptions {
    /// Options forwarded to the proxy server.
    pub stream: SimpleStreamOptions,
    /// Bearer token used to authenticate to the proxy.
    pub auth_token: String,
    /// Proxy base URL, without `/api/stream`.
    pub proxy_url: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProxyRequestOptions<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<ThinkingLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_retention: Option<CacheRetention>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: &'a Option<ProviderHeaders>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: &'a Option<HashMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transport: Option<Transport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking_budgets: &'a Option<ThinkingBudgets>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_retry_delay_ms: Option<u64>,
}

#[derive(Serialize)]
struct ProxyRequest<'a> {
    model: &'a Model,
    context: &'a Context,
    options: ProxyRequestOptions<'a>,
}

/// Proxy event types sent by the Pi proxy server without bulky partial messages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ProxyAssistantMessageEvent {
    /// Assistant stream started.
    Start,
    /// Text content block started.
    TextStart { content_index: usize },
    /// Text delta for an existing text block.
    TextDelta { content_index: usize, delta: String },
    /// Text content block ended.
    TextEnd {
        content_index: usize,
        content_signature: Option<String>,
    },
    /// Thinking content block started.
    ThinkingStart { content_index: usize },
    /// Thinking delta for an existing thinking block.
    ThinkingDelta { content_index: usize, delta: String },
    /// Thinking content block ended.
    ThinkingEnd {
        content_index: usize,
        content_signature: Option<String>,
    },
    /// Tool call content block started.
    ToolcallStart {
        content_index: usize,
        id: String,
        tool_name: String,
    },
    /// Tool call JSON delta.
    ToolcallDelta { content_index: usize, delta: String },
    /// Tool call content block ended.
    ToolcallEnd { content_index: usize },
    /// Successful terminal event.
    Done {
        reason: DoneStopReason,
        usage: Usage,
    },
    /// Error terminal event.
    Error {
        reason: ErrorStopReason,
        error_message: Option<String>,
        usage: Usage,
    },
}

/// State needed to rebuild partial tool-call JSON across proxy deltas.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProxyEventState {
    tool_json: HashMap<usize, String>,
}

/// Error returned when a proxy event does not match the current partial message shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyEventError {
    message: String,
}

impl ProxyEventError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ProxyEventError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl StdError for ProxyEventError {}

/// POST a request to the proxy and stream its SSE assistant events.
#[must_use]
pub fn stream_proxy(
    model: &Model,
    context: &Context,
    options: ProxyStreamOptions,
) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    let output = stream.clone();
    let model = model.clone();
    let context = context.clone();

    thread::spawn(move || run_proxy_request(&model, &context, &options, &output));
    stream
}

fn run_proxy_request(
    model: &Model,
    context: &Context,
    options: &ProxyStreamOptions,
    stream: &AssistantMessageEventStream,
) {
    let partial = initial_proxy_assistant_message(model);
    let mut state = ProxyEventState::default();
    let result = (|| -> Result<(), Box<dyn StdError + Send + Sync>> {
        if options
            .stream
            .stream
            .signal
            .as_ref()
            .is_some_and(|signal| signal.aborted())
        {
            return Err("Request aborted by user".into());
        }
        let base = options.proxy_url.trim_end_matches('/');
        let request_options = ProxyRequestOptions {
            temperature: options.stream.stream.temperature,
            max_tokens: options.stream.stream.max_tokens,
            reasoning: options.stream.reasoning,
            cache_retention: options.stream.stream.cache_retention,
            session_id: &options.stream.stream.session_id,
            headers: &options.stream.stream.headers,
            metadata: &options.stream.stream.metadata,
            transport: options.stream.stream.transport,
            thinking_budgets: &options.stream.thinking_budgets,
            max_retry_delay_ms: options.stream.stream.max_retry_delay_ms,
        };
        let response = reqwest::blocking::Client::new()
            .post(format!("{base}/api/stream"))
            .bearer_auth(&options.auth_token)
            .json(&ProxyRequest {
                model,
                context,
                options: request_options,
            })
            .send()?;
        if !response.status().is_success() {
            let status = response.status();
            let fallback = format!(
                "Proxy error: {} {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("")
            )
            .trim_end()
            .to_owned();
            let message = response
                .json::<Value>()
                .ok()
                .and_then(|body| body.get("error").and_then(Value::as_str).map(str::to_owned))
                .map_or(fallback, |error| format!("Proxy error: {error}"));
            return Err(message.into());
        }

        for line in BufReader::new(response).lines() {
            if options
                .stream
                .stream
                .signal
                .as_ref()
                .is_some_and(|signal| signal.aborted())
            {
                return Err("Request aborted by user".into());
            }
            let line = line?;
            let Some(data) = line
                .strip_prefix("data: ")
                .map(str::trim)
                .filter(|data| !data.is_empty())
            else {
                continue;
            };
            if let Some(event) = process_proxy_event_json(data, &partial, &mut state)? {
                stream.push(event);
            }
        }
        stream.end(None);
        Ok(())
    })();

    if let Err(error) = result {
        let aborted = options
            .stream
            .stream
            .signal
            .as_ref()
            .is_some_and(|signal| signal.aborted());
        let reason = if aborted {
            ErrorStopReason::Aborted
        } else {
            ErrorStopReason::Error
        };
        let error = partial.with_mut(|message| {
            message.stop_reason = error_reason(reason);
            message.error_message = Some(error.to_string());
            message.clone()
        });
        stream.push(AssistantMessageEvent::Error { reason, error });
        stream.end(None);
    }
}

/// Build the initial partial assistant message used by proxy event reconstruction.
#[must_use]
pub fn initial_proxy_assistant_message(model: &Model) -> SharedAssistantMessage {
    SharedAssistantMessage::new(AssistantMessage {
        role: AssistantMessageRole::Assistant,
        stop_reason: StopReason::Stop,
        content: Vec::new(),
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage::default(),
        error_message: None,
        timestamp: now_millis(),
    })
}

/// Parse one proxy JSON payload.
///
/// # Errors
///
/// Returns serde's JSON error when the payload is not a valid proxy event.
pub fn parse_proxy_event_json(json: &str) -> serde_json::Result<ProxyAssistantMessageEvent> {
    serde_json::from_str(json)
}

/// Process one proxy JSON payload and update `partial`.
///
/// # Errors
///
/// Returns a JSON parse error or a shape error when a delta targets the wrong content kind.
pub fn process_proxy_event_json(
    json: &str,
    partial: &SharedAssistantMessage,
    state: &mut ProxyEventState,
) -> Result<Option<AssistantMessageEvent>, Box<dyn StdError + Send + Sync>> {
    let event = parse_proxy_event_json(json)?;
    process_proxy_event(event, partial, state)
        .map_err(|error| Box::new(error) as Box<dyn StdError + Send + Sync>)
}

/// Process one proxy event and update `partial` in place.
///
/// # Errors
///
/// Returns an error when a delta/end event targets content that is absent or has a different kind.
pub fn process_proxy_event(
    proxy_event: ProxyAssistantMessageEvent,
    partial: &SharedAssistantMessage,
    state: &mut ProxyEventState,
) -> Result<Option<AssistantMessageEvent>, ProxyEventError> {
    match proxy_event {
        ProxyAssistantMessageEvent::Start => Ok(Some(AssistantMessageEvent::Start {
            partial: partial.clone(),
        })),
        ProxyAssistantMessageEvent::TextStart { content_index } => {
            partial.with_mut(|message| {
                set_content(
                    message,
                    content_index,
                    AssistantContentBlock::Text(TextContent {
                        content_type: TextContentType::Text,
                        text: String::new(),
                        text_signature: None,
                    }),
                );
            });
            Ok(Some(AssistantMessageEvent::TextStart {
                content_index,
                partial: partial.clone(),
            }))
        }
        ProxyAssistantMessageEvent::TextDelta {
            content_index,
            delta,
        } => {
            partial.with_mut(|message| {
                text_content_mut(message, content_index).map(|text| text.text.push_str(&delta))
            })?;
            Ok(Some(AssistantMessageEvent::TextDelta {
                content_index,
                delta,
                partial: partial.clone(),
            }))
        }
        ProxyAssistantMessageEvent::TextEnd {
            content_index,
            content_signature,
        } => {
            let content = partial.with_mut(|message| {
                let text = text_content_mut(message, content_index)?;
                text.text_signature = content_signature;
                Ok::<_, ProxyEventError>(text.text.clone())
            })?;
            Ok(Some(AssistantMessageEvent::TextEnd {
                content_index,
                content,
                partial: partial.clone(),
            }))
        }
        ProxyAssistantMessageEvent::ThinkingStart { content_index } => {
            partial.with_mut(|message| {
                set_content(
                    message,
                    content_index,
                    AssistantContentBlock::Thinking(ThinkingContent {
                        content_type: ThinkingContentType::Thinking,
                        thinking: String::new(),
                        thinking_signature: None,
                        redacted: None,
                    }),
                );
            });
            Ok(Some(AssistantMessageEvent::ThinkingStart {
                content_index,
                partial: partial.clone(),
            }))
        }
        ProxyAssistantMessageEvent::ThinkingDelta {
            content_index,
            delta,
        } => {
            partial.with_mut(|message| {
                thinking_content_mut(message, content_index)
                    .map(|thinking| thinking.thinking.push_str(&delta))
            })?;
            Ok(Some(AssistantMessageEvent::ThinkingDelta {
                content_index,
                delta,
                partial: partial.clone(),
            }))
        }
        ProxyAssistantMessageEvent::ThinkingEnd {
            content_index,
            content_signature,
        } => {
            let content = partial.with_mut(|message| {
                let thinking = thinking_content_mut(message, content_index)?;
                thinking.thinking_signature = content_signature;
                Ok::<_, ProxyEventError>(thinking.thinking.clone())
            })?;
            Ok(Some(AssistantMessageEvent::ThinkingEnd {
                content_index,
                content,
                partial: partial.clone(),
            }))
        }
        ProxyAssistantMessageEvent::ToolcallStart {
            content_index,
            id,
            tool_name,
        } => {
            state.tool_json.insert(content_index, String::new());
            partial.with_mut(|message| {
                set_content(
                    message,
                    content_index,
                    AssistantContentBlock::ToolCall(ToolCall {
                        content_type: ToolCallType::ToolCall,
                        id,
                        name: tool_name,
                        arguments: HashMap::new(),
                        thought_signature: None,
                    }),
                );
            });
            Ok(Some(AssistantMessageEvent::ToolcallStart {
                content_index,
                partial: partial.clone(),
            }))
        }
        ProxyAssistantMessageEvent::ToolcallDelta {
            content_index,
            delta,
        } => {
            let json = state.tool_json.entry(content_index).or_default();
            json.push_str(&delta);
            let arguments = parse_streaming_json::<HashMap<String, Value>>(Some(json));
            partial.with_mut(|message| {
                tool_call_mut(message, content_index)
                    .map(|tool_call| tool_call.arguments = arguments)
            })?;
            Ok(Some(AssistantMessageEvent::ToolcallDelta {
                content_index,
                delta,
                partial: partial.clone(),
            }))
        }
        ProxyAssistantMessageEvent::ToolcallEnd { content_index } => {
            state.tool_json.remove(&content_index);
            let tool_call =
                partial.with_mut(|message| tool_call_mut(message, content_index).cloned())?;
            Ok(Some(AssistantMessageEvent::ToolcallEnd {
                content_index,
                tool_call,
                partial: partial.clone(),
            }))
        }
        ProxyAssistantMessageEvent::Done { reason, usage } => {
            let message = partial.with_mut(|message| {
                message.stop_reason = done_reason(reason);
                message.usage = usage;
                message.clone()
            });
            Ok(Some(AssistantMessageEvent::Done { reason, message }))
        }
        ProxyAssistantMessageEvent::Error {
            reason,
            error_message,
            usage,
        } => {
            let error = partial.with_mut(|message| {
                message.stop_reason = error_reason(reason);
                message.error_message = error_message;
                message.usage = usage;
                message.clone()
            });
            Ok(Some(AssistantMessageEvent::Error { reason, error }))
        }
    }
}

fn set_content(
    partial: &mut AssistantMessage,
    content_index: usize,
    content: AssistantContentBlock,
) {
    if partial.content.len() <= content_index {
        partial
            .content
            .resize_with(content_index + 1, empty_text_content);
    }
    partial.content[content_index] = content;
}

fn empty_text_content() -> AssistantContentBlock {
    AssistantContentBlock::Text(TextContent {
        content_type: TextContentType::Text,
        text: String::new(),
        text_signature: None,
    })
}

fn text_content_mut(
    partial: &mut AssistantMessage,
    content_index: usize,
) -> Result<&mut TextContent, ProxyEventError> {
    match partial.content.get_mut(content_index) {
        Some(AssistantContentBlock::Text(content)) => Ok(content),
        _ => Err(ProxyEventError::new(
            "Received text event for non-text content",
        )),
    }
}

fn thinking_content_mut(
    partial: &mut AssistantMessage,
    content_index: usize,
) -> Result<&mut ThinkingContent, ProxyEventError> {
    match partial.content.get_mut(content_index) {
        Some(AssistantContentBlock::Thinking(content)) => Ok(content),
        _ => Err(ProxyEventError::new(
            "Received thinking event for non-thinking content",
        )),
    }
}

fn tool_call_mut(
    partial: &mut AssistantMessage,
    content_index: usize,
) -> Result<&mut ToolCall, ProxyEventError> {
    match partial.content.get_mut(content_index) {
        Some(AssistantContentBlock::ToolCall(content)) => Ok(content),
        _ => Err(ProxyEventError::new(
            "Received toolcall event for non-toolCall content",
        )),
    }
}

fn done_reason(reason: DoneStopReason) -> StopReason {
    match reason {
        DoneStopReason::Stop => StopReason::Stop,
        DoneStopReason::Length => StopReason::Length,
        DoneStopReason::ToolUse => StopReason::ToolUse,
    }
}

fn error_reason(reason: ErrorStopReason) -> StopReason {
    match reason {
        ErrorStopReason::Aborted => StopReason::Aborted,
        ErrorStopReason::Error => StopReason::Error,
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            duration.as_millis().try_into().unwrap_or(u64::MAX)
        })
}
