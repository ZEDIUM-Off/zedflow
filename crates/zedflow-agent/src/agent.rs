//! Stateful Pi-compatible agent facade.

use std::collections::{HashSet, VecDeque};
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use futures::channel::oneshot;
use serde_json::Value;
use zedflow_ai::utils::abort_signals::AbortController;
use zedflow_ai::{
    AbortSignal, AssistantContentBlock, AssistantMessage, AssistantMessageRole, ImageContent,
    Message, Model, SimpleStreamOptions, StopReason, TextContent, TextContentType, Transport,
    Usage, UserContentBlock, UserMessage, UserMessageContent, UserMessageRole,
};

use crate::agent_loop::{AgentEventSink, AgentLoopError, run_agent_loop, run_agent_loop_continue};
use crate::harness::messages;
use crate::types::{
    AfterToolCallFn, AgentContext, AgentEvent, AgentFuture, AgentLoopConfig, AgentLoopTurnUpdate,
    AgentMessage, AgentState, AgentTool, BeforeToolCallFn, ConvertToLlmFn, GetApiKeyFn,
    PrepareNextTurnContext, PrepareNextTurnFn, QueueMode, StreamFn, ThinkingLevel,
    ToolExecutionMode, TransformContextFn,
};

/// Event listener registered on [`Agent`].
pub type AgentEventListener =
    Arc<dyn Fn(AgentEvent, AbortSignal) -> AgentFuture<'static, ()> + Send + Sync>;

/// Options for constructing an [`Agent`].
#[derive(Clone)]
pub struct AgentOptions {
    /// Initial transcript and model state.
    pub initial_state: Option<AgentState>,
    /// Message converter used at the LLM boundary.
    pub convert_to_llm: Option<ConvertToLlmFn>,
    /// Optional context transform before conversion.
    pub transform_context: Option<TransformContextFn>,
    /// Provider stream implementation.
    pub stream_fn: Option<StreamFn>,
    /// Dynamic API-key resolver.
    pub get_api_key: Option<GetApiKeyFn>,
    /// Base stream options forwarded to the provider.
    pub stream_options: SimpleStreamOptions,
    /// Hook called before a tool executes.
    pub before_tool_call: Option<BeforeToolCallFn>,
    /// Hook called after a tool executes.
    pub after_tool_call: Option<AfterToolCallFn>,
    /// Optional no-context next-turn hook.
    pub prepare_next_turn: Option<
        Arc<
            dyn Fn(Option<AbortSignal>) -> AgentFuture<'static, Option<AgentLoopTurnUpdate>>
                + Send
                + Sync,
        >,
    >,
    /// Optional context-aware next-turn hook.
    pub prepare_next_turn_with_context: Option<
        Arc<
            dyn Fn(
                    PrepareNextTurnContext,
                    Option<AbortSignal>,
                ) -> AgentFuture<'static, Option<AgentLoopTurnUpdate>>
                + Send
                + Sync,
        >,
    >,
    /// Steering queue drain mode.
    pub steering_mode: QueueMode,
    /// Follow-up queue drain mode.
    pub follow_up_mode: QueueMode,
    /// Session identifier forwarded to providers.
    pub session_id: Option<String>,
    /// Tool execution strategy.
    pub tool_execution: ToolExecutionMode,
}

impl Default for AgentOptions {
    fn default() -> Self {
        Self {
            initial_state: None,
            convert_to_llm: None,
            transform_context: None,
            stream_fn: None,
            get_api_key: None,
            stream_options: SimpleStreamOptions {
                stream: zedflow_ai::StreamOptions {
                    transport: Some(Transport::Auto),
                    ..zedflow_ai::StreamOptions::default()
                },
                ..SimpleStreamOptions::default()
            },
            before_tool_call: None,
            after_tool_call: None,
            prepare_next_turn: None,
            prepare_next_turn_with_context: None,
            steering_mode: QueueMode::OneAtATime,
            follow_up_mode: QueueMode::OneAtATime,
            session_id: None,
            tool_execution: ToolExecutionMode::Parallel,
        }
    }
}

/// Errors returned by the stateful agent facade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentError {
    /// A prompt or continuation was requested while another run is active.
    AlreadyProcessing(String),
    /// Continuation was requested without messages.
    NoMessagesToContinue,
    /// Continuation from an assistant requires queued user/tool messages.
    CannotContinueFromAssistant,
    /// Low-level loop rejected the continuation context.
    Loop(AgentLoopError),
    /// Listener was invoked after active-run state disappeared.
    ListenerOutsideActiveRun,
}

impl fmt::Display for AgentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyProcessing(message) => formatter.write_str(message),
            Self::NoMessagesToContinue => formatter.write_str("No messages to continue from"),
            Self::CannotContinueFromAssistant => {
                formatter.write_str("Cannot continue from message role: assistant")
            }
            Self::Loop(error) => error.fmt(formatter),
            Self::ListenerOutsideActiveRun => {
                formatter.write_str("Agent listener invoked outside active run")
            }
        }
    }
}

impl Error for AgentError {}

impl From<AgentLoopError> for AgentError {
    fn from(value: AgentLoopError) -> Self {
        Self::Loop(value)
    }
}

#[derive(Debug)]
struct PendingMessageQueue {
    messages: VecDeque<AgentMessage>,
    mode: QueueMode,
}

impl PendingMessageQueue {
    fn new(mode: QueueMode) -> Self {
        Self {
            messages: VecDeque::new(),
            mode,
        }
    }

    fn enqueue(&mut self, message: AgentMessage) {
        self.messages.push_back(message);
    }

    fn has_items(&self) -> bool {
        !self.messages.is_empty()
    }

    fn drain(&mut self) -> Vec<AgentMessage> {
        if self.mode == QueueMode::All {
            return self.messages.drain(..).collect();
        }
        self.messages.pop_front().into_iter().collect()
    }

    fn clear(&mut self) {
        self.messages.clear();
    }
}

/// Prompt input accepted by [`Agent::prompt`].
pub enum AgentPromptInput {
    /// Plain text prompt with optional images.
    Text {
        /// Text content.
        text: String,
        /// Image blocks appended after text.
        images: Vec<ImageContent>,
    },
    /// Single agent message.
    Message(AgentMessage),
    /// Batch of agent messages.
    Messages(Vec<AgentMessage>),
}

impl From<&str> for AgentPromptInput {
    fn from(value: &str) -> Self {
        Self::Text {
            text: value.to_string(),
            images: Vec::new(),
        }
    }
}

impl From<String> for AgentPromptInput {
    fn from(value: String) -> Self {
        Self::Text {
            text: value,
            images: Vec::new(),
        }
    }
}

impl From<AgentMessage> for AgentPromptInput {
    fn from(value: AgentMessage) -> Self {
        Self::Message(value)
    }
}

impl From<Vec<AgentMessage>> for AgentPromptInput {
    fn from(value: Vec<AgentMessage>) -> Self {
        Self::Messages(value)
    }
}

/// Stateful wrapper around the low-level agent loop.
pub struct Agent {
    state: Arc<Mutex<AgentState>>,
    listeners: Arc<Mutex<Vec<AgentEventListener>>>,
    steering_queue: Arc<Mutex<PendingMessageQueue>>,
    follow_up_queue: Arc<Mutex<PendingMessageQueue>>,
    active_signal: Arc<Mutex<Option<AbortController>>>,
    idle_waiters: Arc<Mutex<Vec<oneshot::Sender<()>>>>,
    convert_to_llm: ConvertToLlmFn,
    transform_context: Option<TransformContextFn>,
    stream_fn: StreamFn,
    get_api_key: Option<GetApiKeyFn>,
    stream_options: SimpleStreamOptions,
    before_tool_call: Option<BeforeToolCallFn>,
    after_tool_call: Option<AfterToolCallFn>,
    prepare_next_turn: Option<
        Arc<
            dyn Fn(Option<AbortSignal>) -> AgentFuture<'static, Option<AgentLoopTurnUpdate>>
                + Send
                + Sync,
        >,
    >,
    prepare_next_turn_with_context: Option<
        Arc<
            dyn Fn(
                    PrepareNextTurnContext,
                    Option<AbortSignal>,
                ) -> AgentFuture<'static, Option<AgentLoopTurnUpdate>>
                + Send
                + Sync,
        >,
    >,
    session_id: Option<String>,
    tool_execution: ToolExecutionMode,
}

impl Agent {
    /// Creates a new stateful agent.
    #[must_use]
    pub fn new(options: AgentOptions) -> Self {
        let stream_fn = options.stream_fn.unwrap_or_else(default_stream_fn);
        Self {
            state: Arc::new(Mutex::new(
                options.initial_state.unwrap_or_else(default_state),
            )),
            listeners: Arc::new(Mutex::new(Vec::new())),
            steering_queue: Arc::new(Mutex::new(PendingMessageQueue::new(options.steering_mode))),
            follow_up_queue: Arc::new(Mutex::new(PendingMessageQueue::new(options.follow_up_mode))),
            active_signal: Arc::new(Mutex::new(None)),
            idle_waiters: Arc::new(Mutex::new(Vec::new())),
            convert_to_llm: options
                .convert_to_llm
                .unwrap_or_else(default_convert_to_llm),
            transform_context: options.transform_context,
            stream_fn,
            get_api_key: options.get_api_key,
            stream_options: options.stream_options,
            before_tool_call: options.before_tool_call,
            after_tool_call: options.after_tool_call,
            prepare_next_turn: options.prepare_next_turn,
            prepare_next_turn_with_context: options.prepare_next_turn_with_context,
            session_id: options.session_id,
            tool_execution: options.tool_execution,
        }
    }

    /// Subscribes to lifecycle events.
    #[must_use]
    pub fn subscribe(&self, listener: AgentEventListener) -> impl FnOnce() + use<> {
        let mut listeners = lock(&self.listeners);
        listeners.push(listener.clone());
        let listeners = self.listeners.clone();
        move || {
            let mut guard = lock(&listeners);
            guard.retain(|entry| !Arc::ptr_eq(entry, &listener));
        }
    }

    /// Returns a clone of the current state.
    #[must_use]
    pub fn state(&self) -> AgentState {
        lock(&self.state).clone()
    }

    /// Replaces the current system prompt.
    pub fn set_system_prompt(&self, system_prompt: impl Into<String>) {
        lock(&self.state).system_prompt = system_prompt.into();
    }

    /// Replaces the active model.
    pub fn set_model(&self, model: Model) {
        lock(&self.state).model = model;
    }

    /// Replaces available tools.
    pub fn set_tools(&self, tools: Vec<AgentTool>) {
        lock(&self.state).tools = tools;
    }

    /// Replaces the requested thinking level.
    pub fn set_thinking_level(&self, thinking_level: ThinkingLevel) {
        lock(&self.state).thinking_level = thinking_level;
    }

    /// Controls steering queue drain mode.
    pub fn set_steering_mode(&self, mode: QueueMode) {
        lock(&self.steering_queue).mode = mode;
    }

    /// Returns steering queue drain mode.
    #[must_use]
    pub fn steering_mode(&self) -> QueueMode {
        lock(&self.steering_queue).mode
    }

    /// Controls follow-up queue drain mode.
    pub fn set_follow_up_mode(&self, mode: QueueMode) {
        lock(&self.follow_up_queue).mode = mode;
    }

    /// Returns follow-up queue drain mode.
    #[must_use]
    pub fn follow_up_mode(&self) -> QueueMode {
        lock(&self.follow_up_queue).mode
    }

    /// Queues a steering message.
    pub fn steer(&self, message: AgentMessage) {
        lock(&self.steering_queue).enqueue(message);
    }

    /// Queues a follow-up message.
    pub fn follow_up(&self, message: AgentMessage) {
        lock(&self.follow_up_queue).enqueue(message);
    }

    /// Clears queued steering messages.
    pub fn clear_steering_queue(&self) {
        lock(&self.steering_queue).clear();
    }

    /// Clears queued follow-up messages.
    pub fn clear_follow_up_queue(&self) {
        lock(&self.follow_up_queue).clear();
    }

    /// Clears both queues.
    pub fn clear_all_queues(&self) {
        self.clear_steering_queue();
        self.clear_follow_up_queue();
    }

    /// Returns true if either queue has messages.
    #[must_use]
    pub fn has_queued_messages(&self) -> bool {
        lock(&self.steering_queue).has_items() || lock(&self.follow_up_queue).has_items()
    }

    /// Returns active abort signal, if any.
    #[must_use]
    pub fn signal(&self) -> Option<AbortSignal> {
        lock(&self.active_signal)
            .as_ref()
            .map(AbortController::signal)
    }

    /// Aborts the current run, if active.
    pub fn abort(&self) {
        if let Some(controller) = lock(&self.active_signal).as_ref() {
            controller.abort();
        }
    }

    /// Resolves after the current run becomes idle.
    pub async fn wait_for_idle(&self) {
        let receiver = {
            let mut waiters = lock(&self.idle_waiters);
            if lock(&self.active_signal).is_none() {
                return;
            }
            let (sender, receiver) = oneshot::channel();
            waiters.push(sender);
            receiver
        };
        let _ = receiver.await;
    }

    /// Clears transcript state, runtime state, and queues.
    pub fn reset(&self) {
        let mut state = lock(&self.state);
        state.messages.clear();
        state.is_streaming = false;
        state.streaming_message = None;
        state.pending_tool_calls = HashSet::new();
        state.error_message = None;
        drop(state);
        self.clear_all_queues();
    }

    /// Starts a new prompt from text, a message, or a message batch.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::AlreadyProcessing`] when another run is active.
    pub async fn prompt(&self, input: impl Into<AgentPromptInput>) -> Result<(), AgentError> {
        if lock(&self.active_signal).is_some() {
            return Err(AgentError::AlreadyProcessing(
                "Agent is already processing a prompt. Use steer() or followUp() to queue messages, or wait for completion."
                    .to_string(),
            ));
        }
        let messages = normalize_prompt_input(input.into());
        self.run_prompt_messages(messages, false).await
    }

    /// Starts a text prompt with images.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::AlreadyProcessing`] when another run is active.
    pub async fn prompt_text_with_images(
        &self,
        text: impl Into<String>,
        images: Vec<ImageContent>,
    ) -> Result<(), AgentError> {
        self.prompt(AgentPromptInput::Text {
            text: text.into(),
            images,
        })
        .await
    }

    /// Continues from the current transcript.
    ///
    /// # Errors
    ///
    /// Returns an error when already running, empty, or ending on an assistant without queued work.
    pub async fn r#continue(&self) -> Result<(), AgentError> {
        if lock(&self.active_signal).is_some() {
            return Err(AgentError::AlreadyProcessing(
                "Agent is already processing. Wait for completion before continuing.".to_string(),
            ));
        }

        let last_message = lock(&self.state).messages.last().cloned();
        let Some(last_message) = last_message else {
            return Err(AgentError::NoMessagesToContinue);
        };

        if is_assistant_message(&last_message) {
            let queued_steering = lock(&self.steering_queue).drain();
            if !queued_steering.is_empty() {
                return self.run_prompt_messages(queued_steering, true).await;
            }

            let queued_follow_ups = lock(&self.follow_up_queue).drain();
            if !queued_follow_ups.is_empty() {
                return self.run_prompt_messages(queued_follow_ups, false).await;
            }

            return Err(AgentError::CannotContinueFromAssistant);
        }

        self.run_continuation().await
    }

    async fn run_prompt_messages(
        &self,
        messages: Vec<AgentMessage>,
        skip_initial_steering_poll: bool,
    ) -> Result<(), AgentError> {
        self.run_with_lifecycle(|signal| {
            let context = self.create_context_snapshot();
            let config = self.create_loop_config(skip_initial_steering_poll);
            let stream_fn = self.stream_fn.clone();
            let emit = self.event_sink();
            Box::pin(async move {
                run_agent_loop(
                    messages,
                    context,
                    config,
                    emit,
                    Some(signal),
                    Some(stream_fn),
                )
                .await;
                Ok(())
            })
        })
        .await
    }

    async fn run_continuation(&self) -> Result<(), AgentError> {
        self.run_with_lifecycle(|signal| {
            let context = self.create_context_snapshot();
            let config = self.create_loop_config(false);
            let stream_fn = self.stream_fn.clone();
            let emit = self.event_sink();
            Box::pin(async move {
                run_agent_loop_continue(context, config, emit, Some(signal), Some(stream_fn))
                    .await?;
                Ok(())
            })
        })
        .await
    }

    fn create_context_snapshot(&self) -> AgentContext {
        let state = lock(&self.state);
        AgentContext {
            system_prompt: state.system_prompt.clone(),
            messages: state.messages.clone(),
            tools: Some(state.tools.clone()),
        }
    }

    fn create_loop_config(&self, skip_initial_steering_poll: bool) -> AgentLoopConfig {
        let state = lock(&self.state);
        let mut stream_options = self.stream_options.clone();
        stream_options.reasoning = ai_thinking_level(state.thinking_level);
        stream_options.stream.session_id = self.session_id.clone();
        drop(state);

        let steering_queue = self.steering_queue.clone();
        let follow_up_queue = self.follow_up_queue.clone();
        let skip = Arc::new(Mutex::new(skip_initial_steering_poll));
        let active_signal = self.active_signal.clone();
        let prepare_next_turn = self.prepare_next_turn.clone();
        let prepare_next_turn_with_context = self.prepare_next_turn_with_context.clone();

        AgentLoopConfig {
            stream_options,
            model: lock(&self.state).model.clone(),
            convert_to_llm: self.convert_to_llm.clone(),
            transform_context: self.transform_context.clone(),
            get_api_key: self.get_api_key.clone(),
            should_stop_after_turn: None,
            prepare_next_turn: if prepare_next_turn.is_some()
                || prepare_next_turn_with_context.is_some()
            {
                Some(Arc::new(move |context| {
                    let signal = lock(&active_signal).as_ref().map(AbortController::signal);
                    let with_context = prepare_next_turn_with_context.clone();
                    let without_context = prepare_next_turn.clone();
                    let future: AgentFuture<'static, Option<AgentLoopTurnUpdate>> =
                        Box::pin(async move {
                            if let Some(with_context) = with_context {
                                return with_context(context, signal).await;
                            }
                            if let Some(without_context) = without_context {
                                return without_context(signal).await;
                            }
                            None
                        });
                    future
                }) as PrepareNextTurnFn)
            } else {
                None
            },
            get_steering_messages: Some(Arc::new(move || {
                let steering_queue = steering_queue.clone();
                let skip = skip.clone();
                Box::pin(async move {
                    let mut skip_guard = lock(&skip);
                    if *skip_guard {
                        *skip_guard = false;
                        return Vec::new();
                    }
                    drop(skip_guard);
                    lock(&steering_queue).drain()
                })
            })),
            get_follow_up_messages: Some(Arc::new(move || {
                let follow_up_queue = follow_up_queue.clone();
                Box::pin(async move { lock(&follow_up_queue).drain() })
            })),
            tool_execution: self.tool_execution,
            before_tool_call: self.before_tool_call.clone(),
            after_tool_call: self.after_tool_call.clone(),
        }
    }

    async fn run_with_lifecycle(
        &self,
        executor: impl FnOnce(AbortSignal) -> AgentFuture<'static, Result<(), AgentError>>,
    ) -> Result<(), AgentError> {
        if lock(&self.active_signal).is_some() {
            return Err(AgentError::AlreadyProcessing(
                "Agent is already processing.".to_string(),
            ));
        }

        let abort_controller = AbortController::new();
        let signal = abort_controller.signal();
        *lock(&self.active_signal) = Some(abort_controller);

        {
            let mut state = lock(&self.state);
            state.is_streaming = true;
            state.streaming_message = None;
            state.error_message = None;
        }

        let result = executor(signal.clone()).await;
        if let Err(error) = &result {
            self.handle_run_failure(error.to_string(), signal.aborted())
                .await?;
        }
        self.finish_run();
        Ok(())
    }

    async fn handle_run_failure(&self, error: String, aborted: bool) -> Result<(), AgentError> {
        let state = lock(&self.state);
        let failure_message = AssistantMessage {
            role: AssistantMessageRole::Assistant,
            content: vec![AssistantContentBlock::Text(TextContent {
                content_type: TextContentType::Text,
                text: String::new(),
                text_signature: None,
            })],
            api: state.model.api.clone(),
            provider: state.model.provider.clone(),
            model: state.model.id.clone(),
            response_model: None,
            response_id: None,
            diagnostics: None,
            usage: Usage::default(),
            stop_reason: if aborted {
                StopReason::Aborted
            } else {
                StopReason::Error
            },
            error_message: Some(error),
            timestamp: now_millis(),
        };
        drop(state);

        let message = AgentMessage::Llm(Message::Assistant(failure_message));
        self.process_event(AgentEvent::MessageStart {
            message: message.clone(),
        })
        .await?;
        self.process_event(AgentEvent::MessageEnd {
            message: message.clone(),
        })
        .await?;
        self.process_event(AgentEvent::TurnEnd {
            message: message.clone(),
            tool_results: Vec::new(),
        })
        .await?;
        self.process_event(AgentEvent::AgentEnd {
            messages: vec![message],
        })
        .await
    }

    fn finish_run(&self) {
        let mut state = lock(&self.state);
        state.is_streaming = false;
        state.streaming_message = None;
        state.pending_tool_calls = HashSet::new();
        drop(state);

        let waiters = {
            let mut waiters = lock(&self.idle_waiters);
            *lock(&self.active_signal) = None;
            std::mem::take(&mut *waiters)
        };
        for waiter in waiters {
            let _ = waiter.send(());
        }
    }

    fn event_sink(&self) -> AgentEventSink {
        let state = self.state.clone();
        let listeners = self.listeners.clone();
        let active_signal = self.active_signal.clone();
        Arc::new(move |event| {
            let state = state.clone();
            let listeners = listeners.clone();
            let active_signal = active_signal.clone();
            Box::pin(async move { process_event(state, listeners, active_signal, event).await })
        })
    }

    async fn process_event(&self, event: AgentEvent) -> Result<(), AgentError> {
        process_event(
            self.state.clone(),
            self.listeners.clone(),
            self.active_signal.clone(),
            event,
        )
        .await;
        Ok(())
    }
}

async fn process_event(
    state: Arc<Mutex<AgentState>>,
    listeners: Arc<Mutex<Vec<AgentEventListener>>>,
    active_signal: Arc<Mutex<Option<AbortController>>>,
    event: AgentEvent,
) {
    {
        let mut state = lock(&state);
        match &event {
            AgentEvent::MessageStart { message } | AgentEvent::MessageUpdate { message, .. } => {
                state.streaming_message = Some(message.clone());
            }
            AgentEvent::MessageEnd { message } => {
                state.streaming_message = None;
                state.messages.push(message.clone());
            }
            AgentEvent::ToolExecutionStart { tool_call_id, .. } => {
                state.pending_tool_calls.insert(tool_call_id.clone());
            }
            AgentEvent::ToolExecutionEnd { tool_call_id, .. } => {
                state.pending_tool_calls.remove(tool_call_id);
            }
            AgentEvent::TurnEnd { message, .. } => {
                if let AgentMessage::Llm(Message::Assistant(message)) = message {
                    if let Some(error_message) = &message.error_message {
                        state.error_message = Some(error_message.clone());
                    }
                }
            }
            AgentEvent::AgentEnd { .. } => {
                state.streaming_message = None;
            }
            AgentEvent::AgentStart
            | AgentEvent::TurnStart
            | AgentEvent::ToolExecutionUpdate { .. } => {}
        }
    }

    let Some(signal) = lock(&active_signal).as_ref().map(AbortController::signal) else {
        return;
    };
    let listeners = lock(&listeners).clone();
    for listener in listeners {
        listener(event.clone(), signal.clone()).await;
    }
}

fn normalize_prompt_input(input: AgentPromptInput) -> Vec<AgentMessage> {
    match input {
        AgentPromptInput::Message(message) => vec![message],
        AgentPromptInput::Messages(messages) => messages,
        AgentPromptInput::Text { text, images } => {
            let mut blocks = vec![UserContentBlock::Text(TextContent {
                content_type: TextContentType::Text,
                text,
                text_signature: None,
            })];
            blocks.extend(images.into_iter().map(UserContentBlock::Image));
            vec![AgentMessage::Llm(Message::User(UserMessage {
                role: UserMessageRole::User,
                content: UserMessageContent::Blocks(blocks),
                timestamp: now_millis(),
            }))]
        }
    }
}

fn default_state() -> AgentState {
    AgentState {
        system_prompt: String::new(),
        model: Model {
            id: "unknown".to_string(),
            name: "unknown".to_string(),
            api: "unknown".to_string(),
            provider: "unknown".to_string(),
            ..Model::default()
        },
        thinking_level: ThinkingLevel::Off,
        tools: Vec::new(),
        messages: Vec::new(),
        is_streaming: false,
        streaming_message: None,
        pending_tool_calls: HashSet::new(),
        error_message: None,
    }
}

fn default_convert_to_llm() -> ConvertToLlmFn {
    Arc::new(|agent_messages| Box::pin(async move { messages::convert_to_llm(&agent_messages) }))
}

fn default_stream_fn() -> StreamFn {
    Arc::new(|model, context, options| {
        zedflow_ai::create_models().stream_simple(model, context, options)
    })
}

fn ai_thinking_level(level: ThinkingLevel) -> Option<zedflow_ai::ThinkingLevel> {
    match level {
        ThinkingLevel::Off => None,
        ThinkingLevel::Minimal => Some(zedflow_ai::ThinkingLevel::Minimal),
        ThinkingLevel::Low => Some(zedflow_ai::ThinkingLevel::Low),
        ThinkingLevel::Medium => Some(zedflow_ai::ThinkingLevel::Medium),
        ThinkingLevel::High => Some(zedflow_ai::ThinkingLevel::High),
        ThinkingLevel::XHigh => Some(zedflow_ai::ThinkingLevel::XHigh),
    }
}

fn is_assistant_message(message: &AgentMessage) -> bool {
    matches!(message, AgentMessage::Llm(Message::Assistant(_)))
        || matches!(
            message,
            AgentMessage::Custom(value) if value.get("role").and_then(Value::as_str) == Some("assistant")
        )
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
