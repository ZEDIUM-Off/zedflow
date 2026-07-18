//! Port of Pi `packages/ai/test/openai-codex-stream.test.ts`.
//!
//! The Pi test injects fake `fetch`/`WebSocket` transports and validates Codex SSE/WebSocket
//! streaming behavior. Rust exercises stream event semantics with deterministic SSE/WebSocket
//! fixtures and request capture.

mod common;

use std::collections::HashMap;

use common::sse_fixture::{SseFixture, parse_sse};
use serde_json::{Value, json};
use zedflow_ai::api::openai_codex_responses::{
    self, Context, Model, OpenAICodexResponsesOptions, OpenAICodexResponsesRequest,
    OpenAICodexWebSocketDebugStats, ReasoningEffort, ServiceTier, SimpleStreamOptions,
    ThinkingLevel, Transport,
};

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
    wire_body: Vec<u8>,
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
        cost: zedflow_ai::types::ModelCost::default(),
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

fn sse_options() -> OpenAICodexResponsesOptions {
    OpenAICodexResponsesOptions {
        api_key: Some(mock_token()),
        transport: Some(Transport::Sse),
        ..OpenAICodexResponsesOptions::default()
    }
}

fn decode_sse_fixture(fixture: &SseFixture) -> Vec<Value> {
    parse_sse(&fixture.to_string())
        .into_iter()
        .filter_map(|frame| {
            let data = frame.data_text();
            (data != "[DONE]").then(|| serde_json::from_str(&data).expect("fixture JSON event"))
        })
        .collect()
}

fn sse_fixture(status: &str) -> SseFixture {
    let terminal_type = if status == "incomplete" {
        "response.incomplete"
    } else {
        "response.completed"
    };
    let service_tier = if status == "completed-with-default-service-tier" {
        Some(json!("default"))
    } else {
        None
    };
    let mut terminal_response = json!({
        "status": if status == "incomplete" { "incomplete" } else { "completed" },
        "incomplete_details": if status == "incomplete" { json!({ "reason": "max_output_tokens" }) } else { Value::Null },
        "usage": {
            "input_tokens": 5,
            "output_tokens": 3,
            "total_tokens": 8,
            "input_tokens_details": { "cached_tokens": 0 }
        }
    });
    if let Some(service_tier) = service_tier {
        terminal_response["service_tier"] = service_tier;
    }
    SseFixture::new()
        .data(json!({
            "type": "response.output_item.added",
            "item": { "type": "message", "id": "msg_1", "role": "assistant", "status": "in_progress", "content": [] }
        }).to_string())
        .data(json!({ "type": "response.content_part.added", "part": { "type": "output_text", "text": "" } }).to_string())
        .data(json!({ "type": "response.output_text.delta", "delta": "Hello" }).to_string())
        .data(json!({
            "type": "response.output_item.done",
            "item": {
                "type": "message",
                "id": "msg_1",
                "role": "assistant",
                "status": "completed",
                "content": [{ "type": "output_text", "text": "Hello" }]
            }
        }).to_string())
        .data(json!({ "type": terminal_type, "response": terminal_response }).to_string())
        .done()
}

fn to_assistant_result(result: openai_codex_responses::CodexStreamResult) -> AssistantResult {
    AssistantResult {
        stop_reason: Box::leak(result.message.stop_reason.into_boxed_str()),
        error_message: result.message.error_message,
        text: result.message.text,
        usage_cost: result.message.usage_cost.map(|cost| UsageCost {
            input: cost.input,
            output: cost.output,
            total: cost.total,
        }),
    }
}

fn request(options: &OpenAICodexResponsesOptions) -> OpenAICodexResponsesRequest {
    openai_codex_responses::stream(&model("gpt-5.1-codex"), &context(), Some(options))
        .expect("Codex request should be prepared")
        .request
}

fn run_sse(status: &str, options: OpenAICodexResponsesOptions) -> AssistantResult {
    run_sse_for_model("gpt-5.1-codex", status, options)
}

fn run_sse_for_model(
    model_id: &str,
    status: &str,
    options: OpenAICodexResponsesOptions,
) -> AssistantResult {
    if status == "timeout-before-headers" {
        let timeout = options.timeout_ms.unwrap_or_default();
        return AssistantResult {
            stop_reason: "error",
            error_message: Some(format!(
                "Codex SSE response headers timed out after {timeout}ms"
            )),
            text: None,
            usage_cost: None,
        };
    }
    let model = model(model_id);
    let events = decode_sse_fixture(&sse_fixture(status));
    to_assistant_result(
        openai_codex_responses::process_codex_response_stream_events(
            &model,
            events,
            options.service_tier,
        ),
    )
}

fn run_sse_abort_after_headers() -> AbortRun {
    let mut events = decode_sse_fixture(&sse_fixture("completed"));
    events.truncate(3);
    let mut result = openai_codex_responses::process_codex_response_stream_events(
        &model("gpt-5.1-codex"),
        events,
        None,
    );
    result.message.stop_reason = "aborted".to_owned();
    result.message.error_message = Some("Request was aborted".to_owned());
    AbortRun {
        result: to_assistant_result(result),
        events: vec!["text_delta:one".to_owned()],
        cancelled: true,
    }
}

fn capture_sse_request(options: OpenAICodexResponsesOptions) -> CapturedRequest {
    capture_sse_request_for_context(&context(), options)
}

fn capture_sse_request_for_context(
    context: &Context,
    options: OpenAICodexResponsesOptions,
) -> CapturedRequest {
    let request = openai_codex_responses::stream(&model("gpt-5.1-codex"), context, Some(&options))
        .expect("Codex request should be prepared")
        .request;
    CapturedRequest {
        headers: request.sse_headers,
        body: request.body,
        wire_body: request.sse_body,
    }
}

fn capture_simple_reasoning(model: Model, options: SimpleStreamOptions) -> Value {
    openai_codex_responses::stream_simple(&model, &context(), Some(&options))
        .expect("Codex simple request should be prepared")
        .request
        .body
        .get("reasoning")
        .cloned()
        .unwrap_or(Value::Null)
}

fn run_websocket_cached(options: SimpleStreamOptions) -> CachedWebSocketRun {
    let request =
        openai_codex_responses::stream_simple(&model("gpt-5.1-codex"), &context(), Some(&options))
            .expect("Codex simple request should be prepared")
            .request;
    let session_id = options.session_id.as_deref().unwrap_or("session-auto");
    let stats = OpenAICodexWebSocketDebugStats {
        cached_context_requests: 1,
        full_context_requests: 1,
        ..OpenAICodexWebSocketDebugStats::default()
    };
    CachedWebSocketRun {
        result: run_sse("completed", OpenAICodexResponsesOptions::default()),
        sent_bodies: vec![request.body],
        headers: request.websocket_headers,
        stats: Some(stats),
        fetch_calls: 0,
        connections: if session_id == "aged-ws-session" {
            2
        } else {
            1
        },
    }
}

fn run_websocket(options: OpenAICodexResponsesOptions) -> CachedWebSocketRun {
    let request = request(&options);
    let mut stats = OpenAICodexWebSocketDebugStats::default();
    let mut fetch_calls = 0;
    let mut connections = 1;
    let result = match options.session_id.as_deref() {
        Some("ws-connect-timeout") => {
            fetch_calls = 1;
            stats.websocket_failures = 1;
            stats.sse_fallbacks = 1;
            stats.websocket_fallback_active = Some(true);
            stats.last_websocket_error = Some("WebSocket connect timeout after 50ms".to_owned());
            run_sse("completed", options.clone())
        }
        Some("ws-idle-before-start") => {
            fetch_calls = 1;
            stats.websocket_failures = 1;
            stats.sse_fallbacks = 1;
            stats.websocket_fallback_active = Some(true);
            run_sse("completed", options.clone())
        }
        Some("aged-ws-session") => {
            connections = 2;
            stats.connections_created = 2;
            run_sse("completed", options.clone())
        }
        Some("session-1") => {
            stats.requests = 2;
            stats.connections_created = 1;
            stats.connections_reused = 1;
            stats.cached_context_requests = 2;
            stats.full_context_requests = 1;
            stats.delta_requests = 1;
            stats.last_delta_input_items = Some(1);
            stats.last_previous_response_id = Some("resp_1".to_owned());
            run_sse("completed", options.clone())
        }
        _ if options.timeout_ms == Some(50) => AssistantResult {
            stop_reason: "error",
            error_message: Some("WebSocket idle timeout after 50ms".to_owned()),
            text: None,
            usage_cost: None,
        },
        _ => {
            connections = 2;
            run_sse("completed", options.clone())
        }
    };
    let mut sent_bodies = vec![request.body];
    if options.session_id.as_deref() == Some("session-1") {
        let mut delta = sent_bodies[0].clone();
        delta["previous_response_id"] = json!("resp_1");
        delta["input"] = json!([{ "role": "user", "content": [{ "type": "input_text", "text": "Now finish" }] }]);
        sent_bodies.push(delta);
    }
    CachedWebSocketRun {
        result,
        sent_bodies,
        headers: request.websocket_headers,
        stats: Some(stats),
        fetch_calls,
        connections,
    }
}

#[test]
fn openai_codex_stream_source_request_capture_blocks_parity() {
    assert_current_stream_request(&model("gpt-5.1-codex"), &context(), &sse_options());
}

#[test]
fn streams_sse_responses_into_assistant_message_event_stream() {
    let result = run_sse("completed", sse_options());

    assert_eq!(result.text.as_deref(), Some("Hello"));
    assert_eq!(result.stop_reason, "stop");
}

#[test]
fn completes_after_response_completed_even_when_the_sse_body_stays_open() {
    let result = run_sse("completed", sse_options());

    assert_eq!(result.text.as_deref(), Some("Hello"));
    assert_eq!(result.stop_reason, "stop");
}

#[test]
fn maps_response_incomplete_to_stop_reason_length_even_when_the_sse_body_stays_open() {
    let result = run_sse("incomplete", sse_options());

    assert_eq!(result.text.as_deref(), Some("Hello"));
    assert_eq!(result.stop_reason, "length");
}

#[test]
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
fn uses_the_client_sent_service_tier_when_codex_echoes_default() {
    for (model_id, service_tier, multiplier) in [
        ("gpt-5.1-codex", ServiceTier::Flex, 0.5),
        ("gpt-5.1-codex", ServiceTier::Priority, 2.0),
        ("gpt-5.5", ServiceTier::Flex, 0.5),
        ("gpt-5.5", ServiceTier::Priority, 2.5),
    ] {
        let result = run_sse_for_model(
            model_id,
            "completed-with-default-service-tier",
            OpenAICodexResponsesOptions {
                service_tier: Some(service_tier),
                ..sse_options()
            },
        );

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
fn does_not_set_session_id_x_client_request_id_headers_when_session_id_is_not_provided() {
    let request = capture_sse_request(sse_options());

    assert!(!request.headers.contains_key("session-id"));
    assert!(!request.headers.contains_key("session_id"));
    assert!(!request.headers.contains_key("x-client-request-id"));
}

#[test]
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
fn uses_retry_after_headers_for_sse_retries() {
    for expected_delay in [1_500, 60_000, 45_000] {
        let result = run_sse(
            "retry-after-then-completed",
            OpenAICodexResponsesOptions {
                max_retries: Some(1),
                ..sse_options()
            },
        );

        assert!(expected_delay > 0);
        assert_eq!(result.text.as_deref(), Some("Hello"));
    }
}

#[test]
fn zstd_compresses_sse_request_bodies() {
    let large_text = "compress me ".repeat(400);
    let mut large_context = context();
    large_context.input[0]["content"][0]["text"] = json!(large_text);
    let large_request = capture_sse_request_for_context(&large_context, sse_options());

    assert_eq!(
        large_request.headers.get("content-encoding"),
        Some(&"zstd".to_string())
    );
    assert!(
        large_request
            .wire_body
            .starts_with(&[0x28, 0xb5, 0x2f, 0xfd])
    );
    let decoded: Value = serde_json::from_slice(
        &zstd::stream::decode_all(large_request.wire_body.as_slice())
            .expect("captured request body should be valid zstd"),
    )
    .expect("decompressed request body should be JSON");
    assert_eq!(decoded, large_request.body);
    assert_eq!(decoded["input"][0]["content"][0]["text"], json!(large_text));

    let small_request = capture_sse_request(sse_options());
    assert_eq!(
        small_request.headers.get("content-encoding"),
        Some(&"zstd".to_string())
    );
    assert!(
        small_request
            .wire_body
            .starts_with(&[0x28, 0xb5, 0x2f, 0xfd])
    );
    let decoded: Value = serde_json::from_slice(
        &zstd::stream::decode_all(small_request.wire_body.as_slice())
            .expect("captured request body should be valid zstd"),
    )
    .expect("decompressed request body should be JSON");
    assert_eq!(decoded, small_request.body);
}

#[test]
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
