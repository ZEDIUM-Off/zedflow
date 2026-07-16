//! Port of Pi `packages/ai/test/faux-provider.test.ts`.

use std::sync::{Arc, Mutex};

use futures::StreamExt;
use futures::executor::block_on;
use serde_json::json;
use zedflow_ai::compat;
use zedflow_ai::providers::faux::{
    FauxCost, FauxModelDefinition, FauxResponseStep, RegisterFauxProviderOptions, create_faux_core,
    faux_assistant_content_message, faux_assistant_message, faux_provider, faux_text,
    faux_thinking, faux_tool_call,
};
use zedflow_ai::types as typed;
use zedflow_ai::types::{AssistantContentBlock, Context, StopReason};

fn content_text(message: &zedflow_ai::types::AssistantMessage) -> Vec<String> {
    message
        .content
        .iter()
        .map(|block| match block {
            AssistantContentBlock::Text(text) => text.text.clone(),
            AssistantContentBlock::Thinking(thinking) => thinking.thinking.clone(),
            AssistantContentBlock::ToolCall(tool) => {
                format!("{}:{}", tool.name, json!(tool.arguments))
            }
        })
        .collect()
}

fn typed_context(text: &str) -> typed::Context {
    typed::Context {
        messages: vec![typed::Message::User(typed::UserMessage {
            role: typed::UserMessageRole::User,
            content: typed::UserMessageContent::Text(text.to_owned()),
            timestamp: 1,
        })],
        ..typed::Context::default()
    }
}

fn typed_content_text(message: &typed::AssistantMessage) -> Vec<String> {
    message
        .content
        .iter()
        .map(|block| match block {
            typed::AssistantContentBlock::Text(text) => text.text.clone(),
            typed::AssistantContentBlock::Thinking(thinking) => thinking.thinking.clone(),
            typed::AssistantContentBlock::ToolCall(tool) => {
                format!("{}:{}", tool.name, json!(tool.arguments))
            }
        })
        .collect()
}

fn typed_model(handle: &zedflow_ai::providers::faux::FauxProviderHandle) -> typed::Model {
    handle
        .provider
        .get_models()
        .first()
        .cloned()
        .expect("model")
}

fn complete_typed(
    handle: &zedflow_ai::providers::faux::FauxProviderHandle,
    model: &typed::Model,
    context: &typed::Context,
    options: Option<&typed::StreamOptions>,
) -> typed::AssistantMessage {
    block_on(handle.provider.stream(model, context, options).result())
}

fn collect_typed_events(
    stream: typed::AssistantMessageEventStream,
) -> Vec<typed::AssistantMessageEvent> {
    block_on(stream.collect::<Vec<_>>())
}

fn typed_text_content(text: &str) -> typed::TextContent {
    typed::TextContent {
        content_type: typed::TextContentType::Text,
        text: text.to_owned(),
        text_signature: None,
    }
}

fn typed_assistant_text(text: &str) -> typed::AssistantMessage {
    typed::AssistantMessage {
        role: typed::AssistantMessageRole::Assistant,
        content: vec![typed::AssistantContentBlock::Text(typed_text_content(text))],
        api: "faux".to_owned(),
        provider: "faux".to_owned(),
        model: "faux-1".to_owned(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: typed::Usage::default(),
        stop_reason: typed::StopReason::Stop,
        error_message: None,
        timestamp: 1,
    }
}

fn typed_event_name(event: &typed::AssistantMessageEvent) -> &'static str {
    match event {
        typed::AssistantMessageEvent::Start { .. } => "start",
        typed::AssistantMessageEvent::TextStart { .. } => "text_start",
        typed::AssistantMessageEvent::TextDelta { .. } => "text_delta",
        typed::AssistantMessageEvent::TextEnd { .. } => "text_end",
        typed::AssistantMessageEvent::ThinkingStart { .. } => "thinking_start",
        typed::AssistantMessageEvent::ThinkingDelta { .. } => "thinking_delta",
        typed::AssistantMessageEvent::ThinkingEnd { .. } => "thinking_end",
        typed::AssistantMessageEvent::ToolcallStart { .. } => "toolcall_start",
        typed::AssistantMessageEvent::ToolcallDelta { .. } => "toolcall_delta",
        typed::AssistantMessageEvent::ToolcallEnd { .. } => "toolcall_end",
        typed::AssistantMessageEvent::Done { .. } => "done",
        typed::AssistantMessageEvent::Error { .. } => "error",
    }
}

fn next_message(
    faux: &zedflow_ai::providers::faux::FauxCore,
    model: &zedflow_ai::types::Model,
) -> zedflow_ai::types::AssistantMessage {
    block_on((faux.streams.stream)(model, &Context::default(), None).result())
}

fn event_types(stream: zedflow_ai::types::AssistantMessageEventStream) -> Vec<&'static str> {
    block_on(stream.collect::<Vec<_>>())
        .iter()
        .map(typed_event_name)
        .collect()
}

#[test]
fn registers_a_custom_provider_and_estimates_usage() {
    let registration = faux_provider(RegisterFauxProviderOptions::default());
    let model = typed_model(&registration);
    registration.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
        "hello world",
    ))]);

    let response = complete_typed(&registration, &model, &typed_context("hi there"), None);

    assert_eq!(typed_content_text(&response), ["hello world"]);
    assert!(response.usage.input > 0);
    assert!(response.usage.output > 0);
    assert_eq!(
        response.usage.total_tokens,
        response.usage.input + response.usage.output
    );
    assert_eq!(registration.state.call_count(), 1);
}

#[test]
fn supports_helper_blocks_for_text_thinking_and_tool_calls() {
    let faux = create_faux_core(RegisterFauxProviderOptions::default());
    let model = faux.get_model(None).expect("model");
    faux.set_responses(vec![FauxResponseStep::Message(
        faux_assistant_content_message(vec![
            faux_thinking("think"),
            faux_tool_call("echo", json!({"text":"hi"})),
            faux_text("done"),
        ]),
    )]);

    let response = next_message(&faux, &model);
    assert!(matches!(
        response.content[0],
        AssistantContentBlock::Thinking(_)
    ));
    assert!(matches!(
        response.content[1],
        AssistantContentBlock::ToolCall(_)
    ));
    assert!(matches!(
        response.content[2],
        AssistantContentBlock::Text(_)
    ));
}

#[test]
fn supports_multiple_models_with_per_model_reasoning_and_model_aware_factories() {
    let faux = create_faux_core(RegisterFauxProviderOptions {
        models: vec![
            FauxModelDefinition {
                id: "faux-fast".to_owned(),
                reasoning: false,
                ..FauxModelDefinition::default()
            },
            FauxModelDefinition {
                id: "faux-thinker".to_owned(),
                reasoning: true,
                ..FauxModelDefinition::default()
            },
        ],
        ..RegisterFauxProviderOptions::default()
    });
    faux.set_responses(vec![
        FauxResponseStep::Factory(Arc::new(|_, _, _, model| {
            faux_assistant_message(format!("{}:{}", model.id, model.reasoning))
        })),
        FauxResponseStep::Factory(Arc::new(|_, _, _, model| {
            faux_assistant_message(format!("{}:{}", model.id, model.reasoning))
        })),
    ]);

    assert_eq!(
        faux.models
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>(),
        ["faux-fast", "faux-thinker"]
    );
    assert!(!faux.get_model(Some("faux-fast")).expect("fast").reasoning);
    assert!(
        faux.get_model(Some("faux-thinker"))
            .expect("thinker")
            .reasoning
    );
    assert_eq!(
        content_text(&next_message(
            &faux,
            &faux.get_model(Some("faux-fast")).expect("fast")
        )),
        ["faux-fast:false"]
    );
    assert_eq!(
        content_text(&next_message(
            &faux,
            &faux.get_model(Some("faux-thinker")).expect("thinker")
        )),
        ["faux-thinker:true"]
    );
}

#[test]
fn rewrites_api_provider_and_model_on_returned_messages() {
    let registration = compat::register_faux_provider(RegisterFauxProviderOptions {
        api: Some("faux:test".to_owned()),
        provider: Some("faux-provider".to_owned()),
        models: vec![FauxModelDefinition {
            id: "faux-model".to_owned(),
            ..FauxModelDefinition::default()
        }],
        ..RegisterFauxProviderOptions::default()
    });
    registration.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
        "hello",
    ))]);

    let response = block_on(compat::complete(
        &registration.get_model(None).expect("model"),
        &Context::default(),
        None,
    ))
    .expect("faux complete");
    assert_eq!(response.api, "faux:test");
    assert_eq!(response.provider, "faux-provider");
    assert_eq!(response.model, "faux-model");
    registration.unregister();
}

#[test]
fn consumes_queued_responses_in_order_and_errors_when_exhausted() {
    let faux = create_faux_core(RegisterFauxProviderOptions::default());
    let model = faux.get_model(None).expect("model");
    faux.set_responses(vec![
        FauxResponseStep::Message(faux_assistant_message("first")),
        FauxResponseStep::Message(faux_assistant_message("second")),
    ]);

    assert_eq!(content_text(&next_message(&faux, &model)), ["first"]);
    assert_eq!(content_text(&next_message(&faux, &model)), ["second"]);
    let exhausted = next_message(&faux, &model);
    assert_eq!(exhausted.stop_reason, StopReason::Error);
    assert_eq!(
        exhausted.error_message.as_deref(),
        Some("No more faux responses queued")
    );
    assert_eq!(faux.get_pending_response_count(), 0);
    assert_eq!(faux.state.call_count(), 3);
}

#[test]
fn can_replace_and_append_queued_responses() {
    let faux = create_faux_core(RegisterFauxProviderOptions::default());
    let model = faux.get_model(None).expect("model");
    faux.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
        "first",
    ))]);
    assert_eq!(content_text(&next_message(&faux, &model)), ["first"]);

    faux.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
        "second",
    ))]);
    assert_eq!(faux.get_pending_response_count(), 1);
    assert_eq!(content_text(&next_message(&faux, &model)), ["second"]);

    faux.append_responses(vec![
        FauxResponseStep::Message(faux_assistant_message("third")),
        FauxResponseStep::Message(faux_assistant_message("fourth")),
    ]);
    assert_eq!(content_text(&next_message(&faux, &model)), ["third"]);
    assert_eq!(content_text(&next_message(&faux, &model)), ["fourth"]);
    assert_eq!(faux.get_pending_response_count(), 0);
}

#[test]
fn emits_an_error_when_a_response_factory_panics() {
    let faux = create_faux_core(RegisterFauxProviderOptions::default());
    let model = faux.get_model(None).expect("model");
    faux.set_responses(vec![FauxResponseStep::Factory(Arc::new(|_, _, _, _| {
        panic!("boom")
    }))]);

    let stream = (faux.streams.stream)(&model, &Context::default(), None);
    assert_eq!(event_types(stream.clone()), ["error"]);
    assert_eq!(
        block_on(stream.result()).error_message.as_deref(),
        Some("boom")
    );
}

#[test]
fn simulates_prompt_caching_per_session_id() {
    let registration = faux_provider(RegisterFauxProviderOptions::default());
    let model = typed_model(&registration);
    registration.set_responses(vec![
        FauxResponseStep::Message(faux_assistant_message("first")),
        FauxResponseStep::Message(faux_assistant_message("second")),
    ]);
    let options = typed::StreamOptions {
        session_id: Some("session-1".to_owned()),
        cache_retention: Some(typed::CacheRetention::Short),
        ..typed::StreamOptions::default()
    };
    let mut context = typed_context("hello");

    let first = complete_typed(&registration, &model, &context, Some(&options));
    context
        .messages
        .push(typed::Message::Assistant(first.clone()));
    context
        .messages
        .push(typed::Message::User(typed::UserMessage {
            role: typed::UserMessageRole::User,
            content: typed::UserMessageContent::Text("follow up".to_owned()),
            timestamp: 2,
        }));
    let second = complete_typed(&registration, &model, &context, Some(&options));
    assert_eq!(first.usage.cache_read, 0);
    assert!(first.usage.cache_write > 0);
    assert!(second.usage.cache_read > 0);
    assert!(second.usage.input + second.usage.cache_read > second.usage.input);
}

#[test]
fn typed_cache_is_isolated_across_sessions_and_absent_without_session_id() {
    let registration = faux_provider(RegisterFauxProviderOptions::default());
    let model = typed_model(&registration);
    registration.set_responses(vec![
        FauxResponseStep::Message(faux_assistant_message("first")),
        FauxResponseStep::Message(faux_assistant_message("second")),
        FauxResponseStep::Message(faux_assistant_message("third")),
    ]);
    let mut context = typed_context("hello");
    let session_one = typed::StreamOptions {
        session_id: Some("session-1".to_owned()),
        cache_retention: Some(typed::CacheRetention::Short),
        ..typed::StreamOptions::default()
    };
    let session_two = typed::StreamOptions {
        session_id: Some("session-2".to_owned()),
        cache_retention: Some(typed::CacheRetention::Short),
        ..typed::StreamOptions::default()
    };

    let first = complete_typed(&registration, &model, &context, Some(&session_one));
    assert_eq!(first.usage.cache_read, 0);
    assert!(first.usage.cache_write > 0);
    context.messages.push(typed::Message::Assistant(first));
    context
        .messages
        .push(typed::Message::User(typed::UserMessage {
            role: typed::UserMessageRole::User,
            content: typed::UserMessageContent::Text("follow up".to_owned()),
            timestamp: 2,
        }));

    let second = complete_typed(&registration, &model, &context, Some(&session_two));
    assert_eq!(second.usage.cache_read, 0);
    assert!(second.usage.cache_write > 0);

    let third = complete_typed(&registration, &model, &context, None);
    assert_eq!(third.usage.cache_read, 0);
    assert_eq!(third.usage.cache_write, 0);
}

#[test]
fn typed_cache_retention_none_disables_cache_accounting() {
    let registration = faux_provider(RegisterFauxProviderOptions::default());
    let model = typed_model(&registration);
    registration.set_responses(vec![
        FauxResponseStep::Message(faux_assistant_message("first")),
        FauxResponseStep::Message(faux_assistant_message("second")),
    ]);
    let options = typed::StreamOptions {
        session_id: Some("session-1".to_owned()),
        cache_retention: Some(typed::CacheRetention::None),
        ..typed::StreamOptions::default()
    };
    let context = typed_context("hello");

    let first = complete_typed(&registration, &model, &context, Some(&options));
    let second = complete_typed(&registration, &model, &context, Some(&options));
    assert_eq!(first.usage.cache_read, 0);
    assert_eq!(first.usage.cache_write, 0);
    assert_eq!(second.usage.cache_read, 0);
    assert_eq!(second.usage.cache_write, 0);
}

#[test]
fn estimates_prompt_and_output_tokens_from_serialized_context() {
    let registration = faux_provider(RegisterFauxProviderOptions::default());
    let model = typed_model(&registration);
    registration.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
        "done",
    ))]);

    let tool = typed::Tool {
        name: "echo".to_owned(),
        description: "Echo back text".to_owned(),
        parameters: json!({"type":"object","properties":{"text":{"type":"string"}}}),
    };
    let context = typed::Context {
        system_prompt: Some("sys".to_owned()),
        messages: vec![
            typed::Message::User(typed::UserMessage {
                role: typed::UserMessageRole::User,
                content: typed::UserMessageContent::Blocks(vec![
                    typed::UserContentBlock::Text(typed_text_content("hello")),
                    typed::UserContentBlock::Image(typed::ImageContent {
                        content_type: typed::ImageContentType::Image,
                        mime_type: "image/png".to_owned(),
                        data: "abcd".to_owned(),
                    }),
                ]),
                timestamp: 1,
            }),
            typed::Message::Assistant(typed_assistant_text("prior")),
            typed::Message::ToolResult(typed::ToolResultMessage {
                role: typed::ToolResultMessageRole::ToolResult,
                tool_call_id: "tool-1".to_owned(),
                tool_name: "echo".to_owned(),
                content: vec![typed::ToolResultContentBlock::Text(typed_text_content(
                    "tool out",
                ))],
                details: None,
                is_error: false,
                timestamp: 2,
            }),
        ],
        tools: Some(vec![tool.clone()]),
    };

    let response = complete_typed(&registration, &model, &context, None);
    let prompt_text = [
        "system:sys".to_owned(),
        "user:hello\n[image:image/png:4]".to_owned(),
        "assistant:prior".to_owned(),
        "toolResult:echo\ntool out".to_owned(),
        format!(
            "tools:{}",
            serde_json::to_string(&vec![tool]).expect("tool JSON")
        ),
    ]
    .join("\n\n");
    let expected_prompt_tokens = prompt_text.len().div_ceil(4) as u64;
    let expected_output_tokens = "done".len().div_ceil(4) as u64;

    assert_eq!(response.usage.input, expected_prompt_tokens);
    assert_eq!(response.usage.output, expected_output_tokens);
    assert_eq!(response.usage.cache_read, 0);
    assert_eq!(response.usage.cache_write, 0);
    assert_eq!(
        response.usage.total_tokens,
        expected_prompt_tokens + expected_output_tokens
    );
}

#[test]
fn includes_cache_write_in_total_tokens_and_cost() {
    let registration = faux_provider(RegisterFauxProviderOptions {
        models: vec![FauxModelDefinition {
            cost: FauxCost {
                input: 2.0,
                output: 4.0,
                cache_read: 1.0,
                cache_write: 3.0,
            },
            ..FauxModelDefinition::default()
        }],
        ..RegisterFauxProviderOptions::default()
    });
    let model = typed_model(&registration);
    registration.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
        "done",
    ))]);
    let options = typed::StreamOptions {
        session_id: Some("session-1".to_owned()),
        cache_retention: Some(typed::CacheRetention::Short),
        ..typed::StreamOptions::default()
    };

    let response = complete_typed(
        &registration,
        &model,
        &typed_context("hello"),
        Some(&options),
    );

    assert!(response.usage.cache_write > 0);
    assert_eq!(
        response.usage.total_tokens,
        response.usage.input
            + response.usage.output
            + response.usage.cache_read
            + response.usage.cache_write
    );
    assert!(response.usage.cost.cache_write > 0.0);
    assert_eq!(
        response.usage.cost.total,
        response.usage.cost.input
            + response.usage.cost.output
            + response.usage.cost.cache_read
            + response.usage.cost.cache_write
    );
}

#[test]
fn streams_typed_thinking_text_and_toolcall_events_in_order() {
    let registration = faux_provider(RegisterFauxProviderOptions {
        token_size: zedflow_ai::providers::faux::FauxTokenSize {
            min: Some(1),
            max: Some(1),
        },
        ..RegisterFauxProviderOptions::default()
    });
    let model = typed_model(&registration);
    registration.set_responses(vec![FauxResponseStep::Message(
        faux_assistant_content_message(vec![
            faux_thinking("go"),
            faux_text("ok"),
            faux_tool_call("echo", json!({})),
        ]),
    )]);

    let events = collect_typed_events(registration.provider.stream(
        &model,
        &typed_context("hi"),
        None,
    ));

    assert_eq!(
        events.iter().map(typed_event_name).collect::<Vec<_>>(),
        [
            "start",
            "thinking_start",
            "thinking_delta",
            "thinking_end",
            "text_start",
            "text_delta",
            "text_end",
            "toolcall_start",
            "toolcall_delta",
            "toolcall_end",
            "done",
        ]
    );
}

#[test]
fn queued_explicit_error_streams_content_before_one_terminal_error() {
    let registration = faux_provider(RegisterFauxProviderOptions {
        token_size: zedflow_ai::providers::faux::FauxTokenSize {
            min: Some(2),
            max: Some(2),
        },
        ..RegisterFauxProviderOptions::default()
    });
    let model = typed_model(&registration);
    let mut response = faux_assistant_message("partial");
    response.stop_reason = StopReason::Error;
    response.error_message = Some("upstream failed".to_owned());
    registration.set_responses(vec![FauxResponseStep::Message(response)]);

    let events = collect_typed_events(registration.provider.stream(
        &model,
        &typed_context("hi"),
        None,
    ));

    assert_eq!(
        events.iter().map(typed_event_name).collect::<Vec<_>>(),
        ["start", "text_start", "text_delta", "text_end", "error"]
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event,
                typed::AssistantMessageEvent::Done { .. }
                    | typed::AssistantMessageEvent::Error { .. }
            ))
            .count(),
        1
    );
    assert!(matches!(
        events.last(),
        Some(typed::AssistantMessageEvent::Error {
            reason: typed::ErrorStopReason::Error,
            error,
        }) if error.stop_reason == typed::StopReason::Error
            && error.error_message.as_deref() == Some("upstream failed")
    ));
}

#[test]
fn streams_multiple_tool_calls_in_one_message() {
    let registration = faux_provider(RegisterFauxProviderOptions::default());
    let model = typed_model(&registration);
    registration.set_responses(vec![FauxResponseStep::Message(
        faux_assistant_content_message(vec![
            faux_tool_call("echo", json!({"text":"one"})),
            faux_tool_call("echo", json!({"text":"two"})),
        ]),
    )]);

    let stream = registration
        .provider
        .stream(&model, &typed_context("hi"), None);
    let events = collect_typed_events(stream);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, typed::AssistantMessageEvent::ToolcallStart { .. }))
            .count(),
        2
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, typed::AssistantMessageEvent::ToolcallEnd { .. }))
            .count(),
        2
    );
}

#[test]
fn unregisters_the_provider() {
    let registration = compat::register_faux_provider(RegisterFauxProviderOptions::default());
    let model = registration.get_model(None).expect("model");
    registration.unregister();
    let error = block_on(compat::complete(&model, &Context::default(), None))
        .expect_err("provider should be unregistered");
    assert!(error.to_string().contains(&format!(
        "no API provider registered for api: {}",
        registration.api
    )));
}

#[test]
fn supports_async_and_fallible_response_factories() {
    let registration = faux_provider(RegisterFauxProviderOptions {
        models: vec![FauxModelDefinition {
            id: "factory-model".to_owned(),
            name: Some("Factory Model".to_owned()),
            reasoning: true,
            ..FauxModelDefinition::default()
        }],
        ..RegisterFauxProviderOptions::default()
    });
    let model = typed_model(&registration);
    registration.set_responses(vec![
        FauxResponseStep::AsyncFactory(Arc::new(|context, options, state, model| {
            assert_eq!(context.system_prompt.as_deref(), Some("factory system"));
            let prompt = match &context.messages[0] {
                typed::Message::User(message) => match &message.content {
                    typed::UserMessageContent::Text(text) => text.clone(),
                    typed::UserMessageContent::Blocks(_) => panic!("expected text prompt"),
                },
                _ => panic!("expected user prompt"),
            };
            let options = options.expect("factory should receive stream options");
            assert_eq!(options.temperature, Some(0.25));
            let session_id = options.session_id.clone().expect("session id");
            assert_eq!(state.call_count(), 1);
            assert_eq!(model.id, "factory-model");
            assert_eq!(model.name, "Factory Model");
            assert!(model.reasoning);
            Box::pin(async move {
                tokio::task::yield_now().await;
                Ok(faux_assistant_message(format!(
                    "{prompt}:{session_id}:{}:{}",
                    model.id,
                    state.call_count()
                )))
            })
        })),
        FauxResponseStep::AsyncFactory(Arc::new(|_, _, _, _| {
            Box::pin(async { Err(std::io::Error::other("factory failed").into()) })
        })),
    ]);
    let options = typed::StreamOptions {
        temperature: Some(0.25),
        session_id: Some("session-1".to_owned()),
        ..typed::StreamOptions::default()
    };
    let mut context = typed_context("hi");
    context.system_prompt = Some("factory system".to_owned());

    let response = complete_typed(&registration, &model, &context, Some(&options));
    assert_eq!(
        typed_content_text(&response),
        ["hi:session-1:factory-model:1"]
    );

    let events = collect_typed_events(registration.provider.stream(&model, &context, None));
    let factory_error = match events.as_slice() {
        [
            typed::AssistantMessageEvent::Error {
                reason: typed::ErrorStopReason::Error,
                error,
            },
        ] => error,
        events => panic!("expected one factory error, got {events:?}"),
    };
    assert_eq!(
        factory_error.error_message.as_deref(),
        Some("factory failed")
    );
    assert_eq!(factory_error.usage, typed::Usage::default());

    let events = collect_typed_events(registration.provider.stream(&model, &context, None));
    let exhausted = match events.as_slice() {
        [
            typed::AssistantMessageEvent::Error {
                reason: typed::ErrorStopReason::Error,
                error,
            },
        ] => error,
        events => panic!("expected one exhaustion error, got {events:?}"),
    };
    assert_eq!(
        exhausted.error_message.as_deref(),
        Some("No more faux responses queued")
    );
    assert!(exhausted.usage.input > 0);
    assert_eq!(exhausted.usage.output, 0);
    assert_eq!(exhausted.usage.cache_read, 0);
    assert_eq!(exhausted.usage.cache_write, 0);
    assert_eq!(exhausted.usage.cache_write_1h, None);
    assert_eq!(exhausted.usage.reasoning, None);
    assert_eq!(exhausted.usage.total_tokens, exhausted.usage.input);
    assert_eq!(exhausted.usage.cost, typed::UsageCost::default());
    assert_eq!(registration.state.call_count(), 3);
}

#[test]
fn typed_async_factory_stream_is_pending_until_controlled_release() {
    let registration = faux_provider(RegisterFauxProviderOptions {
        token_size: zedflow_ai::providers::faux::FauxTokenSize {
            min: Some(1),
            max: Some(1),
        },
        ..RegisterFauxProviderOptions::default()
    });
    let model = typed_model(&registration);
    let (release, released) = futures::channel::oneshot::channel();
    let released = Arc::new(Mutex::new(Some(released)));
    registration.set_responses(vec![FauxResponseStep::AsyncFactory(Arc::new({
        let released = Arc::clone(&released);
        move |context, options, state, model| {
            assert_eq!(context.messages.len(), 1);
            assert_eq!(
                options.and_then(|options| options.session_id.as_deref()),
                Some("controlled-session")
            );
            assert_eq!(state.call_count(), 1);
            assert_eq!(model.id, "faux-1");
            let released = released
                .lock()
                .expect("release lock")
                .take()
                .expect("factory called once");
            Box::pin(async move {
                released.await.expect("release sender");
                Ok(faux_assistant_message("abcdefgh"))
            })
        }
    }))]);
    let options = typed::StreamOptions {
        session_id: Some("controlled-session".to_owned()),
        ..typed::StreamOptions::default()
    };
    let mut stream =
        registration
            .provider
            .stream(&model, &typed_context("real context"), Some(&options));

    let events = block_on(async {
        let mut first = Box::pin(stream.next());
        assert!(futures::poll!(&mut first).is_pending());
        release.send(()).expect("release receiver");
        let mut events = vec![first.await.expect("start event")];
        events.extend(stream.collect::<Vec<_>>().await);
        events
    });

    assert_eq!(
        events.iter().map(typed_event_name).collect::<Vec<_>>(),
        [
            "start",
            "text_start",
            "text_delta",
            "text_delta",
            "text_end",
            "done",
        ]
    );
}

fn typed_text_deltas(
    token_size: zedflow_ai::providers::faux::FauxTokenSize,
    text: &str,
) -> Vec<String> {
    let registration = faux_provider(RegisterFauxProviderOptions {
        token_size,
        ..RegisterFauxProviderOptions::default()
    });
    let model = typed_model(&registration);
    registration.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
        text,
    ))]);
    collect_typed_events(
        registration
            .provider
            .stream(&model, &typed_context("hi"), None),
    )
    .into_iter()
    .filter_map(|event| match event {
        typed::AssistantMessageEvent::TextDelta { delta, .. } => Some(delta),
        _ => None,
    })
    .collect()
}

#[test]
fn normalizes_and_uses_both_token_size_bounds() {
    let bounded = typed_text_deltas(
        zedflow_ai::providers::faux::FauxTokenSize {
            min: Some(1),
            max: Some(2),
        },
        "abcdefghijklmnopqrstuvwx",
    );
    assert_eq!(
        bounded.iter().map(String::len).collect::<Vec<_>>(),
        [4, 8, 4, 8]
    );

    let max_only = typed_text_deltas(
        zedflow_ai::providers::faux::FauxTokenSize {
            min: None,
            max: Some(2),
        },
        "abcdefghijklmnop",
    );
    assert_eq!(max_only.iter().map(String::len).collect::<Vec<_>>(), [8, 8]);
}

#[test]
fn supports_aborting_before_the_first_chunk() {
    let registration = faux_provider(RegisterFauxProviderOptions::default());
    let model = typed_model(&registration);
    registration.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
        "abcdefghijklmnopqrstuvwxyz",
    ))]);
    let controller = zedflow_ai::utils::abort_signals::AbortController::new();
    controller.abort();
    let options = typed::StreamOptions {
        signal: Some(controller.signal()),
        ..typed::StreamOptions::default()
    };

    let events = collect_typed_events(registration.provider.stream(
        &model,
        &typed_context("hi"),
        Some(&options),
    ));

    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0],
        typed::AssistantMessageEvent::Error {
            reason: typed::ErrorStopReason::Aborted,
            ..
        }
    ));
}

fn assert_mid_stream_abort(content: Vec<AssistantContentBlock>, expected_events: &[&str]) {
    let registration = faux_provider(RegisterFauxProviderOptions {
        tokens_per_second: Some(100.0),
        token_size: zedflow_ai::providers::faux::FauxTokenSize {
            min: Some(1),
            max: Some(1),
        },
        ..RegisterFauxProviderOptions::default()
    });
    let model = typed_model(&registration);
    registration.set_responses(vec![FauxResponseStep::Message(
        faux_assistant_content_message(content),
    )]);
    let controller = zedflow_ai::utils::abort_signals::AbortController::new();
    let options = typed::StreamOptions {
        signal: Some(controller.signal()),
        ..typed::StreamOptions::default()
    };
    let mut stream = registration
        .provider
        .stream(&model, &typed_context("hi"), Some(&options));
    let events = block_on(async {
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            if matches!(
                event,
                typed::AssistantMessageEvent::TextDelta { .. }
                    | typed::AssistantMessageEvent::ThinkingDelta { .. }
                    | typed::AssistantMessageEvent::ToolcallDelta { .. }
            ) {
                controller.abort();
            }
            events.push(event);
        }
        events
    });

    assert_eq!(
        events.iter().map(typed_event_name).collect::<Vec<_>>(),
        expected_events
    );
    assert!(matches!(
        events.last(),
        Some(typed::AssistantMessageEvent::Error {
            reason: typed::ErrorStopReason::Aborted,
            ..
        })
    ));
}

#[test]
fn supports_aborting_mid_text_stream_when_paced() {
    assert_mid_stream_abort(
        vec![faux_text("abcdefghijklmnopqrstuvwxyz")],
        &["start", "text_start", "text_delta", "error"],
    );
}

#[test]
fn supports_aborting_mid_thinking_stream_when_paced() {
    assert_mid_stream_abort(
        vec![faux_thinking("abcdefghijklmnopqrstuvwxyz")],
        &["start", "thinking_start", "thinking_delta", "error"],
    );
}

#[test]
fn supports_aborting_mid_toolcall_stream_when_paced() {
    assert_mid_stream_abort(
        vec![faux_tool_call(
            "echo",
            json!({"text": "abcdefghijklmnopqrstuvwxyz"}),
        )],
        &["start", "toolcall_start", "toolcall_delta", "error"],
    );
}
