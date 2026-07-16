//! Port of Pi `packages/ai/test/openai-completions-tool-choice.test.ts`.
//! Deterministic request-payload cases use `build_request`; stream chunk/event cases use fixture chunks.

use std::collections::HashMap;

use serde_json::{Value, json};
use zedflow_ai::api::openai_completions::{
    self, AssistantMessage, ChatCompletionMessage, ContentBlock, Context, MaxTokensField, Message,
    Model, ModelInput, ModelThinkingLevel, OpenAICompletionsCompat, OpenAICompletionsOptions,
    OpenAIToolChoice, ReasoningEffort, ResolvedOpenAICompletionsCompat, StopReason, ThinkingFormat,
    Tool, ToolCall, UserMessageContent,
};

fn model(provider: &str, id: &str, base_url: &str) -> Model {
    Model {
        id: id.to_owned(),
        api: "openai-completions".to_owned(),
        provider: provider.to_owned(),
        base_url: base_url.to_owned(),
        input: vec![ModelInput::Text],
        reasoning: true,
        thinking_level_map: HashMap::new(),
        headers: HashMap::new(),
        max_tokens: 8192,
        context_window: Some(128_000),
        compat: None,
    }
}

fn user_context(system_prompt: Option<&str>) -> Context {
    Context {
        system_prompt: system_prompt.map(str::to_owned),
        messages: vec![Message::User {
            content: UserMessageContent::Text("Hi".to_owned()),
        }],
        tools: Vec::new(),
    }
}

fn tool() -> Tool {
    Tool {
        name: "ping".to_owned(),
        description: "Ping tool".to_owned(),
        parameters: json!({
            "type": "object",
            "properties": { "ok": { "type": "boolean" } },
        }),
    }
}

fn body(model: &Model, context: &Context, options: OpenAICompletionsOptions) -> Value {
    openai_completions::build_request(model, context, Some(&options))
        .expect("request should build")
        .body
}

fn stream_result(
    model: &Model,
    chunks: Vec<Option<Value>>,
) -> openai_completions::OpenAICompletionsStreamResult {
    openai_completions::process_openai_completions_stream_chunks(model, chunks)
}

fn stop_reason_name(reason: StopReason) -> &'static str {
    match reason {
        StopReason::Stop => "stop",
        StopReason::Length => "length",
        StopReason::ToolUse => "toolUse",
        StopReason::Aborted => "aborted",
        StopReason::Error => "error",
    }
}

fn event_type(event: &openai_completions::OpenAICompletionsStreamEvent) -> &'static str {
    match event {
        openai_completions::OpenAICompletionsStreamEvent::TextStart { .. } => "text_start",
        openai_completions::OpenAICompletionsStreamEvent::TextDelta { .. } => "text_delta",
        openai_completions::OpenAICompletionsStreamEvent::TextEnd { .. } => "text_end",
        openai_completions::OpenAICompletionsStreamEvent::ThinkingStart { .. } => "thinking_start",
        openai_completions::OpenAICompletionsStreamEvent::ThinkingDelta { .. } => "thinking_delta",
        openai_completions::OpenAICompletionsStreamEvent::ThinkingEnd { .. } => "thinking_end",
        openai_completions::OpenAICompletionsStreamEvent::ToolCallStart { .. } => "toolcall_start",
        openai_completions::OpenAICompletionsStreamEvent::ToolCallDelta { .. } => "toolcall_delta",
        openai_completions::OpenAICompletionsStreamEvent::ToolCallEnd { .. } => "toolcall_end",
        openai_completions::OpenAICompletionsStreamEvent::Done { .. } => "done",
    }
}

fn compat_for_replay() -> ResolvedOpenAICompletionsCompat {
    ResolvedOpenAICompletionsCompat {
        supports_store: false,
        supports_developer_role: false,
        supports_reasoning_effort: true,
        supports_usage_in_streaming: true,
        max_tokens_field: MaxTokensField::MaxCompletionTokens,
        requires_tool_result_name: false,
        requires_assistant_after_tool_result: false,
        requires_thinking_as_text: false,
        requires_reasoning_content_on_assistant_messages: false,
        thinking_format: ThinkingFormat::OpenAI,
        supports_strict_mode: true,
        cache_control_format: None,
        send_session_affinity_headers: false,
        supports_long_cache_retention: true,
        zai_tool_stream: false,
        chat_template_kwargs: None,
        chat_template_effort_key: None,
        chat_template_bool_key: "enable_thinking".to_owned(),
    }
}

#[test]
fn forwards_tool_choice_from_simple_options_to_payload() {
    let params = body(
        &model("openai", "gpt-4o-mini", "https://api.openai.com/v1"),
        &Context {
            tools: vec![tool()],
            ..user_context(None)
        },
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            tool_choice: Some(OpenAIToolChoice::Mode("required".to_owned())),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(params.get("tool_choice"), Some(&json!("required")));
    assert!(
        params
            .get("tools")
            .and_then(Value::as_array)
            .is_some_and(|tools| !tools.is_empty())
    );
}

#[test]
fn omits_strict_when_compat_disables_strict_mode() {
    let mut model = model("openai", "gpt-4o-mini", "https://api.openai.com/v1");
    model.compat = Some(OpenAICompletionsCompat {
        supports_strict_mode: Some(false),
        ..OpenAICompletionsCompat::default()
    });
    let params = body(
        &model,
        &Context {
            tools: vec![tool()],
            ..user_context(None)
        },
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            ..OpenAICompletionsOptions::default()
        },
    );

    let function = &params["tools"][0]["function"];
    assert!(function.is_object());
    assert_eq!(function.get("strict"), None);
}

#[test]
fn maps_groq_qwen3_reasoning_levels_to_default_reasoning_effort() {
    let mut model = model("groq", "qwen/qwen3-32b", "https://api.groq.com/openai/v1");
    model
        .thinking_level_map
        .insert(ModelThinkingLevel::Medium, Some("default".to_owned()));
    let params = body(
        &model,
        &user_context(None),
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            reasoning_effort: Some(ReasoningEffort::Medium),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(params.get("reasoning_effort"), Some(&json!("default")));
}

#[test]
fn keeps_normal_reasoning_effort_for_groq_models_without_compat_mapping() {
    let params = body(
        &model(
            "groq",
            "openai/gpt-oss-20b",
            "https://api.groq.com/openai/v1",
        ),
        &user_context(None),
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            reasoning_effort: Some(ReasoningEffort::Medium),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(params.get("reasoning_effort"), Some(&json!("medium")));
}

#[test]
fn enables_tool_stream_for_supported_zai_models_with_tools() {
    let params = body(
        &model("zai", "glm-5.1", "https://api.z.ai/api/paas/v4"),
        &Context {
            tools: vec![tool()],
            ..user_context(None)
        },
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(params.get("tool_stream"), Some(&json!(true)));
}

#[test]
fn stores_zai_tool_stream_support_in_model_compat_metadata() {
    assert!(
        openai_completions::get_compat(&model("zai", "glm-5.1", "https://api.z.ai/api/paas/v4"))
            .zai_tool_stream
    );
    assert!(
        openai_completions::get_compat(&model("zai", "glm-4.7", "https://api.z.ai/api/paas/v4"))
            .zai_tool_stream
    );
    assert!(
        openai_completions::get_compat(&model(
            "zai",
            "glm-5-turbo",
            "https://api.z.ai/api/paas/v4"
        ))
        .zai_tool_stream
    );
    assert!(
        !openai_completions::get_compat(&model(
            "zai",
            "glm-4.5-air",
            "https://api.z.ai/api/paas/v4"
        ))
        .zai_tool_stream
    );
}

#[test]
fn stores_zai_glm_5_2_effort_metadata() {
    let mut model = model("zai", "glm-5.2", "https://api.z.ai/api/paas/v4");
    model.compat = Some(OpenAICompletionsCompat {
        supports_reasoning_effort: Some(true),
        ..OpenAICompletionsCompat::default()
    });
    model.thinking_level_map = HashMap::from([
        (ModelThinkingLevel::Minimal, None),
        (ModelThinkingLevel::Low, Some("high".to_owned())),
        (ModelThinkingLevel::Medium, Some("high".to_owned())),
        (ModelThinkingLevel::High, Some("high".to_owned())),
        (ModelThinkingLevel::XHigh, Some("max".to_owned())),
    ]);

    assert!(openai_completions::get_compat(&model).supports_reasoning_effort);
    assert_eq!(
        model.thinking_level_map.get(&ModelThinkingLevel::Minimal),
        Some(&None)
    );
    assert_eq!(
        model.thinking_level_map.get(&ModelThinkingLevel::XHigh),
        Some(&Some("max".to_owned()))
    );
}

#[test]
fn maps_zai_glm_5_2_thinking_levels_to_reasoning_effort() {
    let mut model = model("zai", "glm-5.2", "https://api.z.ai/api/paas/v4");
    model.compat = Some(OpenAICompletionsCompat {
        supports_reasoning_effort: Some(true),
        ..OpenAICompletionsCompat::default()
    });
    model.thinking_level_map = HashMap::from([
        (ModelThinkingLevel::Low, Some("high".to_owned())),
        (ModelThinkingLevel::Medium, Some("high".to_owned())),
        (ModelThinkingLevel::High, Some("high".to_owned())),
        (ModelThinkingLevel::XHigh, Some("max".to_owned())),
    ]);

    for (reasoning, effort) in [
        (ReasoningEffort::Low, "high"),
        (ReasoningEffort::Medium, "high"),
        (ReasoningEffort::High, "high"),
        (ReasoningEffort::XHigh, "max"),
    ] {
        let params = body(
            &model,
            &user_context(None),
            OpenAICompletionsOptions {
                api_key: Some("test".to_owned()),
                reasoning_effort: Some(reasoning),
                ..OpenAICompletionsOptions::default()
            },
        );
        assert_eq!(
            params.get("thinking"),
            Some(&json!({ "type": "enabled", "clear_thinking": false }))
        );
        assert_eq!(params.get("reasoning_effort"), Some(&json!(effort)));
    }
}

#[test]
fn preserves_zai_thinking_when_replaying_reasoning_content() {
    let mut model = model("zai", "glm-5.2", "https://api.z.ai/api/paas/v4");
    model.compat = Some(OpenAICompletionsCompat {
        supports_reasoning_effort: Some(true),
        ..OpenAICompletionsCompat::default()
    });
    model
        .thinking_level_map
        .insert(ModelThinkingLevel::High, Some("high".to_owned()));
    let context = Context {
        messages: vec![Message::Assistant(AssistantMessage {
            api: "openai-completions".to_owned(),
            provider: "zai".to_owned(),
            model: "glm-5.2".to_owned(),
            content: vec![
                ContentBlock::Thinking {
                    thinking: "prior reasoning".to_owned(),
                    thinking_signature: Some("reasoning_content".to_owned()),
                    redacted: false,
                },
                ContentBlock::ToolCall(ToolCall {
                    id: "call_1".to_owned(),
                    name: "read".to_owned(),
                    arguments: json!({ "path": "README.md" }),
                    thought_signature: None,
                }),
            ],
            stop_reason: StopReason::ToolUse,
        })],
        ..Context::default()
    };
    let params = body(
        &model,
        &context,
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            reasoning_effort: Some(ReasoningEffort::High),
            ..OpenAICompletionsOptions::default()
        },
    );
    let replayed = params["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|message| message["role"] == "assistant")
        .unwrap();

    assert_eq!(
        replayed.get("reasoning_content"),
        Some(&json!("prior reasoning"))
    );
    assert_eq!(
        params.get("thinking"),
        Some(&json!({ "type": "enabled", "clear_thinking": false }))
    );
}

#[test]
fn omits_zai_glm_5_2_reasoning_effort_when_thinking_is_off() {
    let mut model = model("zai", "glm-5.2", "https://api.z.ai/api/paas/v4");
    model.compat = Some(OpenAICompletionsCompat {
        supports_reasoning_effort: Some(true),
        ..OpenAICompletionsCompat::default()
    });
    let params = body(
        &model,
        &user_context(None),
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(params.get("thinking"), Some(&json!({ "type": "disabled" })));
    assert_eq!(params.get("reasoning_effort"), None);
}

#[test]
fn omits_tool_stream_for_unsupported_zai_models() {
    let params = body(
        &model("zai", "glm-4.5-air", "https://api.z.ai/api/paas/v4"),
        &Context {
            tools: vec![tool()],
            ..user_context(None)
        },
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(params.get("tool_stream"), None);
}

#[test]
fn respects_explicit_zai_tool_stream_compat_override() {
    let mut model = model("zai", "glm-4.5-air", "https://api.z.ai/api/paas/v4");
    model.compat = Some(OpenAICompletionsCompat {
        zai_tool_stream: Some(true),
        ..OpenAICompletionsCompat::default()
    });
    let params = body(
        &model,
        &Context {
            tools: vec![tool()],
            ..user_context(None)
        },
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(params.get("tool_stream"), Some(&json!(true)));
}

#[test]
fn omits_tool_stream_when_no_tools_are_provided() {
    let params = body(
        &model("zai", "glm-5.1", "https://api.z.ai/api/paas/v4"),
        &user_context(None),
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(params.get("tool_stream"), None);
}

#[test]
fn maps_non_standard_provider_finish_reason_values_to_stop_reason_error() {
    let result = stream_result(
        &model("zai", "glm-5.1", "https://api.z.ai/api/paas/v4"),
        vec![Some(json!({
            "id": "chatcmpl-zai-error",
            "choices": [{ "delta": {}, "finish_reason": "network_error" }],
        }))],
    );

    assert_eq!(stop_reason_name(result.message.stop_reason), "error");
    assert_eq!(
        result.message.error_message.as_deref(),
        Some("Provider finish_reason: network_error")
    );
}

#[test]
fn ignores_null_stream_chunks_from_openai_compatible_providers() {
    let result = stream_result(
        &model("openai", "gpt-4o-mini", "https://api.openai.com/v1"),
        vec![
            None,
            Some(json!({
                "id": "chatcmpl-test",
                "choices": [{ "delta": { "content": "OK" }, "finish_reason": null }],
            })),
            Some(json!({
                "id": "chatcmpl-test",
                "choices": [{ "delta": {}, "finish_reason": "stop" }],
                "usage": {
                    "prompt_tokens": 3,
                    "completion_tokens": 1,
                    "prompt_tokens_details": { "cached_tokens": 0 },
                    "completion_tokens_details": { "reasoning_tokens": 0 }
                }
            })),
        ],
    );

    assert_eq!(stop_reason_name(result.message.stop_reason), "stop");
    assert_eq!(result.message.error_message, None);
    assert_eq!(result.message.response_id.as_deref(), Some("chatcmpl-test"));
    assert_eq!(result.message.usage.total_tokens, 4);
    assert_eq!(
        result.message.content,
        vec![ContentBlock::Text {
            text: "OK".to_owned()
        }]
    );
}

#[test]
fn errors_when_a_stream_ends_after_only_null_finish_reason_chunks() {
    let result = stream_result(
        &model("openai", "gpt-4o-mini", "https://api.openai.com/v1"),
        vec![
            Some(json!({
                "id": "chatcmpl-truncated",
                "choices": [{ "delta": { "content": "partial answer" }, "finish_reason": null }],
            })),
            Some(json!({
                "id": "chatcmpl-truncated",
                "choices": [{ "delta": { "content": "partial answer" }, "finish_reason": null }],
            })),
        ],
    );

    assert_eq!(stop_reason_name(result.message.stop_reason), "error");
    assert_eq!(
        result.message.error_message.as_deref(),
        Some("Stream ended without finish_reason")
    );
}

#[test]
fn coalesces_tool_call_deltas_by_stable_index_when_provider_mutates_ids_mid_stream() {
    let result = stream_result(
        &model("openai", "gpt-4o-mini", "https://api.openai.com/v1"),
        vec![
            Some(json!({
                "id": "chatcmpl-kimi-bad-stream",
                "choices": [{ "delta": { "tool_calls": [{
                    "index": 0,
                    "id": "functions.read:0",
                    "type": "function",
                    "function": { "name": "read", "arguments": "" }
                }] }, "finish_reason": null }],
            })),
            Some(json!({
                "id": "chatcmpl-kimi-bad-stream",
                "choices": [{ "delta": { "tool_calls": [{
                    "index": 0,
                    "id": "chatcmpl-tool-a",
                    "type": "function",
                    "function": { "name": null, "arguments": "{\"path\":\"README" }
                }] }, "finish_reason": null }],
            })),
            Some(json!({
                "id": "chatcmpl-kimi-bad-stream",
                "choices": [{ "delta": { "tool_calls": [{
                    "index": 0,
                    "id": "chatcmpl-tool-b",
                    "type": "function",
                    "function": { "name": null, "arguments": ".md\"}" }
                }] }, "finish_reason": "tool_calls" }],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 5,
                    "prompt_tokens_details": { "cached_tokens": 0 },
                    "completion_tokens_details": { "reasoning_tokens": 0 }
                }
            })),
        ],
    );

    let tool_indexes = result
        .events
        .iter()
        .filter_map(|event| match event {
            openai_completions::OpenAICompletionsStreamEvent::ToolCallStart { content_index }
            | openai_completions::OpenAICompletionsStreamEvent::ToolCallDelta {
                content_index,
                ..
            }
            | openai_completions::OpenAICompletionsStreamEvent::ToolCallEnd {
                content_index, ..
            } => Some(*content_index),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(stop_reason_name(result.message.stop_reason), "toolUse");
    assert_eq!(tool_indexes, vec![0, 0, 0, 0, 0]);
    assert_eq!(result.message.content.len(), 1);
    match &result.message.content[0] {
        ContentBlock::ToolCall(tool_call) => {
            assert_eq!(tool_call.id, "functions.read:0");
            assert_eq!(tool_call.name, "read");
            assert_eq!(tool_call.arguments, json!({ "path": "README.md" }));
        }
        other => panic!("unexpected content: {other:?}"),
    }
}

#[test]
fn accumulates_mixed_content_reasoning_and_parallel_tool_call_deltas_independently() {
    let result = stream_result(
        &model("openai", "gpt-4o-mini", "https://api.openai.com/v1"),
        vec![
            Some(json!({
                "id": "chatcmpl-mixed-deltas",
                "choices": [{ "delta": {
                    "content": "answer 1",
                    "reasoning_content": "think 1",
                    "tool_calls": [
                        { "index": 0, "id": "tc_read_initial", "type": "function", "function": { "name": "read", "arguments": "{\"path\":\"README" } },
                        { "index": 1, "id": "tc_grep_initial", "type": "function", "function": { "name": "grep", "arguments": "{\"pattern\":\"TODO" } },
                        { "id": "tc_list_no_index", "type": "function", "function": { "name": "list", "arguments": "{\"path\":\"packages" } },
                        { "id": "tc_write_no_index", "type": "function", "function": { "name": "write", "arguments": "{\"path\":\"out" } }
                    ]
                }, "finish_reason": null }],
            })),
            Some(json!({
                "id": "chatcmpl-mixed-deltas",
                "choices": [{ "delta": {
                    "content": " answer 2",
                    "tool_calls": [
                        { "index": 1, "id": "tc_grep_changed", "type": "function", "function": { "arguments": "\",\"path\":\"src" } },
                        { "id": "tc_write_no_index", "type": "function", "function": { "arguments": ".txt\",\"content\":\"ok\"}" } },
                        { "id": "tc_list_no_index", "type": "function", "function": { "arguments": "/ai\"}" } }
                    ]
                }, "finish_reason": null }],
            })),
            Some(json!({
                "id": "chatcmpl-mixed-deltas",
                "choices": [{ "delta": {
                    "content": "\n",
                    "reasoning_content": " think 2",
                    "tool_calls": [
                        { "index": 0, "id": "tc_read_changed", "type": "function", "function": { "arguments": ".md\"}" } },
                        { "index": 1, "type": "function", "function": { "arguments": "\"}" } }
                    ]
                }, "finish_reason": "tool_calls" }],
                "usage": {
                    "prompt_tokens": 10,
                    "completion_tokens": 8,
                    "prompt_tokens_details": { "cached_tokens": 0 },
                    "completion_tokens_details": { "reasoning_tokens": 2 }
                }
            })),
        ],
    );

    let event_types = result.events.iter().map(event_type).collect::<Vec<_>>();
    assert_eq!(stop_reason_name(result.message.stop_reason), "toolUse");
    assert_eq!(
        event_types
            .iter()
            .filter(|kind| **kind == "text_start")
            .count(),
        1
    );
    assert_eq!(
        event_types
            .iter()
            .filter(|kind| **kind == "text_delta")
            .count(),
        3
    );
    assert_eq!(
        event_types
            .iter()
            .filter(|kind| **kind == "text_end")
            .count(),
        1
    );
    assert_eq!(
        event_types
            .iter()
            .filter(|kind| **kind == "thinking_start")
            .count(),
        1
    );
    assert_eq!(
        event_types
            .iter()
            .filter(|kind| **kind == "thinking_delta")
            .count(),
        2
    );
    assert_eq!(
        event_types
            .iter()
            .filter(|kind| **kind == "thinking_end")
            .count(),
        1
    );
    assert_eq!(
        event_types
            .iter()
            .filter(|kind| **kind == "toolcall_start")
            .count(),
        4
    );
    assert_eq!(
        event_types
            .iter()
            .filter(|kind| **kind == "toolcall_delta")
            .count(),
        9
    );
    assert_eq!(
        event_types
            .iter()
            .filter(|kind| **kind == "toolcall_end")
            .count(),
        4
    );
    assert_eq!(result.message.content.len(), 6);
    assert_eq!(
        result.message.content[0],
        ContentBlock::Text {
            text: "answer 1 answer 2\n".to_owned()
        }
    );
    assert_eq!(
        result.message.content[1],
        ContentBlock::Thinking {
            thinking: "think 1 think 2".to_owned(),
            thinking_signature: Some("reasoning_content".to_owned()),
            redacted: false,
        }
    );
    let expected = [
        ("tc_read_initial", "read", json!({ "path": "README.md" })),
        (
            "tc_grep_initial",
            "grep",
            json!({ "pattern": "TODO", "path": "src" }),
        ),
        ("tc_list_no_index", "list", json!({ "path": "packages/ai" })),
        (
            "tc_write_no_index",
            "write",
            json!({ "path": "out.txt", "content": "ok" }),
        ),
    ];
    for (offset, (id, name, arguments)) in expected.into_iter().enumerate() {
        match &result.message.content[offset + 2] {
            ContentBlock::ToolCall(tool_call) => {
                assert_eq!(tool_call.id, id);
                assert_eq!(tool_call.name, name);
                assert_eq!(tool_call.arguments, arguments);
            }
            other => panic!("unexpected content: {other:?}"),
        }
    }
}

#[test]
fn uses_system_messages_for_non_openai_anthropic_openrouter_reasoning_model_instructions() {
    let model = model(
        "openrouter",
        "deepseek/deepseek-v4-pro",
        "https://openrouter.ai/api/v1",
    );
    let messages = openai_completions::convert_messages(
        &model,
        &user_context(Some("Follow instructions.")),
        &openai_completions::get_compat(&model),
    );

    assert_eq!(messages[0].role(), "system");
}

#[test]
fn keeps_developer_messages_for_openai_and_anthropic_openrouter_reasoning_model_instructions() {
    for id in ["openai/gpt-5.2-codex", "anthropic/claude-sonnet-4.5"] {
        let model = model("openrouter", id, "https://openrouter.ai/api/v1");
        let messages = openai_completions::convert_messages(
            &model,
            &user_context(Some("Follow instructions.")),
            &openai_completions::get_compat(&model),
        );

        assert_eq!(messages[0].role(), "developer", "model={id}");
    }
}

#[test]
fn keeps_developer_messages_for_openai_reasoning_model_instructions() {
    let model = model("openai", "gpt-5.5", "https://api.openai.com/v1");
    let messages = openai_completions::convert_messages(
        &model,
        &user_context(Some("Follow instructions.")),
        &openai_completions::get_compat(&model),
    );

    assert_eq!(messages[0].role(), "developer");
}

#[test]
fn stores_openrouter_kimi_k2_6_reasoning_replay_compat_in_builtin_metadata() {
    let mut model = model(
        "openrouter",
        "moonshotai/kimi-k2.6",
        "https://openrouter.ai/api/v1",
    );
    model.compat = Some(OpenAICompletionsCompat {
        supports_developer_role: Some(false),
        requires_reasoning_content_on_assistant_messages: Some(true),
        ..OpenAICompletionsCompat::default()
    });
    let compat = openai_completions::get_compat(&model);

    assert!(!compat.supports_developer_role);
    assert!(compat.requires_reasoning_content_on_assistant_messages);
}

#[test]
fn stores_xiaomi_mimo_reasoning_replay_compat_in_builtin_metadata() {
    let mut model = model(
        "xiaomi-token-plan-ams",
        "mimo-v2",
        "https://api.example.test/v1",
    );
    model.compat = Some(OpenAICompletionsCompat {
        requires_reasoning_content_on_assistant_messages: Some(true),
        thinking_format: Some(ThinkingFormat::DeepSeek),
        supports_developer_role: Some(false),
        ..OpenAICompletionsCompat::default()
    });
    let compat = openai_completions::get_compat(&model);

    assert!(compat.requires_reasoning_content_on_assistant_messages);
    assert_eq!(compat.thinking_format, ThinkingFormat::DeepSeek);
    assert!(!compat.supports_developer_role);
}

#[test]
fn replays_xiaomi_mimo_assistant_tool_calls_with_empty_reasoning_content_when_thinking_is_missing()
{
    let mut model = model(
        "xiaomi-token-plan-ams",
        "mimo-v2",
        "https://api.example.test/v1",
    );
    model.compat = Some(OpenAICompletionsCompat {
        requires_reasoning_content_on_assistant_messages: Some(true),
        thinking_format: Some(ThinkingFormat::DeepSeek),
        supports_reasoning_effort: Some(true),
        ..OpenAICompletionsCompat::default()
    });
    model
        .thinking_level_map
        .insert(ModelThinkingLevel::High, Some("high".to_owned()));
    let context = Context {
        messages: vec![Message::Assistant(AssistantMessage {
            api: "openai-completions".to_owned(),
            provider: "xiaomi-token-plan-ams".to_owned(),
            model: "mimo-v2".to_owned(),
            content: vec![ContentBlock::ToolCall(ToolCall {
                id: "call_1".to_owned(),
                name: "read".to_owned(),
                arguments: json!({ "path": "README.md" }),
                thought_signature: None,
            })],
            stop_reason: StopReason::ToolUse,
        })],
        ..Context::default()
    };
    let params = body(
        &model,
        &context,
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            reasoning_effort: Some(ReasoningEffort::High),
            ..OpenAICompletionsOptions::default()
        },
    );
    let replayed = params["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|message| message["role"] == "assistant")
        .unwrap();

    assert_eq!(replayed.get("reasoning_content"), Some(&json!("")));
    assert_eq!(params.get("thinking"), Some(&json!({ "type": "enabled" })));
    assert_eq!(params.get("reasoning_effort"), Some(&json!("high")));
}

#[test]
fn normalizes_opencode_go_reasoning_deltas_to_reasoning_content_for_replay() {
    let result = stream_result(
        &model("opencode-go", "kimi-k2.6", "https://opencode.ai/api/v1"),
        vec![Some(json!({
            "id": "chatcmpl-opencode-go-reasoning",
            "choices": [{ "delta": { "reasoning": "think" }, "finish_reason": "stop" }],
        }))],
    );

    assert_eq!(
        result.message.content,
        vec![ContentBlock::Thinking {
            thinking: "think".to_owned(),
            thinking_signature: Some("reasoning_content".to_owned()),
            redacted: false,
        }]
    );
}

#[test]
fn keeps_non_opencode_go_reasoning_deltas_on_the_original_reasoning_field() {
    let result = stream_result(
        &model("openai", "gpt-4o-mini", "https://api.openai.com/v1"),
        vec![Some(json!({
            "id": "chatcmpl-reasoning",
            "choices": [{ "delta": { "reasoning": "think" }, "finish_reason": "stop" }],
        }))],
    );

    assert_eq!(
        result.message.content,
        vec![ContentBlock::Thinking {
            thinking: "think".to_owned(),
            thinking_signature: Some("reasoning".to_owned()),
            redacted: false,
        }]
    );
}

#[test]
fn replays_opencode_go_reasoning_thinking_blocks_as_reasoning_content() {
    let model = model("opencode-go", "kimi-k2.6", "https://opencode.ai/api/v1");
    let context = Context {
        messages: vec![Message::Assistant(AssistantMessage {
            api: "openai-completions".to_owned(),
            provider: "opencode-go".to_owned(),
            model: "kimi-k2.6".to_owned(),
            content: vec![
                ContentBlock::Thinking {
                    thinking: "think".to_owned(),
                    thinking_signature: Some("reasoning".to_owned()),
                    redacted: false,
                },
                ContentBlock::ToolCall(ToolCall {
                    id: "call_1".to_owned(),
                    name: "read".to_owned(),
                    arguments: json!({ "path": "README.md" }),
                    thought_signature: None,
                }),
            ],
            stop_reason: StopReason::Stop,
        })],
        ..Context::default()
    };

    let messages = openai_completions::convert_messages(&model, &context, &compat_for_replay());

    match &messages[0] {
        ChatCompletionMessage::Assistant { extra, .. } => {
            assert_eq!(extra.get("reasoning_content"), Some(&json!("think")));
            assert!(!extra.contains_key("reasoning"));
        }
        other => panic!("unexpected message: {other:?}"),
    }
}

#[test]
fn sends_thinking_disabled_for_opencode_go_kimi_k2_6_when_thinking_is_off() {
    let mut model = model("opencode-go", "kimi-k2.6", "https://opencode.ai/api/v1");
    model.compat = Some(OpenAICompletionsCompat {
        thinking_format: Some(ThinkingFormat::DeepSeek),
        ..OpenAICompletionsCompat::default()
    });
    let params = body(
        &model,
        &user_context(None),
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(params.get("thinking"), Some(&json!({ "type": "disabled" })));
}

#[test]
fn sends_thinking_enabled_for_opencode_go_kimi_k2_6_when_thinking_is_enabled() {
    let mut model = model("opencode-go", "kimi-k2.6", "https://opencode.ai/api/v1");
    model.compat = Some(OpenAICompletionsCompat {
        thinking_format: Some(ThinkingFormat::DeepSeek),
        ..OpenAICompletionsCompat::default()
    });
    let params = body(
        &model,
        &user_context(None),
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            reasoning_effort: Some(ReasoningEffort::High),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(params.get("thinking"), Some(&json!({ "type": "enabled" })));
}

#[test]
fn omits_disabled_thinking_for_moonshot_kimi_k2_7_code_models() {
    let mut model = model("moonshotai", "kimi-k2.7-code", "https://api.moonshot.ai/v1");
    model
        .thinking_level_map
        .insert(ModelThinkingLevel::Off, None);
    let params = body(
        &model,
        &user_context(None),
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(params.get("thinking"), None);
}

#[test]
fn keeps_disabled_thinking_for_moonshot_kimi_k2_6_when_thinking_is_off() {
    let mut model = model("moonshotai", "kimi-k2.6", "https://api.moonshot.ai/v1");
    model.compat = Some(OpenAICompletionsCompat {
        thinking_format: Some(ThinkingFormat::DeepSeek),
        ..OpenAICompletionsCompat::default()
    });
    let params = body(
        &model,
        &user_context(None),
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(params.get("thinking"), Some(&json!({ "type": "disabled" })));
}

#[test]
fn sends_max_tokens_for_opencode_completions_models() {
    let params = body(
        &model("opencode", "deepseek-v4", "https://opencode.ai/api/v1"),
        &user_context(None),
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(params.get("max_tokens"), Some(&json!(8192)));
    assert_eq!(params.get("max_completion_tokens"), None);
}

#[test]
fn omits_reasoning_effort_for_opencode_grok_build() {
    let mut model = model("opencode", "grok-build", "https://opencode.ai/api/v1");
    model.compat = Some(OpenAICompletionsCompat {
        supports_reasoning_effort: Some(false),
        ..OpenAICompletionsCompat::default()
    });
    model
        .thinking_level_map
        .insert(ModelThinkingLevel::Off, None);
    let params = body(
        &model,
        &user_context(None),
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            reasoning_effort: Some(ReasoningEffort::High),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(params.get("reasoning_effort"), None);
}

#[test]
fn does_not_double_count_reasoning_tokens_in_completion_usage() {
    let result = stream_result(
        &model("openai", "gpt-4o-mini", "https://api.openai.com/v1"),
        vec![Some(json!({
            "id": "chatcmpl-reasoning-usage",
            "choices": [{ "delta": {}, "finish_reason": "stop" }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 33,
                "prompt_tokens_details": { "cached_tokens": 0 },
                "completion_tokens_details": { "reasoning_tokens": 21 }
            }
        }))],
    );

    assert_eq!(result.message.usage.input, 10);
    assert_eq!(result.message.usage.output, 33);
    assert_eq!(result.message.usage.total_tokens, 43);
}

#[test]
fn preserves_prompt_tokens_details_cache_read_write_fields_from_chunk_usage() {
    let result = stream_result(
        &model("openai", "gpt-4o-mini", "https://api.openai.com/v1"),
        vec![
            Some(json!({
                "id": "chatcmpl-cache-write",
                "choices": [{ "delta": { "content": "OK" }, "finish_reason": null }],
            })),
            Some(json!({
                "id": "chatcmpl-cache-write",
                "choices": [{ "delta": {}, "finish_reason": "stop" }],
                "usage": {
                    "prompt_tokens": 100,
                    "completion_tokens": 5,
                    "prompt_tokens_details": { "cached_tokens": 50, "cache_write_tokens": 30 },
                    "completion_tokens_details": { "reasoning_tokens": 0 }
                }
            })),
        ],
    );

    assert_eq!(result.message.usage.input, 20);
    assert_eq!(result.message.usage.cache_read, 50);
    assert_eq!(result.message.usage.cache_write, 30);
    assert_eq!(result.message.usage.total_tokens, 105);
}

#[test]
fn preserves_prompt_tokens_details_cache_read_write_fields_from_choice_usage_fallback() {
    let result = stream_result(
        &model("openai", "gpt-4o-mini", "https://api.openai.com/v1"),
        vec![
            Some(json!({
                "id": "chatcmpl-cache-write-choice",
                "choices": [{ "delta": { "content": "OK" }, "finish_reason": null }],
            })),
            Some(json!({
                "id": "chatcmpl-cache-write-choice",
                "choices": [{
                    "delta": {},
                    "finish_reason": "stop",
                    "usage": {
                        "prompt_tokens": 100,
                        "completion_tokens": 5,
                        "prompt_tokens_details": { "cached_tokens": 50, "cache_write_tokens": 30 },
                        "completion_tokens_details": { "reasoning_tokens": 0 }
                    }
                }]
            })),
        ],
    );

    assert_eq!(result.message.usage.input, 20);
    assert_eq!(result.message.usage.cache_read, 50);
    assert_eq!(result.message.usage.cache_write, 30);
    assert_eq!(result.message.usage.total_tokens, 105);
}

#[test]
fn uses_openrouter_reasoning_object_instead_of_reasoning_effort() {
    let params = body(
        &model(
            "openrouter",
            "deepseek/deepseek-r1",
            "https://openrouter.ai/api/v1",
        ),
        &user_context(None),
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            reasoning_effort: Some(ReasoningEffort::High),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(params.get("reasoning"), Some(&json!({ "effort": "high" })));
    assert_eq!(params.get("reasoning_effort"), None);
}

#[test]
fn uses_configurable_chat_template_boolean_thinking_kwargs() {
    let mut model = model("local-vllm", "qwen", "http://localhost:8000/v1");
    model.compat = Some(OpenAICompletionsCompat {
        thinking_format: Some(ThinkingFormat::ChatTemplate),
        supports_reasoning_effort: Some(false),
        ..OpenAICompletionsCompat::default()
    });
    let params = body(
        &model,
        &user_context(None),
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            reasoning_effort: Some(ReasoningEffort::High),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(
        params.get("chat_template_kwargs"),
        Some(&json!({ "enable_thinking": true }))
    );
}

#[test]
fn uses_qwen_chat_template_thinking_kwargs() {
    let mut model = model("local-vllm", "qwen", "http://localhost:8000/v1");
    model.compat = Some(OpenAICompletionsCompat {
        thinking_format: Some(ThinkingFormat::QwenChatTemplate),
        supports_reasoning_effort: Some(false),
        ..OpenAICompletionsCompat::default()
    });
    let params = body(
        &model,
        &user_context(None),
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            reasoning_effort: Some(ReasoningEffort::High),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(
        params.get("chat_template_kwargs"),
        Some(&json!({ "enable_thinking": true }))
    );
}

#[test]
fn uses_configurable_chat_template_effort_kwargs_with_static_kwargs() {
    let mut model = model("local-vllm", "qwen", "http://localhost:8000/v1");
    model.compat = Some(OpenAICompletionsCompat {
        thinking_format: Some(ThinkingFormat::ChatTemplate),
        supports_reasoning_effort: Some(false),
        chat_template_kwargs: Some(json!({ "extra": "static" })),
        chat_template_effort_key: Some("thinking_level".to_owned()),
        ..OpenAICompletionsCompat::default()
    });
    let params = body(
        &model,
        &user_context(None),
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            reasoning_effort: Some(ReasoningEffort::High),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(
        params.get("chat_template_kwargs"),
        Some(&json!({ "thinking_level": "high", "extra": "static" }))
    );
}

#[test]
fn uses_ant_ling_compatibility_metadata() {
    let params = body(
        &model("ant-ling", "ant-ling-v1", "https://api.ant-ling.com/v1"),
        &user_context(None),
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            reasoning_effort: Some(ReasoningEffort::High),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(
        params.get("reasoning"),
        Some(&json!({ "enable": true, "effort": "high" }))
    );
    assert_eq!(params.get("reasoning_effort"), None);
}

#[test]
fn omits_ant_ling_reasoning_for_unmapped_direct_reasoning_efforts_and_non_reasoning_models() {
    let mut model = model("ant-ling", "ant-ling-v1", "https://api.ant-ling.com/v1");
    model.reasoning = false;
    let params = body(
        &model,
        &user_context(None),
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            reasoning_effort: Some(ReasoningEffort::High),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(params.get("reasoning"), None);
    assert_eq!(params.get("reasoning_effort"), None);
}
