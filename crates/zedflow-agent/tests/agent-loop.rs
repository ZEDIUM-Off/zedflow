use std::sync::{Arc, Mutex};

use futures::StreamExt;
use futures::channel::oneshot;
use futures::executor::block_on;
use serde_json::{Value, json};
use zedflow_agent::agent_loop::{agent_loop, agent_loop_continue};
use zedflow_agent::types::{
    AfterToolCallResult, AgentContext, AgentEvent, AgentLoopConfig, AgentLoopTurnUpdate,
    AgentMessage, AgentTool, AgentToolResult, AgentToolResultContent, AiContext,
    AssistantMessageEventStream, ConvertToLlmFn, Message, Model, SimpleStreamOptions, StreamFn,
    TextContent, ThinkingLevel, Tool, ToolExecutionMode,
};
use zedflow_ai::types::{
    AssistantContentBlock, AssistantMessage, AssistantMessageEvent, AssistantMessageRole,
    DoneStopReason, StopReason, TextContentType, ToolCall, ToolCallType, Usage, UserMessage,
    UserMessageContent, UserMessageRole,
};

fn model() -> Model {
    Model {
        id: "mock".to_owned(),
        name: "mock".to_owned(),
        api: "openai-responses".to_owned(),
        provider: "openai".to_owned(),
        ..Model::default()
    }
}

fn text(text: impl Into<String>) -> TextContent {
    TextContent {
        content_type: TextContentType::Text,
        text: text.into(),
        text_signature: None,
    }
}

fn user(text: impl Into<String>) -> AgentMessage {
    AgentMessage::Llm(Message::User(UserMessage {
        role: UserMessageRole::User,
        content: UserMessageContent::Text(text.into()),
        timestamp: 0,
    }))
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

fn assistant_text(text_value: &str) -> AssistantMessage {
    assistant(
        vec![AssistantContentBlock::Text(text(text_value))],
        StopReason::Stop,
    )
}

fn tool_call(id: &str, name: &str, args: Value) -> AssistantContentBlock {
    let arguments = args.as_object().expect("tool args are object").clone();
    AssistantContentBlock::ToolCall(ToolCall {
        content_type: ToolCallType::ToolCall,
        id: id.to_owned(),
        name: name.to_owned(),
        arguments: arguments.into_iter().collect(),
        thought_signature: None,
    })
}

fn stream_from_messages(messages: Vec<AssistantMessage>) -> StreamFn {
    let index = Arc::new(Mutex::new(0usize));
    Arc::new(
        move |_model: &Model,
              _context: &AiContext,
              _options: Option<&SimpleStreamOptions>|
              -> AssistantMessageEventStream {
            let mut guard = index.lock().expect("stream index lock");
            let message = messages
                .get(*guard)
                .cloned()
                .unwrap_or_else(|| assistant_text("extra"));
            *guard += 1;
            drop(guard);

            let stream = AssistantMessageEventStream::new();
            let reason = if message.stop_reason == StopReason::ToolUse {
                DoneStopReason::ToolUse
            } else {
                DoneStopReason::Stop
            };
            stream.push(AssistantMessageEvent::Done { reason, message });
            stream
        },
    )
}

fn identity_converter() -> ConvertToLlmFn {
    Arc::new(|messages| {
        Box::pin(async move {
            messages
                .into_iter()
                .filter_map(|message| match message {
                    AgentMessage::Llm(message) => Some(message),
                    AgentMessage::Custom(_) => None,
                })
                .collect()
        })
    })
}

fn config(stream_fn: Option<StreamFn>) -> (AgentLoopConfig, Option<StreamFn>) {
    (
        AgentLoopConfig {
            stream_options: SimpleStreamOptions::default(),
            model: model(),
            convert_to_llm: identity_converter(),
            transform_context: None,
            get_api_key: None,
            should_stop_after_turn: None,
            prepare_next_turn: None,
            get_steering_messages: None,
            get_follow_up_messages: None,
            tool_execution: ToolExecutionMode::Parallel,
            before_tool_call: None,
            after_tool_call: None,
        },
        stream_fn,
    )
}

fn context(tools: Vec<AgentTool>) -> AgentContext {
    AgentContext {
        system_prompt: String::new(),
        messages: Vec::new(),
        tools: Some(tools),
    }
}

fn echo_tool(execution_mode: Option<ToolExecutionMode>) -> AgentTool {
    AgentTool {
        tool: Tool {
            name: "echo".to_owned(),
            description: "Echo tool".to_owned(),
            parameters: json!({
                "type": "object",
                "properties": { "value": { "type": "string" } },
                "required": ["value"]
            }),
        },
        label: "Echo".to_owned(),
        prepare_arguments: None,
        execution_mode,
        execute: Some(Arc::new(|_tool_call_id, args, _signal, _on_update| {
            Box::pin(async move {
                let value = args["value"].as_str().unwrap_or_default().to_owned();
                AgentToolResult {
                    content: vec![AgentToolResultContent::Text(text(format!(
                        "echoed: {value}"
                    )))],
                    details: json!({ "value": value }),
                    terminate: None,
                }
            })
        })),
    }
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

fn collect_stream(
    prompts: Vec<AgentMessage>,
    context: AgentContext,
    config: AgentLoopConfig,
    stream_fn: Option<StreamFn>,
) -> (Vec<AgentEvent>, Vec<AgentMessage>) {
    let mut stream = agent_loop(prompts, context, config, None, stream_fn);
    block_on(async {
        let events = stream.by_ref().collect::<Vec<_>>().await;
        let messages = stream.result().await;
        (events, messages)
    })
}

#[test]
fn emits_pi_lifecycle_order_for_plain_prompt() {
    let (config, stream_fn) = config(Some(stream_from_messages(vec![assistant_text("hi")])));

    let (events, messages) =
        collect_stream(vec![user("hello")], context(Vec::new()), config, stream_fn);

    assert_eq!(
        messages.iter().map(role).collect::<Vec<_>>(),
        ["user", "assistant"]
    );
    assert_eq!(
        events.iter().map(event_type).collect::<Vec<_>>(),
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
}

#[test]
fn transform_context_runs_before_convert_to_llm() {
    let converted = Arc::new(Mutex::new(Vec::<String>::new()));
    let converted_capture = converted.clone();
    let (mut config, stream_fn) = config(Some(stream_from_messages(vec![assistant_text("ok")])));
    config.transform_context = Some(Arc::new(|messages, _signal| {
        Box::pin(async move { messages.into_iter().rev().take(2).collect() })
    }));
    config.convert_to_llm = Arc::new(move |messages| {
        let converted_capture = converted_capture.clone();
        Box::pin(async move {
            let roles = messages
                .iter()
                .map(role)
                .map(str::to_owned)
                .collect::<Vec<_>>();
            *converted_capture.lock().expect("converted lock") = roles;
            messages
                .into_iter()
                .filter_map(|message| match message {
                    AgentMessage::Llm(message) => Some(message),
                    AgentMessage::Custom(_) => None,
                })
                .collect()
        })
    });
    let mut ctx = context(Vec::new());
    ctx.messages = vec![
        user("old 1"),
        AgentMessage::Llm(Message::Assistant(assistant_text("a"))),
    ];

    collect_stream(vec![user("new")], ctx, config, stream_fn);

    assert_eq!(
        *converted.lock().expect("converted lock"),
        ["user", "assistant"]
    );
}

#[test]
fn executes_tool_calls_and_preserves_stop_hook_event_order() {
    let tool_use = assistant(
        vec![tool_call("tool-1", "echo", json!({ "value": "hello" }))],
        StopReason::ToolUse,
    );
    let (mut config, stream_fn) = config(Some(stream_from_messages(vec![
        tool_use,
        assistant_text("unused"),
    ])));
    config.should_stop_after_turn = Some(Arc::new(|ctx| {
        Box::pin(async move {
            assert_eq!(
                ctx.tool_results
                    .iter()
                    .map(|r| r.tool_call_id.as_str())
                    .collect::<Vec<_>>(),
                ["tool-1"]
            );
            assert_eq!(
                ctx.context.messages.iter().map(role).collect::<Vec<_>>(),
                ["user", "assistant", "toolResult"]
            );
            true
        })
    }));

    let (events, messages) = collect_stream(
        vec![user("echo")],
        context(vec![echo_tool(None)]),
        config,
        stream_fn,
    );

    assert_eq!(
        messages.iter().map(role).collect::<Vec<_>>(),
        ["user", "assistant", "toolResult"]
    );
    assert_eq!(
        events.iter().map(event_type).collect::<Vec<_>>(),
        [
            "agent_start",
            "turn_start",
            "message_start",
            "message_end",
            "message_start",
            "message_end",
            "tool_execution_start",
            "tool_execution_end",
            "message_start",
            "message_end",
            "turn_end",
            "agent_end",
        ]
    );
}

#[test]
fn prepares_tool_arguments_before_validation_and_execution() {
    let seen = Arc::new(Mutex::new(Vec::<Value>::new()));
    let seen_tool = seen.clone();
    let mut tool = echo_tool(None);
    tool.prepare_arguments = Some(Arc::new(|args| {
        json!({
            "value": format!("{}:{}", args["oldText"].as_str().unwrap_or_default(), args["newText"].as_str().unwrap_or_default())
        })
    }));
    tool.execute = Some(Arc::new(move |_tool_call_id, args, _signal, _on_update| {
        let seen_tool = seen_tool.clone();
        Box::pin(async move {
            seen_tool.lock().expect("seen lock").push(args.clone());
            AgentToolResult {
                content: vec![AgentToolResultContent::Text(text("ok"))],
                details: args,
                terminate: Some(true),
            }
        })
    }));
    let tool_use = assistant(
        vec![tool_call(
            "tool-1",
            "echo",
            json!({ "oldText": "before", "newText": "after" }),
        )],
        StopReason::ToolUse,
    );
    let (config, stream_fn) = config(Some(stream_from_messages(vec![tool_use])));

    collect_stream(vec![user("edit")], context(vec![tool]), config, stream_fn);

    assert_eq!(
        *seen.lock().expect("seen lock"),
        [json!({ "value": "before:after" })]
    );
}

#[test]
fn parallel_tool_end_events_complete_before_source_order_results() {
    let release_first = Arc::new(Mutex::new(None::<oneshot::Sender<()>>));
    let (tx, rx) = oneshot::channel::<()>();
    *release_first.lock().expect("release lock") = Some(tx);
    let first_rx = Arc::new(Mutex::new(Some(rx)));

    let mut tool = echo_tool(Some(ToolExecutionMode::Parallel));
    let first_rx_tool = first_rx.clone();
    let release_first_tool = release_first.clone();
    tool.execute = Some(Arc::new(move |_tool_call_id, args, _signal, _on_update| {
        let first_rx_tool = first_rx_tool.clone();
        let release_first_tool = release_first_tool.clone();
        Box::pin(async move {
            let value = args["value"].as_str().unwrap_or_default().to_owned();
            if value == "first" {
                let rx = first_rx_tool
                    .lock()
                    .expect("rx lock")
                    .take()
                    .expect("first receiver");
                let _ = rx.await;
            } else if let Some(tx) = release_first_tool.lock().expect("release lock").take() {
                let _ = tx.send(());
            }
            AgentToolResult {
                content: vec![AgentToolResultContent::Text(text(format!(
                    "echoed: {value}"
                )))],
                details: json!({ "value": value }),
                terminate: None,
            }
        })
    }));
    let tool_use = assistant(
        vec![
            tool_call("tool-1", "echo", json!({ "value": "first" })),
            tool_call("tool-2", "echo", json!({ "value": "second" })),
        ],
        StopReason::ToolUse,
    );
    let (mut config, stream_fn) = config(Some(stream_from_messages(vec![
        tool_use,
        assistant_text("done"),
    ])));
    config.tool_execution = ToolExecutionMode::Parallel;

    let (events, _messages) =
        collect_stream(vec![user("both")], context(vec![tool]), config, stream_fn);

    let tool_end_ids = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolExecutionEnd { tool_call_id, .. } => Some(tool_call_id.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let tool_result_ids = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageEnd {
                message: AgentMessage::Llm(Message::ToolResult(result)),
            } => Some(result.tool_call_id.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(tool_end_ids, ["tool-2", "tool-1"]);
    assert_eq!(tool_result_ids, ["tool-1", "tool-2"]);
}

#[test]
fn steering_messages_are_injected_after_tool_batch() {
    let tool_use = assistant(
        vec![
            tool_call("tool-1", "echo", json!({ "value": "first" })),
            tool_call("tool-2", "echo", json!({ "value": "second" })),
        ],
        StopReason::ToolUse,
    );
    let saw_interrupt = Arc::new(Mutex::new(false));
    let saw_interrupt_stream = saw_interrupt.clone();
    let call_index = Arc::new(Mutex::new(0usize));
    let call_index_stream = call_index.clone();
    let stream_fn: StreamFn = Arc::new(move |_model, ctx, _options| {
        let mut index = call_index_stream.lock().expect("call lock");
        if *index == 1 {
            *saw_interrupt_stream.lock().expect("saw lock") =
                ctx.messages.iter().any(|message| match message {
                    Message::User(message) => {
                        message.content == UserMessageContent::Text("interrupt".to_owned())
                    }
                    _ => false,
                });
        }
        let message = if *index == 0 {
            tool_use.clone()
        } else {
            assistant_text("done")
        };
        *index += 1;
        drop(index);
        let stream = AssistantMessageEventStream::new();
        let reason = if message.stop_reason == StopReason::ToolUse {
            DoneStopReason::ToolUse
        } else {
            DoneStopReason::Stop
        };
        stream.push(AssistantMessageEvent::Done { reason, message });
        stream
    });
    let polls = Arc::new(Mutex::new(0usize));
    let polls_config = polls.clone();
    let (mut config, _) = config(Some(stream_fn.clone()));
    config.tool_execution = ToolExecutionMode::Sequential;
    config.get_steering_messages = Some(Arc::new(move || {
        let polls_config = polls_config.clone();
        Box::pin(async move {
            let mut polls = polls_config.lock().expect("polls lock");
            *polls += 1;
            if *polls == 2 {
                vec![user("interrupt")]
            } else {
                Vec::new()
            }
        })
    }));

    let (events, _messages) = collect_stream(
        vec![user("start")],
        context(vec![echo_tool(None)]),
        config,
        Some(stream_fn),
    );

    let message_starts = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::MessageStart {
                message: AgentMessage::Llm(Message::ToolResult(result)),
            } => Some(format!("tool:{}", result.tool_call_id)),
            AgentEvent::MessageStart {
                message: AgentMessage::Llm(Message::User(message)),
            } => match &message.content {
                UserMessageContent::Text(text) => Some(text.clone()),
                UserMessageContent::Blocks(_) => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        message_starts,
        ["start", "tool:tool-1", "tool:tool-2", "interrupt"]
    );
    assert!(*saw_interrupt.lock().expect("saw lock"));
}

#[test]
fn sequential_tool_override_forces_source_order_execution() {
    let order = Arc::new(Mutex::new(Vec::<String>::new()));
    let mut slow = echo_tool(Some(ToolExecutionMode::Sequential));
    slow.tool.name = "slow".to_owned();
    let order_slow = order.clone();
    slow.execute = Some(Arc::new(move |_id, args, _signal, _on_update| {
        let order_slow = order_slow.clone();
        Box::pin(async move {
            order_slow.lock().expect("order lock").push(format!(
                "slow:{}",
                args["value"].as_str().unwrap_or_default()
            ));
            AgentToolResult {
                content: vec![AgentToolResultContent::Text(text("slow"))],
                details: json!({}),
                terminate: None,
            }
        })
    }));
    let mut fast = echo_tool(None);
    fast.tool.name = "fast".to_owned();
    let order_fast = order.clone();
    fast.execute = Some(Arc::new(move |_id, args, _signal, _on_update| {
        let order_fast = order_fast.clone();
        Box::pin(async move {
            order_fast.lock().expect("order lock").push(format!(
                "fast:{}",
                args["value"].as_str().unwrap_or_default()
            ));
            AgentToolResult {
                content: vec![AgentToolResultContent::Text(text("fast"))],
                details: json!({}),
                terminate: None,
            }
        })
    }));
    let tool_use = assistant(
        vec![
            tool_call("tool-1", "slow", json!({ "value": "a" })),
            tool_call("tool-2", "fast", json!({ "value": "b" })),
        ],
        StopReason::ToolUse,
    );
    let (config, stream_fn) = config(Some(stream_from_messages(vec![
        tool_use,
        assistant_text("done"),
    ])));

    collect_stream(
        vec![user("both")],
        context(vec![slow, fast]),
        config,
        stream_fn,
    );

    assert_eq!(*order.lock().expect("order lock"), ["slow:a", "fast:b"]);
}

#[test]
fn prepare_next_turn_snapshot_is_used_before_continuing() {
    let second_prompt = Arc::new(Mutex::new(String::new()));
    let second_prompt_stream = second_prompt.clone();
    let call_index = Arc::new(Mutex::new(0usize));
    let call_index_stream = call_index.clone();
    let stream_fn: StreamFn = Arc::new(move |_model, ctx, _options| {
        let mut index = call_index_stream.lock().expect("call lock");
        if *index == 1 {
            *second_prompt_stream.lock().expect("prompt lock") =
                ctx.system_prompt.clone().unwrap_or_default();
        }
        let message = if *index == 0 {
            assistant(
                vec![tool_call("tool-1", "echo", json!({ "value": "hello" }))],
                StopReason::ToolUse,
            )
        } else {
            assistant_text("done")
        };
        *index += 1;
        drop(index);
        let stream = AssistantMessageEventStream::new();
        let reason = if message.stop_reason == StopReason::ToolUse {
            DoneStopReason::ToolUse
        } else {
            DoneStopReason::Stop
        };
        stream.push(AssistantMessageEvent::Done { reason, message });
        stream
    });
    let (mut config, _) = config(Some(stream_fn.clone()));
    config.prepare_next_turn = Some(Arc::new(|turn| {
        Box::pin(async move {
            Some(AgentLoopTurnUpdate {
                context: Some(AgentContext {
                    system_prompt: "second prompt".to_owned(),
                    messages: turn.context.messages,
                    tools: turn.context.tools,
                }),
                model: None,
                thinking_level: Some(ThinkingLevel::Off),
            })
        })
    }));

    collect_stream(
        vec![user("start")],
        context(vec![echo_tool(None)]),
        config,
        Some(stream_fn),
    );

    assert_eq!(
        &*second_prompt.lock().expect("prompt lock"),
        "second prompt"
    );
}

#[test]
fn terminate_flags_and_after_tool_call_stop_without_next_model_call() {
    let mut tool = echo_tool(None);
    tool.execute = Some(Arc::new(|_id, _args, _signal, _on_update| {
        Box::pin(async move {
            AgentToolResult {
                content: vec![AgentToolResultContent::Text(text("ok"))],
                details: json!({}),
                terminate: None,
            }
        })
    }));
    let tool_use = assistant(
        vec![tool_call("tool-1", "echo", json!({ "value": "hello" }))],
        StopReason::ToolUse,
    );
    let calls = Arc::new(Mutex::new(0usize));
    let calls_stream = calls.clone();
    let stream_fn: StreamFn = Arc::new(move |_model, _ctx, _options| {
        *calls_stream.lock().expect("calls lock") += 1;
        let stream = AssistantMessageEventStream::new();
        stream.push(AssistantMessageEvent::Done {
            reason: DoneStopReason::ToolUse,
            message: tool_use.clone(),
        });
        stream
    });
    let (mut config, _) = config(Some(stream_fn.clone()));
    config.after_tool_call = Some(Arc::new(|_ctx, _signal| {
        Box::pin(async move {
            Some(AfterToolCallResult {
                content: None,
                details: None,
                is_error: None,
                terminate: Some(true),
            })
        })
    }));

    collect_stream(
        vec![user("start")],
        context(vec![tool]),
        config,
        Some(stream_fn),
    );

    assert_eq!(*calls.lock().expect("calls lock"), 1);
}

#[test]
fn continuation_errors_and_events_match_pi_contract() {
    let (config, _) = config(None);
    let empty = AgentContext {
        system_prompt: String::new(),
        messages: Vec::new(),
        tools: Some(Vec::new()),
    };
    match agent_loop_continue(empty, config.clone(), None, None) {
        Err(error) => assert_eq!(error.to_string(), "Cannot continue: no messages in context"),
        Ok(_) => panic!("empty continuation should fail"),
    }

    let ctx = AgentContext {
        system_prompt: String::new(),
        messages: vec![user("hello")],
        tools: Some(Vec::new()),
    };
    let mut stream = agent_loop_continue(
        ctx,
        config,
        None,
        Some(stream_from_messages(vec![assistant_text("ok")])),
    )
    .expect("continuation starts");
    let (events, messages) = block_on(async {
        let events = stream.by_ref().collect::<Vec<_>>().await;
        let messages = stream.result().await;
        (events, messages)
    });

    assert_eq!(messages.iter().map(role).collect::<Vec<_>>(), ["assistant"]);
    assert_eq!(
        events.iter().map(event_type).collect::<Vec<_>>(),
        [
            "agent_start",
            "turn_start",
            "message_start",
            "message_end",
            "turn_end",
            "agent_end"
        ]
    );
}
