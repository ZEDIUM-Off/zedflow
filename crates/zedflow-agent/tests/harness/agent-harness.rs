use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::pin;
use std::sync::{Arc, Mutex};
use std::task::{Context as TaskContext, Poll};
use std::thread;

use futures::task::noop_waker_ref;
use serde_json::{Value, json};
use zedflow_agent::harness::agent_harness::{
    AgentHarness, AgentHarnessError, AgentHarnessHook, AgentHarnessHookResult,
    AgentHarnessSubscriber,
};
use zedflow_agent::harness::env::nodejs::NodeExecutionEnv;
use zedflow_agent::harness::session::memory_storage::InMemorySessionStorage;
use zedflow_agent::harness::session::session::Session;
use zedflow_agent::harness::types::{
    AgentHarnessEvent, AgentHarnessOptions, AgentHarnessOwnEvent, AgentHarnessResources,
    BeforeAgentStartResult, CompactResult, ExecutionEnv, PromptTemplate, Session as SessionTrait,
    Skill, SystemPrompt,
};
use zedflow_agent::types::{
    AgentMessage, AgentTool, AgentToolResult, AgentToolResultContent, QueueMode, ThinkingLevel,
};
use zedflow_ai::{
    AssistantContentBlock, AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream,
    AssistantMessageRole, Context, DoneStopReason, Message, Model, ModelInput, Models, ProviderApi,
    ProviderAuth, ProviderStreams, SimpleStreamOptions, StopReason, TextContent, TextContentType,
    Tool, ToolCall, ToolCallType, Usage, UsageCost, UserContentBlock, UserMessage,
    UserMessageContent, UserMessageRole, create_models, create_provider,
};

#[derive(Debug, Clone, Default)]
struct RequestSnapshot {
    system_prompt: Option<String>,
    user_texts: Vec<String>,
    tool_names: Vec<String>,
}

fn run<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = TaskContext::from_waker(noop_waker_ref());
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => thread::yield_now(),
        }
    }
}

fn hook<F>(handler: F) -> AgentHarnessHook
where
    F: Fn(AgentHarnessOwnEvent) -> Option<AgentHarnessHookResult> + Send + Sync + 'static,
{
    Arc::new(move |event| {
        let result = handler(event);
        Box::pin(async move { result })
    })
}

fn subscriber<F>(handler: F) -> AgentHarnessSubscriber
where
    F: Fn(AgentHarnessEvent) + Send + Sync + 'static,
{
    Arc::new(move |event| {
        handler(event);
        Box::pin(async move { Ok::<(), AgentHarnessError>(()) })
    })
}

fn memory_session() -> Arc<Session<InMemorySessionStorage>> {
    Arc::new(Session::new(InMemorySessionStorage::default()))
}

fn env() -> Arc<dyn ExecutionEnv> {
    Arc::new(NodeExecutionEnv::with_cwd(
        std::env::current_dir()
            .expect("current dir")
            .to_string_lossy()
            .into_owned(),
    ))
}

fn models_with_responses(
    responses: Vec<AssistantMessage>,
    snapshots: Arc<Mutex<Vec<RequestSnapshot>>>,
) -> (Models, Model) {
    let model = test_model("model-1", true);
    let queue = Arc::new(Mutex::new(VecDeque::from(responses)));
    let stream_simple = Arc::new({
        let queue = Arc::clone(&queue);
        let snapshots = Arc::clone(&snapshots);
        move |model: &Model, context: &Context, _options: Option<&SimpleStreamOptions>| {
            snapshots.lock().expect("snapshots").push(RequestSnapshot {
                system_prompt: context.system_prompt.clone(),
                user_texts: user_texts(context),
                tool_names: context
                    .tools
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|tool| tool.name)
                    .collect(),
            });
            let message = queue
                .lock()
                .expect("responses")
                .pop_front()
                .unwrap_or_else(|| assistant_text(model, "missing response"));
            done_stream(message)
        }
    });
    let provider = create_provider(zedflow_ai::CreateProviderOptions {
        id: model.provider.clone(),
        name: Some("Test".into()),
        base_url: None,
        headers: None,
        auth: ProviderAuth::default(),
        models: vec![model.clone()],
        refresh_models: None,
        api: ProviderApi::Single(ProviderStreams {
            stream: Arc::new(|model, _context, _options| done_stream(assistant_text(model, "ok"))),
            stream_simple,
        }),
    });
    let mut models = create_models();
    models.set_provider(provider);
    (models, model)
}

fn test_model(id: &str, reasoning: bool) -> Model {
    Model {
        provider: "test-provider".into(),
        id: id.into(),
        name: id.into(),
        api: "test-api".into(),
        base_url: "http://localhost".into(),
        reasoning,
        input: vec![ModelInput::Text],
        cost: zedflow_ai::ModelCost::default(),
        context_window: 128_000,
        max_tokens: 16_384,
        ..Model::default()
    }
}

fn harness(
    models: Models,
    model: Model,
    session: Arc<Session<InMemorySessionStorage>>,
) -> AgentHarness {
    AgentHarness::new(AgentHarnessOptions {
        env: env(),
        session: session as Arc<dyn SessionTrait>,
        models,
        tools: None,
        resources: None,
        system_prompt: None,
        stream_options: None,
        model,
        thinking_level: None,
        active_tool_names: None,
        steering_mode: None,
        follow_up_mode: None,
    })
    .expect("harness")
}

fn assistant_text(model: &Model, text: &str) -> AssistantMessage {
    AssistantMessage {
        role: AssistantMessageRole::Assistant,
        content: vec![AssistantContentBlock::Text(TextContent {
            content_type: TextContentType::Text,
            text: text.into(),
            text_signature: None,
        })],
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage {
            cost: UsageCost::default(),
            ..Usage::default()
        },
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp: 0,
    }
}

fn assistant_tool_call(model: &Model, name: &str, arguments: Value) -> AssistantMessage {
    let args = arguments
        .as_object()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect::<HashMap<_, _>>();
    AssistantMessage {
        stop_reason: StopReason::ToolUse,
        content: vec![AssistantContentBlock::ToolCall(ToolCall {
            content_type: ToolCallType::ToolCall,
            id: "call-1".into(),
            name: name.into(),
            arguments: args,
            thought_signature: None,
        })],
        ..assistant_text(model, "")
    }
}

fn done_stream(message: AssistantMessage) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    stream.push(AssistantMessageEvent::Done {
        reason: match message.stop_reason {
            StopReason::ToolUse => DoneStopReason::ToolUse,
            StopReason::Length => DoneStopReason::Length,
            _ => DoneStopReason::Stop,
        },
        message,
    });
    stream
}

fn user_agent_message(text: &str) -> AgentMessage {
    AgentMessage::Llm(Message::User(UserMessage {
        role: UserMessageRole::User,
        content: UserMessageContent::Blocks(vec![UserContentBlock::Text(TextContent {
            content_type: TextContentType::Text,
            text: text.into(),
            text_signature: None,
        })]),
        timestamp: 0,
    }))
}

fn text_tool_result(text: &str) -> Vec<AgentToolResultContent> {
    vec![AgentToolResultContent::Text(TextContent {
        content_type: TextContentType::Text,
        text: text.into(),
        text_signature: None,
    })]
}

fn calculate_tool() -> AgentTool {
    AgentTool {
        tool: Tool {
            name: "calculate".into(),
            description: "calculate an expression".into(),
            parameters: json!({
                "type": "object",
                "properties": { "expression": { "type": "string" } },
                "required": ["expression"]
            }),
        },
        label: "Calculate".into(),
        prepare_arguments: None,
        execute: Some(Arc::new(|_id, args, _signal, _update| {
            Box::pin(async move {
                AgentToolResult {
                    content: text_tool_result("4"),
                    details: args,
                    terminate: None,
                }
            })
        })),
        execution_mode: None,
    }
}

fn user_texts(context: &Context) -> Vec<String> {
    context
        .messages
        .iter()
        .filter_map(|message| match message {
            Message::User(message) => Some(user_content_text(&message.content)),
            _ => None,
        })
        .collect()
}

fn user_content_text(content: &UserMessageContent) -> String {
    match content {
        UserMessageContent::Text(text) => text.clone(),
        UserMessageContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|block| match block {
                UserContentBlock::Text(block) => Some(block.text.as_str()),
                UserContentBlock::Image(_) => None,
            })
            .collect::<Vec<_>>()
            .join(""),
    }
}

fn message_text(message: &AgentMessage) -> Option<String> {
    match message {
        AgentMessage::Llm(Message::User(message)) => Some(user_content_text(&message.content)),
        AgentMessage::Llm(Message::Assistant(message)) => {
            message.content.iter().find_map(|block| match block {
                AssistantContentBlock::Text(block) => Some(block.text.clone()),
                _ => None,
            })
        }
        AgentMessage::Llm(Message::ToolResult(message)) => {
            message.content.iter().find_map(|block| match block {
                zedflow_ai::ToolResultContentBlock::Text(block) => Some(block.text.clone()),
                _ => None,
            })
        }
        AgentMessage::Custom(_) => None,
    }
}

#[test]
fn constructs_directly_and_exposes_queue_modes() {
    let snapshots = Arc::new(Mutex::new(Vec::new()));
    let (models, model) = models_with_responses(Vec::new(), snapshots);
    let session = memory_session();
    let harness = AgentHarness::new(AgentHarnessOptions {
        env: env(),
        session: session as Arc<dyn SessionTrait>,
        models,
        tools: None,
        resources: None,
        system_prompt: Some(SystemPrompt::Text("You are helpful.".into())),
        stream_options: None,
        model: model.clone(),
        thinking_level: Some(ThinkingLevel::High),
        active_tool_names: None,
        steering_mode: Some(QueueMode::All),
        follow_up_mode: Some(QueueMode::All),
    })
    .expect("harness");

    assert_eq!(harness.get_model(), model);
    assert_eq!(harness.get_thinking_level(), ThinkingLevel::High);
    assert_eq!(harness.get_steering_mode(), QueueMode::All);
    assert_eq!(harness.get_follow_up_mode(), QueueMode::All);

    harness.set_steering_mode(QueueMode::OneAtATime);
    harness.set_follow_up_mode(QueueMode::OneAtATime);

    assert_eq!(harness.get_steering_mode(), QueueMode::OneAtATime);
    assert_eq!(harness.get_follow_up_mode(), QueueMode::OneAtATime);
}

#[test]
fn appends_before_agent_start_messages_and_persists_them() {
    run(async {
        let snapshots = Arc::new(Mutex::new(Vec::new()));
        let (models, model) = models_with_responses(
            vec![assistant_text(&test_model("model-1", true), "ok")],
            Arc::clone(&snapshots),
        );
        let session = memory_session();
        let harness = harness(models, model, Arc::clone(&session));
        harness.on(
            "before_agent_start",
            hook(|_| {
                Some(AgentHarnessHookResult::BeforeAgentStart(
                    BeforeAgentStartResult {
                        messages: Some(vec![user_agent_message("hook")]),
                        system_prompt: None,
                    },
                ))
            }),
        );

        let response = harness.prompt("hello", None).await.expect("prompt");

        assert_eq!(
            message_text(&AgentMessage::Llm(Message::Assistant(response))),
            Some("ok".into())
        );
        assert_eq!(
            snapshots.lock().expect("snapshots")[0].user_texts,
            vec!["hello", "hook"]
        );
        let persisted = session
            .get_entries()
            .await
            .into_iter()
            .filter_map(|entry| match entry {
                zedflow_agent::harness::types::SessionTreeEntry::Message(entry) => {
                    message_text(&entry.message)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(persisted, vec!["hello", "hook", "ok"]);
    });
}

#[test]
fn invokes_loaded_skill_and_prompt_template_resources() {
    run(async {
        let snapshots = Arc::new(Mutex::new(Vec::new()));
        let response_model = test_model("model-1", true);
        let (models, model) = models_with_responses(
            vec![
                assistant_text(&response_model, "skill ok"),
                assistant_text(&response_model, "template ok"),
            ],
            Arc::clone(&snapshots),
        );
        let session = memory_session();
        let harness = AgentHarness::new(AgentHarnessOptions {
            env: env(),
            session: session as Arc<dyn SessionTrait>,
            models,
            tools: None,
            resources: Some(AgentHarnessResources {
                skills: Some(vec![Skill {
                    name: "inspect".into(),
                    description: "Inspect things".into(),
                    content: "Use inspection tools.".into(),
                    file_path: "/project/.zed/skills/inspect/SKILL.md".into(),
                    disable_model_invocation: None,
                }]),
                prompt_templates: Some(vec![PromptTemplate {
                    name: "review".into(),
                    description: Some("Review".into()),
                    content: "Review $1 with $2".into(),
                }]),
            }),
            system_prompt: None,
            stream_options: None,
            model,
            thinking_level: None,
            active_tool_names: None,
            steering_mode: None,
            follow_up_mode: None,
        })
        .expect("harness");

        harness
            .skill("inspect", Some("Focus on tests."))
            .await
            .expect("skill");
        harness
            .prompt_from_template("review", &["diff".into(), "care".into()])
            .await
            .expect("template");

        let snapshots = snapshots.lock().expect("snapshots");
        assert!(snapshots[0].user_texts[0].contains("<skill name=\"inspect\""));
        assert!(snapshots[0].user_texts[0].contains("Use inspection tools."));
        assert!(snapshots[0].user_texts[0].contains("Focus on tests."));
        assert!(
            snapshots[1]
                .user_texts
                .iter()
                .any(|text| text == "Review diff with care")
        );
    });
}

#[test]
fn runs_tool_hooks_and_persists_patched_tool_result() {
    run(async {
        let snapshots = Arc::new(Mutex::new(Vec::new()));
        let response_model = test_model("model-1", true);
        let (models, model) = models_with_responses(
            vec![assistant_tool_call(
                &response_model,
                "calculate",
                json!({ "expression": "2 + 2" }),
            )],
            snapshots,
        );
        let session = memory_session();
        let seen_calls = Arc::new(Mutex::new(Vec::new()));
        let harness = AgentHarness::new(AgentHarnessOptions {
            env: env(),
            session: Arc::clone(&session) as Arc<dyn SessionTrait>,
            models,
            tools: Some(vec![calculate_tool()]),
            resources: None,
            system_prompt: None,
            stream_options: None,
            model,
            thinking_level: None,
            active_tool_names: Some(vec!["calculate".into()]),
            steering_mode: None,
            follow_up_mode: None,
        })
        .expect("harness");
        harness.on(
            "tool_call",
            hook({
                let seen_calls = Arc::clone(&seen_calls);
                move |event| match event {
                    AgentHarnessOwnEvent::ToolCall(event) => {
                        seen_calls.lock().expect("seen calls").push((
                            event.tool_call_id,
                            event.tool_name,
                            event.input,
                        ));
                        None
                    }
                    _ => None,
                }
            }),
        );
        harness.on(
            "tool_result",
            hook(|event| match event {
                AgentHarnessOwnEvent::ToolResult(event) if event.tool_name == "calculate" => {
                    Some(AgentHarnessHookResult::ToolResult(
                        zedflow_agent::harness::types::ToolResultPatch {
                            content: Some(text_tool_result("patched result")),
                            details: Some(json!({ "patched": true })),
                            is_error: None,
                            terminate: Some(true),
                        },
                    ))
                }
                _ => None,
            }),
        );

        harness.prompt("hello", None).await.expect("prompt");

        let seen = seen_calls.lock().expect("seen calls");
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].1, "calculate");
        assert_eq!(seen[0].2.get("expression"), Some(&json!("2 + 2")));
        let persisted = session
            .get_entries()
            .await
            .into_iter()
            .filter_map(|entry| match entry {
                zedflow_agent::harness::types::SessionTreeEntry::Message(entry) => {
                    message_text(&entry.message)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(persisted.contains(&"patched result".to_string()));
    });
}

#[test]
fn compaction_hook_persists_summary_and_emits_session_event() {
    run(async {
        let snapshots = Arc::new(Mutex::new(Vec::new()));
        let (models, model) = models_with_responses(Vec::new(), snapshots);
        let session = memory_session();
        session
            .append_message(user_agent_message("old request"))
            .await
            .expect("user");
        session
            .append_message(AgentMessage::Llm(Message::Assistant(assistant_text(
                &model,
                "old answer",
            ))))
            .await
            .expect("assistant");
        let seen_compact_event = Arc::new(Mutex::new(false));
        let harness = harness(models, model, Arc::clone(&session));
        harness.subscribe(subscriber({
            let seen_compact_event = Arc::clone(&seen_compact_event);
            move |event| {
                if matches!(
                    event,
                    AgentHarnessEvent::Harness(AgentHarnessOwnEvent::SessionCompact(_))
                ) {
                    *seen_compact_event.lock().expect("seen compact") = true;
                }
            }
        }));
        harness.on(
            "session_before_compact",
            hook(|event| match event {
                AgentHarnessOwnEvent::SessionBeforeCompact(event) => {
                    Some(AgentHarnessHookResult::SessionBeforeCompact(
                        zedflow_agent::harness::types::SessionBeforeCompactResult {
                            cancel: None,
                            compaction: Some(CompactResult {
                                summary: "hook summary".into(),
                                first_kept_entry_id: event.preparation.first_kept_entry_id,
                                tokens_before: event.preparation.tokens_before,
                                details: Some(json!({ "from": "test" })),
                            }),
                        },
                    ))
                }
                _ => None,
            }),
        );

        let result = harness.compact(Some("focus files")).await.expect("compact");

        assert_eq!(result.summary, "hook summary");
        assert!(*seen_compact_event.lock().expect("seen compact"));
        assert!(session.get_entries().await.into_iter().any(|entry| {
            matches!(entry, zedflow_agent::harness::types::SessionTreeEntry::Compaction(entry) if entry.summary == "hook summary")
        }));
    });
}
