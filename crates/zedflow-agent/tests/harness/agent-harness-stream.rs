use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::pin;
use std::sync::{Arc, Mutex};
use std::task::{Context as TaskContext, Poll};
use std::thread;

use futures::task::noop_waker_ref;
use serde_json::{Value, json};
use zedflow_agent::harness::agent_harness::{
    AgentHarness, AgentHarnessHook, AgentHarnessHookResult,
};
use zedflow_agent::harness::env::nodejs::NodeExecutionEnv;
use zedflow_agent::harness::session::memory_storage::{
    InMemorySessionStorage, InMemorySessionStorageOptions,
};
use zedflow_agent::harness::session::session::Session;
use zedflow_agent::harness::types::{
    AgentHarnessOptions, AgentHarnessOwnEvent, AgentHarnessStreamOptions,
    AgentHarnessStreamOptionsPatch, BeforeProviderPayloadResult, BeforeProviderRequestResult,
    ExecutionEnv, Patch, Session as SessionTrait, SessionMetadata,
};
use zedflow_ai::{
    AssistantContentBlock, AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream,
    AssistantMessageRole, CacheRetention, Context, DoneStopReason, Model, ModelInput, Models,
    ProviderApi, ProviderAuth, ProviderStreams, SimpleStreamOptions, StopReason, TextContent,
    TextContentType, Usage, UsageCost, create_models, create_provider,
};

#[derive(Debug, Clone, Default, PartialEq)]
struct CapturedOptions {
    timeout_ms: Option<u64>,
    max_retries: Option<u32>,
    max_retry_delay_ms: Option<u64>,
    session_id: Option<String>,
    cache_retention: Option<CacheRetention>,
    headers: HashMap<String, Option<String>>,
    metadata: Option<HashMap<String, Value>>,
    reasoning: Option<zedflow_ai::ThinkingLevel>,
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

fn env() -> Arc<dyn ExecutionEnv> {
    Arc::new(NodeExecutionEnv::with_cwd(
        std::env::current_dir()
            .expect("current dir")
            .to_string_lossy()
            .into_owned(),
    ))
}

fn memory_session(id: Option<&str>) -> Arc<Session<InMemorySessionStorage>> {
    let options = id.map(|id| InMemorySessionStorageOptions {
        entries: None,
        metadata: Some(SessionMetadata {
            id: id.into(),
            created_at: "now".into(),
        }),
    });
    Arc::new(Session::new(
        InMemorySessionStorage::new(options).expect("memory storage"),
    ))
}

fn models_with_capture(
    responses: Vec<AssistantMessage>,
    captured_options: Arc<Mutex<Vec<CapturedOptions>>>,
    captured_payload: Option<Arc<Mutex<Option<Value>>>>,
) -> (Models, Model) {
    let model = test_model("model-1", true);
    let queue = Arc::new(Mutex::new(VecDeque::from(responses)));
    let stream_simple = Arc::new({
        let queue = Arc::clone(&queue);
        let captured_options = Arc::clone(&captured_options);
        let captured_payload = captured_payload.clone();
        move |model: &Model, _context: &Context, options: Option<&SimpleStreamOptions>| {
            captured_options
                .lock()
                .expect("captured options")
                .push(options.map(capture_options).unwrap_or_default());
            if let (Some(options), Some(captured_payload)) = (options, captured_payload.as_ref()) {
                if let Some(on_payload) = &options.stream.on_payload {
                    let payload = run(on_payload(json!({ "steps": ["provider"] }), model.clone()))
                        .expect("payload hook");
                    *captured_payload.lock().expect("captured payload") = payload;
                }
            }
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
    stream_options: AgentHarnessStreamOptions,
) -> AgentHarness {
    AgentHarness::new(AgentHarnessOptions {
        env: env(),
        session: session as Arc<dyn SessionTrait>,
        models,
        tools: None,
        resources: None,
        system_prompt: None,
        stream_options: Some(stream_options),
        model,
        thinking_level: Some(zedflow_agent::types::ThinkingLevel::High),
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

fn done_stream(message: AssistantMessage) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    stream.push(AssistantMessageEvent::Done {
        reason: DoneStopReason::Stop,
        message,
    });
    stream
}

fn capture_options(options: &SimpleStreamOptions) -> CapturedOptions {
    CapturedOptions {
        timeout_ms: options.stream.timeout_ms,
        max_retries: options.stream.max_retries,
        max_retry_delay_ms: options.stream.max_retry_delay_ms,
        session_id: options.stream.session_id.clone(),
        cache_retention: options.stream.cache_retention,
        headers: options.stream.headers.clone().unwrap_or_default(),
        metadata: options.stream.metadata.clone(),
        reasoning: options.reasoning,
    }
}

#[test]
fn snapshots_stream_options_before_provider_request_hooks() {
    run(async {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let response_model = test_model("model-1", true);
        let (models, model) = models_with_capture(
            vec![assistant_text(&response_model, "ok")],
            Arc::clone(&captured),
            None,
        );
        let session = memory_session(Some("session-1"));
        let harness = harness(
            models,
            model,
            session,
            AgentHarnessStreamOptions {
                timeout_ms: Some(1000),
                max_retries: Some(2),
                max_retry_delay_ms: Some(3000),
                headers: Some(HashMap::from([("x-base".into(), "base".into())])),
                metadata: Some(HashMap::from([("base".into(), json!(true))])),
                cache_retention: Some(CacheRetention::None),
                ..AgentHarnessStreamOptions::default()
            },
        );
        harness.on(
            "before_provider_request",
            hook(|event| match event {
                AgentHarnessOwnEvent::BeforeProviderRequest(event) => {
                    assert_eq!(event.session_id, "session-1");
                    assert_eq!(
                        event.stream_options.headers,
                        Some(HashMap::from([("x-base".into(), "base".into())]))
                    );
                    Some(AgentHarnessHookResult::BeforeProviderRequest(
                        BeforeProviderRequestResult {
                            stream_options: Some(AgentHarnessStreamOptionsPatch {
                                headers: Patch::Set(HashMap::from([(
                                    "x-hook".into(),
                                    Patch::Set("hook".into()),
                                )])),
                                metadata: Patch::Set(HashMap::from([(
                                    "hook".into(),
                                    Patch::Set(json!(true)),
                                )])),
                                ..AgentHarnessStreamOptionsPatch::default()
                            }),
                        },
                    ))
                }
                _ => None,
            }),
        );

        harness.prompt("hello", None).await.expect("prompt");

        let captured = captured.lock().expect("captured");
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].timeout_ms, Some(1000));
        assert_eq!(captured[0].max_retries, Some(2));
        assert_eq!(captured[0].max_retry_delay_ms, Some(3000));
        assert_eq!(captured[0].session_id.as_deref(), Some("session-1"));
        assert_eq!(captured[0].cache_retention, Some(CacheRetention::None));
        assert_eq!(captured[0].reasoning, Some(zedflow_ai::ThinkingLevel::High));
        assert_eq!(
            captured[0].headers,
            HashMap::from([
                ("x-base".into(), Some("base".into())),
                ("x-hook".into(), Some("hook".into()))
            ])
        );
        assert_eq!(
            captured[0].metadata,
            Some(HashMap::from([
                ("base".into(), json!(true)),
                ("hook".into(), json!(true))
            ]))
        );
    });
}

#[test]
fn chains_provider_request_patches_and_deletes_header_metadata_keys() {
    run(async {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let response_model = test_model("model-1", true);
        let (models, model) = models_with_capture(
            vec![assistant_text(&response_model, "ok")],
            Arc::clone(&captured),
            None,
        );
        let harness = harness(
            models,
            model,
            memory_session(None),
            AgentHarnessStreamOptions {
                timeout_ms: Some(1000),
                max_retries: Some(2),
                headers: Some(HashMap::from([
                    ("keep".into(), "base".into()),
                    ("remove".into(), "base".into()),
                ])),
                metadata: Some(HashMap::from([
                    ("keep".into(), json!("base")),
                    ("remove".into(), json!("base")),
                ])),
                ..AgentHarnessStreamOptions::default()
            },
        );
        harness.on(
            "before_provider_request",
            hook(|event| match event {
                AgentHarnessOwnEvent::BeforeProviderRequest(event) => {
                    assert_eq!(
                        event.stream_options.headers.expect("headers").get("remove"),
                        Some(&"base".into())
                    );
                    Some(AgentHarnessHookResult::BeforeProviderRequest(
                        BeforeProviderRequestResult {
                            stream_options: Some(AgentHarnessStreamOptionsPatch {
                                headers: Patch::Set(HashMap::from([
                                    ("first".into(), Patch::Set("1".into())),
                                    ("remove".into(), Patch::Clear),
                                ])),
                                metadata: Patch::Set(HashMap::from([
                                    ("first".into(), Patch::Set(json!(1))),
                                    ("remove".into(), Patch::Clear),
                                ])),
                                ..AgentHarnessStreamOptionsPatch::default()
                            }),
                        },
                    ))
                }
                _ => None,
            }),
        );
        harness.on(
            "before_provider_request",
            hook(|event| match event {
                AgentHarnessOwnEvent::BeforeProviderRequest(event) => {
                    assert_eq!(
                        event.stream_options.headers.expect("headers").get("first"),
                        Some(&"1".into())
                    );
                    Some(AgentHarnessHookResult::BeforeProviderRequest(
                        BeforeProviderRequestResult {
                            stream_options: Some(AgentHarnessStreamOptionsPatch {
                                headers: Patch::Set(HashMap::from([(
                                    "second".into(),
                                    Patch::Set("2".into()),
                                )])),
                                ..AgentHarnessStreamOptionsPatch::default()
                            }),
                        },
                    ))
                }
                _ => None,
            }),
        );

        harness.prompt("hello", None).await.expect("prompt");

        let captured = captured.lock().expect("captured");
        assert_eq!(captured[0].timeout_ms, Some(1000));
        assert_eq!(captured[0].max_retries, Some(2));
        assert_eq!(
            captured[0].headers,
            HashMap::from([
                ("keep".into(), Some("base".into())),
                ("first".into(), Some("1".into())),
                ("second".into(), Some("2".into()))
            ])
        );
        assert_eq!(
            captured[0].metadata,
            Some(HashMap::from([
                ("keep".into(), json!("base")),
                ("first".into(), json!(1))
            ]))
        );
    });
}

#[test]
fn chains_provider_payload_hooks() {
    run(async {
        let captured_options = Arc::new(Mutex::new(Vec::new()));
        let captured_payload = Arc::new(Mutex::new(None));
        let seen_payloads = Arc::new(Mutex::new(Vec::new()));
        let response_model = test_model("model-1", true);
        let (models, model) = models_with_capture(
            vec![assistant_text(&response_model, "ok")],
            captured_options,
            Some(Arc::clone(&captured_payload)),
        );
        let harness = harness(
            models,
            model,
            memory_session(None),
            AgentHarnessStreamOptions::default(),
        );
        harness.on(
            "before_provider_payload",
            hook({
                let seen_payloads = Arc::clone(&seen_payloads);
                move |event| match event {
                    AgentHarnessOwnEvent::BeforeProviderPayload(event) => {
                        seen_payloads.lock().expect("seen").push(event.payload);
                        Some(AgentHarnessHookResult::BeforeProviderPayload(
                            BeforeProviderPayloadResult {
                                payload: json!({ "steps": ["provider", "first"] }),
                            },
                        ))
                    }
                    _ => None,
                }
            }),
        );
        harness.on(
            "before_provider_payload",
            hook({
                let seen_payloads = Arc::clone(&seen_payloads);
                move |event| match event {
                    AgentHarnessOwnEvent::BeforeProviderPayload(event) => {
                        seen_payloads
                            .lock()
                            .expect("seen")
                            .push(event.payload.clone());
                        Some(AgentHarnessHookResult::BeforeProviderPayload(
                            BeforeProviderPayloadResult {
                                payload: json!({ "steps": ["provider", "first", "second"] }),
                            },
                        ))
                    }
                    _ => None,
                }
            }),
        );

        harness.prompt("hello", None).await.expect("prompt");

        assert_eq!(
            *seen_payloads.lock().expect("seen"),
            vec![
                json!({ "steps": ["provider"] }),
                json!({ "steps": ["provider", "first"] })
            ]
        );
        assert_eq!(
            *captured_payload.lock().expect("payload"),
            Some(json!({ "steps": ["provider", "first", "second"] }))
        );
    });
}
