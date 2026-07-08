//! Port of Pi `packages/ai/test/openai-responses-reasoning-replay-e2e.test.ts`.
//!
//! The source test performs live OpenAI Responses and Anthropic requests gated by
//! `OPENAI_API_KEY`/`ANTHROPIC_API_KEY`. P1.T2 forbids live provider calls, and the Rust compat
//! catalog, typed context/message/tool content, provider streaming, and Responses/Codex handoff
//! transports are still documented placeholders, so these parity tests are ignored until those
//! blockers are removed.

const BLOCKER: &str = "live reasoning replay e2e skipped; requires OPENAI_API_KEY/ANTHROPIC_API_KEY plus completed compat::get_model/complete, typed Context/Message/Tool content, and OpenAI Responses/Codex/Anthropic streaming ports";

#[derive(Debug, Clone, PartialEq, Eq)]
enum ContentBlock {
    Text(String),
    Thinking { thinking_signature: Option<String> },
    ToolCall { id: String, name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AssistantResponse {
    stop_reason: &'static str,
    error_message: Option<String>,
    content: Vec<ContentBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CapturedPayload {
    input: Vec<PayloadItem>,
}

type LiveRun = (AssistantResponse, Option<CapturedPayload>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PayloadItem {
    FunctionCall,
    Reasoning,
}

fn response_text(response: &AssistantResponse) -> String {
    response
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text(text) => Some(text.as_str()),
            ContentBlock::Thinking { .. } | ContentBlock::ToolCall { .. } => None,
        })
        .collect()
}

fn count_payload_items(payload: Option<&CapturedPayload>, item: PayloadItem) -> usize {
    payload.map_or(0, |payload| {
        payload.input.iter().filter(|&&entry| entry == item).count()
    })
}

fn run_live_aborted_reasoning_replay_request() -> AssistantResponse {
    let _source_fixture = (
        ("openai", "gpt-5-mini"),
        "OPENAI_API_KEY",
        "You are a helpful assistant. Use the tool.",
        "Use the double_number tool to double 21.",
        "double_number",
        "Doubles a number and returns the result",
        "high",
        ContentBlock::Thinking {
            thinking_signature: Some("required by source test".to_owned()),
        },
        "aborted",
        "Say hello to confirm you can continue.",
    );

    panic!("{BLOCKER}");
}

fn run_live_same_provider_different_model_handoff() -> LiveRun {
    let _source_fixture = (
        ("openai", "gpt-5-mini"),
        ("openai", "gpt-5.2-codex"),
        "OPENAI_API_KEY",
        "You are a helpful assistant. Always use the tool when asked.",
        "Use the double_number tool to double 21.",
        ContentBlock::ToolCall {
            id: "fc_xxx".to_owned(),
            name: "double_number".to_owned(),
        },
        ContentBlock::Text("42".to_owned()),
        "What was the result? Answer with just the number.",
        "high",
    );

    panic!("{BLOCKER}");
}

fn run_live_cross_provider_anthropic_to_openai_codex_handoff() -> LiveRun {
    let _source_fixture = (
        ("anthropic", "claude-sonnet-4-5"),
        ("openai", "gpt-5.2-codex"),
        ("ANTHROPIC_API_KEY", "OPENAI_API_KEY"),
        "You are a helpful assistant. Always use the tool when asked.",
        "Use the double_number tool to double 21.",
        ContentBlock::ToolCall {
            id: "toolu_xxx".to_owned(),
            name: "double_number".to_owned(),
        },
        ContentBlock::Text("42".to_owned()),
        "What was the result? Answer with just the number.",
        5000_u32,
        "high",
    );

    panic!("{BLOCKER}");
}

#[test]
#[ignore = "live provider call skipped; see BLOCKER"]
fn skips_reasoning_only_history_after_an_aborted_turn() {
    let response = run_live_aborted_reasoning_replay_request();

    assert_ne!(
        response.stop_reason, "error",
        "{:?}",
        response.error_message
    );
    assert!(response.error_message.is_none());
    assert!(!response.content.is_empty());
}

#[test]
#[ignore = "live provider call skipped; see BLOCKER"]
fn handles_same_provider_different_model_handoff_with_tool_calls() {
    let (response, captured_payload) = run_live_same_provider_different_model_handoff();

    assert_ne!(
        response.stop_reason, "error",
        "{:?}",
        response.error_message
    );
    assert!(response.error_message.is_none());
    assert!(!response.content.is_empty());

    let function_calls = count_payload_items(captured_payload.as_ref(), PayloadItem::FunctionCall);
    let reasoning_items = count_payload_items(captured_payload.as_ref(), PayloadItem::Reasoning);
    let _debug_counts = (function_calls, reasoning_items);

    assert!(response_text(&response).contains("42"));
}

#[test]
#[ignore = "live provider call skipped; see BLOCKER"]
fn handles_cross_provider_handoff_from_anthropic_to_openai_codex() {
    let (response, captured_payload) = run_live_cross_provider_anthropic_to_openai_codex_handoff();

    let function_calls = count_payload_items(captured_payload.as_ref(), PayloadItem::FunctionCall);
    let reasoning_items = count_payload_items(captured_payload.as_ref(), PayloadItem::Reasoning);
    let _debug_counts = (function_calls, reasoning_items);

    assert_ne!(
        response.stop_reason, "error",
        "{:?}",
        response.error_message
    );
    assert!(response.error_message.is_none());
    assert!(!response.content.is_empty());
    assert!(response_text(&response).contains("42"));
}
