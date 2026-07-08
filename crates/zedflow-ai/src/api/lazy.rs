//! Lazy provider stream helpers ported from Pi's `packages/ai/src/api/lazy.ts`.

use std::collections::VecDeque;
use std::fmt;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Pi API identifier.
pub type Api = String;

/// Pi provider identifier.
pub type ProviderId = String;

/// Model metadata used by lazy setup error messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    /// Model identifier.
    pub id: String,
    /// API kind used by the model.
    pub api: Api,
    /// Provider identifier used by the model.
    pub provider: ProviderId,
}

impl Model {
    /// Creates model metadata for lazy stream wrappers.
    #[must_use]
    pub fn new(id: impl Into<String>, api: impl Into<String>, provider: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            api: api.into(),
            provider: provider.into(),
        }
    }
}

/// Provider request context placeholder for the Pi stream contract.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/types.ts Context`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `carry system prompt, message history, and tool declarations passed through lazyApi to the loaded provider stream functions`.
/// Replacement decision needed before production use.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Context;

/// Stream options placeholder for provider stream calls.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/types.ts StreamOptions`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `carry provider stream options through lazyApi without mutation`.
/// Replacement decision needed before production use.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StreamOptions;

/// Simple stream options placeholder for provider stream calls.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/types.ts SimpleStreamOptions`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `carry simple stream options including reasoning settings through lazyApi without mutation`.
/// Replacement decision needed before production use.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SimpleStreamOptions;

/// Token usage counters for an assistant response.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Usage {
    /// Input tokens.
    pub input: u64,
    /// Output tokens.
    pub output: u64,
    /// Cache-read tokens.
    pub cache_read: u64,
    /// Cache-write tokens.
    pub cache_write: u64,
    /// Total tokens.
    pub total_tokens: u64,
    /// Token cost counters.
    pub cost: UsageCost,
}

/// Token cost counters for an assistant response.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct UsageCost {
    /// Input token cost.
    pub input: f64,
    /// Output token cost.
    pub output: f64,
    /// Cache-read token cost.
    pub cache_read: f64,
    /// Cache-write token cost.
    pub cache_write: f64,
    /// Total cost.
    pub total: f64,
}

/// Assistant message stop reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// Provider stopped normally.
    Stop,
    /// Provider hit a length limit.
    Length,
    /// Provider requested tool use.
    ToolUse,
    /// Provider or setup failed.
    Error,
    /// Provider stream was aborted.
    Aborted,
}

/// Assistant message content placeholder.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/types.ts TextContent | ThinkingContent | ToolCall`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `represent text, thinking, and tool-call content blocks in assistant messages`.
/// Replacement decision needed before production use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssistantContent {
    /// Opaque content block retained until the full content model is ported.
    Opaque(String),
}

/// Assistant response message emitted as a stream final result.
#[derive(Debug, Clone, PartialEq)]
pub struct AssistantMessage {
    /// Message role, always `assistant` for this ported type.
    pub role: &'static str,
    /// Assistant content blocks.
    pub content: Vec<AssistantContent>,
    /// API kind used by the model.
    pub api: Api,
    /// Provider identifier used by the model.
    pub provider: ProviderId,
    /// Requested model identifier.
    pub model: String,
    /// Token usage counters.
    pub usage: Usage,
    /// Final stop reason.
    pub stop_reason: StopReason,
    /// Error text when `stop_reason` is [`StopReason::Error`] or [`StopReason::Aborted`].
    pub error_message: Option<String>,
    /// Unix timestamp in milliseconds.
    pub timestamp: u64,
}

/// Assistant message stream event.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/types.ts AssistantMessageEvent`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `represent all Pi stream event variants; lazy.ts only needs to forward events and synthesize error terminal events`.
/// Replacement decision needed before production use.
#[derive(Debug, Clone, PartialEq)]
pub enum AssistantMessageEvent {
    /// Non-terminal event retained opaquely until all event variants are ported.
    Opaque(String),
    /// Successful terminal event.
    Done {
        /// Successful stop reason.
        reason: StopReason,
        /// Final assistant message.
        message: AssistantMessage,
    },
    /// Error terminal event.
    Error {
        /// Error stop reason.
        reason: StopReason,
        /// Final assistant error message.
        error: AssistantMessage,
    },
}

/// In-memory assistant event stream.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/utils/event-stream.ts AssistantMessageEventStream`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return synchronously, support async iteration, queue pushed events, ignore pushes after completion, and expose the final AssistantMessage result`.
/// Replacement decision needed before production use.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AssistantMessageEventStream {
    events: VecDeque<AssistantMessageEvent>,
    done: bool,
    final_result: Option<AssistantMessage>,
}

impl AssistantMessageEventStream {
    /// Creates an empty assistant message event stream.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Pushes an event onto the stream, ignoring pushes after completion.
    pub fn push(&mut self, event: AssistantMessageEvent) {
        if self.done {
            return;
        }

        if let Some(result) = terminal_result(&event) {
            self.done = true;
            self.final_result = Some(result);
        }

        self.events.push_back(event);
    }

    /// Ends the stream and optionally records a final result.
    pub fn end(&mut self, result: Option<AssistantMessage>) {
        self.done = true;
        if self.final_result.is_none() {
            self.final_result = result;
        }
    }

    /// Returns whether the stream has completed.
    #[must_use]
    pub const fn is_done(&self) -> bool {
        self.done
    }

    /// Returns the final assistant message if the stream has one.
    #[must_use]
    pub fn result(&self) -> Option<&AssistantMessage> {
        self.final_result.as_ref()
    }

    /// Returns the queued events.
    #[must_use]
    pub fn events(&self) -> &VecDeque<AssistantMessageEvent> {
        &self.events
    }

    fn into_parts(self) -> (VecDeque<AssistantMessageEvent>, Option<AssistantMessage>) {
        (self.events, self.final_result)
    }
}

type StreamFn = dyn Fn(&Model, &Context, Option<&StreamOptions>) -> AssistantMessageEventStream
    + Send
    + Sync
    + 'static;
type SimpleStreamFn = dyn Fn(&Model, &Context, Option<&SimpleStreamOptions>) -> AssistantMessageEventStream
    + Send
    + Sync
    + 'static;

/// Provider stream functions returned by Pi API modules.
#[derive(Clone)]
pub struct ProviderStreams {
    stream: Arc<StreamFn>,
    stream_simple: Arc<SimpleStreamFn>,
}

impl ProviderStreams {
    /// Creates provider stream functions.
    #[must_use]
    pub fn new(
        stream: impl Fn(&Model, &Context, Option<&StreamOptions>) -> AssistantMessageEventStream
        + Send
        + Sync
        + 'static,
        stream_simple: impl Fn(
            &Model,
            &Context,
            Option<&SimpleStreamOptions>,
        ) -> AssistantMessageEventStream
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            stream: Arc::new(stream),
            stream_simple: Arc::new(stream_simple),
        }
    }

    /// Calls the provider's full stream function.
    #[must_use]
    pub fn stream(
        &self,
        model: &Model,
        context: &Context,
        options: Option<&StreamOptions>,
    ) -> AssistantMessageEventStream {
        (self.stream)(model, context, options)
    }

    /// Calls the provider's simple stream function.
    #[must_use]
    pub fn stream_simple(
        &self,
        model: &Model,
        context: &Context,
        options: Option<&SimpleStreamOptions>,
    ) -> AssistantMessageEventStream {
        (self.stream_simple)(model, context, options)
    }
}

impl fmt::Debug for ProviderStreams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderStreams").finish_non_exhaustive()
    }
}

/// Returns a stream while running setup and forwarding its events.
///
/// Setup failures terminate the stream with an error event matching Pi's
/// `lazyStream` behavior.
///
/// PORT PLACEHOLDER:
/// Original dependency: `JavaScript Promise scheduling and AsyncIterable streaming`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `return AssistantMessageEventStream synchronously before async setup completes, forward source events as they arrive, and convert setup failures into terminal assistant error events`.
/// Replacement decision needed before production use.
#[must_use]
pub fn lazy_stream<E>(
    model: &Model,
    setup: impl FnOnce() -> std::result::Result<AssistantMessageEventStream, E>,
) -> AssistantMessageEventStream
where
    E: ToString,
{
    let mut outer = AssistantMessageEventStream::new();

    match setup() {
        Ok(inner) => forward_stream(&mut outer, inner),
        Err(error) => {
            let message = create_setup_error_message(model, error);
            outer.push(AssistantMessageEvent::Error {
                reason: StopReason::Error,
                error: message.clone(),
            });
            outer.end(Some(message));
        }
    }

    outer
}

/// Wraps a lazily loaded provider implementation as provider streams.
///
/// The loader is called on each stream invocation. Loader failures are surfaced
/// as assistant error events in the returned stream.
///
/// PORT PLACEHOLDER:
/// Original dependency: `JavaScript dynamic import() host cache`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `load provider modules on first stream call, rely on the host import cache to deduplicate loads, and surface load failures as terminal assistant error events`.
/// Replacement decision needed before production use.
#[must_use]
pub fn lazy_api<E>(
    load: impl Fn() -> std::result::Result<ProviderStreams, E> + Send + Sync + 'static,
) -> ProviderStreams
where
    E: ToString,
{
    let load = Arc::new(load);
    let stream_load = Arc::clone(&load);
    let stream_simple_load = Arc::clone(&load);

    ProviderStreams::new(
        move |model, context, options| {
            lazy_stream(model, || {
                stream_load().map(|streams| streams.stream(model, context, options))
            })
        },
        move |model, context, options| {
            lazy_stream(model, || {
                stream_simple_load().map(|streams| streams.stream_simple(model, context, options))
            })
        },
    )
}

fn forward_stream(target: &mut AssistantMessageEventStream, source: AssistantMessageEventStream) {
    let (events, result) = source.into_parts();
    for event in events {
        target.push(event);
    }
    target.end(result);
}

fn create_setup_error_message(model: &Model, error: impl ToString) -> AssistantMessage {
    AssistantMessage {
        role: "assistant",
        content: Vec::new(),
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        usage: Usage::default(),
        stop_reason: StopReason::Error,
        error_message: Some(error.to_string()),
        timestamp: unix_timestamp_millis(),
    }
}

fn terminal_result(event: &AssistantMessageEvent) -> Option<AssistantMessage> {
    match event {
        AssistantMessageEvent::Done { message, .. } => Some(message.clone()),
        AssistantMessageEvent::Error { error, .. } => Some(error.clone()),
        AssistantMessageEvent::Opaque(_) => None,
    }
}

fn unix_timestamp_millis() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());
    u64::try_from(millis).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Model {
        Model::new("model-id", "api", "provider")
    }

    #[test]
    fn setup_failure_terminates_stream_with_error_message() {
        let stream = lazy_stream(&model(), || {
            Err::<AssistantMessageEventStream, _>("load failed")
        });

        assert!(stream.is_done());
        assert_eq!(stream.events().len(), 1);
        let Some(AssistantMessageEvent::Error { error, .. }) = stream.events().front() else {
            panic!("expected error event");
        };
        assert_eq!(error.api, "api");
        assert_eq!(error.provider, "provider");
        assert_eq!(error.model, "model-id");
        assert_eq!(error.error_message.as_deref(), Some("load failed"));
        assert_eq!(stream.result(), Some(error));
    }

    #[test]
    fn lazy_api_forwards_loaded_provider_events() {
        let provider = lazy_api(|| {
            Ok::<_, String>(ProviderStreams::new(
                |_model, _context, _options| {
                    let mut stream = AssistantMessageEventStream::new();
                    stream.push(AssistantMessageEvent::Opaque("event".to_owned()));
                    stream
                },
                |_model, _context, _options| AssistantMessageEventStream::new(),
            ))
        });

        let stream = provider.stream(&model(), &Context, None);

        assert_eq!(stream.events().len(), 1);
        assert_eq!(
            stream.events().front(),
            Some(&AssistantMessageEvent::Opaque("event".to_owned()))
        );
        assert!(stream.is_done());
    }
}
