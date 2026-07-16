mod common;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use common::http_capture::{CapturedRequest, normalize_header_name};
use futures::executor::block_on;
use serde_json::{Value, json};
use zedflow_ai::api::anthropic_messages::AnthropicOptions;
use zedflow_ai::compat::{get_models, get_providers};
use zedflow_ai::env_api_keys::get_env_api_key;
use zedflow_ai::providers::anthropic::anthropic_provider;
use zedflow_ai::types::{
    AnthropicMessagesCompat, CacheRetention, Context, Message, Model, ModelCompat, ModelCost,
    ModelInput, StreamOptions, Tool, UserMessage, UserMessageContent, UserMessageRole,
};

#[derive(Debug, Clone)]
struct AnthropicEagerE2ECase {
    name: String,
}

fn anthropic_message_model_names() -> Vec<String> {
    let models = get_models().expect("compat::get_models should expose the generated catalog");
    get_providers()
        .expect("compat::get_providers should expose generated providers")
        .into_iter()
        .flat_map(|provider| {
            models
                .iter()
                .filter(|model| model.provider == provider && model.api == "anthropic-messages")
                .map(|model| format!("{provider}/{}", model.id))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn anthropic_messages_cases() -> Vec<AnthropicEagerE2ECase> {
    let models = get_models().expect("compat::get_models should expose the generated catalog");
    get_providers()
        .expect("compat::get_providers should expose generated providers")
        .into_iter()
        .flat_map(|provider| {
            models
                .iter()
                .filter(|model| model.provider == provider && model.api == "anthropic-messages")
                .map(|model| AnthropicEagerE2ECase {
                    name: format!("{provider}/{}", model.id),
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn full_model(supports_eager_tool_input_streaming: Option<bool>) -> Model {
    Model {
        id: "claude-sonnet-4-5".to_owned(),
        name: "Claude Sonnet 4.5".to_owned(),
        api: "anthropic-messages".to_owned(),
        provider: "anthropic".to_owned(),
        base_url: "https://api.anthropic.com".to_owned(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 200_000,
        max_tokens: 64_000,
        headers: None,
        compat: Some(ModelCompat::AnthropicMessages(AnthropicMessagesCompat {
            supports_eager_tool_input_streaming,
            supports_cache_control_on_tools: Some(true),
            ..AnthropicMessagesCompat::default()
        })),
    }
}

fn echo_tool() -> Tool {
    Tool {
        name: "echo_value".to_owned(),
        description: "Echo a string value".to_owned(),
        parameters: json!({
            "type": "object",
            "properties": {
                "value": { "type": "string", "description": "The value to echo" }
            },
            "required": ["value"]
        }),
    }
}

fn tool_context() -> Context {
    Context {
        system_prompt: Some("You are a concise assistant. Use tools when useful.".to_owned()),
        messages: vec![Message::User(UserMessage {
            role: UserMessageRole::User,
            content: UserMessageContent::Text(
                "Call echo_value with value set to eager-input-streaming-compat.".to_owned(),
            ),
            timestamp: 0,
        })],
        tools: Some(vec![echo_tool()]),
    }
}

fn capture_request(model: &Model, options: &AnthropicOptions) -> CapturedRequest {
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
        request
    });
    let mut model = model.clone();
    model.base_url = format!("http://{address}");
    let provider = anthropic_provider().expect("registered Anthropic provider");
    let stream = provider.stream(&model, &tool_context(), Some(&options.stream));
    assert_eq!(
        block_on(stream.result()).stop_reason,
        zedflow_ai::types::StopReason::Stop
    );
    let raw =
        String::from_utf8(server.join().expect("capture server")).expect("HTTP request UTF-8");
    let (head, body) = raw.split_once("\r\n\r\n").expect("HTTP request");
    let mut lines = head.lines();
    let request_line = lines.next().expect("request line");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or_default();
    let headers = lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((normalize_header_name(name), value.trim().to_owned()))
        })
        .collect();
    CapturedRequest {
        method,
        url: format!("http://{address}{path}"),
        headers,
        body: Some(body.as_bytes().to_vec()),
    }
}

fn options() -> AnthropicOptions {
    AnthropicOptions {
        stream: StreamOptions {
            api_key: Some("fake-key".to_owned()),
            cache_retention: Some(CacheRetention::Short),
            max_tokens: Some(128),
            ..StreamOptions::default()
        },
        thinking_enabled: Some(false),
        ..AnthropicOptions::default()
    }
}

fn first_tool(payload: &Value) -> &Value {
    payload
        .get("tools")
        .and_then(Value::as_array)
        .and_then(|tools| tools.first())
        .expect("Anthropic payload should include the echo tool")
}

#[test]
fn covers_every_generated_anthropic_messages_model() {
    let mut actual = anthropic_messages_cases()
        .into_iter()
        .map(|test_case| test_case.name)
        .collect::<Vec<_>>();
    let mut expected = anthropic_message_model_names();

    actual.sort();
    expected.sort();

    assert_eq!(actual, expected);
}

#[test]
fn configured_eager_tool_streaming_sets_payload_and_headers() {
    let model = full_model(Some(true));
    let request = capture_request(&model, &options());
    let payload = request
        .body_json()
        .expect("captured Anthropic request should contain JSON payload");
    let tool = first_tool(&payload);

    assert_eq!(tool.get("eager_input_streaming"), Some(&json!(true)));
    assert_eq!(
        tool.pointer("/input_schema/required"),
        Some(&json!(["value"]))
    );
    assert_eq!(
        request.headers.get("x-api-key"),
        Some(&"fake-key".to_owned())
    );
    assert_eq!(
        request.headers.get("anthropic-version"),
        Some(&"2023-06-01".to_owned())
    );
    assert_eq!(
        request.headers.get("anthropic-beta"),
        Some(&"interleaved-thinking-2025-05-14".to_owned())
    );
}

#[test]
fn non_eager_tool_streaming_uses_fine_grained_beta_and_tool_cache_control() {
    let model = full_model(Some(false));
    let request = capture_request(&model, &options());
    let payload = request
        .body_json()
        .expect("captured Anthropic request should contain JSON payload");
    let tool = first_tool(&payload);

    assert_eq!(tool.get("eager_input_streaming"), None);
    assert_eq!(
        tool.get("cache_control"),
        Some(&json!({ "type": "ephemeral" }))
    );
    assert_eq!(
        request.headers.get("anthropic-beta"),
        Some(&"fine-grained-tool-streaming-2025-05-14,interleaved-thinking-2025-05-14".to_owned())
    );
}

fn run_live_probe(force_eager: bool) {
    let Some(api_key) = get_env_api_key("anthropic", None) else {
        return;
    };
    let model = full_model(force_eager.then_some(true));
    let options = AnthropicOptions {
        stream: StreamOptions {
            api_key: Some(api_key),
            cache_retention: Some(CacheRetention::None),
            max_tokens: Some(128),
            ..StreamOptions::default()
        },
        thinking_enabled: Some(false),
        ..AnthropicOptions::default()
    };
    let provider = anthropic_provider().expect("registered Anthropic provider");
    let response = block_on(
        provider
            .stream(&model, &tool_context(), Some(&options.stream))
            .result(),
    );
    assert_ne!(response.stop_reason, zedflow_ai::types::StopReason::Error);
}

#[test]
#[ignore = "live capability: requires ANTHROPIC_API_KEY and network"]
fn generated_compat_settings_accept_configured_tool_streaming() {
    run_live_probe(false);
}

#[test]
#[ignore = "live capability: requires ANTHROPIC_API_KEY and network"]
fn forced_eager_input_streaming_probe_accepts_forced_eager_input_streaming() {
    run_live_probe(true);
}
