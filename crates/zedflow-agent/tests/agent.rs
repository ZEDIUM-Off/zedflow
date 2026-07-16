use std::sync::{Arc, Mutex};

use futures::FutureExt;
use futures::channel::oneshot;
use futures::executor::block_on;
use serde_json::{Value, json};
use zedflow_agent::agent::{Agent, AgentOptions};
use zedflow_agent::types::{
    AgentEvent, AgentMessage, AgentTool, AgentToolResult, AgentToolResultContent,
    AgentToolUpdateCallback, AssistantMessageEventStream, Message, Model, StreamFn, TextContent,
    ThinkingLevel, Tool,
};
use zedflow_ai::types::{
    AbortSignal, AssistantContentBlock, AssistantMessage, AssistantMessageEvent,
    AssistantMessageRole, DoneStopReason, ErrorStopReason, StopReason, TextContentType, ToolCall,
    ToolCallType, Usage, UserMessage, UserMessageContent, UserMessageRole,
};

fn model(id: &str) -> Model {
    Model {
        id: id.to_owned(),
        name: id.to_owned(),
        api: "openai-responses".to_owned(),
        provider: "openai".to_owned(),
        ..Model::default()
    }
}

fn text(value: impl Into<String>) -> TextContent {
    TextContent {
        content_type: TextContentType::Text,
        text: value.into(),
        text_signature: None,
    }
}

fn user(value: impl Into<String>) -> AgentMessage {
    AgentMessage::Llm(Message::User(UserMessage {
        role: UserMessageRole::User,
        content: UserMessageContent::Text(value.into()),
        timestamp: 0,
    }))
}

fn assistant_text(value: &str) -> AssistantMessage {
    assistant(
        vec![AssistantContentBlock::Text(text(value))],
        StopReason::Stop,
    )
}

fn assistant(content: Vec<AssistantContentBlock>, stop_reason: StopReason) -> AssistantMessage {
    AssistantMessage {
        role: AssistantMessageRole::Assistant,
        content,
        api: "openai-responses".to_owned(),
        provider: "openai".to_owned(),
        model: "mock".to_owned(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage::default(),
        stop_reason,
        error_message: None,
        timestamp: 0,
    }
}

fn error_assistant(message: &str, stop_reason: StopReason) -> AssistantMessage {
    let mut assistant = assistant(Vec::new(), stop_reason);
    assistant.error_message = Some(message.to_owned());
    assistant
}

fn tool_call(id: &str, name: &str, args: Value) -> AssistantContentBlock {
    AssistantContentBlock::ToolCall(ToolCall {
        content_type: ToolCallType::ToolCall,
        id: id.to_owned(),
        name: name.to_owned(),
        arguments: args
            .as_object()
            .expect("tool args are object")
            .clone()
            .into_iter()
            .collect(),
        thought_signature: None,
    })
}

fn done_stream(message: AssistantMessage) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    let reason = if message.stop_reason == StopReason::ToolUse {
        DoneStopReason::ToolUse
    } else {
        DoneStopReason::Stop
    };
    stream.push(AssistantMessageEvent::Done { reason, message });
    stream
}

fn stream_from_messages(messages: Vec<AssistantMessage>) -> StreamFn {
    let index = Arc::new(Mutex::new(0usize));
    Arc::new(move |_model, _context, _options| {
        let mut index = index.lock().expect("stream index lock");
        let message = messages
            .get(*index)
            .cloned()
            .unwrap_or_else(|| assistant_text("extra"));
        *index += 1;
        done_stream(message)
    })
}

fn event_type(event: &AgentEvent) -> &'static str {
    match event {
        AgentEvent::AgentStart => "agent_start",
        AgentEvent::AgentEnd { .. } => "agent_end",
        AgentEvent::TurnStart => "turn_start",
        AgentEvent::TurnEnd { .. } => "turn_end",
        AgentEvent::MessageStart { .. } => "message_start",
        AgentEvent::MessageUpdate { .. } => "message_update",
        AgentEvent::MessageEnd { .. } => "message_end",
        AgentEvent::ToolExecutionStart { .. } => "tool_execution_start",
        AgentEvent::ToolExecutionUpdate { .. } => "tool_execution_update",
        AgentEvent::ToolExecutionEnd { .. } => "tool_execution_end",
    }
}

fn role(message: &AgentMessage) -> &'static str {
    match message {
        AgentMessage::Llm(Message::User(_)) => "user",
        AgentMessage::Llm(Message::Assistant(_)) => "assistant",
        AgentMessage::Llm(Message::ToolResult(_)) => "toolResult",
        AgentMessage::Custom(_) => "custom",
    }
}

fn noop_tool(name: &str) -> AgentTool {
    AgentTool {
        tool: Tool {
            name: name.to_owned(),
            description: "noop".to_owned(),
            parameters: json!({ "type": "object" }),
        },
        label: name.to_owned(),
        prepare_arguments: None,
        execute: Some(Arc::new(|_id, _args, _signal, _on_update| {
            Box::pin(async move {
                AgentToolResult {
                    content: vec![AgentToolResultContent::Text(text("ok"))],
                    details: json!({}),
                    terminate: Some(true),
                }
            })
        })),
        execution_mode: None,
    }
}

fn collect_events(agent: &Agent) -> Arc<Mutex<Vec<AgentEvent>>> {
    let events = Arc::new(Mutex::new(Vec::new()));
    let capture = events.clone();
    agent.subscribe(Arc::new(move |event, _signal| {
        let capture = capture.clone();
        Box::pin(async move {
            capture.lock().expect("events lock").push(event);
        })
    }));
    events
}

#[test]
fn creates_default_and_custom_state() {
    let agent = Agent::new(AgentOptions::default());
    let state = agent.state();

    assert_eq!(state.system_prompt, "");
    assert_eq!(state.thinking_level, ThinkingLevel::Off);
    assert!(state.tools.is_empty());
    assert!(state.messages.is_empty());
    assert!(!state.is_streaming);
    assert!(state.streaming_message.is_none());
    assert!(state.pending_tool_calls.is_empty());
    assert!(state.error_message.is_none());

    let custom = Agent::new(AgentOptions {
        initial_state: Some(zedflow_agent::types::AgentState {
            system_prompt: "custom prompt".to_owned(),
            model: model("custom-model"),
            thinking_level: ThinkingLevel::Low,
            tools: Vec::new(),
            messages: Vec::new(),
            is_streaming: false,
            streaming_message: None,
            pending_tool_calls: Default::default(),
            error_message: None,
        }),
        ..AgentOptions::default()
    });

    let state = custom.state();
    assert_eq!(state.system_prompt, "custom prompt");
    assert_eq!(state.model.id, "custom-model");
    assert_eq!(state.thinking_level, ThinkingLevel::Low);
}

#[test]
fn subscribe_does_not_emit_for_state_mutators_and_unsubscribes() {
    let agent = Agent::new(AgentOptions::default());
    let count = Arc::new(Mutex::new(0usize));
    let count_listener = count.clone();
    let unsubscribe = agent.subscribe(Arc::new(move |_event, _signal| {
        let count_listener = count_listener.clone();
        Box::pin(async move {
            *count_listener.lock().expect("count lock") += 1;
        })
    }));

    agent.set_system_prompt("test prompt");
    assert_eq!(*count.lock().expect("count lock"), 0);
    unsubscribe();
    agent.set_system_prompt("another prompt");
    assert_eq!(*count.lock().expect("count lock"), 0);
}

#[test]
fn prompt_emits_full_lifecycle_and_updates_state() {
    let agent = Agent::new(AgentOptions {
        stream_fn: Some(stream_from_messages(vec![assistant_text("ok")])),
        ..AgentOptions::default()
    });
    let events = collect_events(&agent);

    block_on(agent.prompt("hello")).expect("prompt succeeds");

    assert_eq!(
        events
            .lock()
            .expect("events lock")
            .iter()
            .map(event_type)
            .collect::<Vec<_>>(),
        [
            "agent_start",
            "turn_start",
            "message_start",
            "message_end",
            "message_start",
            "message_end",
            "turn_end",
            "agent_end",
        ]
    );
    let state = agent.state();
    assert!(!state.is_streaming);
    assert_eq!(
        state.messages.iter().map(role).collect::<Vec<_>>(),
        ["user", "assistant"]
    );
}

#[test]
fn provider_error_event_emits_lifecycle_and_sets_error_state() {
    let stream_fn: StreamFn = Arc::new(|_model, _context, _options| {
        let stream = AssistantMessageEventStream::new();
        stream.push(AssistantMessageEvent::Error {
            reason: ErrorStopReason::Error,
            error: error_assistant("provider exploded", StopReason::Error),
        });
        stream
    });
    let agent = Agent::new(AgentOptions {
        stream_fn: Some(stream_fn),
        ..AgentOptions::default()
    });
    let events = collect_events(&agent);

    block_on(agent.prompt("hello")).expect("prompt resolves after provider error event");

    assert_eq!(
        events
            .lock()
            .expect("events lock")
            .iter()
            .map(event_type)
            .collect::<Vec<_>>(),
        [
            "agent_start",
            "turn_start",
            "message_start",
            "message_end",
            "message_start",
            "message_end",
            "turn_end",
            "agent_end",
        ]
    );
    assert_eq!(
        agent.state().error_message.as_deref(),
        Some("provider exploded")
    );
}

#[test]
fn awaits_async_subscribers_before_prompt_resolves() {
    let (tx, rx) = oneshot::channel::<()>();
    let rx = Arc::new(Mutex::new(Some(rx)));
    let finished = Arc::new(Mutex::new(false));
    let agent = Agent::new(AgentOptions {
        stream_fn: Some(stream_from_messages(vec![assistant_text("ok")])),
        ..AgentOptions::default()
    });
    let rx_listener = rx.clone();
    let finished_listener = finished.clone();
    agent.subscribe(Arc::new(move |event, _signal| {
        let rx_listener = rx_listener.clone();
        let finished_listener = finished_listener.clone();
        Box::pin(async move {
            if matches!(event, AgentEvent::AgentEnd { .. }) {
                let rx = rx_listener
                    .lock()
                    .expect("rx lock")
                    .take()
                    .expect("barrier");
                let _ = rx.await;
                *finished_listener.lock().expect("finished lock") = true;
            }
        })
    }));

    let mut prompt = Box::pin(agent.prompt("hello"));
    assert!(prompt.as_mut().now_or_never().is_none());
    assert!(agent.state().is_streaming);
    assert!(!*finished.lock().expect("finished lock"));

    tx.send(()).expect("release subscriber");
    block_on(prompt).expect("prompt completes");
    assert!(*finished.lock().expect("finished lock"));
    assert!(!agent.state().is_streaming);
}

#[test]
fn wait_for_idle_waits_for_async_subscribers() {
    let (tx, rx) = oneshot::channel::<()>();
    let rx = Arc::new(Mutex::new(Some(rx)));
    let finished = Arc::new(Mutex::new(false));
    let agent = Agent::new(AgentOptions {
        stream_fn: Some(stream_from_messages(vec![assistant_text("ok")])),
        ..AgentOptions::default()
    });
    let rx_listener = rx.clone();
    let finished_listener = finished.clone();
    let _ = agent.subscribe(Arc::new(move |event, _signal| {
        let rx_listener = rx_listener.clone();
        let finished_listener = finished_listener.clone();
        Box::pin(async move {
            if matches!(event, AgentEvent::AgentEnd { .. }) {
                let rx = rx_listener
                    .lock()
                    .expect("rx lock")
                    .take()
                    .expect("barrier");
                let _ = rx.await;
                *finished_listener.lock().expect("finished lock") = true;
            }
        })
    }));

    let mut prompt = Box::pin(agent.prompt("hello"));
    assert!(prompt.as_mut().now_or_never().is_none());

    let mut idle = Box::pin(agent.wait_for_idle());
    assert!(idle.as_mut().now_or_never().is_none());
    assert!(!*finished.lock().expect("finished lock"));

    tx.send(()).expect("release subscriber");
    block_on(prompt).expect("prompt completes");
    block_on(idle);
    assert!(*finished.lock().expect("finished lock"));
    assert!(!agent.state().is_streaming);
}

#[test]
fn passes_active_abort_signal_to_subscribers() {
    let held_stream = Arc::new(Mutex::new(None::<AssistantMessageEventStream>));
    let held_stream_fn = held_stream.clone();
    let stream_fn: StreamFn = Arc::new(move |_model, _context, _options| {
        let stream = AssistantMessageEventStream::new();
        stream.push(AssistantMessageEvent::Start {
            partial: assistant_text(""),
        });
        *held_stream_fn.lock().expect("held stream lock") = Some(stream.clone());
        stream
    });
    let signal = Arc::new(Mutex::new(None::<AbortSignal>));
    let signal_listener = signal.clone();
    let agent = Agent::new(AgentOptions {
        stream_fn: Some(stream_fn),
        ..AgentOptions::default()
    });
    agent.subscribe(Arc::new(move |event, signal| {
        let signal_listener = signal_listener.clone();
        Box::pin(async move {
            if matches!(event, AgentEvent::AgentStart) {
                *signal_listener.lock().expect("signal lock") = Some(signal);
            }
        })
    }));

    let mut prompt = Box::pin(agent.prompt("hello"));
    assert!(prompt.as_mut().now_or_never().is_none());
    assert!(
        !signal
            .lock()
            .expect("signal lock")
            .as_ref()
            .expect("signal")
            .aborted()
    );

    agent.abort();
    held_stream
        .lock()
        .expect("held stream lock")
        .as_ref()
        .expect("stream")
        .push(AssistantMessageEvent::Error {
            reason: ErrorStopReason::Aborted,
            error: error_assistant("aborted", StopReason::Aborted),
        });
    block_on(prompt).expect("aborted prompt resolves");

    assert!(
        signal
            .lock()
            .expect("signal lock")
            .as_ref()
            .expect("signal")
            .aborted()
    );
}

#[test]
fn ignores_tool_updates_after_tool_execution_settles() {
    let delayed_update = Arc::new(Mutex::new(None::<AgentToolUpdateCallback>));
    let delayed_update_tool = delayed_update.clone();
    let mut tool = noop_tool("delayed_tool");
    tool.execute = Some(Arc::new(move |_id, _args, _signal, on_update| {
        let delayed_update_tool = delayed_update_tool.clone();
        Box::pin(async move {
            if let Some(on_update) = on_update {
                delayed_update_tool
                    .lock()
                    .expect("update lock")
                    .replace(on_update);
            }
            AgentToolResult {
                content: vec![AgentToolResultContent::Text(text("ok"))],
                details: json!({ "status": "done" }),
                terminate: Some(true),
            }
        })
    }));
    let agent = Agent::new(AgentOptions {
        initial_state: Some(zedflow_agent::types::AgentState {
            tools: vec![tool],
            ..default_agent_state()
        }),
        stream_fn: Some(stream_from_messages(vec![assistant(
            vec![tool_call("call-1", "delayed_tool", json!({}))],
            StopReason::ToolUse,
        )])),
        ..AgentOptions::default()
    });
    let events = collect_events(&agent);

    block_on(agent.prompt("run tool")).expect("tool prompt succeeds");
    let event_count = events.lock().expect("events lock").len();
    delayed_update
        .lock()
        .expect("update lock")
        .as_ref()
        .expect("delayed update")(AgentToolResult {
        content: vec![AgentToolResultContent::Text(text("late"))],
        details: json!({ "status": "late" }),
        terminate: None,
    });

    let events = events.lock().expect("events lock");
    assert_eq!(events.len(), event_count);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, AgentEvent::ToolExecutionUpdate { .. }))
            .count(),
        0
    );
}

#[test]
fn forwards_tool_execution_updates_before_tool_settles() {
    let mut tool = noop_tool("progress_tool");
    tool.execute = Some(Arc::new(move |_id, _args, _signal, on_update| {
        Box::pin(async move {
            if let Some(on_update) = on_update {
                on_update(AgentToolResult {
                    content: vec![AgentToolResultContent::Text(text("working"))],
                    details: json!({ "status": "working" }),
                    terminate: None,
                });
            }
            AgentToolResult {
                content: vec![AgentToolResultContent::Text(text("ok"))],
                details: json!({ "status": "done" }),
                terminate: Some(true),
            }
        })
    }));
    let agent = Agent::new(AgentOptions {
        initial_state: Some(zedflow_agent::types::AgentState {
            tools: vec![tool],
            ..default_agent_state()
        }),
        stream_fn: Some(stream_from_messages(vec![assistant(
            vec![tool_call("call-1", "progress_tool", json!({}))],
            StopReason::ToolUse,
        )])),
        ..AgentOptions::default()
    });
    let events = collect_events(&agent);

    block_on(agent.prompt("run tool")).expect("tool prompt succeeds");

    let updates = events
        .lock()
        .expect("events lock")
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolExecutionUpdate {
                tool_call_id,
                tool_name,
                partial_result,
                ..
            } => Some((
                tool_call_id.clone(),
                tool_name.clone(),
                partial_result.clone(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].0, "call-1");
    assert_eq!(updates[0].1, "progress_tool");
    assert_eq!(updates[0].2["details"], json!({ "status": "working" }));
}

#[test]
fn ignores_settled_parallel_tool_update_while_another_tool_runs() {
    let release_slow = Arc::new(Mutex::new(None::<oneshot::Sender<()>>));
    let (tx, rx) = oneshot::channel::<()>();
    *release_slow.lock().expect("release lock") = Some(tx);
    let rx = Arc::new(Mutex::new(Some(rx)));
    let settled_update = Arc::new(Mutex::new(None::<AgentToolUpdateCallback>));

    let mut settled = noop_tool("settled_tool");
    let settled_update_tool = settled_update.clone();
    settled.execute = Some(Arc::new(move |_id, _args, _signal, on_update| {
        let settled_update_tool = settled_update_tool.clone();
        Box::pin(async move {
            if let Some(on_update) = on_update {
                settled_update_tool
                    .lock()
                    .expect("update lock")
                    .replace(on_update);
            }
            AgentToolResult {
                content: vec![AgentToolResultContent::Text(text("done"))],
                details: json!({}),
                terminate: Some(true),
            }
        })
    }));

    let mut slow = noop_tool("slow_tool");
    let rx_tool = rx.clone();
    slow.execute = Some(Arc::new(move |_id, _args, _signal, _on_update| {
        let rx_tool = rx_tool.clone();
        Box::pin(async move {
            let rx = rx_tool
                .lock()
                .expect("rx lock")
                .take()
                .expect("slow receiver");
            let _ = rx.await;
            AgentToolResult {
                content: vec![AgentToolResultContent::Text(text("done"))],
                details: json!({}),
                terminate: Some(true),
            }
        })
    }));

    let agent = Agent::new(AgentOptions {
        initial_state: Some(zedflow_agent::types::AgentState {
            tools: vec![settled, slow],
            ..default_agent_state()
        }),
        stream_fn: Some(stream_from_messages(vec![assistant(
            vec![
                tool_call("call-1", "settled_tool", json!({})),
                tool_call("call-2", "slow_tool", json!({})),
            ],
            StopReason::ToolUse,
        )])),
        ..AgentOptions::default()
    });
    let events = collect_events(&agent);

    let mut prompt = Box::pin(agent.prompt("run tools"));
    assert!(prompt.as_mut().now_or_never().is_none());
    let event_count = events.lock().expect("events lock").len();
    settled_update
        .lock()
        .expect("update lock")
        .as_ref()
        .expect("settled update")(AgentToolResult {
        content: vec![AgentToolResultContent::Text(text("late"))],
        details: json!({}),
        terminate: None,
    });
    assert_eq!(events.lock().expect("events lock").len(), event_count);

    release_slow
        .lock()
        .expect("release lock")
        .take()
        .expect("release sender")
        .send(())
        .expect("release slow");
    block_on(prompt).expect("prompt completes");
    assert_eq!(
        events
            .lock()
            .expect("events lock")
            .iter()
            .filter(|event| matches!(event, AgentEvent::ToolExecutionUpdate { .. }))
            .count(),
        0
    );
}

#[test]
fn state_mutators_queues_and_abort_are_supported() {
    let agent = Agent::new(AgentOptions::default());

    agent.set_system_prompt("custom");
    assert_eq!(agent.state().system_prompt, "custom");
    agent.set_model(model("next-model"));
    assert_eq!(agent.state().model.id, "next-model");
    agent.set_thinking_level(ThinkingLevel::High);
    assert_eq!(agent.state().thinking_level, ThinkingLevel::High);
    agent.set_tools(vec![noop_tool("test")]);
    assert_eq!(agent.state().tools.len(), 1);

    agent.steer(user("steering"));
    agent.follow_up(user("follow up"));
    assert!(agent.has_queued_messages());
    assert_eq!(agent.state().messages.len(), 0);
    agent.abort();
}

#[test]
fn prompt_and_continue_reject_while_streaming() {
    let held_stream = Arc::new(Mutex::new(None::<AssistantMessageEventStream>));
    let held_stream_fn = held_stream.clone();
    let stream_fn: StreamFn = Arc::new(move |_model, _context, _options| {
        let stream = AssistantMessageEventStream::new();
        stream.push(AssistantMessageEvent::Start {
            partial: assistant_text(""),
        });
        *held_stream_fn.lock().expect("held stream lock") = Some(stream.clone());
        stream
    });
    let agent = Agent::new(AgentOptions {
        stream_fn: Some(stream_fn),
        ..AgentOptions::default()
    });
    let mut first_prompt = Box::pin(agent.prompt("first"));
    assert!(first_prompt.as_mut().now_or_never().is_none());
    assert!(agent.state().is_streaming);

    let prompt_error = block_on(agent.prompt("second")).expect_err("second prompt rejects");
    assert_eq!(
        prompt_error.to_string(),
        "Agent is already processing a prompt. Use steer() or followUp() to queue messages, or wait for completion."
    );
    let continue_error = block_on(agent.r#continue()).expect_err("continue rejects");
    assert_eq!(
        continue_error.to_string(),
        "Agent is already processing. Wait for completion before continuing."
    );

    held_stream
        .lock()
        .expect("held stream lock")
        .as_ref()
        .expect("stream")
        .push(AssistantMessageEvent::Error {
            reason: ErrorStopReason::Aborted,
            error: error_assistant("aborted", StopReason::Aborted),
        });
    block_on(first_prompt).expect("first prompt completes");
}

#[test]
fn continue_processes_queued_follow_up_after_assistant_tail() {
    let agent = Agent::new(AgentOptions {
        stream_fn: Some(stream_from_messages(vec![
            assistant_text("initial response"),
            assistant_text("processed"),
        ])),
        ..AgentOptions::default()
    });
    block_on(agent.prompt("initial")).expect("seed prompt succeeds");
    agent.follow_up(user("queued follow-up"));

    block_on(agent.r#continue()).expect("continue succeeds");

    let messages = agent.state().messages;
    assert!(messages.iter().any(|message| match message {
        AgentMessage::Llm(Message::User(message)) =>
            message.content == UserMessageContent::Text("queued follow-up".to_owned()),
        _ => false,
    }));
    assert_eq!(messages.last().map(role), Some("assistant"));
}

#[test]
fn continue_keeps_one_at_a_time_steering_from_assistant_tail() {
    let responses = Arc::new(Mutex::new(0usize));
    let responses_stream = responses.clone();
    let stream_fn: StreamFn = Arc::new(move |_model, _context, _options| {
        let mut responses = responses_stream.lock().expect("responses lock");
        *responses += 1;
        done_stream(assistant_text(&format!("processed {}", responses)))
    });
    let agent = Agent::new(AgentOptions {
        stream_fn: Some(stream_fn),
        ..AgentOptions::default()
    });
    block_on(agent.prompt("initial")).expect("seed prompt succeeds");
    agent.steer(user("steering 1"));
    agent.steer(user("steering 2"));

    block_on(agent.r#continue()).expect("continue succeeds");

    assert_eq!(*responses.lock().expect("responses lock"), 3);
    assert_eq!(
        agent
            .state()
            .messages
            .iter()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(role)
            .collect::<Vec<_>>(),
        ["user", "assistant", "user", "assistant"]
    );
}

#[test]
fn prepare_next_turn_receives_active_signal() {
    let saw_signal = Arc::new(Mutex::new(false));
    let saw_signal_hook = saw_signal.clone();
    let request_count = Arc::new(Mutex::new(0usize));
    let request_count_stream = request_count.clone();
    let stream_fn: StreamFn = Arc::new(move |_model, _context, _options| {
        let mut count = request_count_stream.lock().expect("count lock");
        *count += 1;
        if *count == 1 {
            done_stream(assistant(
                vec![tool_call("tool-1", "noop", json!({}))],
                StopReason::ToolUse,
            ))
        } else {
            done_stream(assistant_text("done"))
        }
    });
    let mut tool = noop_tool("noop");
    tool.execute = Some(Arc::new(|_id, _args, _signal, _on_update| {
        Box::pin(async move {
            AgentToolResult {
                content: vec![AgentToolResultContent::Text(text("ok"))],
                details: json!({}),
                terminate: None,
            }
        })
    }));
    let agent = Agent::new(AgentOptions {
        initial_state: Some(zedflow_agent::types::AgentState {
            tools: vec![tool],
            ..default_agent_state()
        }),
        prepare_next_turn: Some(Arc::new(move |signal| {
            let saw_signal_hook = saw_signal_hook.clone();
            Box::pin(async move {
                *saw_signal_hook.lock().expect("signal lock") = signal.is_some();
                None
            })
        })),
        stream_fn: Some(stream_fn),
        ..AgentOptions::default()
    });

    block_on(agent.prompt("start")).expect("prompt succeeds");

    assert_eq!(*request_count.lock().expect("count lock"), 2);
    assert!(*saw_signal.lock().expect("signal lock"));
}

#[test]
fn forwards_session_id_to_stream_options() {
    let received = Arc::new(Mutex::new(Vec::<Option<String>>::new()));
    let received_stream = received.clone();
    let stream_fn: StreamFn = Arc::new(move |_model, _context, options| {
        received_stream
            .lock()
            .expect("received lock")
            .push(options.and_then(|options| options.stream.session_id.clone()));
        done_stream(assistant_text("ok"))
    });
    let agent = Agent::new(AgentOptions {
        session_id: Some("session-abc".to_owned()),
        stream_fn: Some(stream_fn),
        ..AgentOptions::default()
    });

    block_on(agent.prompt("hello")).expect("first prompt succeeds");

    assert_eq!(
        received.lock().expect("received lock").as_slice(),
        [Some("session-abc".to_owned())]
    );
}

fn default_agent_state() -> zedflow_agent::types::AgentState {
    zedflow_agent::types::AgentState {
        system_prompt: String::new(),
        model: model("mock"),
        thinking_level: ThinkingLevel::Off,
        tools: Vec::new(),
        messages: Vec::new(),
        is_streaming: false,
        streaming_message: None,
        pending_tool_calls: Default::default(),
        error_message: None,
    }
}
