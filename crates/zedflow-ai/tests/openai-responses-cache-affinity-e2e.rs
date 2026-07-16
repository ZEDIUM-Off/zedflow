//! Port of Pi `packages/ai/test/openai-responses-cache-affinity-e2e.test.ts`.
//!
//! The source test is a live OpenAI Responses request gated by `OPENAI_API_KEY`. P1.T2 forbids
//! live provider calls, and the Rust compat model catalog/complete path plus Responses streaming
//! transport are still documented port placeholders, so this parity test is ignored until those
//! blockers are removed.

const BLOCKER: &str = "live OpenAI Responses cache-affinity test skipped; requires OPENAI_API_KEY plus completed compat::get_model/complete and OpenAI Responses streaming transport ports";
const EXPECTED_TEXT: &str = "openai cache affinity e2e success";
const SESSION_ID: &str = "0195d6e4-4cf9-7f44-a2d8-f8f7f49ee9d3";

#[allow(
    dead_code,
    reason = "constructed only by the capability-gated live response"
)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum ContentBlock {
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OpenAIResponsesResponse {
    stop_reason: &'static str,
    error_message: Option<String>,
    content: Vec<ContentBlock>,
}

fn run_live_openai_responses_cache_affinity_request() -> OpenAIResponsesResponse {
    let _source_fixture = (
        "openai",
        "gpt-5.4",
        SESSION_ID,
        "You are a helpful assistant. Reply exactly as requested.",
        "Reply with exactly: openai cache affinity e2e success",
    );

    panic!("{BLOCKER}");
}

fn response_text(response: &OpenAIResponsesResponse) -> String {
    response
        .content
        .iter()
        .map(|block| match block {
            ContentBlock::Text(text) => text.as_str(),
        })
        .collect()
}

#[test]
#[ignore = "live provider call skipped; see BLOCKER"]
fn handles_direct_openai_responses_requests_with_aligned_cache_affinity_identifiers() {
    let response = run_live_openai_responses_cache_affinity_request();

    assert_ne!(
        response.stop_reason, "error",
        "{:?}",
        response.error_message
    );
    assert!(response.error_message.is_none());
    assert!(response_text(&response).contains(EXPECTED_TEXT));
}
