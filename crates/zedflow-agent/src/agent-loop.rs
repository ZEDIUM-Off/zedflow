//! Core Pi-compatible agent loop.
//!
//! The async `run_*` functions preserve Pi's event order while consuming the
//! `zedflow-ai` assistant event stream directly. The stream-returning helpers
//! dispatch the loop onto the current Tokio runtime or a private fallback runtime.

use std::any::Any;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::channel::mpsc;
use futures::future::{join_all, poll_fn, ready};
use futures::stream::FuturesOrdered;
use futures::{FutureExt, StreamExt};
use serde_json::{Map, Value};
use zedflow_ai::{
    AssistantContentBlock, AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream,
    Context as AiContext, ErrorStopReason, EventStream, Message, StopReason, TextContent,
    TextContentType, ToolCall, ToolResultContentBlock, ToolResultMessage, ToolResultMessageRole,
    validate_tool_arguments,
};

pub use crate::types::AgentEventSink;
use crate::types::{
    AgentCallbackError, AgentContext, AgentEvent, AgentLoopConfig, AgentLoopTurnUpdate,
    AgentMessage, AgentTool, AgentToolResult, AgentToolResultContent, StreamFn, ThinkingLevel,
    ToolExecutionMode,
};

/// Event stream returned by low-level agent-loop adapters.
pub type AgentEventStream = EventStream<AgentEvent, Vec<AgentMessage>>;

/// Errors raised before a continuation can safely call a provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentLoopError {
    /// Continuation requires at least one existing context message.
    CannotContinueNoMessages,
    /// Pi rejects continuation from an assistant message.
    CannotContinueFromAssistant,
}

impl fmt::Display for AgentLoopError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CannotContinueNoMessages => {
                formatter.write_str("Cannot continue: no messages in context")
            }
            Self::CannotContinueFromAssistant => {
                formatter.write_str("Cannot continue from message role: assistant")
            }
        }
    }
}

impl Error for AgentLoopError {}

/// Starts an agent loop with new prompt messages and returns its live event stream.
#[must_use]
pub fn agent_loop(
    prompts: Vec<AgentMessage>,
    context: AgentContext,
    config: AgentLoopConfig,
    signal: Option<zedflow_ai::AbortSignal>,
    stream_fn: Option<StreamFn>,
) -> AgentEventStream {
    let stream = create_agent_stream();
    let progress = Arc::new(Mutex::new(AgentLoopProgress::default()));
    let emit_stream = stream.clone();
    let emit_progress = progress.clone();
    let emit: AgentEventSink = Arc::new(move |event| {
        let emit_stream = emit_stream.clone();
        let emit_progress = emit_progress.clone();
        Box::pin(async move {
            lock_progress(&emit_progress).record(&event);
            emit_stream.push(event);
            Ok(())
        })
    });
    let model = config.model.clone();

    spawn_agent_loop_worker(
        stream.clone(),
        model,
        progress,
        run_agent_loop(prompts, context, config, emit, signal, stream_fn),
    );
    stream
}

/// Continues an agent loop from existing context and returns a populated event stream.
///
/// # Errors
///
/// Returns [`AgentLoopError`] when the context is empty or ends with an assistant message.
pub fn agent_loop_continue(
    context: AgentContext,
    config: AgentLoopConfig,
    signal: Option<zedflow_ai::AbortSignal>,
    stream_fn: Option<StreamFn>,
) -> Result<AgentEventStream, AgentLoopError> {
    validate_continuation_context(&context)?;

    let stream = create_agent_stream();
    let progress = Arc::new(Mutex::new(AgentLoopProgress::default()));
    let emit_stream = stream.clone();
    let emit_progress = progress.clone();
    let emit: AgentEventSink = Arc::new(move |event| {
        let emit_stream = emit_stream.clone();
        let emit_progress = emit_progress.clone();
        Box::pin(async move {
            lock_progress(&emit_progress).record(&event);
            emit_stream.push(event);
            Ok(())
        })
    });
    let model = config.model.clone();

    spawn_agent_loop_worker(
        stream.clone(),
        model,
        progress,
        run_agent_loop_continue(context, config, emit, signal, stream_fn),
    );
    Ok(stream)
}

/// Runs a prompt-started agent loop.
///
/// # Errors
///
/// Returns the event sink error when an emitted lifecycle event is rejected.
pub async fn run_agent_loop(
    prompts: Vec<AgentMessage>,
    context: AgentContext,
    config: AgentLoopConfig,
    emit: AgentEventSink,
    signal: Option<zedflow_ai::AbortSignal>,
    stream_fn: Option<StreamFn>,
) -> Result<Vec<AgentMessage>, AgentCallbackError> {
    let mut new_messages = prompts.clone();
    let mut current_context = context;
    current_context.messages.extend(prompts.clone());

    emit_event(&emit, AgentEvent::AgentStart).await?;
    emit_event(&emit, AgentEvent::TurnStart).await?;
    for prompt in prompts {
        emit_event(
            &emit,
            AgentEvent::MessageStart {
                message: prompt.clone(),
            },
        )
        .await?;
        emit_event(&emit, AgentEvent::MessageEnd { message: prompt }).await?;
    }

    run_loop(
        current_context,
        &mut new_messages,
        config,
        signal,
        emit,
        stream_fn,
    )
    .await?;
    Ok(new_messages)
}

/// Runs a continuation from the current context without injecting prompt messages.
///
/// # Errors
///
/// Returns a boxed [`AgentLoopError`] when the context is invalid, or the event sink error when an
/// emitted lifecycle event is rejected.
pub async fn run_agent_loop_continue(
    context: AgentContext,
    config: AgentLoopConfig,
    emit: AgentEventSink,
    signal: Option<zedflow_ai::AbortSignal>,
    stream_fn: Option<StreamFn>,
) -> Result<Vec<AgentMessage>, AgentCallbackError> {
    validate_continuation_context(&context)
        .map_err(|error| Box::new(error) as AgentCallbackError)?;

    let mut new_messages = Vec::new();
    emit_event(&emit, AgentEvent::AgentStart).await?;
    emit_event(&emit, AgentEvent::TurnStart).await?;

    run_loop(context, &mut new_messages, config, signal, emit, stream_fn).await?;
    Ok(new_messages)
}

fn create_agent_stream() -> AgentEventStream {
    EventStream::new(
        |event| matches!(event, AgentEvent::AgentEnd { .. }),
        |event| match event {
            AgentEvent::AgentEnd { messages } => messages.clone(),
            _ => Vec::new(),
        },
    )
}

#[derive(Default)]
struct AgentLoopProgress {
    messages: Vec<AgentMessage>,
    agent_started: bool,
    turn_open: bool,
    message_open: bool,
}

impl AgentLoopProgress {
    fn record(&mut self, event: &AgentEvent) {
        match event {
            AgentEvent::AgentStart => self.agent_started = true,
            AgentEvent::TurnStart => self.turn_open = true,
            AgentEvent::MessageStart { .. } => self.message_open = true,
            AgentEvent::MessageEnd { message } => {
                self.message_open = false;
                self.messages.push(message.clone());
            }
            AgentEvent::TurnEnd { .. } => self.turn_open = false,
            AgentEvent::AgentEnd { .. }
            | AgentEvent::MessageUpdate { .. }
            | AgentEvent::ToolExecutionStart { .. }
            | AgentEvent::ToolExecutionUpdate { .. }
            | AgentEvent::ToolExecutionEnd { .. } => {}
        }
    }
}

fn lock_progress(
    progress: &Mutex<AgentLoopProgress>,
) -> std::sync::MutexGuard<'_, AgentLoopProgress> {
    progress
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn spawn_agent_loop_worker(
    stream: AgentEventStream,
    model: zedflow_ai::Model,
    progress: Arc<Mutex<AgentLoopProgress>>,
    task: impl Future<Output = Result<Vec<AgentMessage>, AgentCallbackError>> + Send + 'static,
) {
    let failure_stream = stream.clone();
    let failure_progress = progress.clone();
    let worker_model = model.clone();
    let worker = async move {
        let result = AssertUnwindSafe(task).catch_unwind().await;
        match result {
            Ok(Ok(messages)) => stream.end(Some(messages)),
            Ok(Err(error)) => {
                finish_agent_loop_error(&stream, &worker_model, &progress, error.to_string())
            }
            Err(panic) => finish_agent_loop_error(
                &stream,
                &worker_model,
                &progress,
                format!("agent loop worker panicked: {}", panic_message(&panic)),
            ),
        }
    };

    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(worker);
    } else {
        std::thread::spawn(move || {
            match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime.block_on(worker),
                Err(error) => finish_agent_loop_error(
                    &failure_stream,
                    &model,
                    &failure_progress,
                    format!("agent loop runtime construction failed: {error}"),
                ),
            }
        });
    }
}

fn finish_agent_loop_error(
    stream: &AgentEventStream,
    model: &zedflow_ai::Model,
    progress: &Mutex<AgentLoopProgress>,
    error: String,
) {
    if stream.is_done() {
        return;
    }
    let AgentLoopProgress {
        mut messages,
        agent_started,
        turn_open,
        message_open,
    } = std::mem::take(&mut *lock_progress(progress));
    let message = AgentMessage::Llm(Message::Assistant(stream_error_message(
        model, error, false,
    )));
    if !agent_started {
        stream.push(AgentEvent::AgentStart);
    }
    if !turn_open {
        stream.push(AgentEvent::TurnStart);
    }
    if !message_open {
        stream.push(AgentEvent::MessageStart {
            message: message.clone(),
        });
    }
    stream.push(AgentEvent::MessageEnd {
        message: message.clone(),
    });
    stream.push(AgentEvent::TurnEnd {
        message: message.clone(),
        tool_results: Vec::new(),
    });
    messages.push(message);
    stream.push(AgentEvent::AgentEnd { messages });
}

fn panic_message(panic: &Box<dyn Any + Send>) -> &str {
    panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("unknown panic")
}

async fn run_loop(
    initial_context: AgentContext,
    new_messages: &mut Vec<AgentMessage>,
    initial_config: AgentLoopConfig,
    signal: Option<zedflow_ai::AbortSignal>,
    emit: AgentEventSink,
    stream_fn: Option<StreamFn>,
) -> Result<(), AgentCallbackError> {
    let mut current_context = initial_context;
    let mut config = initial_config;
    let mut first_turn = true;
    let mut pending_messages = drain_queue(&config.get_steering_messages).await;

    loop {
        let mut has_more_tool_calls = true;

        while has_more_tool_calls || !pending_messages.is_empty() {
            if first_turn {
                first_turn = false;
            } else {
                emit_event(&emit, AgentEvent::TurnStart).await?;
            }

            if !pending_messages.is_empty() {
                for message in pending_messages.drain(..) {
                    emit_event(
                        &emit,
                        AgentEvent::MessageStart {
                            message: message.clone(),
                        },
                    )
                    .await?;
                    emit_event(
                        &emit,
                        AgentEvent::MessageEnd {
                            message: message.clone(),
                        },
                    )
                    .await?;
                    current_context.messages.push(message.clone());
                    new_messages.push(message);
                }
            }

            let message = stream_assistant_response(
                &mut current_context,
                &config,
                signal.clone(),
                emit.clone(),
                stream_fn.clone(),
            )
            .await?;
            new_messages.push(AgentMessage::Llm(Message::Assistant(message.clone())));

            if matches!(message.stop_reason, StopReason::Error | StopReason::Aborted) {
                emit_event(
                    &emit,
                    AgentEvent::TurnEnd {
                        message: AgentMessage::Llm(Message::Assistant(message)),
                        tool_results: Vec::new(),
                    },
                )
                .await?;
                emit_event(
                    &emit,
                    AgentEvent::AgentEnd {
                        messages: new_messages.clone(),
                    },
                )
                .await?;
                return Ok(());
            }

            let tool_calls = assistant_tool_calls(&message);
            let mut tool_results = Vec::new();
            has_more_tool_calls = false;
            if !tool_calls.is_empty() {
                let executed = execute_tool_calls(
                    &current_context,
                    &message,
                    tool_calls,
                    &config,
                    signal.clone(),
                    emit.clone(),
                )
                .await?;
                tool_results.extend(executed.messages);
                has_more_tool_calls = !executed.terminate;

                for result in &tool_results {
                    current_context
                        .messages
                        .push(AgentMessage::Llm(Message::ToolResult(result.clone())));
                    new_messages.push(AgentMessage::Llm(Message::ToolResult(result.clone())));
                }
            }

            emit_event(
                &emit,
                AgentEvent::TurnEnd {
                    message: AgentMessage::Llm(Message::Assistant(message.clone())),
                    tool_results: tool_results.clone(),
                },
            )
            .await?;

            if let Some(update) = prepare_next_turn(
                &config,
                message.clone(),
                tool_results.clone(),
                current_context.clone(),
                new_messages.clone(),
            )
            .await?
            {
                apply_turn_update(&mut current_context, &mut config, update);
            }

            if should_stop_after_turn(
                &config,
                message,
                tool_results,
                current_context.clone(),
                new_messages.clone(),
            )
            .await
            {
                emit_event(
                    &emit,
                    AgentEvent::AgentEnd {
                        messages: new_messages.clone(),
                    },
                )
                .await?;
                return Ok(());
            }

            pending_messages = drain_queue(&config.get_steering_messages).await;
        }

        let follow_up_messages = drain_queue(&config.get_follow_up_messages).await;
        if follow_up_messages.is_empty() {
            break;
        }
        pending_messages = follow_up_messages;
    }

    emit_event(
        &emit,
        AgentEvent::AgentEnd {
            messages: new_messages.clone(),
        },
    )
    .await?;
    Ok(())
}

async fn stream_assistant_response(
    context: &mut AgentContext,
    config: &AgentLoopConfig,
    signal: Option<zedflow_ai::AbortSignal>,
    emit: AgentEventSink,
    stream_fn: Option<StreamFn>,
) -> Result<AssistantMessage, AgentCallbackError> {
    let mut messages = context.messages.clone();
    if let Some(transform) = &config.transform_context {
        messages = transform(messages, signal.clone()).await;
    }

    let llm_messages = (config.convert_to_llm)(messages).await;
    let llm_context = AiContext {
        system_prompt: Some(context.system_prompt.clone()),
        messages: llm_messages,
        tools: context
            .tools
            .as_ref()
            .map(|tools| tools.iter().map(|tool| tool.tool.clone()).collect()),
    };

    let mut options = config.stream_options.clone();
    options.stream.signal = signal.clone();
    let resolved_api_key = if let Some(get_api_key) = &config.get_api_key {
        get_api_key(config.model.provider.clone()).await
    } else {
        None
    };
    if resolved_api_key.is_some() {
        options.stream.api_key = resolved_api_key;
    }

    let default_stream_fn = default_stream_fn();
    let stream_function = stream_fn.unwrap_or(default_stream_fn);
    let mut response = match stream_function(&config.model, &llm_context, Some(&options)).await {
        Ok(response) => response,
        Err(error) => error_assistant_stream(&config.model, error.to_string(), false),
    };

    let mut partial_message = None;
    let mut added_partial = false;

    while let Some(event) = response.next().await {
        match &event {
            AssistantMessageEvent::Start { partial } => {
                partial_message = Some(partial.clone());
                let snapshot = partial.snapshot();
                context
                    .messages
                    .push(AgentMessage::Llm(Message::Assistant(snapshot.clone())));
                added_partial = true;
                emit_event(
                    &emit,
                    AgentEvent::MessageStart {
                        message: AgentMessage::Llm(Message::Assistant(snapshot)),
                    },
                )
                .await?;
            }
            AssistantMessageEvent::Done { .. } | AssistantMessageEvent::Error { .. } => {
                let final_message = response.result().await;
                replace_or_push_assistant(context, added_partial, final_message.clone());
                if !added_partial {
                    emit_event(
                        &emit,
                        AgentEvent::MessageStart {
                            message: AgentMessage::Llm(Message::Assistant(final_message.clone())),
                        },
                    )
                    .await?;
                }
                emit_event(
                    &emit,
                    AgentEvent::MessageEnd {
                        message: AgentMessage::Llm(Message::Assistant(final_message.clone())),
                    },
                )
                .await?;
                return Ok(final_message);
            }
            _ => {
                if added_partial && let Some(partial) = partial_from_event(&event) {
                    partial_message = Some(partial.clone());
                    let snapshot = partial.snapshot();
                    if let Some(last) = context.messages.last_mut() {
                        *last = AgentMessage::Llm(Message::Assistant(snapshot.clone()));
                    }
                    emit_event(
                        &emit,
                        AgentEvent::MessageUpdate {
                            assistant_message_event: event.clone(),
                            message: AgentMessage::Llm(Message::Assistant(snapshot)),
                        },
                    )
                    .await?;
                }
            }
        }
    }

    let final_message = response.result().await;
    replace_or_push_assistant(context, added_partial, final_message.clone());
    if !added_partial {
        emit_event(
            &emit,
            AgentEvent::MessageStart {
                message: AgentMessage::Llm(Message::Assistant(final_message.clone())),
            },
        )
        .await?;
    }
    if partial_message.is_some() || !added_partial {
        emit_event(
            &emit,
            AgentEvent::MessageEnd {
                message: AgentMessage::Llm(Message::Assistant(final_message.clone())),
            },
        )
        .await?;
    }
    Ok(final_message)
}

fn replace_or_push_assistant(
    context: &mut AgentContext,
    added_partial: bool,
    final_message: AssistantMessage,
) {
    if added_partial {
        if let Some(last) = context.messages.last_mut() {
            *last = AgentMessage::Llm(Message::Assistant(final_message));
            return;
        }
    }
    context
        .messages
        .push(AgentMessage::Llm(Message::Assistant(final_message)));
}

fn partial_from_event(
    event: &AssistantMessageEvent,
) -> Option<&zedflow_ai::types::SharedAssistantMessage> {
    match event {
        AssistantMessageEvent::TextStart { partial, .. }
        | AssistantMessageEvent::TextDelta { partial, .. }
        | AssistantMessageEvent::TextEnd { partial, .. }
        | AssistantMessageEvent::ThinkingStart { partial, .. }
        | AssistantMessageEvent::ThinkingDelta { partial, .. }
        | AssistantMessageEvent::ThinkingEnd { partial, .. }
        | AssistantMessageEvent::ToolcallStart { partial, .. }
        | AssistantMessageEvent::ToolcallDelta { partial, .. }
        | AssistantMessageEvent::ToolcallEnd { partial, .. } => Some(partial),
        _ => None,
    }
}

fn assistant_tool_calls(message: &AssistantMessage) -> Vec<ToolCall> {
    message
        .content
        .iter()
        .filter_map(|content| match content {
            AssistantContentBlock::ToolCall(tool_call) => Some(tool_call.clone()),
            _ => None,
        })
        .collect()
}

async fn execute_tool_calls(
    current_context: &AgentContext,
    assistant_message: &AssistantMessage,
    tool_calls: Vec<ToolCall>,
    config: &AgentLoopConfig,
    signal: Option<zedflow_ai::AbortSignal>,
    emit: AgentEventSink,
) -> Result<ExecutedToolCallBatch, AgentCallbackError> {
    let has_sequential_tool_call = tool_calls.iter().any(|tool_call| {
        current_context
            .tools
            .as_ref()
            .and_then(|tools| tools.iter().find(|tool| tool.tool.name == tool_call.name))
            .and_then(|tool| tool.execution_mode)
            == Some(ToolExecutionMode::Sequential)
    });

    if config.tool_execution == ToolExecutionMode::Sequential || has_sequential_tool_call {
        execute_tool_calls_sequential(
            current_context,
            assistant_message,
            tool_calls,
            config,
            signal,
            emit,
        )
        .await
    } else {
        execute_tool_calls_parallel(
            current_context,
            assistant_message,
            tool_calls,
            config,
            signal,
            emit,
        )
        .await
    }
}

struct ExecutedToolCallBatch {
    messages: Vec<ToolResultMessage>,
    terminate: bool,
}

async fn execute_tool_calls_sequential(
    current_context: &AgentContext,
    assistant_message: &AssistantMessage,
    tool_calls: Vec<ToolCall>,
    config: &AgentLoopConfig,
    signal: Option<zedflow_ai::AbortSignal>,
    emit: AgentEventSink,
) -> Result<ExecutedToolCallBatch, AgentCallbackError> {
    let mut finalized_calls = Vec::new();
    let mut messages = Vec::new();

    for tool_call in tool_calls {
        emit_tool_execution_start(&tool_call, &emit).await?;

        let preparation = prepare_tool_call(
            current_context,
            assistant_message,
            tool_call.clone(),
            config,
            signal.clone(),
        )
        .await;
        let finalized = match preparation {
            PreparedToolCallOutcome::Immediate { result, is_error } => FinalizedToolCallOutcome {
                tool_call,
                result,
                is_error,
            },
            PreparedToolCallOutcome::Prepared(prepared) => {
                let executed =
                    execute_prepared_tool_call(&prepared, signal.clone(), emit.clone()).await?;
                finalize_executed_tool_call(
                    current_context,
                    assistant_message,
                    prepared,
                    executed,
                    config,
                    signal.clone(),
                )
                .await
            }
        };

        emit_tool_execution_end(&finalized, &emit).await?;
        let tool_result_message = create_tool_result_message(&finalized);
        emit_tool_result_message(tool_result_message.clone(), &emit).await?;
        finalized_calls.push(finalized);
        messages.push(tool_result_message);

        if signal
            .as_ref()
            .is_some_and(zedflow_ai::AbortSignal::aborted)
        {
            break;
        }
    }

    Ok(ExecutedToolCallBatch {
        messages,
        terminate: should_terminate_tool_batch(&finalized_calls),
    })
}

async fn execute_tool_calls_parallel(
    current_context: &AgentContext,
    assistant_message: &AssistantMessage,
    tool_calls: Vec<ToolCall>,
    config: &AgentLoopConfig,
    signal: Option<zedflow_ai::AbortSignal>,
    emit: AgentEventSink,
) -> Result<ExecutedToolCallBatch, AgentCallbackError> {
    let mut finalized_entries = Vec::new();

    for tool_call in tool_calls {
        emit_tool_execution_start(&tool_call, &emit).await?;

        let preparation = prepare_tool_call(
            current_context,
            assistant_message,
            tool_call.clone(),
            config,
            signal.clone(),
        )
        .await;
        match preparation {
            PreparedToolCallOutcome::Immediate { result, is_error } => {
                let finalized = FinalizedToolCallOutcome {
                    tool_call,
                    result,
                    is_error,
                };
                emit_tool_execution_end(&finalized, &emit).await?;
                finalized_entries.push(ready(Ok(finalized)).boxed());
            }
            PreparedToolCallOutcome::Prepared(prepared) => {
                let context = current_context.clone();
                let assistant = assistant_message.clone();
                let cfg = config.clone();
                let signal = signal.clone();
                let emit = emit.clone();
                finalized_entries.push(
                    async move {
                        let executed =
                            execute_prepared_tool_call(&prepared, signal.clone(), emit.clone())
                                .await?;
                        let finalized = finalize_executed_tool_call(
                            &context, &assistant, prepared, executed, &cfg, signal,
                        )
                        .await;
                        emit_tool_execution_end(&finalized, &emit).await?;
                        Ok(finalized)
                    }
                    .boxed(),
                );
            }
        }

        if signal
            .as_ref()
            .is_some_and(zedflow_ai::AbortSignal::aborted)
        {
            break;
        }
    }

    let ordered_finalized_calls = join_all(finalized_entries)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, AgentCallbackError>>()?;
    let mut messages = Vec::new();
    for finalized in &ordered_finalized_calls {
        let tool_result_message = create_tool_result_message(finalized);
        emit_tool_result_message(tool_result_message.clone(), &emit).await?;
        messages.push(tool_result_message);
    }

    Ok(ExecutedToolCallBatch {
        messages,
        terminate: should_terminate_tool_batch(&ordered_finalized_calls),
    })
}

#[derive(Clone)]
struct PreparedToolCall {
    tool_call: ToolCall,
    tool: AgentTool,
    args: Value,
}

enum PreparedToolCallOutcome {
    Prepared(PreparedToolCall),
    Immediate {
        result: AgentToolResult,
        is_error: bool,
    },
}

struct ExecutedToolCallOutcome {
    result: AgentToolResult,
    is_error: bool,
}

#[derive(Clone)]
struct FinalizedToolCallOutcome {
    tool_call: ToolCall,
    result: AgentToolResult,
    is_error: bool,
}

fn should_terminate_tool_batch(finalized_calls: &[FinalizedToolCallOutcome]) -> bool {
    !finalized_calls.is_empty()
        && finalized_calls
            .iter()
            .all(|finalized| finalized.result.terminate == Some(true))
}

fn prepare_tool_call_arguments(tool: &AgentTool, tool_call: &ToolCall) -> Result<ToolCall, String> {
    let Some(prepare_arguments) = &tool.prepare_arguments else {
        return Ok(tool_call.clone());
    };
    let prepared_arguments = prepare_arguments(arguments_to_value(&tool_call.arguments))
        .map_err(|error| error.to_string())?;
    let arguments = value_to_arguments(prepared_arguments)
        .ok_or_else(|| "Prepared tool arguments must be an object".to_owned())?;
    if arguments == tool_call.arguments {
        return Ok(tool_call.clone());
    }
    Ok(ToolCall {
        arguments,
        ..tool_call.clone()
    })
}

async fn prepare_tool_call(
    current_context: &AgentContext,
    assistant_message: &AssistantMessage,
    tool_call: ToolCall,
    config: &AgentLoopConfig,
    signal: Option<zedflow_ai::AbortSignal>,
) -> PreparedToolCallOutcome {
    let Some(tool) = current_context
        .tools
        .as_ref()
        .and_then(|tools| tools.iter().find(|tool| tool.tool.name == tool_call.name))
        .cloned()
    else {
        return immediate_error_tool_result(format!("Tool {} not found", tool_call.name));
    };

    let prepared_tool_call = match prepare_tool_call_arguments(&tool, &tool_call) {
        Ok(tool_call) => tool_call,
        Err(error) => return immediate_error_tool_result(error),
    };
    let mut validated_args = match validate_tool_arguments(&tool.tool, &prepared_tool_call) {
        Ok(args) => args,
        Err(error) => return immediate_error_tool_result(error.to_string()),
    };

    if let Some(before_tool_call) = &config.before_tool_call {
        let before_result = before_tool_call(
            crate::types::BeforeToolCallContext {
                assistant_message: assistant_message.clone(),
                tool_call: tool_call.clone(),
                args: validated_args.clone(),
                context: current_context.clone(),
            },
            signal.clone(),
        )
        .await;

        let before_result = match before_result {
            Ok(before_result) => before_result,
            Err(error) => return immediate_error_tool_result(error.to_string()),
        };

        if signal
            .as_ref()
            .is_some_and(zedflow_ai::AbortSignal::aborted)
        {
            return immediate_error_tool_result("Operation aborted");
        }

        if let Some(before_result) = before_result {
            if before_result.block.unwrap_or(false) {
                return immediate_error_tool_result(
                    before_result
                        .reason
                        .unwrap_or_else(|| "Tool execution was blocked".to_string()),
                );
            }
            if let Some(args) = before_result.args {
                validated_args = args;
            }
        }
    }

    if signal
        .as_ref()
        .is_some_and(zedflow_ai::AbortSignal::aborted)
    {
        return immediate_error_tool_result("Operation aborted");
    }

    PreparedToolCallOutcome::Prepared(PreparedToolCall {
        tool_call,
        tool,
        args: validated_args,
    })
}

async fn execute_prepared_tool_call(
    prepared: &PreparedToolCall,
    signal: Option<zedflow_ai::AbortSignal>,
    emit: AgentEventSink,
) -> Result<ExecutedToolCallOutcome, AgentCallbackError> {
    let accepting_updates = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let (update_sender, update_receiver) = mpsc::unbounded();
    let update_flag = accepting_updates.clone();
    let tool_call_id = prepared.tool_call.id.clone();
    let tool_name = prepared.tool_call.name.clone();
    let args = arguments_to_value(&prepared.tool_call.arguments);
    let on_update = Arc::new(move |partial_result: AgentToolResult| {
        if !update_flag.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let event = AgentEvent::ToolExecutionUpdate {
            tool_call_id: tool_call_id.clone(),
            tool_name: tool_name.clone(),
            args: args.clone(),
            partial_result: serde_json::to_value(partial_result).unwrap_or(Value::Null),
        };
        let _ = update_sender.unbounded_send(event);
    });

    let mut update_receiver = update_receiver.fuse();
    let tool_future = (prepared.tool.execute)(
        &prepared.tool_call.id,
        prepared.args.clone(),
        signal,
        Some(on_update),
    )
    .fuse();
    futures::pin_mut!(tool_future);
    const UPDATE_POLL_BUDGET: usize = 64;

    let mut update_emits = FuturesOrdered::new();
    let mut update_error = None;
    let result = poll_fn(|context| {
        let mut receiver_budget_exhausted = true;
        for _ in 0..UPDATE_POLL_BUDGET {
            match update_receiver.poll_next_unpin(context) {
                Poll::Ready(Some(event)) => update_emits.push_back(emit(event)),
                Poll::Ready(None) | Poll::Pending => {
                    receiver_budget_exhausted = false;
                    break;
                }
            }
        }

        let mut emit_budget_exhausted = true;
        for _ in 0..UPDATE_POLL_BUDGET {
            match update_emits.poll_next_unpin(context) {
                Poll::Ready(Some(result)) => {
                    if let Err(error) = result
                        && update_error.is_none()
                    {
                        update_error = Some(error);
                    }
                }
                Poll::Ready(None) | Poll::Pending => {
                    emit_budget_exhausted = false;
                    break;
                }
            }
        }

        match tool_future.as_mut().poll(context) {
            Poll::Ready(result) => Poll::Ready(result),
            Poll::Pending => {
                if receiver_budget_exhausted || emit_budget_exhausted {
                    context.waker().wake_by_ref();
                }
                Poll::Pending
            }
        }
    })
    .await;
    accepting_updates.store(false, std::sync::atomic::Ordering::SeqCst);
    while let Some(Some(event)) = update_receiver.next().now_or_never() {
        update_emits.push_back(emit(event));
    }
    while let Some(result) = update_emits.next().await {
        if let Err(error) = result
            && update_error.is_none()
        {
            update_error = Some(error);
        }
    }
    if let Some(error) = update_error {
        return Err(error);
    }

    Ok(match result {
        Ok(result) => ExecutedToolCallOutcome {
            result,
            is_error: false,
        },
        Err(error) => ExecutedToolCallOutcome {
            result: create_error_tool_result(error.to_string()),
            is_error: true,
        },
    })
}

async fn finalize_executed_tool_call(
    current_context: &AgentContext,
    assistant_message: &AssistantMessage,
    prepared: PreparedToolCall,
    executed: ExecutedToolCallOutcome,
    config: &AgentLoopConfig,
    signal: Option<zedflow_ai::AbortSignal>,
) -> FinalizedToolCallOutcome {
    let mut result = executed.result;
    let mut is_error = executed.is_error;

    if let Some(after_tool_call) = &config.after_tool_call {
        match after_tool_call(
            crate::types::AfterToolCallContext {
                assistant_message: assistant_message.clone(),
                tool_call: prepared.tool_call.clone(),
                args: prepared.args.clone(),
                result: result.clone(),
                is_error,
                context: current_context.clone(),
            },
            signal,
        )
        .await
        {
            Ok(Some(after_result)) => {
                if let Some(content) = after_result.content {
                    result.content = content;
                }
                if let Some(details) = after_result.details {
                    result.details = details;
                }
                if after_result.terminate.is_some() {
                    result.terminate = after_result.terminate;
                }
                if let Some(next_is_error) = after_result.is_error {
                    is_error = next_is_error;
                }
            }
            Ok(None) => {}
            Err(error) => {
                result = create_error_tool_result(error.to_string());
                is_error = true;
            }
        }
    }

    FinalizedToolCallOutcome {
        tool_call: prepared.tool_call,
        result,
        is_error,
    }
}

fn immediate_error_tool_result(message: impl Into<String>) -> PreparedToolCallOutcome {
    PreparedToolCallOutcome::Immediate {
        result: create_error_tool_result(message),
        is_error: true,
    }
}

fn create_error_tool_result(message: impl Into<String>) -> AgentToolResult {
    AgentToolResult {
        content: vec![AgentToolResultContent::Text(TextContent {
            content_type: TextContentType::Text,
            text: message.into(),
            text_signature: None,
        })],
        details: Value::Object(Map::new()),
        terminate: None,
    }
}

async fn emit_tool_execution_start(
    tool_call: &ToolCall,
    emit: &AgentEventSink,
) -> Result<(), AgentCallbackError> {
    emit_event(
        emit,
        AgentEvent::ToolExecutionStart {
            tool_call_id: tool_call.id.clone(),
            tool_name: tool_call.name.clone(),
            args: arguments_to_value(&tool_call.arguments),
        },
    )
    .await
}

async fn emit_tool_execution_end(
    finalized: &FinalizedToolCallOutcome,
    emit: &AgentEventSink,
) -> Result<(), AgentCallbackError> {
    emit_event(
        emit,
        AgentEvent::ToolExecutionEnd {
            tool_call_id: finalized.tool_call.id.clone(),
            tool_name: finalized.tool_call.name.clone(),
            result: serde_json::to_value(&finalized.result).unwrap_or(Value::Null),
            is_error: finalized.is_error,
        },
    )
    .await
}

fn create_tool_result_message(finalized: &FinalizedToolCallOutcome) -> ToolResultMessage {
    ToolResultMessage {
        role: ToolResultMessageRole::ToolResult,
        tool_call_id: finalized.tool_call.id.clone(),
        tool_name: finalized.tool_call.name.clone(),
        content: finalized
            .result
            .content
            .iter()
            .cloned()
            .map(tool_result_content)
            .collect(),
        details: Some(finalized.result.details.clone()),
        is_error: finalized.is_error,
        timestamp: now_millis(),
    }
}

async fn emit_tool_result_message(
    tool_result_message: ToolResultMessage,
    emit: &AgentEventSink,
) -> Result<(), AgentCallbackError> {
    let message = AgentMessage::Llm(Message::ToolResult(tool_result_message));
    emit_event(
        emit,
        AgentEvent::MessageStart {
            message: message.clone(),
        },
    )
    .await?;
    emit_event(emit, AgentEvent::MessageEnd { message }).await
}

fn tool_result_content(content: AgentToolResultContent) -> ToolResultContentBlock {
    match content {
        AgentToolResultContent::Text(text) => ToolResultContentBlock::Text(text),
        AgentToolResultContent::Image(image) => ToolResultContentBlock::Image(image),
    }
}

async fn prepare_next_turn(
    config: &AgentLoopConfig,
    message: AssistantMessage,
    tool_results: Vec<ToolResultMessage>,
    context: AgentContext,
    new_messages: Vec<AgentMessage>,
) -> Result<Option<AgentLoopTurnUpdate>, AgentCallbackError> {
    let Some(prepare_next_turn) = config.prepare_next_turn.as_ref() else {
        return Ok(None);
    };
    prepare_next_turn(crate::types::PrepareNextTurnContext {
        message,
        tool_results,
        context,
        new_messages,
    })
    .await
}

async fn should_stop_after_turn(
    config: &AgentLoopConfig,
    message: AssistantMessage,
    tool_results: Vec<ToolResultMessage>,
    context: AgentContext,
    new_messages: Vec<AgentMessage>,
) -> bool {
    let Some(should_stop_after_turn) = &config.should_stop_after_turn else {
        return false;
    };
    should_stop_after_turn(crate::types::ShouldStopAfterTurnContext {
        message,
        tool_results,
        context,
        new_messages,
    })
    .await
}

fn apply_turn_update(
    current_context: &mut AgentContext,
    config: &mut AgentLoopConfig,
    update: AgentLoopTurnUpdate,
) {
    if let Some(context) = update.context {
        *current_context = context;
    }
    if let Some(model) = update.model {
        config.model = model;
    }
    if update.thinking_level.is_some() {
        config.stream_options.reasoning = update.thinking_level.and_then(ai_thinking_level);
    }
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

async fn drain_queue(queue: &Option<crate::types::MessageQueueFn>) -> Vec<AgentMessage> {
    match queue {
        Some(queue) => queue().await,
        None => Vec::new(),
    }
}

fn validate_continuation_context(context: &AgentContext) -> Result<(), AgentLoopError> {
    let Some(last_message) = context.messages.last() else {
        return Err(AgentLoopError::CannotContinueNoMessages);
    };
    if is_assistant_message(last_message) {
        return Err(AgentLoopError::CannotContinueFromAssistant);
    }
    Ok(())
}

fn is_assistant_message(message: &AgentMessage) -> bool {
    matches!(message, AgentMessage::Llm(Message::Assistant(_)))
        || matches!(
            message,
            AgentMessage::Custom(value) if value.get("role").and_then(Value::as_str) == Some("assistant")
        )
}

fn arguments_to_value(arguments: &HashMap<String, Value>) -> Value {
    Value::Object(
        arguments
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    )
}

fn value_to_arguments(value: Value) -> Option<HashMap<String, Value>> {
    let Value::Object(object) = value else {
        return None;
    };
    Some(object.into_iter().collect())
}

async fn emit_event(emit: &AgentEventSink, event: AgentEvent) -> Result<(), AgentCallbackError> {
    emit(event).await
}

fn default_stream_fn() -> StreamFn {
    Arc::new(|model, context, options| {
        Box::pin(
            async move { Ok(zedflow_ai::create_models().stream_simple(model, context, options)) },
        )
    })
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn stream_error_message(
    model: &zedflow_ai::Model,
    message: String,
    aborted: bool,
) -> AssistantMessage {
    AssistantMessage {
        role: zedflow_ai::AssistantMessageRole::Assistant,
        content: vec![AssistantContentBlock::Text(TextContent {
            content_type: TextContentType::Text,
            text: String::new(),
            text_signature: None,
        })],
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: zedflow_ai::Usage::default(),
        stop_reason: if aborted {
            StopReason::Aborted
        } else {
            StopReason::Error
        },
        error_message: Some(message),
        timestamp: now_millis(),
    }
}

/// Builds an assistant event stream containing one terminal error event.
#[must_use]
pub fn error_assistant_stream(
    model: &zedflow_ai::Model,
    message: impl Into<String>,
    aborted: bool,
) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    let error = stream_error_message(model, message.into(), aborted);
    stream.push(AssistantMessageEvent::Error {
        reason: if aborted {
            ErrorStopReason::Aborted
        } else {
            ErrorStopReason::Error
        },
        error,
    });
    stream
}
