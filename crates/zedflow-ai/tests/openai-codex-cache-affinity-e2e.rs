//! Port of Pi `packages/ai/test/openai-codex-cache-affinity-e2e.test.ts`.
//!
//! The source test is a live OpenAI Codex SSE request gated by local auth. P1.T2 forbids live
//! provider calls, and the Rust Codex transport/compat model catalog are still documented port
//! placeholders, so this parity test is represented as ignored until those blockers are removed.

const BLOCKER: &str = "live OpenAI Codex SSE cache-affinity test skipped; requires local openai-codex credentials plus completed compat::get_model/complete and Codex SSE transport ports";
const EXPECTED_TEXT: &str = "cache affinity e2e success";
const SESSION_ID: &str = "0195d6e4-4cf9-7f44-a2d8-f8f7f49ee9d3";

#[derive(Debug, Clone, PartialEq, Eq)]
enum ContentBlock {
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodexResponse {
    stop_reason: &'static str,
    error_message: Option<String>,
    content: Vec<ContentBlock>,
}

fn run_live_codex_sse_cache_affinity_request() -> CodexResponse {
    let _source_fixture = (
        "openai-codex",
        "gpt-5.5",
        SESSION_ID,
        "sse",
        "You are a helpful assistant. Reply exactly as requested.",
        "Reply with exactly: cache affinity e2e success",
    );

    panic!("{BLOCKER}");
}

fn response_text(response: &CodexResponse) -> String {
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
fn handles_sse_requests_with_aligned_cache_affinity_identifiers() {
    let response = run_live_codex_sse_cache_affinity_request();

    assert_ne!(
        response.stop_reason, "error",
        "{:?}",
        response.error_message
    );
    assert!(response.error_message.is_none());
    assert!(response_text(&response).contains(EXPECTED_TEXT));
}
