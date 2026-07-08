//! Port of Pi `packages/ai/test/openai-completions-tool-choice.test.ts`.
//!
//! Most Pi cases assert the OpenAI Chat Completions request payload or streamed
//! chunk decoding through a fake OpenAI client. The Rust `stream`/
//! `stream_simple` path is still a documented request-capture blocker, so those cases
//! are represented as ignored parity tests until a local fake-client/request
//! capture seam exists.

use std::collections::HashMap;

use serde_json::{Value, json};
use zedflow_ai::api::openai_completions::{
    self, AssistantMessage, ChatCompletionMessage, ContentBlock, Context, MaxTokensField, Message,
    Model, ModelInput, ResolvedOpenAICompletionsCompat, StopReason, ThinkingFormat, ToolCall,
    UserMessageContent,
};

const REQUEST_BLOCKER: &str = "OpenAI Chat Completions request construction is not ported yet; keep ignored until stream_simple exposes/captures request params without live provider calls.";
const STREAM_BLOCKER: &str = "OpenAI Chat Completions streaming chunk decoding is not ported yet; keep ignored until stream accepts an injected fake chunk stream.";
const MODEL_METADATA_BLOCKER: &str =
    "built-in provider model compat metadata is not fully ported to zedflow-ai yet.";

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

fn blocked_value(reason: &str) -> Value {
    panic!("{reason}")
}

fn blocked_message(reason: &str) -> Value {
    panic!("{reason}")
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
    }
}

#[test]
#[ignore = "request payload capture is not ported"]
fn forwards_tool_choice_from_simple_options_to_payload() {
    let params = blocked_value(REQUEST_BLOCKER);

    assert_eq!(params.get("tool_choice"), Some(&json!("required")));
    assert!(
        params
            .get("tools")
            .and_then(Value::as_array)
            .is_some_and(|tools| !tools.is_empty())
    );
}

#[test]
#[ignore = "request payload capture is not ported"]
fn omits_strict_when_compat_disables_strict_mode() {
    let params = blocked_value(REQUEST_BLOCKER);
    let tool = &params["tools"][0]["function"];

    assert!(tool.is_object());
    assert_eq!(tool.get("strict"), None);
}

#[test]
#[ignore = "reasoning request payload mapping is not ported"]
fn maps_groq_qwen3_reasoning_levels_to_default_reasoning_effort() {
    let params = blocked_value(REQUEST_BLOCKER);

    assert_eq!(params.get("reasoning_effort"), Some(&json!("default")));
}

#[test]
#[ignore = "reasoning request payload mapping is not ported"]
fn keeps_normal_reasoning_effort_for_groq_models_without_compat_mapping() {
    let params = blocked_value(REQUEST_BLOCKER);

    assert_eq!(params.get("reasoning_effort"), Some(&json!("medium")));
}

#[test]
#[ignore = "z.ai request payload mapping is not ported"]
fn enables_tool_stream_for_supported_zai_models_with_tools() {
    let params = blocked_value(REQUEST_BLOCKER);

    assert_eq!(params.get("tool_stream"), Some(&json!(true)));
}

#[test]
#[ignore = "z.ai built-in compat metadata is not ported"]
fn stores_zai_tool_stream_support_in_model_compat_metadata() {
    let metadata = blocked_value(MODEL_METADATA_BLOCKER);

    assert_eq!(metadata["glm-5.1"]["zaiToolStream"], json!(true));
    assert_eq!(metadata["glm-4.7"]["zaiToolStream"], json!(true));
    assert_eq!(metadata["glm-5-turbo"]["zaiToolStream"], json!(true));
    assert!(metadata["glm-4.5-air"].get("zaiToolStream").is_none());
}

#[test]
#[ignore = "z.ai built-in thinking-level metadata is not ported"]
fn stores_zai_glm_5_2_effort_metadata() {
    let metadata = blocked_value(MODEL_METADATA_BLOCKER);

    assert_eq!(metadata["supportsReasoningEffort"], json!(true));
    assert_eq!(
        metadata["thinkingLevelMap"],
        json!({ "minimal": null, "low": "high", "medium": "high", "high": "high", "xhigh": "max" })
    );
}

#[test]
#[ignore = "z.ai request payload mapping is not ported"]
fn maps_zai_glm_5_2_thinking_levels_to_reasoning_effort() {
    let cases = [
        ("low", "high"),
        ("medium", "high"),
        ("high", "high"),
        ("xhigh", "max"),
    ];

    for (reasoning, effort) in cases {
        let params = blocked_value(REQUEST_BLOCKER);
        assert_eq!(
            params.get("thinking"),
            Some(&json!({ "type": "enabled", "clear_thinking": false }))
        );
        assert_eq!(
            params.get("reasoning_effort"),
            Some(&json!(effort)),
            "reasoning={reasoning}"
        );
    }
}

#[test]
#[ignore = "z.ai request payload mapping is not ported"]
fn preserves_zai_thinking_when_replaying_reasoning_content() {
    let params = blocked_value(REQUEST_BLOCKER);
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
#[ignore = "z.ai request payload mapping is not ported"]
fn omits_zai_glm_5_2_reasoning_effort_when_thinking_is_off() {
    let params = blocked_value(REQUEST_BLOCKER);

    assert_eq!(params.get("thinking"), Some(&json!({ "type": "disabled" })));
    assert_eq!(params.get("reasoning_effort"), None);
}

#[test]
#[ignore = "z.ai request payload mapping is not ported"]
fn omits_tool_stream_for_unsupported_zai_models() {
    let params = blocked_value(REQUEST_BLOCKER);

    assert_eq!(params.get("tool_stream"), None);
}

#[test]
#[ignore = "z.ai request payload mapping is not ported"]
fn respects_explicit_zai_tool_stream_compat_override() {
    let params = blocked_value(REQUEST_BLOCKER);

    assert_eq!(params.get("tool_stream"), Some(&json!(true)));
}

#[test]
#[ignore = "z.ai request payload mapping is not ported"]
fn omits_tool_stream_when_no_tools_are_provided() {
    let params = blocked_value(REQUEST_BLOCKER);

    assert_eq!(params.get("tool_stream"), None);
}

#[test]
#[ignore = "stream finish_reason decoding is not ported"]
fn maps_non_standard_provider_finish_reason_values_to_stop_reason_error() {
    let response = blocked_message(STREAM_BLOCKER);

    assert_eq!(response.get("stopReason"), Some(&json!("error")));
}

#[test]
#[ignore = "stream null-chunk handling is not ported"]
fn ignores_null_stream_chunks_from_openai_compatible_providers() {
    let response = blocked_message(STREAM_BLOCKER);

    assert_eq!(
        response["content"],
        json!([{ "type": "text", "text": "hello" }])
    );
    assert_eq!(response.get("stopReason"), Some(&json!("stop")));
}

#[test]
#[ignore = "stream null finish_reason handling is not ported"]
fn errors_when_a_stream_ends_after_only_null_finish_reason_chunks() {
    let error = blocked_message(STREAM_BLOCKER);

    assert!(
        error["message"]
            .as_str()
            .unwrap_or_default()
            .contains("stream ended without final finish_reason")
    );
}

#[test]
#[ignore = "stream tool-call delta coalescing is not ported"]
fn coalesces_tool_call_deltas_by_stable_index_when_provider_mutates_ids_mid_stream() {
    let response = blocked_message(STREAM_BLOCKER);

    assert_eq!(response["stopReason"], json!("toolUse"));
    assert_eq!(response["content"][0]["id"], json!("call_initial"));
    assert_eq!(response["content"][0]["name"], json!("read"));
    assert_eq!(
        response["content"][0]["arguments"],
        json!({ "path": "README.md" })
    );
}

#[test]
#[ignore = "stream mixed content/reasoning/tool-call decoding is not ported"]
fn accumulates_mixed_content_reasoning_and_parallel_tool_call_deltas_independently() {
    let response = blocked_message(STREAM_BLOCKER);

    assert_eq!(response["stopReason"], json!("toolUse"));
    assert_eq!(
        response["content"][0],
        json!({ "type": "text", "text": "answer 1 answer 2\n" })
    );
    assert_eq!(
        response["content"][1],
        json!({ "type": "thinking", "thinking": "think 1 think 2", "thinkingSignature": "reasoning_content" })
    );
    assert_eq!(response["content"][2]["id"], json!("tc_read_initial"));
    assert_eq!(response["content"][3]["id"], json!("tc_grep_initial"));
    assert_eq!(response["content"][4]["id"], json!("tc_list_no_index"));
    assert_eq!(response["content"][5]["id"], json!("tc_write_no_index"));
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
#[ignore = "OpenRouter Kimi K2.6 built-in compat metadata is not ported"]
fn stores_openrouter_kimi_k2_6_reasoning_replay_compat_in_builtin_metadata() {
    let metadata = blocked_value(MODEL_METADATA_BLOCKER);

    assert_eq!(metadata.get("supportsDeveloperRole"), Some(&json!(false)));
    assert_eq!(
        metadata.get("requiresReasoningContentOnAssistantMessages"),
        Some(&json!(true))
    );
}

#[test]
#[ignore = "Xiaomi MiMo built-in compat metadata is not ported"]
fn stores_xiaomi_mimo_reasoning_replay_compat_in_builtin_metadata() {
    let metadata = blocked_value(MODEL_METADATA_BLOCKER);

    assert_eq!(
        metadata.get("requiresReasoningContentOnAssistantMessages"),
        Some(&json!(true))
    );
    assert_eq!(metadata.get("thinkingFormat"), Some(&json!("deepseek")));
    assert_eq!(metadata.get("maxTokensField"), None);
    assert_eq!(metadata.get("supportsDeveloperRole"), None);
}

#[test]
#[ignore = "Xiaomi MiMo request payload mapping is not ported"]
fn replays_xiaomi_mimo_assistant_tool_calls_with_empty_reasoning_content_when_thinking_is_missing()
{
    let params = blocked_value(REQUEST_BLOCKER);
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
#[ignore = "stream reasoning delta decoding is not ported"]
fn normalizes_opencode_go_reasoning_deltas_to_reasoning_content_for_replay() {
    let response = blocked_message(STREAM_BLOCKER);

    assert_eq!(
        response["content"],
        json!([{ "type": "thinking", "thinking": "think", "thinkingSignature": "reasoning_content" }])
    );
}

#[test]
#[ignore = "stream reasoning delta decoding is not ported"]
fn keeps_non_opencode_go_reasoning_deltas_on_the_original_reasoning_field() {
    let response = blocked_message(STREAM_BLOCKER);

    assert_eq!(
        response["content"],
        json!([{ "type": "thinking", "thinking": "think", "thinkingSignature": "reasoning" }])
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
#[ignore = "OpenCode Go thinking request payload mapping is not ported"]
fn sends_thinking_disabled_for_opencode_go_kimi_k2_6_when_thinking_is_off() {
    let params = blocked_value(REQUEST_BLOCKER);

    assert_eq!(params.get("thinking"), Some(&json!({ "type": "disabled" })));
}

#[test]
#[ignore = "OpenCode Go thinking request payload mapping is not ported"]
fn sends_thinking_enabled_for_opencode_go_kimi_k2_6_when_thinking_is_enabled() {
    let params = blocked_value(REQUEST_BLOCKER);

    assert_eq!(params.get("thinking"), Some(&json!({ "type": "enabled" })));
}

#[test]
#[ignore = "Moonshot Kimi thinking request payload mapping is not ported"]
fn omits_disabled_thinking_for_moonshot_kimi_k2_7_code_models() {
    let params = blocked_value(REQUEST_BLOCKER);

    assert_eq!(params.get("thinking"), None);
}

#[test]
#[ignore = "Moonshot Kimi thinking request payload mapping is not ported"]
fn keeps_disabled_thinking_for_moonshot_kimi_k2_6_when_thinking_is_off() {
    let params = blocked_value(REQUEST_BLOCKER);

    assert_eq!(params.get("thinking"), Some(&json!({ "type": "disabled" })));
}

#[test]
#[ignore = "max_tokens request payload mapping is not ported"]
fn sends_max_tokens_for_opencode_completions_models() {
    let params = blocked_value(REQUEST_BLOCKER);

    assert_eq!(params.get("max_tokens"), Some(&json!(8192)));
    assert_eq!(params.get("max_completion_tokens"), None);
}

#[test]
#[ignore = "OpenCode Grok Build reasoning request mapping is not ported"]
fn omits_reasoning_effort_for_opencode_grok_build() {
    let params = blocked_value(REQUEST_BLOCKER);

    assert_eq!(params.get("reasoning_effort"), None);
}

#[test]
#[ignore = "stream usage accounting is not ported"]
fn does_not_double_count_reasoning_tokens_in_completion_usage() {
    let response = blocked_message(STREAM_BLOCKER);

    assert_eq!(response["usage"]["output"], json!(5));
    assert_eq!(response["usage"]["totalTokens"], json!(15));
}

#[test]
#[ignore = "stream usage accounting is not ported"]
fn preserves_prompt_tokens_details_cache_read_write_fields_from_chunk_usage() {
    let response = blocked_message(STREAM_BLOCKER);

    assert_eq!(response["usage"]["cacheRead"], json!(7));
    assert_eq!(response["usage"]["cacheWrite"], json!(3));
}

#[test]
#[ignore = "stream usage accounting is not ported"]
fn preserves_prompt_tokens_details_cache_read_write_fields_from_choice_usage_fallback() {
    let response = blocked_message(STREAM_BLOCKER);

    assert_eq!(response["usage"]["cacheRead"], json!(7));
    assert_eq!(response["usage"]["cacheWrite"], json!(3));
}

#[test]
#[ignore = "OpenRouter reasoning object request mapping is not ported"]
fn uses_openrouter_reasoning_object_instead_of_reasoning_effort() {
    let params = blocked_value(REQUEST_BLOCKER);

    assert_eq!(params.get("reasoning"), Some(&json!({ "effort": "high" })));
    assert_eq!(params.get("reasoning_effort"), None);
}

#[test]
#[ignore = "chat template thinking kwargs request mapping is not ported"]
fn uses_configurable_chat_template_boolean_thinking_kwargs() {
    let params = blocked_value(REQUEST_BLOCKER);

    assert_eq!(
        params.get("chat_template_kwargs"),
        Some(&json!({ "enable_thinking": true }))
    );
}

#[test]
#[ignore = "qwen chat template thinking kwargs request mapping is not ported"]
fn uses_qwen_chat_template_thinking_kwargs() {
    let params = blocked_value(REQUEST_BLOCKER);

    assert_eq!(
        params.get("chat_template_kwargs"),
        Some(&json!({ "enable_thinking": true }))
    );
}

#[test]
#[ignore = "chat template effort kwargs request mapping is not ported"]
fn uses_configurable_chat_template_effort_kwargs_with_static_kwargs() {
    let params = blocked_value(REQUEST_BLOCKER);

    assert_eq!(
        params.get("chat_template_kwargs"),
        Some(&json!({ "thinking_level": "high", "extra": "static" }))
    );
}

#[test]
#[ignore = "Ant Ling built-in compat metadata/request mapping is not fully ported"]
fn uses_ant_ling_compatibility_metadata() {
    let params = blocked_value(REQUEST_BLOCKER);

    assert_eq!(
        params.get("reasoning"),
        Some(&json!({ "enable": true, "effort": "high" }))
    );
    assert_eq!(params.get("reasoning_effort"), None);
}

#[test]
#[ignore = "Ant Ling reasoning request mapping is not fully ported"]
fn omits_ant_ling_reasoning_for_unmapped_direct_reasoning_efforts_and_non_reasoning_models() {
    let params = blocked_value(REQUEST_BLOCKER);

    assert_eq!(params.get("reasoning"), None);
    assert_eq!(params.get("reasoning_effort"), None);
}
