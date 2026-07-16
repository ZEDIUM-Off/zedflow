use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use futures::{StreamExt, executor::block_on};
use serde_json::{Value, json};
use zedflow_ai::models::ProviderApi;
use zedflow_ai::providers::github_copilot;
use zedflow_ai::types::{
    AssistantMessageEvent, Context, ImageContent, ImageContentType, Message, StreamOptions,
    TextContent, TextContentType, UserContentBlock, UserMessage, UserMessageContent,
    UserMessageRole,
};

#[derive(Debug)]
struct CapturedRequest {
    path: String,
    headers: HashMap<String, String>,
    body: Value,
}

fn capture_server() -> (String, thread::JoinHandle<CapturedRequest>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local capture server");
    let base_url = format!("http://{}", listener.local_addr().expect("local address"));
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept request");
        let mut raw = Vec::new();
        let body_start = loop {
            let mut chunk = [0_u8; 4096];
            let read = socket.read(&mut chunk).expect("read request");
            assert!(read > 0, "request ended before headers");
            raw.extend_from_slice(&chunk[..read]);
            if let Some(index) = raw.windows(4).position(|window| window == b"\r\n\r\n") {
                break index + 4;
            }
        };
        let head = String::from_utf8(raw[..body_start - 4].to_vec()).expect("header UTF-8");
        let mut lines = head.lines();
        let request_line = lines.next().expect("request line");
        let path = request_line
            .split_whitespace()
            .nth(1)
            .expect("request path")
            .to_owned();
        let headers: HashMap<String, String> = lines
            .filter_map(|line| {
                let (name, value) = line.split_once(':')?;
                Some((name.to_ascii_lowercase(), value.trim().to_owned()))
            })
            .collect();
        let content_length = headers["content-length"]
            .parse::<usize>()
            .expect("content length");
        while raw.len() - body_start < content_length {
            let mut chunk = [0_u8; 4096];
            let read = socket.read(&mut chunk).expect("read request body");
            assert!(read > 0, "request ended before body");
            raw.extend_from_slice(&chunk[..read]);
        }
        let body = serde_json::from_slice(&raw[body_start..body_start + content_length])
            .expect("Responses JSON body");

        let response_body = concat!(
            "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-copilot\",\"status\":\"in_progress\"}}\n\n",
            "data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"type\":\"message\",\"id\":\"msg-copilot\",\"content\":[]}}\n\n",
            "data: {\"type\":\"response.output_text.delta\",\"output_index\":0,\"delta\":\"done\"}\n\n",
            "data: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"type\":\"message\",\"id\":\"msg-copilot\",\"content\":[{\"type\":\"output_text\",\"text\":\"done\"}]}}\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-copilot\",\"status\":\"completed\",\"usage\":{\"input_tokens\":2,\"output_tokens\":1,\"total_tokens\":3}}}\n\n",
        );
        write!(
            socket,
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\n\r\n{}",
            response_body.len(),
            response_body
        )
        .expect("write SSE response");

        CapturedRequest {
            path,
            headers,
            body,
        }
    });
    (base_url, server)
}

#[test]
fn registered_copilot_responses_uses_exact_route_headers_body_and_terminal_result() {
    let (base_url, server) = capture_server();
    let copilot = github_copilot::github_copilot_provider().expect("Copilot provider");
    let mut model = copilot
        .get_models()
        .into_iter()
        .find(|model| model.id == "gpt-5-mini")
        .expect("Copilot Responses model");
    model.base_url = base_url;
    let ProviderApi::ByApi(apis) = copilot.api else {
        panic!("expected mixed Copilot API dispatch");
    };
    let responses = apis
        .get("openai-responses")
        .expect("Responses registration");
    let context = Context {
        system_prompt: Some("sys".into()),
        messages: vec![Message::User(UserMessage {
            role: UserMessageRole::User,
            content: UserMessageContent::Blocks(vec![
                UserContentBlock::Text(TextContent {
                    content_type: TextContentType::Text,
                    text: "hi".into(),
                    text_signature: None,
                }),
                UserContentBlock::Image(ImageContent {
                    content_type: ImageContentType::Image,
                    data: "aGVsbG8=".into(),
                    mime_type: "image/png".into(),
                }),
            ]),
            timestamp: 1,
        })],
        tools: None,
    };
    let mut stream = (responses.stream)(
        &model,
        &context,
        Some(&StreamOptions {
            api_key: Some("tid_copilot_test_token".into()),
            ..StreamOptions::default()
        }),
    );
    let (message, terminal_count) = block_on(async {
        let mut terminal_count = 0;
        while let Some(event) = stream.next().await {
            if matches!(
                event,
                AssistantMessageEvent::Done { .. } | AssistantMessageEvent::Error { .. }
            ) {
                terminal_count += 1;
            }
        }
        (stream.result().await, terminal_count)
    });
    assert_eq!(terminal_count, 1);
    assert_eq!(message.response_id.as_deref(), Some("resp-copilot"));
    assert_eq!(message.usage.total_tokens, 3);
    assert_eq!(message.stop_reason, zedflow_ai::types::StopReason::Stop);

    let request = server.join().expect("capture server");
    assert_eq!(request.path, "/responses");
    assert_eq!(
        request.headers.get("authorization").map(String::as_str),
        Some("Bearer tid_copilot_test_token")
    );
    assert_eq!(
        request.headers.get("user-agent").map(String::as_str),
        Some("GitHubCopilotChat/0.35.0")
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
    assert_eq!(
        request
            .headers
            .get("copilot-vision-request")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(request.body["model"], json!("gpt-5-mini"));
    assert_eq!(request.body["stream"], json!(true));
    assert_eq!(request.body["store"], json!(false));
    assert!(request.body.get("reasoning").is_none());
    assert_eq!(
        request.body["input"],
        json!([
            {"role":"developer","content":"sys"},
            {"role":"user","content":[
                {"type":"input_text","text":"hi"},
                {"type":"input_image","detail":"auto","image_url":"data:image/png;base64,aGVsbG8="}
            ]}
        ])
    );
}
