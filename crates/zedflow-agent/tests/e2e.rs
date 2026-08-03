#[path = "scratch/simple.rs"]
mod scratch_simple;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use serde_json::{Value, json};
use zedflow_agent::{
    Agent, AgentEvent, AgentEventListener, AgentMessage, AgentOptions, AgentState, AgentTool,
    AgentToolResult, AgentToolResultContent, StreamFn, ThinkingLevel,
};
use zedflow_ai::{
    AssistantContentBlock, AssistantMessage, AssistantMessageRole, Context as AiContext, Message,
    Model, StopReason, TextContent, TextContentType, Tool, ToolCall, ToolCallType,
    ToolResultContentBlock, ToolResultMessage, ToolResultMessageRole, Usage, UserContentBlock,
    UserMessage, UserMessageContent, UserMessageRole,
    providers::faux::{
        DEFAULT_MODEL_ID, FauxModelDefinition, FauxResponseStep, FauxTokenSize,
        RegisterFauxProviderOptions, faux_assistant_content_message, faux_assistant_message,
        faux_provider, faux_text, faux_thinking, faux_tool_call,
    },
};

fn agent_state(model: Model, tools: Vec<AgentTool>, messages: Vec<AgentMessage>) -> AgentState {
    AgentState {
        system_prompt: "You are a helpful assistant. Keep your responses concise.".to_string(),
        model,
        thinking_level: ThinkingLevel::Off,
        tools,
        messages,
        is_streaming: false,
        streaming_message: None,
        pending_tool_calls: HashSet::new(),
        error_message: None,
    }
}

fn faux_agent_with_options(
    responses: Vec<FauxResponseStep>,
    options: RegisterFauxProviderOptions,
    tools: Vec<AgentTool>,
    messages: Vec<AgentMessage>,
    seen_contexts: Option<Arc<Mutex<Vec<AiContext>>>>,
) -> Agent {
    let handle = faux_provider(options);
    handle.set_responses(responses);

    let provider = handle.provider.clone();
    let model = provider
        .get_models()
        .into_iter()
        .find(|model| model.id == DEFAULT_MODEL_ID)
        .or_else(|| provider.get_models().into_iter().next())
        .expect("faux provider should expose a model");
    let stream_fn: StreamFn = Arc::new(
        move |model: &Model,
              context: &AiContext,
              options: Option<&zedflow_ai::SimpleStreamOptions>| {
            if let Some(seen_contexts) = &seen_contexts {
                seen_contexts
                    .lock()
                    .expect("context capture lock poisoned")
                    .push(context.clone());
            }
            let provider = provider.clone();
            let model = model.clone();
            let context = context.clone();
            let options = options.cloned();
            Box::pin(async move { Ok(provider.stream_simple(&model, &context, options.as_ref())) })
        },
    );

    Agent::new(AgentOptions {
        initial_state: Some(agent_state(model, tools, messages)),
        stream_fn: Some(stream_fn),
        ..AgentOptions::default()
    })
}

fn faux_agent(responses: Vec<FauxResponseStep>, tools: Vec<AgentTool>) -> Agent {
    faux_agent_with_options(
        responses,
        RegisterFauxProviderOptions::default(),
        tools,
        Vec::new(),
        None,
    )
}

fn text_from_agent_message(message: &AgentMessage) -> String {
    match message {
        AgentMessage::Llm(Message::Assistant(message)) => message
            .content
            .iter()
            .filter_map(|block| match block {
                AssistantContentBlock::Text(text) => Some(text.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        AgentMessage::Llm(Message::ToolResult(message)) => message
            .content
            .iter()
            .filter_map(|block| match block {
                ToolResultContentBlock::Text(text) => Some(text.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn calculate_tool() -> AgentTool {
    AgentTool {
        tool: Tool {
            name: "calculate".to_string(),
            description: "Evaluates the fixed arithmetic expressions used by the e2e port."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "expression": { "type": "string" }
                },
                "required": ["expression"],
                "additionalProperties": false
            }),
        },
        label: "Calculate".to_string(),
        prepare_arguments: None,
        execute: Arc::new(|_, args: Value, _, _| {
            Box::pin(async move {
                let expression = args
                    .get("expression")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let answer = match expression {
                    "123 * 456" => "56088",
                    "5 + 3" => "8",
                    _ => "unsupported expression",
                };
                Ok(AgentToolResult {
                    content: vec![AgentToolResultContent::Text(TextContent {
                        content_type: TextContentType::Text,
                        text: format!("{expression} = {answer}"),
                        text_signature: None,
                    })],
                    details: json!({}),
                    terminate: None,
                })
            })
        }),
        execution_mode: None,
    }
}

fn tool_call_response() -> FauxResponseStep {
    let mut response = faux_assistant_content_message(vec![
        faux_text("Let me calculate that."),
        faux_tool_call("calculate", json!({ "expression": "123 * 456" })),
    ]);
    response.stop_reason = StopReason::ToolUse;
    FauxResponseStep::Message(response)
}

fn assistant_message(
    model: &Model,
    content: Vec<AssistantContentBlock>,
    stop_reason: StopReason,
) -> AssistantMessage {
    AssistantMessage {
        role: AssistantMessageRole::Assistant,
        content,
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage::default(),
        stop_reason,
        error_message: None,
        timestamp: 1,
    }
}

fn user_message(text: &str) -> AgentMessage {
    AgentMessage::Llm(Message::User(UserMessage {
        role: UserMessageRole::User,
        content: UserMessageContent::Blocks(vec![UserContentBlock::Text(TextContent {
            content_type: TextContentType::Text,
            text: text.to_string(),
            text_signature: None,
        })]),
        timestamp: 1,
    }))
}

fn tool_result_message() -> AgentMessage {
    AgentMessage::Llm(Message::ToolResult(ToolResultMessage {
        role: ToolResultMessageRole::ToolResult,
        tool_call_id: "calc-1".to_string(),
        tool_name: "calculate".to_string(),
        content: vec![ToolResultContentBlock::Text(TextContent {
            content_type: TextContentType::Text,
            text: "5 + 3 = 8".to_string(),
            text_signature: None,
        })],
        details: Some(json!({})),
        is_error: false,
        timestamp: 1,
    }))
}

fn event_name(event: &AgentEvent) -> &'static str {
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

#[test]
fn faux_provider_handles_basic_text_prompt() {
    let agent = faux_agent(
        vec![FauxResponseStep::Message(faux_assistant_message("4"))],
        Vec::new(),
    );

    block_on(agent.prompt("What is 2+2? Answer with just the number.")).unwrap();

    let state = agent.state();
    assert!(!state.is_streaming);
    assert_eq!(state.messages.len(), 2);
    assert!(matches!(
        state.messages[0],
        AgentMessage::Llm(Message::User(_))
    ));
    assert!(matches!(
        state.messages[1],
        AgentMessage::Llm(Message::Assistant(_))
    ));
    assert!(text_from_agent_message(&state.messages[1]).contains('4'));
}

#[test]
fn faux_provider_executes_tools_and_tracks_pending_tool_calls() {
    let tool_call_response = tool_call_response();
    let tool_call_id = match &tool_call_response {
        FauxResponseStep::Message(message) => message
            .content
            .iter()
            .find_map(|block| match block {
                AssistantContentBlock::ToolCall(call) => Some(call.id.clone()),
                _ => None,
            })
            .expect("faux response should contain a tool call"),
        _ => unreachable!("tool_call_response always returns a message"),
    };
    let agent = Arc::new(faux_agent(
        vec![
            tool_call_response,
            FauxResponseStep::Message(faux_assistant_message("The result is 56088.")),
        ],
        vec![calculate_tool()],
    ));
    let pending_during_events = Arc::new(Mutex::new(Vec::<(&'static str, Vec<String>)>::new()));
    let listener_agent = Arc::clone(&agent);
    let listener_pending = Arc::clone(&pending_during_events);
    let listener: AgentEventListener = Arc::new(move |event, _| {
        if matches!(
            event,
            AgentEvent::ToolExecutionStart { .. } | AgentEvent::ToolExecutionEnd { .. }
        ) {
            let mut ids = listener_agent
                .state()
                .pending_tool_calls
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            ids.sort();
            listener_pending
                .lock()
                .expect("pending capture lock poisoned")
                .push((event_name(&event), ids));
        }
        Box::pin(async { Ok(()) })
    });
    let _unsubscribe = agent.subscribe(listener);

    block_on(agent.prompt("Calculate 123 * 456 using the calculator tool.")).unwrap();

    let state = agent.state();
    assert!(!state.is_streaming);
    assert!(state.messages.len() >= 4);
    let tool_result = state
        .messages
        .iter()
        .find(|message| matches!(message, AgentMessage::Llm(Message::ToolResult(_))))
        .expect("tool result message should be recorded");
    assert!(text_from_agent_message(tool_result).contains("123 * 456 = 56088"));
    assert!(text_from_agent_message(state.messages.last().unwrap()).contains("56088"));
    assert!(state.pending_tool_calls.is_empty());
    assert_eq!(
        *pending_during_events
            .lock()
            .expect("pending capture lock poisoned"),
        vec![
            ("tool_execution_start", vec![tool_call_id]),
            ("tool_execution_end", Vec::new()),
        ]
    );
}

#[test]
fn faux_provider_emits_lifecycle_updates_while_streaming() {
    let agent = faux_agent(
        vec![FauxResponseStep::Message(faux_assistant_message(
            "1 2 3 4 5",
        ))],
        Vec::new(),
    );
    let events = Arc::new(Mutex::new(Vec::<&'static str>::new()));
    let captured_events = Arc::clone(&events);
    let listener: AgentEventListener = Arc::new(move |event, _| {
        captured_events
            .lock()
            .expect("event capture lock poisoned")
            .push(event_name(&event));
        Box::pin(async { Ok(()) })
    });
    let _unsubscribe = agent.subscribe(listener);

    block_on(agent.prompt("Count from 1 to 5.")).unwrap();

    let events = events.lock().expect("event capture lock poisoned");
    for expected in [
        "agent_start",
        "turn_start",
        "message_start",
        "message_update",
        "message_end",
        "turn_end",
        "agent_end",
    ] {
        assert!(events.contains(&expected), "missing event {expected}");
    }
    assert!(
        events.iter().position(|event| *event == "agent_start")
            < events.iter().position(|event| *event == "message_start")
    );
    assert!(
        events.iter().position(|event| *event == "message_start")
            < events.iter().position(|event| *event == "message_end")
    );
    assert!(
        events.iter().position(|event| *event == "message_end")
            < events.iter().rposition(|event| *event == "agent_end")
    );
    let state = agent.state();
    assert!(!state.is_streaming);
    assert_eq!(state.messages.len(), 2);
}

#[test]
fn faux_provider_maintains_context_across_multiple_turns() {
    let seen_contexts = Arc::new(Mutex::new(Vec::new()));
    let agent = faux_agent_with_options(
        vec![
            FauxResponseStep::Message(faux_assistant_message("Nice to meet you, Alice.")),
            FauxResponseStep::Message(faux_assistant_message("Your name is Alice.")),
        ],
        RegisterFauxProviderOptions::default(),
        Vec::new(),
        Vec::new(),
        Some(Arc::clone(&seen_contexts)),
    );

    block_on(agent.prompt("My name is Alice.")).unwrap();
    assert_eq!(agent.state().messages.len(), 2);

    block_on(agent.prompt("What is my name?")).unwrap();
    let state = agent.state();
    assert_eq!(state.messages.len(), 4);
    assert!(text_from_agent_message(&state.messages[3]).contains("Alice"));

    let seen_contexts = seen_contexts.lock().expect("context capture lock poisoned");
    assert_eq!(seen_contexts.len(), 2);
    assert!(
        seen_contexts[1]
            .messages
            .iter()
            .any(|message| match message {
                Message::User(user) => match &user.content {
                    UserMessageContent::Text(text) => text.contains("Alice"),
                    UserMessageContent::Blocks(blocks) => blocks.iter().any(|block| match block {
                        UserContentBlock::Text(text) => text.text.contains("Alice"),
                        UserContentBlock::Image(_) => false,
                    }),
                },
                _ => false,
            })
    );
}

#[test]
fn faux_provider_preserves_thinking_content_blocks() {
    let agent = faux_agent_with_options(
        vec![FauxResponseStep::Message(faux_assistant_content_message(
            vec![faux_thinking("step by step"), faux_text("4")],
        ))],
        RegisterFauxProviderOptions {
            models: vec![FauxModelDefinition {
                id: DEFAULT_MODEL_ID.to_string(),
                reasoning: true,
                ..FauxModelDefinition::default()
            }],
            ..RegisterFauxProviderOptions::default()
        },
        Vec::new(),
        Vec::new(),
        None,
    );
    agent.set_thinking_level(ThinkingLevel::Low);

    block_on(agent.prompt("What is 2+2?")).unwrap();

    let state = agent.state();
    let AgentMessage::Llm(Message::Assistant(message)) = &state.messages[1] else {
        panic!("expected assistant message");
    };
    assert_eq!(
        message.content,
        vec![
            AssistantContentBlock::Thinking(zedflow_ai::ThinkingContent {
                content_type: zedflow_ai::ThinkingContentType::Thinking,
                thinking: "step by step".to_string(),
                thinking_signature: None,
                redacted: None,
            }),
            AssistantContentBlock::Text(TextContent {
                content_type: TextContentType::Text,
                text: "4".to_string(),
                text_signature: None,
            }),
        ]
    );
}

#[test]
fn continue_rejects_empty_context() {
    let agent = faux_agent(Vec::new(), Vec::new());

    let error = block_on(agent.r#continue()).expect_err("empty context should fail");

    assert_eq!(error.to_string(), "No messages to continue from");
}

#[test]
fn continue_rejects_assistant_tail_without_queued_messages() {
    let agent = faux_agent_with_options(
        Vec::new(),
        RegisterFauxProviderOptions::default(),
        Vec::new(),
        Vec::new(),
        None,
    );
    let model = agent.state().model;
    let agent = faux_agent_with_options(
        Vec::new(),
        RegisterFauxProviderOptions::default(),
        Vec::new(),
        vec![AgentMessage::Llm(Message::Assistant(assistant_message(
            &model,
            vec![AssistantContentBlock::Text(TextContent {
                content_type: TextContentType::Text,
                text: "Hello".to_string(),
                text_signature: None,
            })],
            StopReason::Stop,
        )))],
        None,
    );

    let error = block_on(agent.r#continue()).expect_err("assistant tail should fail");

    assert_eq!(
        error.to_string(),
        "Cannot continue from message role: assistant"
    );
}

#[test]
fn continue_from_user_message_gets_response() {
    let agent = faux_agent_with_options(
        vec![FauxResponseStep::Message(faux_assistant_message(
            "HELLO WORLD",
        ))],
        RegisterFauxProviderOptions::default(),
        Vec::new(),
        vec![user_message("Say exactly: HELLO WORLD")],
        None,
    );

    block_on(agent.r#continue()).unwrap();

    let state = agent.state();
    assert!(!state.is_streaming);
    assert_eq!(state.messages.len(), 2);
    assert!(matches!(
        state.messages[0],
        AgentMessage::Llm(Message::User(_))
    ));
    assert!(matches!(
        state.messages[1],
        AgentMessage::Llm(Message::Assistant(_))
    ));
    assert!(
        text_from_agent_message(&state.messages[1])
            .to_uppercase()
            .contains("HELLO WORLD")
    );
}

#[test]
fn continue_from_tool_result_processes_result() {
    let base_agent = faux_agent(Vec::new(), vec![calculate_tool()]);
    let model = base_agent.state().model;
    let mut args = HashMap::new();
    args.insert("expression".to_string(), json!("5 + 3"));
    let agent = faux_agent_with_options(
        vec![FauxResponseStep::Message(faux_assistant_message(
            "The answer is 8.",
        ))],
        RegisterFauxProviderOptions::default(),
        vec![calculate_tool()],
        vec![
            user_message("What is 5 + 3?"),
            AgentMessage::Llm(Message::Assistant(assistant_message(
                &model,
                vec![
                    AssistantContentBlock::Text(TextContent {
                        content_type: TextContentType::Text,
                        text: "Let me calculate that.".to_string(),
                        text_signature: None,
                    }),
                    AssistantContentBlock::ToolCall(ToolCall {
                        content_type: ToolCallType::ToolCall,
                        id: "calc-1".to_string(),
                        name: "calculate".to_string(),
                        arguments: args,
                        thought_signature: None,
                    }),
                ],
                StopReason::ToolUse,
            ))),
            tool_result_message(),
        ],
        None,
    );

    block_on(agent.r#continue()).unwrap();

    let state = agent.state();
    assert!(!state.is_streaming);
    assert!(state.messages.len() >= 4);
    assert!(matches!(
        state.messages.last(),
        Some(AgentMessage::Llm(Message::Assistant(_)))
    ));
    assert!(text_from_agent_message(state.messages.last().unwrap()).contains('8'));
}

#[test]
fn abort_during_token_paced_streaming_records_an_aborted_message() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("runtime");
    runtime.block_on(async {
        let agent = faux_agent_with_options(
            vec![FauxResponseStep::Message(faux_assistant_message(
                "one two three four five six seven eight",
            ))],
            RegisterFauxProviderOptions {
                tokens_per_second: Some(20.0),
                token_size: FauxTokenSize {
                    min: Some(1),
                    max: Some(1),
                },
                ..RegisterFauxProviderOptions::default()
            },
            Vec::new(),
            Vec::new(),
            None,
        );

        let prompt = agent.prompt("Count slowly.");
        tokio::pin!(prompt);
        tokio::select! {
            result = &mut prompt => result.expect("prompt should finish"),
            _ = tokio::time::sleep(std::time::Duration::from_millis(30)) => {
                agent.abort();
                prompt.await.expect("aborted prompt should settle");
            }
        }

        let state = agent.state();
        assert!(!state.is_streaming);
        let AgentMessage::Llm(Message::Assistant(message)) =
            state.messages.last().expect("assistant message")
        else {
            panic!("expected assistant message");
        };
        assert_eq!(message.stop_reason, StopReason::Aborted);
        assert_eq!(state.error_message, message.error_message);
    });
}
