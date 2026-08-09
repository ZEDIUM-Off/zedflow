//! Differential oracle for builtin compat dispatch and provider conversion.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use base64::Engine;
use futures::executor::block_on;
use serde_json::{Value, json};
use zedflow_ai::compat;
use zedflow_ai::types::{
    AssistantContentBlock, AssistantMessage, AssistantMessageRole, Context, Message, ProviderEnv,
    StopReason, StreamOptions, Transport, Usage, UserMessage, UserMessageContent, UserMessageRole,
};

const APIS: &[&str] = &[
    "anthropic-messages",
    "openai-completions",
    "openai-responses",
    "openai-codex-responses",
    "azure-openai-responses",
    "google-generative-ai",
    "google-vertex",
    "mistral-conversations",
    "bedrock-converse-stream",
];

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (0_u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}

fn event_frame(event: &str, payload: Value) -> Vec<u8> {
    fn header(out: &mut Vec<u8>, name: &str, value: &str) {
        out.push(u8::try_from(name.len()).expect("header name"));
        out.extend_from_slice(name.as_bytes());
        out.push(7); // AWS event-stream string
        out.extend_from_slice(
            &u16::try_from(value.len())
                .expect("header value")
                .to_be_bytes(),
        );
        out.extend_from_slice(value.as_bytes());
    }

    let mut headers = Vec::new();
    header(&mut headers, ":message-type", "event");
    header(&mut headers, ":event-type", event);
    header(&mut headers, ":content-type", "application/json");
    let payload = payload.to_string();
    let total = 16 + headers.len() + payload.len();
    let mut frame = Vec::with_capacity(total);
    frame.extend_from_slice(&u32::try_from(total).expect("frame size").to_be_bytes());
    frame.extend_from_slice(
        &u32::try_from(headers.len())
            .expect("headers size")
            .to_be_bytes(),
    );
    frame.extend_from_slice(&crc32(&frame).to_be_bytes());
    frame.extend_from_slice(&headers);
    frame.extend_from_slice(payload.as_bytes());
    frame.extend_from_slice(&crc32(&frame).to_be_bytes());
    frame
}

fn bedrock_body(model: &str) -> Vec<u8> {
    [
        event_frame("messageStart", json!({"role":"assistant"})),
        event_frame(
            "contentBlockDelta",
            json!({"contentBlockIndex":0,"delta":{"text":format!("reply:{model}")}}),
        ),
        event_frame("contentBlockStop", json!({"contentBlockIndex":0})),
        event_frame("messageStop", json!({"stopReason":"end_turn"})),
        event_frame(
            "metadata",
            json!({"usage":{"inputTokens":1,"outputTokens":1,"totalTokens":2},"metrics":{"latencyMs":1}}),
        ),
    ]
    .concat()
}

fn read_request(stream: &mut TcpStream) -> Option<Value> {
    let mut request = Vec::new();
    let mut chunk = [0_u8; 4096];
    let header_end = loop {
        let read = stream.read(&mut chunk).expect("read faux request");
        assert!(read > 0, "request ended before headers");
        request.extend_from_slice(&chunk[..read]);
        if let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
            break end + 4;
        }
    };
    let headers = String::from_utf8_lossy(&request[..header_end]).into_owned();
    let content_length = headers
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .and_then(|value| value.trim().parse::<usize>().ok())
        })
        .unwrap_or(0);
    while request.len() < header_end + content_length {
        let read = stream.read(&mut chunk).expect("read faux body");
        assert!(read > 0, "request body ended early");
        request.extend_from_slice(&chunk[..read]);
    }
    let body = &request[header_end..header_end + content_length];
    let decoded = if headers
        .to_ascii_lowercase()
        .contains("content-encoding: zstd")
    {
        zstd::stream::decode_all(body).expect("decode faux zstd request")
    } else {
        body.to_vec()
    };
    serde_json::from_slice(&decoded).ok()
}

fn response_for(index: usize, model: &str) -> (&'static str, Vec<u8>) {
    let text = format!("reply:{model}");
    let sse = |events: Vec<Value>| {
        events
            .into_iter()
            .map(|event| format!("data: {event}\n\n"))
            .collect::<String>()
            .into_bytes()
    };
    match APIS[index] {
        "anthropic-messages" => {
            let events = [
                (
                    "message_start",
                    json!({"type":"message_start","message":{"id":format!("a-{model}"),"usage":{"input_tokens":1,"output_tokens":0}}}),
                ),
                (
                    "content_block_start",
                    json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
                ),
                (
                    "content_block_delta",
                    json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":text}}),
                ),
                (
                    "content_block_stop",
                    json!({"type":"content_block_stop","index":0}),
                ),
                (
                    "message_delta",
                    json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":1}}),
                ),
                ("message_stop", json!({"type":"message_stop"})),
            ];
            let body = events
                .into_iter()
                .map(|(event, data)| format!("event: {event}\ndata: {data}\n\n"))
                .collect::<String>()
                .into_bytes();
            ("text/event-stream", body)
        }
        "openai-completions" => {
            let mut body = sse(vec![
                json!({"id":format!("c-{model}"),"model":model,"choices":[{"delta":{"content":text},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}),
            ]);
            body.extend_from_slice(b"data: [DONE]\n\n");
            ("text/event-stream", body)
        }
        "openai-responses" | "openai-codex-responses" | "azure-openai-responses" => (
            "text/event-stream",
            sse(vec![
                json!({"type":"response.output_item.added","output_index":0,"item":{"type":"message","id":format!("m-{model}"),"status":"in_progress","role":"assistant","content":[]}}),
                json!({"type":"response.output_text.delta","output_index":0,"content_index":0,"delta":text}),
                json!({"type":"response.output_item.done","output_index":0,"item":{"type":"message","id":format!("m-{model}"),"status":"completed","role":"assistant","content":[{"type":"output_text","text":format!("reply:{model}"),"annotations":[]}]}}),
                json!({"type":"response.completed","response":{"id":format!("r-{model}"),"status":"completed","output":[],"usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}}}),
            ]),
        ),
        "google-generative-ai" | "google-vertex" => (
            "text/event-stream",
            sse(vec![
                json!({"responseId":format!("g-{model}"),"candidates":[{"content":{"role":"model","parts":[{"text":text}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":1,"candidatesTokenCount":1,"totalTokenCount":2}}),
            ]),
        ),
        "mistral-conversations" => (
            "text/event-stream",
            sse(vec![
                json!({"id":format!("m-{model}"),"choices":[{"delta":{"content":text},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}),
            ]),
        ),
        "bedrock-converse-stream" => ("application/vnd.amazon.eventstream", bedrock_body(model)),
        _ => unreachable!(),
    }
}

fn faux_server(
    models: Vec<String>,
) -> (
    String,
    Arc<Mutex<Vec<Option<Value>>>>,
    thread::JoinHandle<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind faux transport");
    let address = format!("http://{}", listener.local_addr().expect("faux address"));
    let captures = Arc::new(Mutex::new(vec![None; models.len()]));
    let server_captures = Arc::clone(&captures);
    let handle = thread::spawn(move || {
        for (index, model) in models.iter().enumerate() {
            let (mut stream, _) = listener.accept().expect("accept faux request");
            server_captures.lock().expect("captures lock")[index] = read_request(&mut stream);
            let (content_type, body) = response_for(index, model);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            )
            .expect("write faux headers");
            stream.write_all(&body).expect("write faux response");
        }
    });
    (address, captures, handle)
}

fn context() -> Context {
    Context {
        system_prompt: Some("oracle-system".into()),
        messages: vec![
            Message::User(UserMessage {
                role: UserMessageRole::User,
                content: UserMessageContent::Text("hello oracle".into()),
                timestamp: 1,
            }),
            Message::Assistant(AssistantMessage {
                role: AssistantMessageRole::Assistant,
                content: vec![zedflow_ai::types::AssistantContentBlock::Text(
                    zedflow_ai::types::TextContent {
                        content_type: zedflow_ai::types::TextContentType::Text,
                        text: "prior".into(),
                        text_signature: None,
                    },
                )],
                api: "oracle".into(),
                provider: "oracle".into(),
                model: "oracle".into(),
                usage: Usage::default(),
                stop_reason: StopReason::Stop,
                error_message: None,
                timestamp: 1,
                response_id: None,
                response_model: None,
                diagnostics: None,
            }),
            Message::User(UserMessage {
                role: UserMessageRole::User,
                content: UserMessageContent::Text("continue".into()),
                timestamp: 1,
            }),
        ],
        ..Context::default()
    }
}

fn rust_observation() -> Value {
    compat::reset_api_providers().expect("builtin providers");
    let mut models = APIS
        .iter()
        .map(|api| {
            compat::get_models()
                .into_iter()
                .find(|model| model.api == *api)
                .unwrap_or_else(|| panic!("missing builtin model for {api}"))
        })
        .collect::<Vec<_>>();
    let model_ids = models.iter().map(|model| model.id.clone()).collect();
    let (address, transport_captures, server) = faux_server(model_ids);
    let codex_payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(json!({"https://api.openai.com/auth":{"chatgpt_account_id":"oracle"}}).to_string());
    let codex_key = format!("x.{codex_payload}.x");
    let observations = models
        .iter_mut()
        .enumerate()
        .map(|(index, model)| {
            model.base_url = address.clone();
            let payload = Arc::new(Mutex::new(None));
            let captured = Arc::clone(&payload);
            let mut env = ProviderEnv::new();
            env.insert("GOOGLE_CLOUD_PROJECT".into(), "oracle".into());
            env.insert("GOOGLE_CLOUD_LOCATION".into(), "global".into());
            env.insert("AWS_REGION".into(), "us-east-1".into());
            env.insert("AWS_BEDROCK_SKIP_AUTH".into(), "1".into());
            let options = StreamOptions {
                api_key: Some(if model.api == "openai-codex-responses" {
                    codex_key.clone()
                } else {
                    "oracle-key".into()
                }),
                env: Some(env),
                transport: (model.api == "openai-codex-responses").then_some(Transport::Sse),
                on_payload: Some(Arc::new(move |value, _| {
                    *captured.lock().expect("payload lock") = Some(value);
                    Box::pin(async { Ok(None) })
                })),
                ..StreamOptions::default()
            };
            let response = block_on(compat::complete(model, &context(), Some(options)))
                .unwrap_or_else(|error| panic!("{} compat complete: {error}", model.api));
            let text = response.content.iter().find_map(|block| match block {
                AssistantContentBlock::Text(text) => Some(text.text.clone()),
                _ => None,
            });
            let request = payload
                .lock()
                .expect("payload lock")
                .clone()
                .or_else(|| transport_captures.lock().expect("captures lock")[index].clone())
                .unwrap_or_else(|| panic!("{} request capture: {:?}", model.api, response.error_message));
            json!({
                "api": response.api,
                "provider": response.provider,
                "model": response.model,
                "role": "assistant",
                "stopReason": match response.stop_reason { StopReason::Stop => "stop", StopReason::Length => "length", StopReason::ToolUse => "toolUse", StopReason::Aborted => "aborted", StopReason::Error => "error" },
                "text": text,
                "request": request,
            })
        })
        .collect::<Vec<_>>();
    server.join().expect("faux transport server");
    Value::Array(observations)
}

#[test]
fn rust_compat_matches_frozen_typescript_oracle() {
    let fixture = format!(
        "{}/tests/fixtures/frozen-oracle.ts",
        env!("CARGO_MANIFEST_DIR")
    );
    let output = std::process::Command::new("node")
        .arg(fixture)
        .output()
        .expect("Node 22+ is required to run the frozen TypeScript oracle");
    assert!(
        output.status.success(),
        "TypeScript oracle failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let oracle: Value = serde_json::from_slice(&output.stdout).expect("oracle JSON");
    assert_eq!(rust_observation(), oracle);
}
