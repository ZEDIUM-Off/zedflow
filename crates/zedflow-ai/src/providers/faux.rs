//! Faux provider helpers ported from Pi's `packages/ai/src/providers/faux.ts`.
//!
//! This is intentionally small: the Rust provider/event contracts are still placeholders, so the
//! port preserves the test-facing queue, defaults, model handles, and terminal stream behavior.
//! Token pacing, abort signals, prompt-cache accounting, and per-block delta events need the final
//! Rust `types.ts` equivalent before they can be faithfully implemented.

use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::api::lazy::{
    AssistantContent, AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream,
    Context, Model as StreamModel, ProviderStreams, SimpleStreamOptions, StopReason, StreamOptions,
    Usage, UsageCost,
};
use crate::models::{self as registry_models, CreateProviderOptions, Provider, create_provider};

/// Default faux API prefix used by Pi.
pub const DEFAULT_API: &str = "faux";
/// Default faux provider id used by Pi.
pub const DEFAULT_PROVIDER: &str = "faux";
/// Default faux model id used by Pi.
pub const DEFAULT_MODEL_ID: &str = "faux-1";
/// Default faux model display name used by Pi.
pub const DEFAULT_MODEL_NAME: &str = "Faux Model";
/// Default faux base URL used by Pi.
pub const DEFAULT_BASE_URL: &str = "http://localhost:0";
/// Default minimum generated token chunk size used by Pi.
pub const DEFAULT_MIN_TOKEN_SIZE: usize = 3;
/// Default maximum generated token chunk size used by Pi.
pub const DEFAULT_MAX_TOKEN_SIZE: usize = 5;

/// Faux model definition accepted by [`create_faux_core`].
#[derive(Debug, Clone, PartialEq)]
pub struct FauxModelDefinition {
    /// Model id.
    pub id: String,
    /// Optional model display name.
    pub name: Option<String>,
    /// Whether the model supports reasoning content.
    pub reasoning: bool,
    /// Accepted input modalities.
    pub input: Vec<FauxInputKind>,
    /// Token costs.
    pub cost: FauxCost,
    /// Context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
}

impl Default for FauxModelDefinition {
    fn default() -> Self {
        Self {
            id: DEFAULT_MODEL_ID.into(),
            name: Some(DEFAULT_MODEL_NAME.into()),
            reasoning: false,
            input: vec![FauxInputKind::Text, FauxInputKind::Image],
            cost: FauxCost::default(),
            context_window: 128_000,
            max_tokens: 16_384,
        }
    }
}

/// Input modality supported by a faux model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FauxInputKind {
    /// Text input.
    Text,
    /// Image input.
    Image,
}

/// Faux model token cost counters.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FauxCost {
    /// Input token cost.
    pub input: f64,
    /// Output token cost.
    pub output: f64,
    /// Prompt-cache read token cost.
    pub cache_read: f64,
    /// Prompt-cache write token cost.
    pub cache_write: f64,
}

/// Options accepted by [`create_faux_core`] and [`faux_provider`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RegisterFauxProviderOptions {
    /// API id. When omitted, Pi generates a unique id with the `faux` prefix.
    pub api: Option<String>,
    /// Provider id.
    pub provider: Option<String>,
    /// Faux model definitions.
    pub models: Vec<FauxModelDefinition>,
    /// Optional token pacing. Stored for parity; not used until async event streams exist.
    pub tokens_per_second: Option<f64>,
    /// Optional token-size bounds. Stored for parity; not used until delta streams exist.
    pub token_size: FauxTokenSize,
}

/// Token chunk size bounds for faux streaming.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FauxTokenSize {
    /// Minimum token chunk size.
    pub min: Option<usize>,
    /// Maximum token chunk size.
    pub max: Option<usize>,
}

/// Shared faux provider state.
#[derive(Debug, Clone, Default)]
pub struct FauxProviderState {
    call_count: Arc<AtomicU64>,
}

impl FauxProviderState {
    /// Number of stream calls handled by this faux provider.
    #[must_use]
    pub fn call_count(&self) -> u64 {
        self.call_count.load(Ordering::SeqCst)
    }

    fn increment(&self) {
        self.call_count.fetch_add(1, Ordering::SeqCst);
    }
}

/// A queued faux response.
#[derive(Clone)]
pub enum FauxResponseStep {
    /// Static assistant message response.
    Message(AssistantMessage),
    /// Dynamic assistant message response.
    Factory(Arc<FauxResponseFactory>),
}

impl fmt::Debug for FauxResponseStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(message) => f.debug_tuple("Message").field(message).finish(),
            Self::Factory(_) => f.write_str("Factory(<callback>)"),
        }
    }
}

/// Dynamic faux response callback.
pub type FauxResponseFactory = dyn Fn(&Context, Option<&StreamOptions>, &FauxProviderState, &StreamModel) -> AssistantMessage
    + Send
    + Sync
    + 'static;

/// Core faux provider returned by [`create_faux_core`].
#[derive(Clone)]
pub struct FauxCore {
    /// API id used by generated faux models.
    pub api: String,
    /// Provider id used by generated faux models.
    pub provider: String,
    /// Faux stream models.
    pub models: Vec<StreamModel>,
    /// Stream functions backed by the pending response queue.
    pub streams: ProviderStreams,
    /// Shared call count state.
    pub state: FauxProviderState,
    pending_responses: Arc<Mutex<VecDeque<FauxResponseStep>>>,
}

/// Faux provider handle matching Pi's test helper shape.
#[derive(Clone)]
pub struct FauxProviderHandle {
    /// Provider registered in the current minimal Rust model registry.
    pub provider: Provider,
    /// API id used by generated faux models.
    pub api: String,
    /// Faux stream models.
    pub models: Vec<StreamModel>,
    /// Shared call count state.
    pub state: FauxProviderState,
    core: FauxCore,
}

/// Creates a text content block.
#[must_use]
pub fn faux_text(text: impl Into<String>) -> AssistantContent {
    AssistantContent::Opaque(text.into())
}

/// Creates a thinking content block.
#[must_use]
pub fn faux_thinking(thinking: impl Into<String>) -> AssistantContent {
    AssistantContent::Opaque(thinking.into())
}

/// Creates a tool-call content block.
#[must_use]
pub fn faux_tool_call(name: impl AsRef<str>, arguments: Value) -> AssistantContent {
    AssistantContent::Opaque(format!("{}:{}", name.as_ref(), arguments))
}

/// Creates a normal faux assistant message from plain text.
#[must_use]
pub fn faux_assistant_message(text: impl Into<String>) -> AssistantMessage {
    faux_assistant_content_message(vec![faux_text(text)])
}

/// Creates a normal faux assistant message from content blocks.
#[must_use]
pub fn faux_assistant_content_message(content: Vec<AssistantContent>) -> AssistantMessage {
    AssistantMessage {
        role: "assistant",
        content,
        api: DEFAULT_API.into(),
        provider: DEFAULT_PROVIDER.into(),
        model: DEFAULT_MODEL_ID.into(),
        usage: default_usage(),
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp: now_millis(),
    }
}

/// Creates the queue-backed faux provider core.
#[must_use]
pub fn create_faux_core(options: RegisterFauxProviderOptions) -> FauxCore {
    let api = options.api.unwrap_or_else(|| random_id(DEFAULT_API));
    let provider = options.provider.unwrap_or_else(|| DEFAULT_PROVIDER.into());
    let pending_responses = Arc::new(Mutex::new(VecDeque::new()));
    let state = FauxProviderState::default();
    let models = model_definitions(&options.models)
        .into_iter()
        .map(|definition| StreamModel::new(definition.id, api.clone(), provider.clone()))
        .collect::<Vec<_>>();

    let stream_pending = Arc::clone(&pending_responses);
    let stream_state = state.clone();
    let stream_api = api.clone();
    let stream_provider = provider.clone();
    let streams = ProviderStreams::new(
        move |request_model, context, stream_options| {
            stream_next_response(
                &stream_pending,
                &stream_state,
                &stream_api,
                &stream_provider,
                request_model,
                context,
                stream_options,
            )
        },
        {
            let pending_responses = Arc::clone(&pending_responses);
            let state = state.clone();
            let api = api.clone();
            let provider = provider.clone();
            move |request_model, context, _stream_options: Option<&SimpleStreamOptions>| {
                stream_next_response(
                    &pending_responses,
                    &state,
                    &api,
                    &provider,
                    request_model,
                    context,
                    None,
                )
            }
        },
    );

    FauxCore {
        api,
        provider,
        models,
        streams,
        state,
        pending_responses,
    }
}

impl FauxCore {
    /// Returns the default model or the requested model id.
    #[must_use]
    pub fn get_model(&self, model_id: Option<&str>) -> Option<StreamModel> {
        match model_id {
            Some(model_id) => self
                .models
                .iter()
                .find(|model| model.id == model_id)
                .cloned(),
            None => self.models.first().cloned(),
        }
    }

    /// Replaces pending faux responses.
    pub fn set_responses(&self, responses: Vec<FauxResponseStep>) {
        *self
            .pending_responses
            .lock()
            .expect("faux response queue lock poisoned") = responses.into();
    }

    /// Appends pending faux responses.
    pub fn append_responses(&self, responses: Vec<FauxResponseStep>) {
        self.pending_responses
            .lock()
            .expect("faux response queue lock poisoned")
            .extend(responses);
    }

    /// Returns pending faux response count.
    #[must_use]
    pub fn get_pending_response_count(&self) -> usize {
        self.pending_responses
            .lock()
            .expect("faux response queue lock poisoned")
            .len()
    }
}

impl FauxProviderHandle {
    /// Returns the default model or the requested model id.
    #[must_use]
    pub fn get_model(&self, model_id: Option<&str>) -> Option<StreamModel> {
        self.core.get_model(model_id)
    }

    /// Replaces pending faux responses.
    pub fn set_responses(&self, responses: Vec<FauxResponseStep>) {
        self.core.set_responses(responses);
    }

    /// Appends pending faux responses.
    pub fn append_responses(&self, responses: Vec<FauxResponseStep>) {
        self.core.append_responses(responses);
    }

    /// Returns pending faux response count.
    #[must_use]
    pub fn get_pending_response_count(&self) -> usize {
        self.core.get_pending_response_count()
    }
}

/// Creates a faux provider handle for tests using explicit model collections.
#[must_use]
pub fn faux_provider(options: RegisterFauxProviderOptions) -> FauxProviderHandle {
    let core = create_faux_core(options);
    let provider_id = core.provider.clone();
    let api = core.api.clone();
    let registry_models = core
        .models
        .iter()
        .map(|model| registry_models::Model {
            provider: provider_id.clone(),
            id: model.id.clone(),
            api: api.clone(),
        })
        .collect();
    let streams = core.streams.clone();
    let provider = create_provider(CreateProviderOptions {
        id: provider_id,
        name: Some("Faux".into()),
        models: registry_models,
        refresh_models: None,
        stream: Arc::new(move |model, _options| {
            let stream_model =
                StreamModel::new(model.id.clone(), model.api.clone(), model.provider.clone());
            let stream = streams.stream(&stream_model, &Context, None);
            let text = stream.result().map_or_else(String::new, message_to_text);
            vec![registry_models::AssistantMessage { text }]
        }),
    });

    FauxProviderHandle {
        provider,
        api: core.api.clone(),
        models: core.models.clone(),
        state: core.state.clone(),
        core,
    }
}

fn stream_next_response(
    pending_responses: &Arc<Mutex<VecDeque<FauxResponseStep>>>,
    state: &FauxProviderState,
    api: &str,
    provider: &str,
    request_model: &StreamModel,
    context: &Context,
    stream_options: Option<&StreamOptions>,
) -> AssistantMessageEventStream {
    let step = pending_responses
        .lock()
        .expect("faux response queue lock poisoned")
        .pop_front();
    state.increment();

    let mut stream = AssistantMessageEventStream::new();
    let message = match step {
        Some(FauxResponseStep::Message(message)) => {
            clone_message(&message, api, provider, &request_model.id)
        }
        Some(FauxResponseStep::Factory(factory)) => clone_message(
            &factory(context, stream_options, state, request_model),
            api,
            provider,
            &request_model.id,
        ),
        None => create_error_message(
            "No more faux responses queued",
            api,
            provider,
            &request_model.id,
        ),
    };

    if matches!(message.stop_reason, StopReason::Error | StopReason::Aborted) {
        stream.push(AssistantMessageEvent::Error {
            reason: message.stop_reason,
            error: message,
        });
    } else {
        stream.push(AssistantMessageEvent::Done {
            reason: message.stop_reason,
            message,
        });
    }
    stream
}

fn model_definitions(definitions: &[FauxModelDefinition]) -> Vec<FauxModelDefinition> {
    if definitions.is_empty() {
        vec![FauxModelDefinition::default()]
    } else {
        definitions.to_vec()
    }
}

fn clone_message(
    message: &AssistantMessage,
    api: &str,
    provider: &str,
    model_id: &str,
) -> AssistantMessage {
    let mut cloned = message.clone();
    cloned.api = api.into();
    cloned.provider = provider.into();
    cloned.model = model_id.into();
    if cloned.timestamp == 0 {
        cloned.timestamp = now_millis();
    }
    cloned
}

fn create_error_message(
    message: &str,
    api: &str,
    provider: &str,
    model_id: &str,
) -> AssistantMessage {
    AssistantMessage {
        role: "assistant",
        content: Vec::new(),
        api: api.into(),
        provider: provider.into(),
        model: model_id.into(),
        usage: default_usage(),
        stop_reason: StopReason::Error,
        error_message: Some(message.into()),
        timestamp: now_millis(),
    }
}

fn default_usage() -> Usage {
    Usage {
        input: 0,
        output: 0,
        cache_read: 0,
        cache_write: 0,
        total_tokens: 0,
        cost: UsageCost::default(),
    }
}

fn message_to_text(message: &AssistantMessage) -> String {
    message
        .content
        .iter()
        .map(|block| match block {
            AssistantContent::Opaque(text) => text.as_str(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn random_id(prefix: &str) -> String {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    format!(
        "{prefix}:{}:{}",
        now_millis(),
        NEXT_ID.fetch_add(1, Ordering::SeqCst)
    )
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            duration.as_millis().try_into().unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queues_and_streams_faux_responses() {
        let faux = create_faux_core(RegisterFauxProviderOptions::default());
        let model = faux.get_model(None).expect("default model");
        faux.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
            "hi",
        ))]);

        let stream = faux.streams.stream(&model, &Context, None);

        assert_eq!(faux.state.call_count(), 1);
        assert_eq!(faux.get_pending_response_count(), 0);
        assert_eq!(stream.result().map(message_to_text).as_deref(), Some("hi"));
    }

    #[test]
    fn reports_error_when_queue_is_empty() {
        let faux = create_faux_core(RegisterFauxProviderOptions::default());
        let model = faux.get_model(None).expect("default model");

        let stream = faux.streams.stream(&model, &Context, None);

        assert_eq!(faux.state.call_count(), 1);
        assert_eq!(
            stream
                .result()
                .and_then(|message| message.error_message.as_deref()),
            Some("No more faux responses queued"),
        );
    }
}
