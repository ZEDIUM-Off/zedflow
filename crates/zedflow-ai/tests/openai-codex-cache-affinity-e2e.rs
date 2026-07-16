//! Port of Pi `packages/ai/test/openai-codex-cache-affinity-e2e.test.ts`.
//!
//! The source test is a live OpenAI Codex SSE request gated by local auth.
//! It is capability-gated here and uses the Codex SSE live transport directly.

mod common;

use futures::executor::block_on;
use serde_json::json;
use zedflow_ai::api::openai_codex_responses::{
    Context, Model, OpenAICodexResponsesOptions, Transport, stream_live,
};

const EXPECTED_TEXT: &str = "cache affinity e2e success";
const SESSION_ID: &str = "0195d6e4-4cf9-7f44-a2d8-f8f7f49ee9d3";

fn run_live_codex_sse_cache_affinity_request(
    api_key: String,
) -> zedflow_ai::types::AssistantMessage {
    let model = Model {
        id: "gpt-5.5".to_owned(),
        provider: "openai-codex".to_owned(),
        base_url: Some("https://chatgpt.com/backend-api".to_owned()),
        reasoning: true,
        thinking_level_map: Default::default(),
        headers: Default::default(),
        max_tokens: Some(128_000),
    };
    let context = Context {
        system_prompt: Some("You are a helpful assistant. Reply exactly as requested.".to_owned()),
        tools: Vec::new(),
        input: vec![json!({
            "role": "user",
            "content": [{ "type": "input_text", "text": "Reply with exactly: cache affinity e2e success" }]
        })],
    };
    let stream = stream_live(
        &model,
        &context,
        Some(&OpenAICodexResponsesOptions {
            api_key: Some(api_key),
            session_id: Some(SESSION_ID.to_owned()),
            transport: Some(Transport::Sse),
            timeout_ms: Some(30_000),
            ..OpenAICodexResponsesOptions::default()
        }),
    )
    .expect("Codex SSE live stream should start");
    block_on(stream.result())
}

fn response_text(response: &zedflow_ai::types::AssistantMessage) -> String {
    response
        .content
        .iter()
        .filter_map(|block| match block {
            zedflow_ai::types::AssistantContentBlock::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn handles_sse_requests_with_aligned_cache_affinity_identifiers() {
    if let Some(message) = common::live_credentials::openai_codex().skip_message() {
        eprintln!("{message}");
        return;
    }

    let api_key = common::live_credentials::api_key("openai-codex")
        .expect("capability helper reported Codex credentials");
    let response = run_live_codex_sse_cache_affinity_request(api_key);

    assert_ne!(
        response.stop_reason,
        zedflow_ai::types::StopReason::Error,
        "{:?}",
        response.error_message
    );
    assert!(response.error_message.is_none());
    assert!(response_text(&response).contains(EXPECTED_TEXT));
}
