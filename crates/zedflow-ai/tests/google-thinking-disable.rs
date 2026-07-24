use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use futures::{StreamExt, executor::block_on};
use serde_json::{Value, json};
use zedflow_ai::auth::types::{AuthContext, AuthFuture};
use zedflow_ai::models::create_models_with_auth_context;
use zedflow_ai::providers::google::{GOOGLE_API_KEY_AUTH_NAME, google_provider};
use zedflow_ai::types::{
    AssistantMessage, AssistantMessageEvent, Context, ErrorStopReason, Message, Model, ModelCost,
    SimpleStreamOptions, StopReason, Tool, UserMessage, UserMessageContent, UserMessageRole,
};
use zedflow_ai::utils::abort_signals::AbortController;

#[derive(Debug)]
struct GeminiEnv;

impl AuthContext for GeminiEnv {
    fn env<'a>(&'a self, name: &'a str) -> AuthFuture<'a, Option<String>> {
        Box::pin(async move { (name == "GEMINI_API_KEY").then(|| "env-test-key".to_owned()) })
    }

    fn file_exists<'a>(&'a self, _path: &'a str) -> AuthFuture<'a, bool> {
        Box::pin(async { false })
    }
}

fn read_request(socket: &mut TcpStream) -> (String, Value) {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let count = socket.read(&mut buffer).expect("read request");
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
        let header_end = bytes.windows(4).position(|window| window == b"\r\n\r\n");
        if let Some(header_end) = header_end {
            let headers = String::from_utf8_lossy(&bytes[..header_end]);
            let length = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .and_then(|value| value.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            if bytes.len() >= header_end + 4 + length {
                break;
            }
        }
    }
    let header_end = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("request headers");
    (
        String::from_utf8_lossy(&bytes[..header_end]).into_owned(),
        serde_json::from_slice(&bytes[header_end + 4..]).expect("Google REST request JSON"),
    )
}

fn test_model(model_id: &str, address: std::net::SocketAddr) -> Model {
    Model {
        id: model_id.into(),
        name: model_id.into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: format!("http://{address}/v1beta"),
        reasoning: true,
        max_tokens: 8192,
        cost: ModelCost {
            input: 1.0,
            output: 2.0,
            cache_read: 0.5,
            cache_write: 3.0,
        },
        ..Model::default()
    }
}

fn test_context() -> Context {
    Context {
        system_prompt: Some("System instruction".into()),
        messages: vec![Message::User(UserMessage {
            role: UserMessageRole::User,
            content: UserMessageContent::Text("Hello".into()),
            timestamp: 0,
        })],
        tools: Some(vec![Tool {
            name: "lookup".into(),
            description: "Look something up".into(),
            parameters: json!({ "type": "object", "properties": { "query": { "type": "string" } }, "required": ["query"] }),
        }]),
    }
}

fn simple_options(api_key: Option<&str>) -> SimpleStreamOptions {
    let mut stream = zedflow_ai::types::StreamOptions {
        api_key: api_key.map(str::to_owned),
        max_tokens: Some(160),
        ..zedflow_ai::types::StreamOptions::default()
    };
    stream.extra.insert("toolChoice".into(), json!("any"));
    SimpleStreamOptions {
        stream,
        reasoning: None,
        thinking_budgets: None,
    }
}

fn success_sse() -> (String, String) {
    let first = format!(
        "data: {}\r\n\r\n",
        json!({
            "responseId": "resp-1",
            "candidates": [{ "content": { "role": "model", "parts": [{ "text": "po" }] } }]
        })
    );
    let second = format!(
        "data: {}\r\n\r\n",
        json!({
            "candidates": [{ "content": { "role": "model", "parts": [{ "text": "ng" }] }, "finishReason": "STOP" }],
            "usageMetadata": { "promptTokenCount": 10, "cachedContentTokenCount": 3,
                "candidatesTokenCount": 4, "thoughtsTokenCount": 2, "totalTokenCount": 16 }
        })
    );
    (first, second)
}

fn capture_disabled_payload(
    model_id: &str,
    explicit_key: Option<&str>,
) -> (Value, Vec<AssistantMessageEvent>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture server");
    let address = listener.local_addr().expect("capture address");
    let expected_path = format!("/v1beta/models/{model_id}:streamGenerateContent?alt=sse");
    let expected_key = explicit_key.unwrap_or("env-test-key").to_owned();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept Google request");
        let (headers, payload) = read_request(&mut socket);
        let headers = headers.to_ascii_lowercase();
        assert!(
            headers.starts_with(&format!("post {expected_path} http/1.1").to_ascii_lowercase())
        );
        assert!(headers.contains(&format!("x-goog-api-key: {expected_key}")));
        assert!(headers.contains("content-type: application/json"));
        let (first, second) = success_sse();
        write!(socket, "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n{:X}\r\n{}\r\n", first.len(), first).expect("write first Google SSE chunk");
        socket.flush().expect("flush first Google SSE chunk");
        thread::sleep(Duration::from_millis(10));
        write!(socket, "{:X}\r\n{}\r\n0\r\n\r\n", second.len(), second)
            .expect("write final Google SSE chunk");
        payload
    });

    let model = test_model(model_id, address);
    let options = simple_options(explicit_key);
    let mut models = create_models_with_auth_context(GeminiEnv);
    let provider = google_provider().expect("registered Google provider");
    assert_eq!(
        provider
            .auth
            .api_key
            .as_ref()
            .expect("Google API-key auth")
            .name(),
        GOOGLE_API_KEY_AUTH_NAME
    );
    models.set_provider(provider);
    let mut stream = models.stream_simple(&model, &test_context(), Some(&options));
    let events = block_on(async {
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }
        events
    });
    (server.join().expect("capture server"), events)
}

fn partial(event: &AssistantMessageEvent) -> Option<AssistantMessage> {
    match event {
        AssistantMessageEvent::Start { partial }
        | AssistantMessageEvent::TextStart { partial, .. }
        | AssistantMessageEvent::TextDelta { partial, .. }
        | AssistantMessageEvent::TextEnd { partial, .. }
        | AssistantMessageEvent::ThinkingStart { partial, .. }
        | AssistantMessageEvent::ThinkingDelta { partial, .. }
        | AssistantMessageEvent::ThinkingEnd { partial, .. }
        | AssistantMessageEvent::ToolcallStart { partial, .. }
        | AssistantMessageEvent::ToolcallDelta { partial, .. }
        | AssistantMessageEvent::ToolcallEnd { partial, .. } => Some(partial.snapshot()),
        AssistantMessageEvent::Done { .. } | AssistantMessageEvent::Error { .. } => None,
    }
}

fn text(message: &AssistantMessage) -> String {
    serde_json::to_value(message).expect("canonical message JSON")["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect()
}

fn assert_progressive_success(events: &[AssistantMessageEvent]) {
    assert_eq!(events.len(), 6);
    assert!(matches!(events[0], AssistantMessageEvent::Start { .. }));
    assert!(matches!(events[1], AssistantMessageEvent::TextStart { .. }));
    assert!(matches!(&events[2], AssistantMessageEvent::TextDelta { delta, .. } if delta == "po"));
    assert!(matches!(&events[3], AssistantMessageEvent::TextDelta { delta, .. } if delta == "ng"));
    assert!(matches!(events[4], AssistantMessageEvent::TextEnd { .. }));
    assert!(matches!(events[5], AssistantMessageEvent::Done { .. }));
    for event in &events[..5] {
        let partial = partial(event).expect("partial");
        assert_eq!(text(&partial), "pong");
        assert_eq!(partial.usage.total_tokens, 16);
    }
    let AssistantMessageEvent::Done { message, .. } = &events[5] else {
        unreachable!()
    };
    assert_eq!(
        (
            message.usage.input,
            message.usage.output,
            message.usage.cache_read
        ),
        (7, 6, 3)
    );
    assert!((message.usage.cost.input - 0.000_007).abs() < f64::EPSILON);
    assert!((message.usage.cost.output - 0.000_012).abs() < f64::EPSILON);
    assert!((message.usage.cost.cache_read - 0.000_001_5).abs() < f64::EPSILON);
    assert!((message.usage.cost.total - 0.000_020_5).abs() < f64::EPSILON);
}

fn expected_rest(thinking: Value) -> Value {
    json!({
        "contents": [{ "role": "user", "parts": [{ "text": "Hello" }] }],
        "systemInstruction": { "parts": [{ "text": "System instruction" }] },
        "tools": [{ "functionDeclarations": [{
            "name": "lookup", "description": "Look something up",
            "parametersJsonSchema": { "type": "object", "properties": { "query": { "type": "string" } }, "required": ["query"] }
        }] }],
        "toolConfig": { "functionCallingConfig": { "mode": "ANY" } },
        "generationConfig": { "maxOutputTokens": 160, "thinkingConfig": thinking }
    })
}

#[test]
fn disables_thinking_for_gemini_2_5_through_registered_transport_and_env_auth() {
    let (payload, events) = capture_disabled_payload("gemini-2.5-flash", None);
    assert_eq!(payload, expected_rest(json!({ "thinkingBudget": 0 })));
    assert!(payload.get("model").is_none() && payload.get("config").is_none());
    assert_progressive_success(&events);
}

#[test]
fn disables_thinking_for_gemini_3_flash_through_registered_transport_and_env_auth() {
    let (payload, events) = capture_disabled_payload("gemini-3-flash-preview", None);
    assert_eq!(
        payload,
        expected_rest(json!({ "thinkingLevel": "MINIMAL" }))
    );
    assert_progressive_success(&events);
}

#[test]
fn explicit_api_key_precedes_gemini_env_for_gemini_3_1_pro() {
    let (payload, events) = capture_disabled_payload("gemini-3.1-pro-preview", Some("manual-key"));
    assert_eq!(payload, expected_rest(json!({ "thinkingLevel": "LOW" })));
    assert_progressive_success(&events);
}

#[test]
fn abort_during_incremental_sse_suppresses_later_events_and_terminates_once() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind abort server");
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let _ = read_request(&mut socket);
        let (first, second) = success_sse();
        write!(socket, "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\nconnection: close\r\n\r\n{:X}\r\n{}\r\n", first.len(), first).unwrap();
        socket.flush().unwrap();
        thread::sleep(Duration::from_millis(100));
        let _ = write!(socket, "{:X}\r\n{}\r\n0\r\n\r\n", second.len(), second);
    });
    let controller = AbortController::new();
    let options = zedflow_ai::types::StreamOptions {
        api_key: Some("test-key".into()),
        signal: Some(controller.signal()),
        ..zedflow_ai::types::StreamOptions::default()
    };
    let provider = google_provider().unwrap();
    let mut stream = provider.stream(
        &test_model("gemini-2.5-flash", address),
        &test_context(),
        Some(&options),
    );
    let events = block_on(async {
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            let abort =
                matches!(&event, AssistantMessageEvent::TextDelta { delta, .. } if delta == "po");
            events.push(event);
            if abort {
                controller.abort();
            }
        }
        events
    });
    server.join().unwrap();
    assert!(events.iter().any(
        |event| matches!(event, AssistantMessageEvent::TextDelta { delta, .. } if delta == "po")
    ));
    assert!(!events.iter().any(
        |event| matches!(event, AssistantMessageEvent::TextDelta { delta, .. } if delta == "ng")
    ));
    assert!(
        matches!(events.last(), Some(AssistantMessageEvent::Error { reason: ErrorStopReason::Aborted, error }) if error.stop_reason == StopReason::Aborted)
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
fn abort_between_decoded_events_suppresses_remaining_buffered_sse() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let _ = read_request(&mut socket);
        let (first, second) = success_sse();
        let body = format!("{first}{second}");
        write!(socket, "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}", body.len(), body).unwrap();
    });
    let controller = AbortController::new();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let events = runtime.block_on(async {
        let options = zedflow_ai::types::StreamOptions {
            api_key: Some("test-key".into()),
            signal: Some(controller.signal()),
            ..Default::default()
        };
        let provider = google_provider().unwrap();
        let mut stream = provider.stream(
            &test_model("gemini-2.5-flash", address),
            &test_context(),
            Some(&options),
        );
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            let abort =
                matches!(&event, AssistantMessageEvent::TextDelta { delta, .. } if delta == "po");
            events.push(event);
            if abort {
                controller.abort();
            }
        }
        events
    });
    server.join().unwrap();
    assert!(!events.iter().any(
        |event| matches!(event, AssistantMessageEvent::TextDelta { delta, .. } if delta == "ng")
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
    assert!(matches!(
        events.last(),
        Some(AssistantMessageEvent::Error {
            reason: ErrorStopReason::Aborted,
            ..
        })
    ));
}

#[test]
fn abort_while_awaiting_http_headers_emits_only_one_aborted_terminal() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (request_seen, wait_for_request) = std::sync::mpsc::channel();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let _ = read_request(&mut socket);
        request_seen.send(()).unwrap();
        thread::sleep(Duration::from_millis(100));
        let _ = write!(socket, "HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n");
    });
    let controller = AbortController::new();
    let aborter = controller.clone();
    let abort_thread = thread::spawn(move || {
        wait_for_request.recv().unwrap();
        aborter.abort();
    });
    let options = zedflow_ai::types::StreamOptions {
        api_key: Some("test-key".into()),
        signal: Some(controller.signal()),
        ..Default::default()
    };
    let provider = google_provider().unwrap();
    let mut stream = provider.stream(
        &test_model("gemini-2.5-flash", address),
        &test_context(),
        Some(&options),
    );
    let events = block_on(async {
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }
        events
    });
    abort_thread.join().unwrap();
    server.join().unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(
        &events[0],
        AssistantMessageEvent::Error {
            reason: ErrorStopReason::Aborted,
            error
        } if error.stop_reason == StopReason::Aborted
    ));
}

#[test]
fn non_success_status_uses_normalized_error_body_and_one_terminal() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let _ = read_request(&mut socket);
        let body = r#"{"error":"quota"}"#;
        write!(socket, "HTTP/1.1 429 Too Many Requests\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}", body.len(), body).unwrap();
    });
    let provider = google_provider().unwrap();
    let options = zedflow_ai::types::StreamOptions {
        api_key: Some("test-key".into()),
        ..Default::default()
    };
    let mut stream = provider.stream(
        &test_model("gemini-2.5-flash", address),
        &test_context(),
        Some(&options),
    );
    let events = block_on(async {
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }
        events
    });
    server.join().unwrap();
    assert_eq!(events.len(), 1);
    assert!(
        matches!(&events[0], AssistantMessageEvent::Error { reason: ErrorStopReason::Error, error }
        if error.error_message.as_deref() == Some("429: {\"error\":\"quota\"}"))
    );
}
