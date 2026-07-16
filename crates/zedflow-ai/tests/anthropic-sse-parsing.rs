//! Deterministic raw Anthropic SSE parity fixtures.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use std::thread;

use futures::{StreamExt, executor::block_on};
use serde_json::{Value, json};
use zedflow_ai::api::anthropic_messages::assistant_message_from_sse;
use zedflow_ai::providers::anthropic::anthropic_provider;
use zedflow_ai::types::{
    AssistantContentBlock, AssistantMessageEvent, Context, ErrorStopReason, Message, Model,
    ModelCost, ModelInput, StopReason, StreamOptions, Tool, UserMessage, UserMessageContent,
    UserMessageRole,
};

fn test_model() -> Model {
    Model {
        id: "claude-haiku-4-5".to_owned(),
        name: "Claude Haiku 4.5".to_owned(),
        api: "anthropic-messages".to_owned(),
        provider: "anthropic".to_owned(),
        base_url: "https://api.anthropic.com".to_owned(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 5.0,
            cache_read: 0.1,
            cache_write: 1.25,
        },
        context_window: 200_000,
        max_tokens: 4096,
        headers: None,
        compat: None,
    }
}

fn test_context() -> Context {
    Context {
        system_prompt: None,
        messages: vec![Message::User(UserMessage {
            role: UserMessageRole::User,
            content: UserMessageContent::Text("Say hello.".to_owned()),
            timestamp: 0,
        })],
        tools: Some(vec![Tool {
            name: "edit".to_owned(),
            description: "Edit a file".to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "text": { "type": "string" }
                },
                "required": ["path", "text"]
            }),
        }]),
    }
}

fn sse(event: &str, data: Value) -> String {
    format!("event: {event}\ndata: {data}\n\n")
}

fn minimal_anthropic_events() -> String {
    format!(
        "{}{}{}{}{}{}",
        sse(
            "message_start",
            json!({
                "type": "message_start",
                "message": {
                    "id": "msg_test",
                    "usage": {
                        "input_tokens": 12,
                        "output_tokens": 0,
                        "cache_read_input_tokens": 0,
                        "cache_creation_input_tokens": 0
                    }
                }
            })
        ),
        sse(
            "content_block_start",
            json!({
                "type": "content_block_start",
                "index": 0,
                "content_block": { "type": "text", "text": "" }
            })
        ),
        sse(
            "content_block_delta",
            json!({
                "type": "content_block_delta",
                "index": 0,
                "delta": { "type": "text_delta", "text": "Hello" }
            })
        ),
        sse(
            "content_block_stop",
            json!({ "type": "content_block_stop", "index": 0 })
        ),
        sse(
            "message_delta",
            json!({
                "type": "message_delta",
                "delta": { "stop_reason": "end_turn" },
                "usage": { "output_tokens": 5 }
            })
        ),
        sse("message_stop", json!({ "type": "message_stop" })),
    )
}

#[test]
fn repairs_malformed_sse_json_and_malformed_streamed_tool_json() {
    let malformed_tool_json_delta = "{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\\\"A\\H\\\",\\\"text\\\":\\\"col1\tcol2\\\"}\"}}";
    let raw = format!(
        "{}{}event: content_block_delta\ndata: {malformed_tool_json_delta}\n\n{}{}{}",
        sse(
            "message_start",
            json!({
                "type": "message_start",
                "message": { "id": "msg_test", "usage": { "input_tokens": 12 } }
            })
        ),
        sse(
            "content_block_start",
            json!({
                "type": "content_block_start",
                "index": 0,
                "content_block": { "type": "tool_use", "id": "toolu_test", "name": "edit", "input": {} }
            })
        ),
        sse(
            "content_block_stop",
            json!({ "type": "content_block_stop", "index": 0 })
        ),
        sse(
            "message_delta",
            json!({ "type": "message_delta", "delta": { "stop_reason": "tool_use" }, "usage": { "output_tokens": 5 } })
        ),
        sse("message_stop", json!({ "type": "message_stop" })),
    );

    let result = assistant_message_from_sse(&test_model(), &test_context(), &raw, false).unwrap();

    assert_eq!(result.stop_reason, StopReason::ToolUse);
    let AssistantContentBlock::ToolCall(tool_call) = &result.content[0] else {
        panic!("expected tool call");
    };
    assert_eq!(tool_call.arguments.get("path"), Some(&json!("A\\H")));
    assert_eq!(tool_call.arguments.get("text"), Some(&json!("col1\tcol2")));
}

#[test]
fn preserves_refusal_stop_details_from_message_delta() {
    let explanation = "This request triggered restrictions.";
    let raw = format!(
        "{}{}{}",
        sse(
            "message_start",
            json!({ "type": "message_start", "message": { "id": "msg_01", "usage": { "input_tokens": 412 } } })
        ),
        sse(
            "message_delta",
            json!({
                "type": "message_delta",
                "delta": {
                    "stop_reason": "refusal",
                    "stop_details": { "type": "refusal", "category": "cyber", "explanation": explanation }
                },
                "usage": { "output_tokens": 0 }
            })
        ),
        sse("message_stop", json!({ "type": "message_stop" })),
    );

    let result = assistant_message_from_sse(&test_model(), &test_context(), &raw, false).unwrap();

    assert_eq!(result.stop_reason, StopReason::Error);
    assert_eq!(result.error_message.as_deref(), Some(explanation));
}

#[test]
fn ignores_unknown_sse_events_after_message_stop() {
    let raw = format!(
        "{}event: done\ndata: [DONE]\n\nevent: proxy.stats\ndata: not json\n\n",
        minimal_anthropic_events()
    );

    let result = assistant_message_from_sse(&test_model(), &test_context(), &raw, false).unwrap();

    assert_eq!(result.stop_reason, StopReason::Stop);
    assert_eq!(result.error_message, None);
    let AssistantContentBlock::Text(text) = &result.content[0] else {
        panic!("expected text");
    };
    assert_eq!(text.text, "Hello");
}

fn serve_status(status: u16, body: &'static str) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture server");
    let address = listener.local_addr().expect("capture server address");
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept request");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let read = socket.read(&mut buffer).expect("read request");
            request.extend_from_slice(&buffer[..read]);
            let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n")
            else {
                continue;
            };
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if request.len() >= header_end + 4 + length {
                break;
            }
        }
        write!(socket, "HTTP/1.1 {status} Test\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).expect("write response");
        String::from_utf8(request).expect("request is UTF-8")
    });
    (format!("http://{address}"), server)
}

#[test]
fn production_transport_preserves_non_success_status_and_body() {
    let (base_url, server) = serve_status(
        429,
        r#"{"type":"error","error":{"message":"rate limited"}}"#,
    );
    let mut model = test_model();
    model.base_url = base_url;
    let observed_status = Arc::new(AtomicU16::new(0));
    let hook_status = Arc::clone(&observed_status);
    let options = StreamOptions {
        api_key: Some("test-key".to_owned()),
        on_payload: Some(Arc::new(|payload, _| {
            Box::pin(async move { Ok(Some(payload)) })
        })),
        on_response: Some(Arc::new(move |response, _| {
            hook_status.store(response.status, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        })),
        ..StreamOptions::default()
    };
    let provider = anthropic_provider().expect("registered Anthropic provider");
    let mut stream = provider.stream(&model, &test_context(), Some(&options));
    assert!(!stream.is_done());
    let event = block_on(stream.next()).expect("terminal error event");
    let AssistantMessageEvent::Error { reason, error } = event else {
        panic!("expected registered error event");
    };
    assert_eq!(reason, ErrorStopReason::Error);
    assert!(
        error
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains("429") && message.contains("rate limited"))
    );
    assert_eq!(block_on(stream.next()), None);
    assert_eq!(observed_status.load(Ordering::SeqCst), 429);
    let request = server.join().expect("capture server");
    assert!(request.to_ascii_lowercase().contains("x-api-key: test-key"));
}

#[test]
fn registered_transport_parses_sse_lines_split_across_http_chunks() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture server");
    let address = listener.local_addr().expect("capture server address");
    let body = minimal_anthropic_events().into_bytes();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept request");
        let mut request = [0_u8; 4096];
        let _ = socket.read(&mut request).expect("read request");
        socket.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n").expect("write headers");
        for chunk in [&body[..37], &body[37..91], &body[91..]] {
            write!(socket, "{:X}\r\n", chunk.len()).expect("write chunk length");
            socket.write_all(chunk).expect("write split SSE chunk");
            socket.write_all(b"\r\n").expect("write chunk ending");
            socket.flush().expect("flush chunk");
            thread::sleep(std::time::Duration::from_millis(5));
        }
        socket.write_all(b"0\r\n\r\n").expect("finish chunks");
    });
    let mut model = test_model();
    model.base_url = format!("http://{address}");
    let provider = anthropic_provider().expect("registered Anthropic provider");
    let options = StreamOptions {
        api_key: Some("test-key".to_owned()),
        ..StreamOptions::default()
    };
    let mut stream = provider.stream(&model, &test_context(), Some(&options));
    assert!(!stream.is_done());
    let events = block_on(async {
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }
        events
    });
    server.join().expect("capture server");
    assert!(events.iter().any(
        |event| matches!(event, AssistantMessageEvent::TextDelta { delta, .. } if delta == "Hello")
    ));
    assert!(matches!(
        events.last(),
        Some(AssistantMessageEvent::Done { .. })
    ));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event,
                AssistantMessageEvent::Done { .. } | AssistantMessageEvent::Error { .. }
            ))
            .count(),
        1
    );
}

#[test]
fn abort_wins_against_ready_multi_event_chunk() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture server");
    let address = listener.local_addr().expect("capture server address");
    let body = format!(
        "{}{}{}{}{}{}{}",
        sse(
            "message_start",
            json!({ "type": "message_start", "message": { "id": "msg", "usage": {} } })
        ),
        sse(
            "content_block_start",
            json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" } })
        ),
        sse(
            "content_block_delta",
            json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": "first" } })
        ),
        sse(
            "content_block_delta",
            json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": "later" } })
        ),
        sse(
            "content_block_stop",
            json!({ "type": "content_block_stop", "index": 0 })
        ),
        sse(
            "message_delta",
            json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn" }, "usage": {} })
        ),
        sse("message_stop", json!({ "type": "message_stop" }))
    );
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept request");
        let mut request = [0_u8; 4096];
        let _ = socket.read(&mut request).expect("read request");
        write!(socket, "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}", body.len(), body).expect("write ready multi-event response");
    });
    let controller = zedflow_ai::utils::abort_signals::AbortController::new();
    let hook_controller = controller.clone();
    let mut model = test_model();
    model.base_url = format!("http://{address}");
    let options = StreamOptions {
        api_key: Some("test-key".to_owned()),
        signal: Some(controller.signal()),
        on_response: Some(Arc::new(move |_, _| {
            hook_controller.abort();
            Box::pin(async { Ok(()) })
        })),
        ..StreamOptions::default()
    };
    let provider = anthropic_provider().expect("registered Anthropic provider");
    let mut stream = provider.stream(&model, &test_context(), Some(&options));
    let result_stream = stream.clone();
    let events = block_on(async {
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }
        events
    });
    let result = block_on(result_stream.result());
    server.join().expect("capture server");

    assert!(matches!(
        events.as_slice(),
        [AssistantMessageEvent::Error {
            reason: ErrorStopReason::Aborted,
            ..
        }]
    ));
    assert_eq!(result.stop_reason, StopReason::Aborted);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AssistantMessageEvent::TextDelta { .. }))
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event,
                AssistantMessageEvent::Done { .. } | AssistantMessageEvent::Error { .. }
            ))
            .count(),
        1
    );
}

#[test]
fn production_transport_honors_abort_before_dispatch() {
    let controller = zedflow_ai::utils::abort_signals::AbortController::new();
    controller.abort();
    let options = StreamOptions {
        api_key: Some("test-key".to_owned()),
        signal: Some(controller.signal()),
        ..StreamOptions::default()
    };
    let provider = anthropic_provider().expect("registered Anthropic provider");
    let mut stream = provider.stream(&test_model(), &test_context(), Some(&options));
    let event = block_on(stream.next()).expect("terminal abort event");
    let AssistantMessageEvent::Error { reason, error } = event else {
        panic!("expected abort event");
    };
    assert_eq!(reason, ErrorStopReason::Aborted);
    assert_eq!(error.stop_reason, StopReason::Aborted);
    assert_eq!(error.error_message.as_deref(), Some("Request was aborted"));
    assert_eq!(block_on(stream.next()), None);
}

#[test]
fn production_transport_honors_abort_while_reading_sse() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture server");
    let address = listener.local_addr().expect("capture server address");
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept request");
        let mut buffer = [0_u8; 4096];
        let _ = socket.read(&mut buffer).expect("read request");
        let before = concat!(
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg\",\"usage\":{}}}\n\n",
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"before\"}}\n\n"
        );
        write!(socket, "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n{:X}\r\n{}\r\n", before.len(), before).expect("write first SSE chunk");
        socket.flush().expect("flush first SSE chunk");
        thread::sleep(std::time::Duration::from_millis(100));
        let after = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"after\"}}\n\n";
        let _ = write!(socket, "{:X}\r\n{}\r\n0\r\n\r\n", after.len(), after);
    });
    let controller = zedflow_ai::utils::abort_signals::AbortController::new();
    let mut model = test_model();
    model.base_url = format!("http://{address}");
    let options = StreamOptions {
        api_key: Some("test-key".to_owned()),
        signal: Some(controller.signal()),
        ..StreamOptions::default()
    };
    let provider = anthropic_provider().expect("registered Anthropic provider");
    let mut stream = provider.stream(&model, &test_context(), Some(&options));
    assert!(
        !stream.is_done(),
        "registered stream must return immediately"
    );
    let events = block_on(async {
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            let saw_before = matches!(&event, AssistantMessageEvent::TextDelta { delta, .. } if delta == "before");
            events.push(event);
            if saw_before {
                controller.abort();
            }
        }
        events
    });
    server.join().expect("capture server");
    assert!(events.iter().any(
        |event| matches!(event, AssistantMessageEvent::TextDelta { delta, .. } if delta == "before")
    ));
    assert!(!events.iter().any(
        |event| matches!(event, AssistantMessageEvent::TextDelta { delta, .. } if delta == "after")
    ));
    assert!(matches!(
        events.last(),
        Some(AssistantMessageEvent::Error {
            reason: ErrorStopReason::Aborted,
            ..
        })
    ));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event,
                AssistantMessageEvent::Done { .. } | AssistantMessageEvent::Error { .. }
            ))
            .count(),
        1
    );
}
