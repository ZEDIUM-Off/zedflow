use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use futures::executor::block_on;
use serde_json::{Value, json};
use zedflow_ai::providers::github_copilot::github_copilot_provider;
use zedflow_ai::providers::github_copilot_models::{GITHUB_COPILOT_MODELS, GithubCopilotModel};
use zedflow_ai::types::{
    Context, Message, Model, StopReason, StreamOptions, UserMessage, UserMessageContent,
    UserMessageRole,
};

const EXTENDED_THINKING_LEVELS: &[&str] = &["off", "minimal", "low", "medium", "high", "xhigh"];

#[derive(Debug)]
struct CapturedRequest {
    path: String,
    headers: HashMap<String, String>,
    body: Value,
}

fn get_github_copilot_model(id: &str) -> &'static GithubCopilotModel {
    GITHUB_COPILOT_MODELS
        .iter()
        .find(|model| model.id == id)
        .expect("github-copilot model fixture should exist")
}

fn thinking_map_value<'a>(model: &'a GithubCopilotModel, level: &str) -> Option<Option<&'a str>> {
    model.thinking_level_map.and_then(|map| {
        map.iter()
            .find(|(key, _)| *key == level)
            .map(|(_, value)| *value)
    })
}

fn supported_thinking_levels(model: &GithubCopilotModel) -> Vec<&'static str> {
    if !model.reasoning {
        return vec!["off"];
    }

    EXTENDED_THINKING_LEVELS
        .iter()
        .copied()
        .filter(|level| {
            let mapped = thinking_map_value(model, level);
            if mapped == Some(None) {
                return false;
            }
            if *level == "xhigh" {
                return mapped.is_some();
            }
            true
        })
        .collect()
}

fn context() -> Context {
    Context {
        system_prompt: Some("You are a helpful assistant.".to_owned()),
        messages: vec![Message::User(UserMessage {
            role: UserMessageRole::User,
            content: UserMessageContent::Text("Hello".to_owned()),
            timestamp: 0,
        })],
        tools: None,
    }
}

fn runtime_model(id: &str) -> Model {
    github_copilot_provider()
        .expect("Copilot provider")
        .get_models()
        .into_iter()
        .find(|model| model.id == id)
        .expect("Copilot runtime model")
}

fn capture_registered_request(interleaved_thinking: bool) -> CapturedRequest {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture server");
    let address = listener.local_addr().expect("capture address");
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
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if request.len() >= header_end + 4 + content_length {
                break;
            }
        }
        socket
            .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
            .expect("write response");
        request
    });

    let provider = github_copilot_provider().expect("Copilot provider");
    let mut model = runtime_model("claude-sonnet-4.6");
    model.base_url = format!("http://{address}");
    assert_eq!(model.api, "anthropic-messages");
    let mut options = StreamOptions {
        api_key: Some("tid_copilot_session_test_token".to_owned()),
        max_tokens: Some(u32::try_from(model.max_tokens).expect("max tokens fit")),
        ..StreamOptions::default()
    };
    options.extra.insert(
        "interleavedThinking".to_owned(),
        Value::Bool(interleaved_thinking),
    );
    let stream = provider.stream(&model, &context(), Some(&options));
    assert_eq!(block_on(stream.result()).stop_reason, StopReason::Stop);

    let raw = String::from_utf8(server.join().expect("capture server")).expect("request UTF-8");
    let (head, body) = raw.split_once("\r\n\r\n").expect("HTTP request");
    let mut lines = head.lines();
    let request_line = lines.next().expect("request line");
    let path = request_line
        .split_whitespace()
        .nth(1)
        .expect("request path")
        .to_owned();
    let headers = lines
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.to_ascii_lowercase(), value.trim().to_owned()))
        })
        .collect();
    CapturedRequest {
        path,
        headers,
        body: serde_json::from_str(body).expect("Anthropic JSON body"),
    }
}

#[test]
fn applies_copilot_specific_adaptive_thinking_effort_overrides() {
    let opus47 = get_github_copilot_model("claude-opus-4.7");
    assert_eq!(thinking_map_value(opus47, "minimal"), Some(Some("low")));
    assert_eq!(thinking_map_value(opus47, "xhigh"), Some(Some("xhigh")));
    assert!(supported_thinking_levels(opus47).contains(&"xhigh"));

    let sonnet46 = get_github_copilot_model("claude-sonnet-4.6");
    assert_eq!(thinking_map_value(sonnet46, "minimal"), Some(Some("low")));
    assert_eq!(thinking_map_value(sonnet46, "xhigh"), Some(Some("max")));
    assert!(supported_thinking_levels(sonnet46).contains(&"xhigh"));
}

#[test]
fn registered_dispatch_uses_bearer_headers_and_anthropic_payload() {
    let request = capture_registered_request(true);
    assert_eq!(request.path, "/v1/messages");
    assert_eq!(
        request.headers.get("authorization").map(String::as_str),
        Some("Bearer tid_copilot_session_test_token")
    );
    assert!(!request.headers.contains_key("x-api-key"));
    assert!(
        request
            .headers
            .get("user-agent")
            .is_some_and(|value| value.contains("GitHubCopilotChat"))
    );
    assert_eq!(
        request
            .headers
            .get("copilot-integration-id")
            .map(String::as_str),
        Some("vscode-chat")
    );
    assert_eq!(
        request.headers.get("x-initiator").map(String::as_str),
        Some("user")
    );
    assert_eq!(
        request.headers.get("openai-intent").map(String::as_str),
        Some("conversation-edits")
    );
    assert!(
        !request
            .headers
            .get("anthropic-beta")
            .is_some_and(|value| value.contains("fine-grained-tool-streaming"))
    );
    assert_eq!(request.body["model"], json!("claude-sonnet-4.6"));
    assert_eq!(request.body["stream"], json!(true));
    assert_eq!(request.body["max_tokens"], json!(32_000));
    assert!(
        request.body["messages"]
            .as_array()
            .is_some_and(|messages| !messages.is_empty())
    );
}

#[test]
fn omits_interleaved_thinking_beta_for_adaptive_thinking_models() {
    let request = capture_registered_request(true);
    assert!(
        !request
            .headers
            .get("anthropic-beta")
            .is_some_and(|value| value.contains("interleaved-thinking-2025-05-14"))
    );
}
