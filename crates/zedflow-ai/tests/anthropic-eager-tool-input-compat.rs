use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use futures::executor::block_on;
use serde_json::{Value, json};
use zedflow_ai::providers::anthropic::anthropic_provider;
use zedflow_ai::types::{
    AnthropicMessagesCompat, CacheRetention, Context, Message, Model, ModelCompat, ModelCost,
    ModelInput, StreamOptions, Tool, UserMessage, UserMessageContent, UserMessageRole,
};

fn model(base_url: String, supports_eager: Option<bool>) -> Model {
    Model {
        id: "claude-opus-4-8".to_owned(),
        name: "Claude Opus 4.8".to_owned(),
        api: "anthropic-messages".to_owned(),
        provider: "test-anthropic".to_owned(),
        base_url,
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 200_000,
        max_tokens: 32_000,
        headers: None,
        compat: Some(ModelCompat::AnthropicMessages(AnthropicMessagesCompat {
            supports_eager_tool_input_streaming: supports_eager,
            force_adaptive_thinking: Some(true),
            ..AnthropicMessagesCompat::default()
        })),
    }
}

fn context(with_tools: bool) -> Context {
    Context {
        system_prompt: None,
        messages: vec![Message::User(UserMessage {
            role: UserMessageRole::User,
            content: UserMessageContent::Text("Use the tool".to_owned()),
            timestamp: 0,
        })],
        tools: with_tools.then(|| vec![Tool {
            name: "lookup".to_owned(),
            description: "Look up a value".to_owned(),
            parameters: json!({ "type": "object", "properties": { "value": { "type": "string" } }, "required": ["value"] }),
        }]),
    }
}

fn capture(supports_eager: Option<bool>, with_tools: bool) -> (String, Value) {
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
        socket.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").expect("write response");
        String::from_utf8(request).expect("request is UTF-8")
    });
    let model = model(format!("http://{address}"), supports_eager);
    let provider = anthropic_provider().expect("registered Anthropic provider");
    let options = StreamOptions {
        api_key: Some("test-key".to_owned()),
        cache_retention: Some(CacheRetention::None),
        ..StreamOptions::default()
    };
    let stream = provider.stream(&model, &context(with_tools), Some(&options));
    assert!(
        !stream.is_done(),
        "registered stream must return immediately"
    );
    let result = block_on(stream.result());
    assert_eq!(result.stop_reason, zedflow_ai::types::StopReason::Stop);
    let request = server.join().expect("capture server");
    let (headers, body) = request.split_once("\r\n\r\n").expect("HTTP request");
    (
        headers.to_ascii_lowercase(),
        serde_json::from_str(body).expect("JSON request body"),
    )
}

#[test]
fn sends_per_tool_eager_input_streaming_by_default() {
    let (headers, body) = capture(None, true);
    assert_eq!(body["tools"][0]["eager_input_streaming"], json!(true));
    assert!(!headers.contains("anthropic-beta:"));
    assert!(headers.contains("x-api-key: test-key"));
    assert!(headers.contains("anthropic-version: 2023-06-01"));
}

#[test]
fn uses_legacy_fine_grained_beta_when_eager_streaming_is_disabled() {
    let (headers, body) = capture(Some(false), true);
    assert!(body["tools"][0].get("eager_input_streaming").is_none());
    assert!(headers.contains("anthropic-beta: fine-grained-tool-streaming-2025-05-14"));
}

#[test]
fn omits_legacy_beta_when_there_are_no_tools() {
    let (headers, body) = capture(Some(false), false);
    assert!(body.get("tools").is_none());
    assert!(!headers.contains("anthropic-beta:"));
}
