use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use futures::executor::block_on;
use serde_json::{Value, json};
use zedflow_ai::providers::anthropic::anthropic_provider;
use zedflow_ai::types::{
    AnthropicMessagesCompat, CacheRetention, Context, Message, Model, ModelCompat, ModelCost,
    ModelInput, SimpleStreamOptions, StreamOptions, ThinkingBudgets, ThinkingLevel, UserMessage,
    UserMessageContent, UserMessageRole,
};

fn model(base_url: String, force_adaptive: Option<bool>) -> Model {
    Model {
        id: "vendor--claude-opus-latest".to_owned(),
        name: "Vendor Proxy Opus Latest".to_owned(),
        api: "anthropic-messages".to_owned(),
        provider: "vendor-proxy".to_owned(),
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
            force_adaptive_thinking: force_adaptive,
            ..AnthropicMessagesCompat::default()
        })),
    }
}

fn context() -> Context {
    Context {
        system_prompt: None,
        messages: vec![Message::User(UserMessage {
            role: UserMessageRole::User,
            content: UserMessageContent::Text("Hello".to_owned()),
            timestamp: 0,
        })],
        tools: None,
    }
}

fn capture(
    force_adaptive: Option<bool>,
    reasoning: Option<ThinkingLevel>,
    thinking_budgets: Option<ThinkingBudgets>,
) -> Value {
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
    let model = model(format!("http://{address}"), force_adaptive);
    let provider = anthropic_provider().expect("registered Anthropic provider");
    let options = SimpleStreamOptions {
        stream: StreamOptions {
            api_key: Some("fake-key".to_owned()),
            cache_retention: Some(CacheRetention::None),
            ..StreamOptions::default()
        },
        reasoning,
        thinking_budgets,
    };
    let stream = provider.stream_simple(&model, &context(), Some(&options));
    assert!(
        !stream.is_done(),
        "registered simple stream must return immediately"
    );
    let result = block_on(stream.result());
    assert_eq!(result.stop_reason, zedflow_ai::types::StopReason::Stop);
    let request = server.join().expect("capture server");
    let (_, body) = request.split_once("\r\n\r\n").expect("HTTP request");
    serde_json::from_str(body).expect("JSON request body")
}

#[test]
fn custom_model_uses_budget_thinking_by_default() {
    let payload = capture(None, Some(ThinkingLevel::Medium), None);
    assert_eq!(payload["thinking"]["type"], json!("enabled"));
    assert!(payload.get("output_config").is_none());
}

#[test]
fn compat_override_forces_adaptive_thinking() {
    let payload = capture(Some(true), Some(ThinkingLevel::Medium), None);
    assert_eq!(
        payload["thinking"],
        json!({ "type": "adaptive", "display": "summarized" })
    );
    assert_eq!(payload["output_config"], json!({ "effort": "medium" }));
}

#[test]
fn adaptive_model_supports_native_xhigh_effort() {
    let payload = capture(Some(true), Some(ThinkingLevel::XHigh), None);
    assert_eq!(
        payload["thinking"],
        json!({ "type": "adaptive", "display": "summarized" })
    );
    assert_eq!(payload["output_config"], json!({ "effort": "xhigh" }));
}

#[test]
fn adaptive_model_can_opt_out() {
    let payload = capture(Some(false), Some(ThinkingLevel::Medium), None);
    assert_eq!(payload["thinking"]["type"], json!("enabled"));
    assert!(payload.get("output_config").is_none());
}

#[test]
fn adaptive_model_maps_low_and_high_efforts() {
    let low = capture(Some(true), Some(ThinkingLevel::Low), None);
    let high = capture(Some(true), Some(ThinkingLevel::High), None);
    assert_eq!(low["output_config"], json!({ "effort": "low" }));
    assert_eq!(high["output_config"], json!({ "effort": "high" }));
}

#[test]
fn budget_model_uses_custom_medium_budget() {
    let payload = capture(
        Some(false),
        Some(ThinkingLevel::Medium),
        Some(ThinkingBudgets {
            medium: Some(4096),
            ..ThinkingBudgets::default()
        }),
    );
    assert_eq!(payload["thinking"]["budget_tokens"], json!(4096));
}

#[test]
fn reasoning_off_preserves_disabled_thinking_regardless_of_override() {
    let payload = capture(Some(true), None, None);
    assert_eq!(payload["thinking"], json!({ "type": "disabled" }));
    assert!(payload.get("output_config").is_none());
}
