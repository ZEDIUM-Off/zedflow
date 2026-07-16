//! Faux provider helpers ported from Pi's `packages/ai/src/providers/faux.ts`.
//!
//! The core queue remains synchronous, but emitted provider events use the public typed stream
//! contract from `types.rs`.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::FutureExt;
use futures::future::BoxFuture;
use serde_json::Value;

use crate::models::{CreateProviderOptions, Provider, ProviderApi, ProviderAuth, create_provider};
use crate::types::{
    AssistantContentBlock, AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream,
    AssistantMessageRole, Context, DoneStopReason, ErrorStopReason, Model, ModelCost, ModelInput,
    ProviderStreams, SimpleStreamOptions, StopReason, StreamOptions, TextContent, TextContentType,
    ThinkingContent, ThinkingContentType, ToolCall, ToolCallType, Usage, UsageCost,
};

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
    /// Optional output pacing in estimated tokens per second.
    pub tokens_per_second: Option<f64>,
    /// Optional token-size bounds for streamed deltas.
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
#[allow(
    clippy::large_enum_variant,
    reason = "preserve the public faux response API"
)]
#[derive(Clone)]
pub enum FauxResponseStep {
    /// Static assistant message response.
    Message(AssistantMessage),
    /// Synchronous dynamic assistant message response.
    Factory(Arc<FauxResponseFactory>),
    /// Asynchronous, fallible dynamic assistant message response.
    AsyncFactory(Arc<AsyncFauxResponseFactory>),
}

impl fmt::Debug for FauxResponseStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(message) => f.debug_tuple("Message").field(message).finish(),
            Self::Factory(_) => f.write_str("Factory(<callback>)"),
            Self::AsyncFactory(_) => f.write_str("AsyncFactory(<callback>)"),
        }
    }
}

/// Dynamic faux response callback.
pub type FauxResponseFactory = dyn Fn(&Context, Option<&StreamOptions>, &FauxProviderState, &Model) -> AssistantMessage
    + Send
    + Sync
    + 'static;

/// Error returned by an asynchronous faux response factory.
pub type FauxFactoryError = Box<dyn std::error::Error + Send + Sync>;

/// Asynchronous, fallible dynamic faux response callback using the public typed request contract.
pub type AsyncFauxResponseFactory = dyn for<'a> Fn(
        &'a Context,
        Option<&'a StreamOptions>,
        &'a FauxProviderState,
        &'a Model,
    ) -> BoxFuture<'a, Result<AssistantMessage, FauxFactoryError>>
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
    pub models: Vec<Model>,
    /// Stream functions backed by the pending response queue.
    pub streams: ProviderStreams,
    /// Shared call count state.
    pub state: FauxProviderState,
    pending_responses: Arc<Mutex<VecDeque<FauxResponseStep>>>,
    prompt_cache: Arc<Mutex<HashMap<String, String>>>,
    token_size: FauxTokenSize,
    tokens_per_second: Option<f64>,
}

/// Faux provider handle matching Pi's test helper shape.
#[derive(Clone)]
pub struct FauxProviderHandle {
    /// Provider registered in the current minimal Rust model registry.
    pub provider: Provider,
    /// API id used by generated faux models.
    pub api: String,
    /// Faux stream models.
    pub models: Vec<Model>,
    /// Shared call count state.
    pub state: FauxProviderState,
    core: FauxCore,
}

/// Creates a text content block.
#[must_use]
pub fn faux_text(text: impl Into<String>) -> AssistantContentBlock {
    AssistantContentBlock::Text(TextContent {
        content_type: TextContentType::Text,
        text: text.into(),
        text_signature: None,
    })
}

/// Creates a thinking content block.
#[must_use]
pub fn faux_thinking(thinking: impl Into<String>) -> AssistantContentBlock {
    AssistantContentBlock::Thinking(ThinkingContent {
        content_type: ThinkingContentType::Thinking,
        thinking: thinking.into(),
        thinking_signature: None,
        redacted: None,
    })
}

/// Creates a tool-call content block.
#[must_use]
pub fn faux_tool_call(name: impl AsRef<str>, arguments: Value) -> AssistantContentBlock {
    AssistantContentBlock::ToolCall(ToolCall {
        content_type: ToolCallType::ToolCall,
        id: random_id("tool"),
        name: name.as_ref().to_owned(),
        arguments: arguments
            .as_object()
            .map(|value| value.clone().into_iter().collect())
            .unwrap_or_default(),
        thought_signature: None,
    })
}

/// Creates a normal faux assistant message from plain text.
#[must_use]
pub fn faux_assistant_message(text: impl Into<String>) -> AssistantMessage {
    faux_assistant_content_message(vec![faux_text(text)])
}

/// Creates a normal faux assistant message from content blocks.
#[must_use]
pub fn faux_assistant_content_message(content: Vec<AssistantContentBlock>) -> AssistantMessage {
    AssistantMessage {
        role: AssistantMessageRole::Assistant,
        content,
        api: DEFAULT_API.into(),
        provider: DEFAULT_PROVIDER.into(),
        model: DEFAULT_MODEL_ID.into(),
        response_model: None,
        response_id: None,
        diagnostics: None,
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
    let prompt_cache = Arc::new(Mutex::new(HashMap::new()));
    let state = FauxProviderState::default();
    let token_size = normalize_token_size(options.token_size);
    let tokens_per_second = options.tokens_per_second;
    let models = model_definitions(&options.models)
        .into_iter()
        .map(|definition| Model {
            id: definition.id.clone(),
            name: definition.name.unwrap_or(definition.id),
            api: api.clone(),
            provider: provider.clone(),
            base_url: DEFAULT_BASE_URL.into(),
            reasoning: definition.reasoning,
            input: definition
                .input
                .into_iter()
                .map(|input| match input {
                    FauxInputKind::Text => ModelInput::Text,
                    FauxInputKind::Image => ModelInput::Image,
                })
                .collect(),
            cost: ModelCost {
                input: definition.cost.input,
                output: definition.cost.output,
                cache_read: definition.cost.cache_read,
                cache_write: definition.cost.cache_write,
            },
            context_window: u64::from(definition.context_window),
            max_tokens: u64::from(definition.max_tokens),
            ..Model::default()
        })
        .collect::<Vec<_>>();

    let streams = ProviderStreams {
        stream: Arc::new({
            let pending_responses = Arc::clone(&pending_responses);
            let state = state.clone();
            let api = api.clone();
            let provider = provider.clone();
            let prompt_cache = Arc::clone(&prompt_cache);
            move |request_model, context, stream_options| {
                stream_next_response(
                    &pending_responses,
                    &state,
                    &api,
                    &provider,
                    request_model,
                    context,
                    stream_options,
                    &prompt_cache,
                    token_size,
                    tokens_per_second,
                )
            }
        }),
        stream_simple: Arc::new({
            let pending_responses = Arc::clone(&pending_responses);
            let state = state.clone();
            let api = api.clone();
            let provider = provider.clone();
            let prompt_cache = Arc::clone(&prompt_cache);
            move |request_model, context, stream_options: Option<&SimpleStreamOptions>| {
                stream_next_response(
                    &pending_responses,
                    &state,
                    &api,
                    &provider,
                    request_model,
                    context,
                    stream_options.map(|options| &options.stream),
                    &prompt_cache,
                    token_size,
                    tokens_per_second,
                )
            }
        }),
    };

    FauxCore {
        api,
        provider,
        models,
        streams,
        state,
        pending_responses,
        prompt_cache,
        token_size,
        tokens_per_second,
    }
}

impl FauxCore {
    /// Returns the default model or the requested model id.
    #[must_use]
    pub fn get_model(&self, model_id: Option<&str>) -> Option<Model> {
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

    /// Streams with typed public events.
    #[must_use]
    pub fn stream_typed(
        &self,
        request_model: &Model,
        context: &Context,
        options: Option<&StreamOptions>,
    ) -> AssistantMessageEventStream {
        stream_next_response(
            &self.pending_responses,
            &self.state,
            &self.api,
            &self.provider,
            request_model,
            context,
            options,
            &self.prompt_cache,
            self.token_size,
            self.tokens_per_second,
        )
    }

    /// Streams with compat options such as session/cache retention.
    #[must_use]
    pub fn stream_compat(
        &self,
        request_model: &Model,
        context: &Context,
        options: Option<&StreamOptions>,
    ) -> AssistantMessageEventStream {
        self.stream_typed(request_model, context, options)
    }
}

impl FauxProviderHandle {
    /// Returns the default model or the requested model id.
    #[must_use]
    pub fn get_model(&self, model_id: Option<&str>) -> Option<Model> {
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
    let registry_models = core.models.clone();
    let stream_core = core.clone();
    let stream = Arc::new(
        move |model: &Model, context: &Context, options: Option<&StreamOptions>| {
            stream_core.stream_typed(model, context, options)
        },
    );
    let simple_core = core.clone();
    let provider = create_provider(CreateProviderOptions {
        id: provider_id,
        name: Some("Faux".into()),
        base_url: Some(DEFAULT_BASE_URL.into()),
        headers: None,
        auth: ProviderAuth::default(),
        models: registry_models,
        refresh_models: None,
        api: ProviderApi::Single(ProviderStreams {
            stream,
            stream_simple: Arc::new(
                move |model: &Model, context: &Context, options: Option<&SimpleStreamOptions>| {
                    simple_core.stream_typed(model, context, options.map(|options| &options.stream))
                },
            ),
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

#[allow(
    clippy::too_many_arguments,
    reason = "faux stream controls remain independently configurable"
)]
fn stream_next_response(
    pending_responses: &Arc<Mutex<VecDeque<FauxResponseStep>>>,
    state: &FauxProviderState,
    api: &str,
    provider: &str,
    request_model: &Model,
    context: &Context,
    options: Option<&StreamOptions>,
    prompt_cache: &Arc<Mutex<HashMap<String, String>>>,
    token_size: FauxTokenSize,
    tokens_per_second: Option<f64>,
) -> AssistantMessageEventStream {
    let step = pending_responses
        .lock()
        .expect("faux response queue lock poisoned")
        .pop_front();
    state.increment();

    let stream = AssistantMessageEventStream::new();
    let worker_stream = stream.clone();
    let state = state.clone();
    let api = api.to_owned();
    let provider = provider.to_owned();
    let request_model = request_model.clone();
    let context = context.clone();
    let options = options.cloned();
    let prompt_cache = Arc::clone(prompt_cache);

    crate::utils::runtime::spawn_worker(async move {
        let resolved = match step {
            Some(FauxResponseStep::Message(message)) => Ok(Some(message)),
            Some(FauxResponseStep::Factory(factory)) => {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    factory(&context, options.as_ref(), &state, &request_model)
                }))
                .map(Some)
                .map_err(|error| panic_message(error).to_owned())
            }
            Some(FauxResponseStep::AsyncFactory(factory)) => {
                let future = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    factory(&context, options.as_ref(), &state, &request_model)
                }));
                match future {
                    Ok(future) => match std::panic::AssertUnwindSafe(future).catch_unwind().await {
                        Ok(Ok(message)) => Ok(Some(message)),
                        Ok(Err(error)) => Err(error.to_string()),
                        Err(error) => Err(panic_message(error).to_owned()),
                    },
                    Err(error) => Err(panic_message(error).to_owned()),
                }
            }
            None => Ok(None),
        };

        let (lazy_message, exhausted) = match resolved {
            Ok(Some(message)) => (
                clone_message(&message, &api, &provider, &request_model.id),
                false,
            ),
            Ok(None) => (
                create_error_message(
                    "No more faux responses queued",
                    &api,
                    &provider,
                    &request_model.id,
                ),
                true,
            ),
            Err(error) => {
                worker_stream.push(AssistantMessageEvent::Error {
                    reason: ErrorStopReason::Error,
                    error: create_error_message(&error, &api, &provider, &request_model.id),
                });
                return;
            }
        };
        let mut message = lazy_message;
        apply_usage(
            &mut message,
            &context,
            options.as_ref(),
            &request_model.cost,
            &prompt_cache,
        );
        if exhausted {
            worker_stream.push(AssistantMessageEvent::Error {
                reason: ErrorStopReason::Error,
                error: message,
            });
            return;
        }
        stream_with_deltas(
            worker_stream,
            message,
            token_size,
            tokens_per_second,
            options.and_then(|options| options.signal),
        )
        .await;
    });
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
        role: AssistantMessageRole::Assistant,
        content: Vec::new(),
        api: api.into(),
        provider: provider.into(),
        model: model_id.into(),
        response_model: None,
        response_id: None,
        diagnostics: None,
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
        cache_write_1h: None,
        reasoning: None,
        total_tokens: 0,
        cost: UsageCost::default(),
    }
}

fn apply_usage(
    message: &mut AssistantMessage,
    context: &Context,
    options: Option<&StreamOptions>,
    model_cost: &ModelCost,
    prompt_cache: &Arc<Mutex<HashMap<String, String>>>,
) {
    let prompt_text = serialize_context(context);
    let prompt_tokens = estimate_tokens(&prompt_text);
    let output_tokens = estimate_tokens(&typed_message_to_text(message));
    let mut input = prompt_tokens;
    let mut cache_read = 0;
    let mut cache_write = 0;

    if let Some(session_id) = options.and_then(|options| options.session_id.as_deref())
        && !matches!(
            options.and_then(|options| options.cache_retention),
            Some(crate::types::CacheRetention::None)
        )
    {
        let mut cache = prompt_cache
            .lock()
            .expect("faux prompt cache lock poisoned");
        if let Some(previous_prompt) = cache.get(session_id) {
            let cached_chars = common_prefix_len(previous_prompt, &prompt_text);
            cache_read = estimate_tokens(&previous_prompt[..cached_chars]);
            cache_write = estimate_tokens(&prompt_text[cached_chars..]);
            input = prompt_tokens.saturating_sub(cache_read);
        } else {
            cache_write = prompt_tokens;
        }
        cache.insert(session_id.to_owned(), prompt_text);
    }

    message.usage.input = input;
    message.usage.output = output_tokens;
    message.usage.cache_read = cache_read;
    message.usage.cache_write = cache_write;
    message.usage.cache_write_1h = None;
    message.usage.reasoning = None;
    message.usage.total_tokens = input
        .saturating_add(output_tokens)
        .saturating_add(cache_read)
        .saturating_add(cache_write);
    message.usage.cost = calculate_cost(model_cost, &message.usage);
}

fn calculate_cost(model_cost: &ModelCost, usage: &Usage) -> UsageCost {
    let long_write = usage.cache_write_1h.unwrap_or(0) as f64;
    let short_write = usage
        .cache_write
        .saturating_sub(usage.cache_write_1h.unwrap_or(0)) as f64;
    let input = model_cost.input * usage.input as f64 / 1_000_000.0;
    let output = model_cost.output * usage.output as f64 / 1_000_000.0;
    let cache_read = model_cost.cache_read * usage.cache_read as f64 / 1_000_000.0;
    let cache_write =
        (model_cost.cache_write * short_write + model_cost.input * 2.0 * long_write) / 1_000_000.0;
    UsageCost {
        input,
        output,
        cache_read,
        cache_write,
        total: input + output + cache_read + cache_write,
    }
}

async fn stream_with_deltas(
    stream: AssistantMessageEventStream,
    message: AssistantMessage,
    token_size: FauxTokenSize,
    tokens_per_second: Option<f64>,
    signal: Option<crate::types::AbortSignal>,
) {
    let mut partial = message.clone();
    partial.content.clear();

    if abort_stream_if_requested(&stream, &signal, &partial) {
        return;
    }
    stream.push(AssistantMessageEvent::Start {
        partial: partial.clone(),
    });

    for (index, block) in message.content.iter().enumerate() {
        if abort_stream_if_requested(&stream, &signal, &partial) {
            return;
        }
        match block {
            AssistantContentBlock::Text(text) => {
                partial
                    .content
                    .push(AssistantContentBlock::Text(TextContent {
                        content_type: TextContentType::Text,
                        text: String::new(),
                        text_signature: text.text_signature.clone(),
                    }));
                stream.push(AssistantMessageEvent::TextStart {
                    content_index: index,
                    partial: partial.clone(),
                });
                for chunk in chunks(&text.text, token_size) {
                    schedule_chunk(chunk, tokens_per_second).await;
                    if abort_stream_if_requested(&stream, &signal, &partial) {
                        return;
                    }
                    if let AssistantContentBlock::Text(partial_text) = &mut partial.content[index] {
                        partial_text.text.push_str(chunk);
                    }
                    stream.push(AssistantMessageEvent::TextDelta {
                        content_index: index,
                        delta: chunk.to_owned(),
                        partial: partial.clone(),
                    });
                }
                stream.push(AssistantMessageEvent::TextEnd {
                    content_index: index,
                    content: text.text.clone(),
                    partial: partial.clone(),
                });
            }
            AssistantContentBlock::Thinking(thinking) => {
                partial
                    .content
                    .push(AssistantContentBlock::Thinking(ThinkingContent {
                        content_type: ThinkingContentType::Thinking,
                        thinking: String::new(),
                        thinking_signature: thinking.thinking_signature.clone(),
                        redacted: thinking.redacted,
                    }));
                stream.push(AssistantMessageEvent::ThinkingStart {
                    content_index: index,
                    partial: partial.clone(),
                });
                for chunk in chunks(&thinking.thinking, token_size) {
                    schedule_chunk(chunk, tokens_per_second).await;
                    if abort_stream_if_requested(&stream, &signal, &partial) {
                        return;
                    }
                    if let AssistantContentBlock::Thinking(partial_thinking) =
                        &mut partial.content[index]
                    {
                        partial_thinking.thinking.push_str(chunk);
                    }
                    stream.push(AssistantMessageEvent::ThinkingDelta {
                        content_index: index,
                        delta: chunk.to_owned(),
                        partial: partial.clone(),
                    });
                }
                stream.push(AssistantMessageEvent::ThinkingEnd {
                    content_index: index,
                    content: thinking.thinking.clone(),
                    partial: partial.clone(),
                });
            }
            AssistantContentBlock::ToolCall(tool_call) => {
                partial
                    .content
                    .push(AssistantContentBlock::ToolCall(ToolCall {
                        content_type: ToolCallType::ToolCall,
                        id: tool_call.id.clone(),
                        name: tool_call.name.clone(),
                        arguments: HashMap::new(),
                        thought_signature: tool_call.thought_signature.clone(),
                    }));
                stream.push(AssistantMessageEvent::ToolcallStart {
                    content_index: index,
                    partial: partial.clone(),
                });
                let arguments =
                    serde_json::to_string(&tool_call.arguments).unwrap_or_else(|_| "{}".to_owned());
                for chunk in chunks(&arguments, token_size) {
                    schedule_chunk(chunk, tokens_per_second).await;
                    if abort_stream_if_requested(&stream, &signal, &partial) {
                        return;
                    }
                    stream.push(AssistantMessageEvent::ToolcallDelta {
                        content_index: index,
                        delta: chunk.to_owned(),
                        partial: partial.clone(),
                    });
                }
                if let AssistantContentBlock::ToolCall(partial_tool_call) =
                    &mut partial.content[index]
                {
                    partial_tool_call.arguments = tool_call.arguments.clone();
                }
                stream.push(AssistantMessageEvent::ToolcallEnd {
                    content_index: index,
                    tool_call: tool_call.clone(),
                    partial: partial.clone(),
                });
            }
        }
    }

    if matches!(message.stop_reason, StopReason::Error | StopReason::Aborted) {
        stream.push(AssistantMessageEvent::Error {
            reason: error_stop_reason(message.stop_reason),
            error: message,
        });
    } else {
        stream.push(AssistantMessageEvent::Done {
            reason: done_stop_reason(message.stop_reason),
            message,
        });
    }
}

fn abort_stream_if_requested(
    stream: &AssistantMessageEventStream,
    signal: &Option<crate::types::AbortSignal>,
    partial: &AssistantMessage,
) -> bool {
    if signal
        .as_ref()
        .is_some_and(crate::types::AbortSignal::aborted)
    {
        stream.push(AssistantMessageEvent::Error {
            reason: ErrorStopReason::Aborted,
            error: aborted_message(partial.clone()),
        });
        true
    } else {
        false
    }
}

async fn schedule_chunk(chunk: &str, tokens_per_second: Option<f64>) {
    let Some(tokens_per_second) = tokens_per_second.filter(|rate| *rate > 0.0) else {
        tokio::task::yield_now().await;
        return;
    };
    tokio::time::sleep(Duration::from_secs_f64(
        estimate_tokens(chunk) as f64 / tokens_per_second,
    ))
    .await;
}

fn normalize_token_size(token_size: FauxTokenSize) -> FauxTokenSize {
    let min = token_size
        .min
        .unwrap_or(DEFAULT_MIN_TOKEN_SIZE)
        .min(token_size.max.unwrap_or(DEFAULT_MAX_TOKEN_SIZE))
        .max(1);
    let max = token_size.max.unwrap_or(DEFAULT_MAX_TOKEN_SIZE).max(min);
    FauxTokenSize {
        min: Some(min),
        max: Some(max),
    }
}

fn chunks(text: &str, token_size: FauxTokenSize) -> Vec<&str> {
    let token_size = normalize_token_size(token_size);
    let min = token_size.min.expect("normalized minimum token size");
    let max = token_size.max.expect("normalized maximum token size");
    if text.is_empty() {
        return vec![""];
    }

    let mut out = Vec::new();
    let mut start = 0;
    let mut use_max = false;
    while start < text.len() {
        let token_size = if use_max { max } else { min };
        let char_count = token_size.saturating_mul(4).max(1);
        let end = text[start..]
            .char_indices()
            .nth(char_count)
            .map_or(text.len(), |(index, _)| start + index);
        out.push(&text[start..end]);
        start = end;
        use_max = !use_max;
    }
    out
}

fn estimate_tokens(text: &str) -> u64 {
    u64::try_from(text.len().div_ceil(4)).unwrap_or(u64::MAX)
}

#[cfg(test)]
fn message_to_text(message: &AssistantMessage) -> String {
    message
        .content
        .iter()
        .map(|block| match block {
            AssistantContentBlock::Text(text) => text.text.clone(),
            AssistantContentBlock::Thinking(thinking) => thinking.thinking.clone(),
            AssistantContentBlock::ToolCall(tool) => format!(
                "{}:{}",
                tool.name,
                serde_json::to_string(&tool.arguments).unwrap_or_default()
            ),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn done_stop_reason(reason: StopReason) -> DoneStopReason {
    match reason {
        StopReason::Stop => DoneStopReason::Stop,
        StopReason::Length => DoneStopReason::Length,
        StopReason::ToolUse => DoneStopReason::ToolUse,
        StopReason::Error | StopReason::Aborted => DoneStopReason::Stop,
    }
}

fn error_stop_reason(reason: StopReason) -> ErrorStopReason {
    match reason {
        StopReason::Aborted => ErrorStopReason::Aborted,
        _ => ErrorStopReason::Error,
    }
}

fn aborted_message(mut partial: AssistantMessage) -> AssistantMessage {
    partial.stop_reason = StopReason::Aborted;
    partial.error_message = Some("Request was aborted".to_owned());
    partial.timestamp = now_millis();
    partial
}

fn typed_message_to_text(message: &AssistantMessage) -> String {
    message
        .content
        .iter()
        .map(|block| match block {
            AssistantContentBlock::Text(text) => text.text.clone(),
            AssistantContentBlock::Thinking(thinking) => thinking.thinking.clone(),
            AssistantContentBlock::ToolCall(tool) => format!(
                "{}:{}",
                tool.name,
                serde_json::to_string(&tool.arguments).unwrap_or_else(|_| "{}".to_owned())
            ),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn serialize_context(context: &Context) -> String {
    let mut parts = Vec::new();
    if let Some(system_prompt) = context.system_prompt.as_deref() {
        parts.push(format!("system:{system_prompt}"));
    }
    parts.extend(context.messages.iter().map(|message| {
        format!(
            "{}:{}",
            message_role(message),
            message_to_context_text(message)
        )
    }));
    if let Some(tools) = context.tools.as_ref().filter(|tools| !tools.is_empty()) {
        parts.push(format!(
            "tools:{}",
            serde_json::to_string(tools).unwrap_or_else(|_| "[]".to_owned())
        ));
    }
    parts.join("\n\n")
}

fn message_role(message: &crate::types::Message) -> &'static str {
    match message {
        crate::types::Message::User(_) => "user",
        crate::types::Message::Assistant(_) => "assistant",
        crate::types::Message::ToolResult(_) => "toolResult",
    }
}

fn message_to_context_text(message: &crate::types::Message) -> String {
    match message {
        crate::types::Message::User(message) => user_content_to_text(&message.content),
        crate::types::Message::Assistant(message) => typed_message_to_text(message),
        crate::types::Message::ToolResult(message) => {
            let mut parts = vec![message.tool_name.clone()];
            parts.extend(message.content.iter().map(tool_result_content_to_text));
            parts.join("\n")
        }
    }
}

fn user_content_to_text(content: &crate::types::UserMessageContent) -> String {
    match content {
        crate::types::UserMessageContent::Text(text) => text.clone(),
        crate::types::UserMessageContent::Blocks(blocks) => blocks
            .iter()
            .map(user_block_to_text)
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn user_block_to_text(block: &crate::types::UserContentBlock) -> String {
    match block {
        crate::types::UserContentBlock::Text(text) => text.text.clone(),
        crate::types::UserContentBlock::Image(image) => {
            format!("[image:{}:{}]", image.mime_type, image.data.len())
        }
    }
}

fn tool_result_content_to_text(block: &crate::types::ToolResultContentBlock) -> String {
    match block {
        crate::types::ToolResultContentBlock::Text(text) => text.text.clone(),
        crate::types::ToolResultContentBlock::Image(image) => {
            format!("[image:{}:{}]", image.mime_type, image.data.len())
        }
    }
}

fn common_prefix_len(a: &str, b: &str) -> usize {
    let mut matched = 0;
    for ((a_index, a_char), (_, b_char)) in a.char_indices().zip(b.char_indices()) {
        if a_char != b_char {
            return matched;
        }
        matched = a_index + a_char.len_utf8();
    }
    matched.min(a.len()).min(b.len())
}

fn panic_message(error: Box<dyn std::any::Any + Send>) -> &'static str {
    if let Some(message) = error.downcast_ref::<&'static str>() {
        message
    } else if let Some(message) = error.downcast_ref::<String>() {
        Box::leak(message.clone().into_boxed_str())
    } else {
        "faux response factory panicked"
    }
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
    use futures::executor::block_on;

    use super::*;

    #[test]
    fn queues_and_streams_faux_responses() {
        let faux = create_faux_core(RegisterFauxProviderOptions::default());
        let model = faux.get_model(None).expect("default model");
        faux.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
            "hi",
        ))]);

        let stream = (faux.streams.stream)(&model, &Context::default(), None);

        assert_eq!(faux.state.call_count(), 1);
        assert_eq!(faux.get_pending_response_count(), 0);
        assert_eq!(message_to_text(&block_on(stream.result())), "hi");
    }

    #[test]
    fn reports_error_when_queue_is_empty() {
        let faux = create_faux_core(RegisterFauxProviderOptions::default());
        let model = faux.get_model(None).expect("default model");

        let stream = (faux.streams.stream)(&model, &Context::default(), None);

        assert_eq!(faux.state.call_count(), 1);
        assert_eq!(
            block_on(stream.result()).error_message.as_deref(),
            Some("No more faux responses queued"),
        );
    }
}
