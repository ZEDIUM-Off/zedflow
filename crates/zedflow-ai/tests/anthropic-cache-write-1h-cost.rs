use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use futures::{StreamExt, executor::block_on};
use serde_json::{Value, json};
use zedflow_ai::providers::anthropic::anthropic_provider;
use zedflow_ai::types::{
    Context, Message, Model, ModelCost, ModelInput, StreamOptions, UserMessage, UserMessageContent,
    UserMessageRole,
};

fn model(base_url: String) -> Model {
    Model {
        id: "claude-opus-4-8".to_owned(),
        name: "Claude Opus 4.8".to_owned(),
        api: "anthropic-messages".to_owned(),
        provider: "anthropic".to_owned(),
        base_url,
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 200_000,
        max_tokens: 32_000,
        headers: None,
        compat: None,
    }
}

fn context() -> Context {
    Context {
        system_prompt: None,
        messages: vec![Message::User(UserMessage {
            role: UserMessageRole::User,
            content: UserMessageContent::Text("hi".to_owned()),
            timestamp: 0,
        })],
        tools: None,
    }
}

fn sse(cache_creation: Option<Value>) -> String {
    let mut usage = json!({
        "input_tokens": 100,
        "output_tokens": 0,
        "cache_read_input_tokens": 0,
        "cache_creation_input_tokens": 1_000_000
    });
    if let Some(cache_creation) = cache_creation {
        usage["cache_creation"] = cache_creation;
    }
    [
        ("message_start", json!({ "type": "message_start", "message": { "id": "msg_test", "usage": usage } })),
        ("content_block_start", json!({ "type": "content_block_start", "index": 0, "content_block": { "type": "text", "text": "" } })),
        ("content_block_delta", json!({ "type": "content_block_delta", "index": 0, "delta": { "type": "text_delta", "text": "Hi" } })),
        ("content_block_stop", json!({ "type": "content_block_stop", "index": 0 })),
        ("message_delta", json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn" }, "usage": { "input_tokens": 100, "output_tokens": 5, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 1_000_000 } })),
        ("message_stop", json!({ "type": "message_stop" })),
    ]
    .into_iter()
    .map(|(event, data)| format!("event: {event}\ndata: {data}\n\n"))
    .collect()
}

fn serve(body: String) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture server");
    let address = listener.local_addr().expect("capture server address");
    let handle = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept request");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let read = socket.read(&mut buffer).expect("read request");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            if let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(str::trim)
                            .and_then(|value| value.parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if request.len() >= header_end + 4 + length {
                    break;
                }
            }
        }
        write!(socket, "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}", body.len(), body).expect("write SSE response");
        String::from_utf8(request).expect("request is UTF-8")
    });
    (format!("http://{address}"), handle)
}

fn captured_result(cache_creation: Option<Value>) -> zedflow_ai::types::AssistantMessage {
    let (base_url, server) = serve(sse(cache_creation));
    let model = model(base_url);
    let provider = anthropic_provider().expect("registered Anthropic provider");
    let options = StreamOptions {
        api_key: Some("test-key".to_owned()),
        ..StreamOptions::default()
    };
    let mut stream = provider.stream(&model, &context(), Some(&options));
    assert!(
        !stream.is_done(),
        "registered stream must return before HTTP completes"
    );
    let result_stream = stream.clone();
    let events = block_on(async move {
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }
        events
    });
    let result = block_on(result_stream.result());
    let request = server.join().expect("capture server");
    assert!(request.starts_with("POST /v1/messages HTTP/1.1"));
    assert!(request.to_ascii_lowercase().contains("x-api-key: test-key"));
    assert!(matches!(
        events.first(),
        Some(zedflow_ai::types::AssistantMessageEvent::Start { .. })
    ));
    assert!(events.iter().any(|event| matches!(event, zedflow_ai::types::AssistantMessageEvent::TextDelta { delta, .. } if delta == "Hi")));
    assert!(matches!(
        events.last(),
        Some(zedflow_ai::types::AssistantMessageEvent::Done { .. })
    ));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event,
                zedflow_ai::types::AssistantMessageEvent::Done { .. }
                    | zedflow_ai::types::AssistantMessageEvent::Error { .. }
            ))
            .count(),
        1
    );
    result
}

#[test]
fn prices_one_hour_portion_at_twice_input_and_rest_at_five_minute_rate() {
    let result = captured_result(Some(json!({
        "ephemeral_5m_input_tokens": 600_000,
        "ephemeral_1h_input_tokens": 400_000
    })));

    assert_eq!(result.usage.cache_write, 1_000_000);
    assert_eq!(result.usage.cache_write_1h, Some(400_000));
    assert!((result.usage.cost.cache_write - 7.75).abs() < 1e-10);
}

#[test]
fn falls_back_to_five_minute_rate_without_cache_creation_breakdown() {
    let result = captured_result(None);

    assert_eq!(result.usage.cache_write, 1_000_000);
    assert_eq!(result.usage.cache_write_1h.unwrap_or(0), 0);
    assert!((result.usage.cost.cache_write - 6.25).abs() < 1e-10);
}
