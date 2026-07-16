use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use futures::{StreamExt, executor::block_on};
use serde_json::{Value, json};
use zedflow_ai::api::anthropic_messages::stream_registered;
use zedflow_ai::compat::{get_model, get_models};
use zedflow_ai::env_api_keys::{ProviderEnv, find_env_keys, get_env_api_key};
use zedflow_ai::types::{
    CacheRetention, Context, Message, ModelCompat, ModelInput, StreamOptions, Tool, UserMessage,
    UserMessageContent, UserMessageRole,
};

#[test]
fn ports_fireworks_catalog_env_and_compat() {
    let kimi = get_model("fireworks", "accounts/fireworks/models/kimi-k2p6").unwrap();
    assert_eq!(kimi.api, "anthropic-messages");
    assert_eq!(kimi.provider, "fireworks");
    assert_eq!(kimi.base_url, "https://api.fireworks.ai/inference");
    assert!(kimi.reasoning);
    assert_eq!(kimi.input, vec![ModelInput::Text, ModelInput::Image]);
    assert_eq!(kimi.context_window, 262_000);
    assert_eq!(kimi.max_tokens, 262_000);
    assert_eq!(
        (
            kimi.cost.input,
            kimi.cost.output,
            kimi.cost.cache_read,
            kimi.cost.cache_write
        ),
        (0.95, 4.0, 0.16, 0.0)
    );
    let Some(ModelCompat::AnthropicMessages(compat)) = kimi.compat else {
        panic!("expected Anthropic compat");
    };
    assert_eq!(compat.send_session_affinity_headers, Some(true));
    assert_eq!(compat.supports_eager_tool_input_streaming, Some(false));
    assert_eq!(compat.supports_cache_control_on_tools, Some(false));
    assert_eq!(compat.supports_long_cache_retention, Some(false));

    let models = get_models().unwrap();
    let turbo = models
        .iter()
        .find(|model| {
            model.provider == "fireworks"
                && model.id.starts_with("accounts/fireworks/routers/")
                && model.id.ends_with("-turbo")
        })
        .expect("turbo router");
    assert_eq!(turbo.api, "anthropic-messages");
    assert_eq!(turbo.base_url, "https://api.fireworks.ai/inference");
    assert_eq!(turbo.input, vec![ModelInput::Text, ModelInput::Image]);

    let base = get_model("fireworks", "accounts/fireworks/models/glm-5p2").unwrap();
    let fast = get_model("fireworks", "accounts/fireworks/routers/glm-5p2-fast").unwrap();
    assert_eq!(fast.api, base.api);
    assert_eq!(fast.base_url, base.base_url);
    assert_eq!(fast.compat, base.compat);
    assert_eq!(fast.thinking_level_map, base.thinking_level_map);

    let env = ProviderEnv::from([("FIREWORKS_API_KEY".into(), "test-fireworks-key".into())]);
    assert_eq!(
        find_env_keys("fireworks", Some(&env)),
        Some(vec!["FIREWORKS_API_KEY"])
    );
    assert_eq!(
        get_env_api_key("fireworks", Some(&env)).as_deref(),
        Some("test-fireworks-key")
    );
}

fn serve() -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let read = socket.read(&mut buffer).unwrap();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..end]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(str::trim)
                            .and_then(|v| v.parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if request.len() >= end + 4 + length {
                    break;
                }
            }
        }
        let body = [
            ("message_start", json!({"type":"message_start","message":{"id":"msg_fw","usage":{"input_tokens":1,"output_tokens":0}}})),
            ("message_delta", json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"input_tokens":1,"output_tokens":1}})),
            ("message_stop", json!({"type":"message_stop"})),
        ].into_iter().map(|(event,data)| format!("event: {event}\ndata: {data}\n\n")).collect::<String>();
        write!(socket, "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}", body.len(), body).unwrap();
        String::from_utf8(request).unwrap()
    });
    (format!("http://{address}"), handle)
}

#[test]
fn registered_anthropic_transport_applies_fireworks_affinity_and_tool_compat() {
    let (base_url, server) = serve();
    let mut model = get_model("fireworks", "accounts/fireworks/models/kimi-k2p6").unwrap();
    model.base_url = base_url;
    let context = Context {
        system_prompt: None,
        messages: vec![Message::User(UserMessage {
            role: UserMessageRole::User,
            content: UserMessageContent::Text("Use the tool".into()),
            timestamp: 0,
        })],
        tools: Some(vec![Tool {
            name: "lookup".into(),
            description: "Look up a value".into(),
            parameters: json!({"type":"object","properties":{"value":{"type":"string"}},"required":["value"]}),
        }]),
    };
    let options = StreamOptions {
        api_key: Some("test-key".into()),
        session_id: Some("fireworks-session-1".into()),
        cache_retention: Some(CacheRetention::Short),
        ..StreamOptions::default()
    };
    let mut stream = stream_registered(&model, &context, Some(&options));
    block_on(async { while stream.next().await.is_some() {} });
    let request = server.join().unwrap();
    let (headers, raw_body) = request.split_once("\r\n\r\n").unwrap();
    assert!(
        headers
            .to_ascii_lowercase()
            .contains("x-session-affinity: fireworks-session-1")
    );
    let body: Value = serde_json::from_str(raw_body).unwrap();
    let tools = body["tools"].as_array().expect("tools");
    assert!(
        tools
            .iter()
            .all(|tool| tool.get("eager_input_streaming").is_none())
    );
    assert!(tools.last().unwrap().get("cache_control").is_none());
}
