//! Port of Pi `packages/ai/test/openai-completions-response-model.test.ts`.
//!
//! The Pi test is deterministic and injects a fake OpenAI client. The Rust
//! `openai_completions::stream` entrypoint is still a request-capture blocker and has
//! no fake-client/chunk transport seam yet, so these parity cases stay ignored
//! until that source behavior is ported.

use std::collections::HashMap;

use serde_json::{Value, json};
use zedflow_ai::api::openai_completions::{
    self, Context, Message, Model, ModelInput, OpenAICompletionsOptions, UserMessageContent,
};

const BLOCKER: &str = "openai_completions::stream does not accept an injected OpenAI-compatible chunk stream or decode response chunks yet";

#[derive(Debug, Clone, PartialEq, Eq)]
struct AssistantResult {
    model: String,
    response_model: Option<String>,
    provider: String,
    stop_reason: &'static str,
}

fn open_router_auto() -> Model {
    Model {
        id: "openrouter/auto".to_owned(),
        api: "openai-completions".to_owned(),
        provider: "openrouter".to_owned(),
        base_url: "https://openrouter.ai/api/v1".to_owned(),
        input: vec![ModelInput::Text],
        reasoning: false,
        thinking_level_map: HashMap::new(),
        headers: HashMap::new(),
        compat: None,
    }
}

fn user_hi_context() -> Context {
    Context {
        messages: vec![Message::User {
            content: UserMessageContent::Text("hi".to_owned()),
        }],
        ..Context::default()
    }
}

fn run_openai_completions_chunks(
    model: &Model,
    context: &Context,
    chunks: Vec<Value>,
) -> AssistantResult {
    let options = OpenAICompletionsOptions {
        api_key: Some("test".to_owned()),
        ..OpenAICompletionsOptions::default()
    };
    let error = openai_completions::stream(model, context, Some(&options))
        .expect_err("OpenAI completions response chunk decoding is still a port placeholder");
    assert!(
        error.to_string().contains("request-capture blocker")
            || error.to_string().contains("port placeholder"),
        "unexpected placeholder error: {error}"
    );

    panic!("{BLOCKER}: model={model:?}; context={context:?}; chunks={chunks:?}");
}

#[test]
#[ignore = "openai_completions::stream cannot consume a fake OpenAI chunk stream yet"]
fn surfaces_routed_chunk_model_on_response_model_without_changing_model() {
    let model = open_router_auto();
    let context = user_hi_context();
    let chunks = vec![
        json!({
            "id": "chatcmpl-1",
            "model": "anthropic/claude-opus-4.8",
            "choices": [{ "index": 0, "delta": { "content": "hi" } }],
        }),
        json!({
            "id": "chatcmpl-1",
            "model": "anthropic/claude-opus-4.8",
            "choices": [{ "index": 0, "delta": {}, "finish_reason": "stop" }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "prompt_tokens_details": { "cached_tokens": 0 },
                "completion_tokens_details": { "reasoning_tokens": 0 },
            },
        }),
    ];

    let message = run_openai_completions_chunks(&model, &context, chunks);

    assert_eq!(message.model, "openrouter/auto");
    assert_eq!(
        message.response_model.as_deref(),
        Some("anthropic/claude-opus-4.8")
    );
    assert_eq!(message.provider, "openrouter");
    assert_eq!(message.stop_reason, "stop");
}

#[test]
#[ignore = "openai_completions::stream cannot consume a fake OpenAI chunk stream yet"]
fn leaves_response_model_unset_when_chunks_echo_the_requested_id() {
    let model = open_router_auto();
    let context = user_hi_context();
    let chunks = vec![
        json!({
            "id": "chatcmpl-2",
            "model": "openrouter/auto",
            "choices": [{ "index": 0, "delta": { "content": "hi" } }],
        }),
        json!({
            "id": "chatcmpl-2",
            "model": "openrouter/auto",
            "choices": [{ "index": 0, "delta": {}, "finish_reason": "stop" }],
            "usage": {
                "prompt_tokens": 1,
                "completion_tokens": 1,
                "prompt_tokens_details": { "cached_tokens": 0 },
                "completion_tokens_details": { "reasoning_tokens": 0 },
            },
        }),
    ];

    let message = run_openai_completions_chunks(&model, &context, chunks);

    assert_eq!(message.model, "openrouter/auto");
    assert_eq!(message.response_model, None);
}

#[test]
#[ignore = "openai_completions::stream cannot consume a fake OpenAI chunk stream yet"]
fn ignores_empty_or_missing_chunk_model() {
    let model = open_router_auto();
    let context = user_hi_context();
    let chunks = vec![
        json!({
            "id": "chatcmpl-3",
            "choices": [{ "index": 0, "delta": { "content": "hi" } }],
        }),
        json!({
            "id": "chatcmpl-3",
            "model": "",
            "choices": [{ "index": 0, "delta": { "content": "!" } }],
        }),
        json!({
            "id": "chatcmpl-3",
            "choices": [{ "index": 0, "delta": {}, "finish_reason": "stop" }],
            "usage": {
                "prompt_tokens": 1,
                "completion_tokens": 2,
                "prompt_tokens_details": { "cached_tokens": 0 },
                "completion_tokens_details": { "reasoning_tokens": 0 },
            },
        }),
    ];

    let message = run_openai_completions_chunks(&model, &context, chunks);

    assert_eq!(message.model, "openrouter/auto");
    assert_eq!(message.response_model, None);
}
