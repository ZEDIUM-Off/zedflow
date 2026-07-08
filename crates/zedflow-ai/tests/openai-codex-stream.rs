//! Port of Pi `packages/ai/test/openai-codex-stream.test.ts`.
//!
//! The Pi test injects fake `fetch`/`WebSocket` transports and validates Codex SSE/WebSocket
//! streaming behavior. Rust now exposes deterministic request capture; full stream-drain parity
//! still waits for a non-live fake SSE/WebSocket seam.

use std::collections::HashMap;

use serde_json::{Value, json};
use zedflow_ai::api::openai_codex_responses::{
    self, Context, Model, OpenAICodexResponsesOptions, OpenAICodexWebSocketDebugStats,
    ReasoningEffort, ServiceTier, SimpleStreamOptions, ThinkingLevel, Transport,
};

const BLOCKER: &str = "Codex stream drain needs a non-live fetch/WebSocket/SSE transport seam, zstd body capture, and AssistantMessageEventStream result processing";

#[derive(Debug, Clone, PartialEq)]
struct AssistantResult {
    stop_reason: &'static str,
    error_message: Option<String>,
    text: Option<String>,
    usage_cost: Option<UsageCost>,
}

#[derive(Debug, Clone, PartialEq)]
struct UsageCost {
    input: f64,
    output: f64,
    total: f64,
}

#[derive(Debug, Clone, PartialEq)]
struct CapturedRequest {
    headers: HashMap<String, String>,
    body: Value,
    body_was_zstd: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct AbortRun {
    result: AssistantResult,
    events: Vec<String>,
    cancelled: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct CachedWebSocketRun {
    result: AssistantResult,
    sent_bodies: Vec<Value>,
    headers: HashMap<String, String>,
    stats: Option<OpenAICodexWebSocketDebugStats>,
    fetch_calls: usize,
    connections: usize,
}

fn mock_token() -> String {
    "aaa.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9hY2NvdW50X2lkIjoiYWNjX3Rlc3QifX0.bbb".to_string()
}

fn model(id: &str) -> Model {
    Model {
        id: id.to_string(),
        provider: "openai-codex".to_string(),
        base_url: Some("https://chatgpt.com/backend-api".to_string()),
        reasoning: true,
        thinking_level_map: HashMap::new(),
        headers: HashMap::new(),
        max_tokens: Some(128_000),
    }
}

fn context() -> Context {
    Context {
        system_prompt: Some("You are a helpful assistant.".to_string()),
        tools: Vec::new(),
        input: vec![json!({
            "role": "user",
            "content": [{ "type": "input_text", "text": "Say hello" }],
        })],
    }
}

fn assert_current_stream_request(
    model: &Model,
    context: &Context,
    options: &OpenAICodexResponsesOptions,
) {
    let stream = openai_codex_responses::stream(model, context, Some(options))
        .expect("Codex request should be prepared");
    assert_eq!(stream.request.max_retries, options.max_retries.unwrap_or(0));
}

fn assert_current_stream_simple_request(
    model: &Model,
    context: &Context,
    options: &SimpleStreamOptions,
) {
    let stream = openai_codex_responses::stream_simple(model, context, Some(options))
        .expect("Codex simple request should be prepared");
    assert_eq!(stream.request.max_retries, options.max_retries.unwrap_or(0));
}

fn sse_options() -> OpenAICodexResponsesOptions {
    OpenAICodexResponsesOptions {
        api_key: Some(mock_token()),
        transport: Some(Transport::Sse),
        ..OpenAICodexResponsesOptions::default()
    }
}

fn run_sse(status: &str, options: OpenAICodexResponsesOptions) -> AssistantResult {
    let _ = status;
    assert_current_stream_request(&model("gpt-5.1-codex"), &context(), &options);
    panic!("{BLOCKER}");
}

fn run_sse_abort_after_headers() -> AbortRun {
    assert_current_stream_request(&model("gpt-5.1-codex"), &context(), &sse_options());
    panic!("{BLOCKER}");
}

fn capture_sse_request(options: OpenAICodexResponsesOptions) -> CapturedRequest {
    assert_current_stream_request(&model("gpt-5.1-codex"), &context(), &options);
    panic!("{BLOCKER}");
}

fn capture_simple_reasoning(model: Model, options: SimpleStreamOptions) -> Value {
    assert_current_stream_simple_request(&model, &context(), &options);
    panic!("{BLOCKER}");
}

fn run_websocket_cached(options: SimpleStreamOptions) -> CachedWebSocketRun {
    assert_current_stream_simple_request(&model("gpt-5.1-codex"), &context(), &options);
    panic!("{BLOCKER}");
}

fn run_websocket(options: OpenAICodexResponsesOptions) -> CachedWebSocketRun {
    assert_current_stream_request(&model("gpt-5.1-codex"), &context(), &options);
    panic!("{BLOCKER}");
}

#[test]
fn openai_codex_stream_source_request_capture_blocks_parity() {
    assert_current_stream_request(&model("gpt-5.1-codex"), &context(), &sse_options());
}

#[test]
#[ignore = "stream cannot consume fake SSE responses yet"]
fn streams_sse_responses_into_assistant_message_event_stream() {
    let result = run_sse("completed", sse_options());

    assert_eq!(result.text.as_deref(), Some("Hello"));
    assert_eq!(result.stop_reason, "stop");
}

#[test]
#[ignore = "stream cannot terminate from fake response.completed SSE before body close yet"]
fn completes_after_response_completed_even_when_the_sse_body_stays_open() {
    let result = run_sse("completed", sse_options());

    assert_eq!(result.text.as_deref(), Some("Hello"));
    assert_eq!(result.stop_reason, "stop");
}

#[test]
#[ignore = "stream cannot map fake response.incomplete SSE events yet"]
fn maps_response_incomplete_to_stop_reason_length_even_when_the_sse_body_stays_open() {
    let result = run_sse("incomplete", sse_options());

    assert_eq!(result.text.as_deref(), Some("Hello"));
    assert_eq!(result.stop_reason, "length");
}

#[test]
#[ignore = "stream has no abortable fake SSE fetch seam yet"]
fn aborts_sse_fetch_after_the_configured_http_timeout_when_response_headers_do_not_arrive() {
    let result = run_sse(
        "timeout-before-headers",
        OpenAICodexResponsesOptions {
            timeout_ms: Some(10),
            ..sse_options()
        },
    );

    assert_eq!(result.stop_reason, "error");
    assert_eq!(
        result.error_message.as_deref(),
        Some("Codex SSE response headers timed out after 10ms")
    );
}

#[test]
#[ignore = "stream has no abortable fake SSE body-read seam yet"]
fn aborts_sse_body_reads_after_response_headers_arrive() {
    let run = run_sse_abort_after_headers();

    assert_eq!(run.result.stop_reason, "aborted");
    assert_eq!(
        run.result.error_message.as_deref(),
        Some("Request was aborted")
    );
    assert!(run.events.contains(&"text_delta:one".to_string()));
    assert!(!run.events.contains(&"text_delta:two".to_string()));
    assert!(run.cancelled);
}

#[test]
#[ignore = "stream cannot capture fake SSE request headers/body yet"]
fn sets_session_id_x_client_request_id_headers_and_prompt_cache_key_when_session_id_is_provided() {
    let request = capture_sse_request(OpenAICodexResponsesOptions {
        session_id: Some("test-session-123".to_string()),
        ..sse_options()
    });

    assert_eq!(
        request.headers.get("session-id"),
        Some(&"test-session-123".to_string())
    );
    assert!(!request.headers.contains_key("session_id"));
    assert_eq!(
        request.headers.get("x-client-request-id"),
        Some(&"test-session-123".to_string())
    );
    assert_eq!(
        request.body.get("prompt_cache_key"),
        Some(&json!("test-session-123"))
    );
}

#[test]
#[ignore = "stream cannot expose on_payload/captured payload yet"]
fn clamps_prompt_cache_key_to_openais_64_character_limit() {
    let request = capture_sse_request(OpenAICodexResponsesOptions {
        session_id: Some("x".repeat(67)),
        ..sse_options()
    });

    assert_eq!(
        request.body.get("prompt_cache_key"),
        Some(&json!("x".repeat(64)))
    );
}

#[test]
#[ignore = "stream_simple cannot capture fake request reasoning payload yet"]
fn preserves_gpt_5_5_xhigh_reasoning_effort_from_simple_options() {
    let mut model = model("gpt-5.5");
    model
        .thinking_level_map
        .insert(ReasoningEffort::XHigh, Some("xhigh".to_string()));
    let reasoning = capture_simple_reasoning(
        model,
        SimpleStreamOptions {
            api_key: Some(mock_token()),
            reasoning: Some(ThinkingLevel::XHigh),
            transport: Some(Transport::Sse),
            ..SimpleStreamOptions::default()
        },
    );

    assert_eq!(reasoning, json!({ "effort": "xhigh", "summary": "auto" }));
}

#[test]
#[ignore = "stream cannot capture fake request reasoning payload yet"]
fn clamps_minimal_reasoning_effort_to_low() {
    for model_id in ["gpt-5.3-codex", "gpt-5.4", "gpt-5.5"] {
        let mut model = model(model_id);
        model
            .thinking_level_map
            .insert(ReasoningEffort::Minimal, Some("low".to_string()));
        let reasoning = capture_simple_reasoning(
            model,
            SimpleStreamOptions {
                api_key: Some(mock_token()),
                reasoning: Some(ThinkingLevel::Minimal),
                transport: Some(Transport::Sse),
                ..SimpleStreamOptions::default()
            },
        );

        assert_eq!(reasoning, json!({ "effort": "low", "summary": "auto" }));
    }
}

#[test]
#[ignore = "stream cannot process fake service_tier usage costs yet"]
fn uses_the_client_sent_service_tier_when_codex_echoes_default() {
    for (model_id, service_tier, multiplier) in [
        ("gpt-5.1-codex", ServiceTier::Flex, 0.5),
        ("gpt-5.1-codex", ServiceTier::Priority, 2.0),
        ("gpt-5.5", ServiceTier::Flex, 0.5),
        ("gpt-5.5", ServiceTier::Priority, 2.5),
    ] {
        let result = run_sse(
            "completed-with-default-service-tier",
            OpenAICodexResponsesOptions {
                service_tier: Some(service_tier),
                ..sse_options()
            },
        );

        assert_eq!(model_id.is_empty(), false);
        assert_eq!(
            result.usage_cost,
            Some(UsageCost {
                input: multiplier,
                output: 2.0 * multiplier,
                total: 3.0 * multiplier,
            })
        );
    }
}

#[test]
#[ignore = "stream cannot capture fake SSE request headers yet"]
fn does_not_set_session_id_x_client_request_id_headers_when_session_id_is_not_provided() {
    let request = capture_sse_request(sse_options());

    assert!(!request.headers.contains_key("session-id"));
    assert!(!request.headers.contains_key("session_id"));
    assert!(!request.headers.contains_key("x-client-request-id"));
}

#[test]
#[ignore = "stream_simple cannot use fake WebSocket cached context yet"]
fn forwards_auto_transport_from_stream_simple_options_and_uses_cached_websocket_context() {
    let run = run_websocket_cached(SimpleStreamOptions {
        api_key: Some(mock_token()),
        session_id: Some("session-auto".to_string()),
        transport: Some(Transport::Auto),
        ..SimpleStreamOptions::default()
    });

    assert_eq!(run.sent_bodies.len(), 1);
    assert_eq!(
        run.headers.get("session-id"),
        Some(&"session-auto".to_string())
    );
    assert!(!run.headers.contains_key("session_id"));
    assert_eq!(
        run.headers.get("x-client-request-id"),
        Some(&"session-auto".to_string())
    );
    assert_eq!(run.fetch_calls, 0);
    assert_eq!(
        run.stats,
        Some(OpenAICodexWebSocketDebugStats {
            cached_context_requests: 1,
            full_context_requests: 1,
            ..OpenAICodexWebSocketDebugStats::default()
        })
    );
}

#[test]
#[ignore = "stream cannot fall back from fake WebSocket connect timeout to fake SSE yet"]
fn falls_back_to_sse_when_websocket_connect_does_not_open_before_the_connect_timeout() {
    let run = run_websocket(OpenAICodexResponsesOptions {
        session_id: Some("ws-connect-timeout".to_string()),
        transport: Some(Transport::Auto),
        timeout_ms: Some(300_000),
        websocket_connect_timeout_ms: Some(50),
        ..sse_options()
    });

    assert_eq!(run.result.text.as_deref(), Some("Hello"));
    assert_eq!(run.fetch_calls, 1);
    let stats = run.stats.expect("debug stats");
    assert_eq!(stats.websocket_failures, 1);
    assert_eq!(stats.sse_fallbacks, 1);
    assert_eq!(stats.websocket_fallback_active, Some(true));
    assert_eq!(
        stats.last_websocket_error.as_deref(),
        Some("WebSocket connect timeout after 50ms")
    );
}

#[test]
#[ignore = "stream cannot reconnect fake WebSockets yet"]
fn reconnects_once_when_the_websocket_connection_limit_is_reached_before_output_starts() {
    let run = run_websocket(OpenAICodexResponsesOptions {
        api_key: Some(mock_token()),
        ..OpenAICodexResponsesOptions::default()
    });

    assert_eq!(run.result.stop_reason, "stop");
    assert_eq!(run.connections, 2);
    assert_eq!(run.fetch_calls, 0);
}

#[test]
#[ignore = "stream cannot fall back from fake idle WebSocket to fake SSE yet"]
fn falls_back_to_sse_when_a_websocket_is_idle_before_the_first_event() {
    let run = run_websocket(OpenAICodexResponsesOptions {
        session_id: Some("ws-idle-before-start".to_string()),
        transport: Some(Transport::Auto),
        timeout_ms: Some(50),
        ..sse_options()
    });

    assert_eq!(run.sent_bodies.len(), 1);
    assert_eq!(run.result.text.as_deref(), Some("Hello"));
    assert_eq!(run.fetch_calls, 1);
    let stats = run.stats.expect("debug stats");
    assert_eq!(stats.websocket_failures, 1);
    assert_eq!(stats.sse_fallbacks, 1);
    assert_eq!(stats.websocket_fallback_active, Some(true));
}

#[test]
#[ignore = "stream cannot report fake idle WebSocket errors yet"]
fn errors_when_a_websocket_is_idle_after_the_stream_started() {
    let run = run_websocket(OpenAICodexResponsesOptions {
        transport: Some(Transport::Auto),
        timeout_ms: Some(50),
        ..sse_options()
    });

    assert_eq!(run.result.stop_reason, "error");
    assert_eq!(
        run.result.error_message.as_deref(),
        Some("WebSocket idle timeout after 50ms")
    );
    assert_eq!(run.fetch_calls, 0);
}

#[test]
#[ignore = "stream cannot age out fake cached WebSocket sessions yet"]
fn opens_a_fresh_cached_websocket_before_the_backend_connection_age_limit() {
    let run = run_websocket(OpenAICodexResponsesOptions {
        session_id: Some("aged-ws-session".to_string()),
        transport: Some(Transport::WebSocketCached),
        ..sse_options()
    });

    assert_eq!(run.connections, 2);
    assert_eq!(
        run.stats,
        Some(OpenAICodexWebSocketDebugStats {
            connections_created: 2,
            connections_reused: 0,
            ..OpenAICodexWebSocketDebugStats::default()
        })
    );
}

#[test]
#[ignore = "stream cannot send fake cached WebSocket input deltas yet"]
fn sends_only_response_input_deltas_in_websocket_cached_mode() {
    let run = run_websocket(OpenAICodexResponsesOptions {
        session_id: Some("session-1".to_string()),
        transport: Some(Transport::WebSocketCached),
        ..sse_options()
    });

    assert_eq!(run.sent_bodies.len(), 2);
    assert_eq!(run.sent_bodies[0].get("store"), Some(&json!(false)));
    assert_eq!(run.sent_bodies[0].get("previous_response_id"), None);
    assert_eq!(
        run.sent_bodies[0].get("input"),
        Some(
            &json!([{ "role": "user", "content": [{ "type": "input_text", "text": "Say hello" }] }])
        )
    );
    assert_eq!(run.sent_bodies[1].get("store"), Some(&json!(false)));
    assert_eq!(
        run.sent_bodies[1].get("previous_response_id"),
        Some(&json!("resp_1"))
    );
    assert_eq!(
        run.sent_bodies[1].get("input"),
        Some(
            &json!([{ "role": "user", "content": [{ "type": "input_text", "text": "Now finish" }] }])
        )
    );
    let stats = run.stats.expect("debug stats");
    assert_eq!(stats.requests, 2);
    assert_eq!(stats.connections_created, 1);
    assert_eq!(stats.connections_reused, 1);
    assert_eq!(stats.cached_context_requests, 2);
    assert_eq!(stats.store_true_requests, 0);
    assert_eq!(stats.full_context_requests, 1);
    assert_eq!(stats.delta_requests, 1);
    assert_eq!(stats.last_delta_input_items, Some(1));
    assert_eq!(stats.last_previous_response_id.as_deref(), Some("resp_1"));
}

#[test]
#[ignore = "stream cannot exercise fake retry-after SSE retries yet"]
fn uses_retry_after_headers_for_sse_retries() {
    for expected_delay in [1_500, 60_000, 45_000] {
        let result = run_sse(
            "retry-after-then-completed",
            OpenAICodexResponsesOptions {
                max_retries: Some(1),
                ..sse_options()
            },
        );

        assert_eq!(expected_delay > 0, true);
        assert_eq!(result.text.as_deref(), Some("Hello"));
    }
}

#[test]
#[ignore = "stream cannot capture zstd-compressed SSE request bodies yet"]
fn zstd_compresses_sse_request_bodies() {
    let large_request = capture_sse_request(sse_options());

    assert_eq!(
        large_request.headers.get("content-encoding"),
        Some(&"zstd".to_string())
    );
    assert!(large_request.body_was_zstd);
    assert_eq!(
        large_request.body["input"][0]["content"][0]["text"],
        json!("compress me ".repeat(400))
    );

    let small_request = capture_sse_request(sse_options());
    assert_eq!(
        small_request.headers.get("content-encoding"),
        Some(&"zstd".to_string())
    );
    assert!(small_request.body_was_zstd);
}

#[test]
#[ignore = "stream cannot exercise fake exponential backoff SSE retries yet"]
fn uses_exponential_backoff_across_repeated_sse_retries_without_retry_headers() {
    let result = run_sse(
        "rate-limit-three-times-then-completed",
        OpenAICodexResponsesOptions {
            max_retries: Some(3),
            ..sse_options()
        },
    );

    assert_eq!([1_000, 2_000, 4_000], [1_000, 2_000, 4_000]);
    assert_eq!(result.text.as_deref(), Some("Hello"));
}
