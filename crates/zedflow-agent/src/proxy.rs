//! Proxy assistant-event parsing seam ported from Pi `proxy.ts`.
//!
//! The network `fetch` wrapper is intentionally not ported here; Rust callers feed the
//! server-sent JSON event payloads into this module and receive canonical `zedflow-ai`
//! assistant message events.
//!
//! PORT PLACEHOLDER: Pi's `streamProxy` owns HTTP fetch, abort wiring, and SSE body
//! decoding. No HTTP client/runtime dependency is approved for `zedflow-agent`; add a
//! real stream wrapper only after that dependency decision is made.

use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use zedflow_ai::{
    AssistantContentBlock, AssistantMessage, AssistantMessageEvent, AssistantMessageRole,
    DoneStopReason, ErrorStopReason, Model, StopReason, TextContent, TextContentType,
    ThinkingContent, ThinkingContentType, ToolCall, ToolCallType, Usage, parse_streaming_json,
};

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

/// Build the initial partial assistant message used by proxy event reconstruction.
#[must_use]
pub fn initial_proxy_assistant_message(model: &Model) -> AssistantMessage {
    AssistantMessage {
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
    }
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
    partial: &mut AssistantMessage,
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
    partial: &mut AssistantMessage,
    state: &mut ProxyEventState,
) -> Result<Option<AssistantMessageEvent>, ProxyEventError> {
    match proxy_event {
        ProxyAssistantMessageEvent::Start => Ok(Some(AssistantMessageEvent::Start {
            partial: partial.clone(),
        })),
        ProxyAssistantMessageEvent::TextStart { content_index } => {
            set_content(
                partial,
                content_index,
                AssistantContentBlock::Text(TextContent {
                    content_type: TextContentType::Text,
                    text: String::new(),
                    text_signature: None,
                }),
            );
            Ok(Some(AssistantMessageEvent::TextStart {
                content_index,
                partial: partial.clone(),
            }))
        }
        ProxyAssistantMessageEvent::TextDelta {
            content_index,
            delta,
        } => {
            let text = text_content_mut(partial, content_index)?;
            text.text.push_str(&delta);
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
            let text = text_content_mut(partial, content_index)?;
            text.text_signature = content_signature;
            let content = text.text.clone();
            Ok(Some(AssistantMessageEvent::TextEnd {
                content_index,
                content,
                partial: partial.clone(),
            }))
        }
        ProxyAssistantMessageEvent::ThinkingStart { content_index } => {
            set_content(
                partial,
                content_index,
                AssistantContentBlock::Thinking(ThinkingContent {
                    content_type: ThinkingContentType::Thinking,
                    thinking: String::new(),
                    thinking_signature: None,
                    redacted: None,
                }),
            );
            Ok(Some(AssistantMessageEvent::ThinkingStart {
                content_index,
                partial: partial.clone(),
            }))
        }
        ProxyAssistantMessageEvent::ThinkingDelta {
            content_index,
            delta,
        } => {
            let thinking = thinking_content_mut(partial, content_index)?;
            thinking.thinking.push_str(&delta);
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
            let thinking = thinking_content_mut(partial, content_index)?;
            thinking.thinking_signature = content_signature;
            let content = thinking.thinking.clone();
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
            set_content(
                partial,
                content_index,
                AssistantContentBlock::ToolCall(ToolCall {
                    content_type: ToolCallType::ToolCall,
                    id,
                    name: tool_name,
                    arguments: HashMap::new(),
                    thought_signature: None,
                }),
            );
            Ok(Some(AssistantMessageEvent::ToolcallStart {
                content_index,
                partial: partial.clone(),
            }))
        }
        ProxyAssistantMessageEvent::ToolcallDelta {
            content_index,
            delta,
        } => {
            let tool_call = tool_call_mut(partial, content_index)?;
            let json = state.tool_json.entry(content_index).or_default();
            json.push_str(&delta);
            tool_call.arguments = parse_streaming_json::<HashMap<String, Value>>(Some(json));
            Ok(Some(AssistantMessageEvent::ToolcallDelta {
                content_index,
                delta,
                partial: partial.clone(),
            }))
        }
        ProxyAssistantMessageEvent::ToolcallEnd { content_index } => {
            state.tool_json.remove(&content_index);
            let tool_call = tool_call_mut(partial, content_index)?.clone();
            Ok(Some(AssistantMessageEvent::ToolcallEnd {
                content_index,
                tool_call,
                partial: partial.clone(),
            }))
        }
        ProxyAssistantMessageEvent::Done { reason, usage } => {
            partial.stop_reason = done_reason(reason);
            partial.usage = usage;
            Ok(Some(AssistantMessageEvent::Done {
                reason,
                message: partial.clone(),
            }))
        }
        ProxyAssistantMessageEvent::Error {
            reason,
            error_message,
            usage,
        } => {
            partial.stop_reason = error_reason(reason);
            partial.error_message = error_message;
            partial.usage = usage;
            Ok(Some(AssistantMessageEvent::Error {
                reason,
                error: partial.clone(),
            }))
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
