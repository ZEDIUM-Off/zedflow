use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

use futures::StreamExt;
use serde_json::Value;
use zedflow_agent::{ProxyStreamOptions, stream_proxy};
use zedflow_ai::{
    AssistantContentBlock, AssistantMessageEvent, Context, DoneStopReason, Model,
    SimpleStreamOptions,
};

#[test]
fn posts_request_and_decodes_sse_events() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (request_tx, request_rx) = mpsc::channel();
    thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(socket.try_clone().unwrap());
        let mut headers = String::new();
        let mut content_length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length: ") {
                content_length = value.trim().parse().unwrap();
            }
            headers.push_str(&line);
        }
        let mut body = vec![0; content_length];
        reader.read_exact(&mut body).unwrap();
        request_tx.send((headers, body)).unwrap();

        let events = concat!(
            "data: {\"type\":\"start\"}\n\n",
            "data: {\"type\":\"text_start\",\"contentIndex\":0}\n\n",
            "data: {\"type\":\"text_delta\",\"contentIndex\":0,\"delta\":\"hello\"}\n\n",
            "data: {\"type\":\"text_end\",\"contentIndex\":0}\n\n",
            "data: {\"type\":\"done\",\"reason\":\"stop\",\"usage\":{\"input\":0,\"output\":0,\"cacheRead\":0,\"cacheWrite\":0,\"totalTokens\":0,\"cost\":{\"input\":0,\"output\":0,\"cacheRead\":0,\"cacheWrite\":0,\"total\":0}}}\n\n"
        );
        write!(
            socket,
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\n\r\n{}",
            events.len(),
            events
        )
        .unwrap();
    });

    let model = Model {
        id: "model-id".into(),
        provider: "provider-id".into(),
        ..Model::default()
    };
    let mut stream = stream_proxy(
        &model,
        &Context::default(),
        ProxyStreamOptions {
            stream: SimpleStreamOptions::default(),
            auth_token: "secret".into(),
            proxy_url: format!("http://{address}"),
        },
    );
    let events = futures::executor::block_on(async move {
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }
        events
    });

    let (headers, body) = request_rx.recv().unwrap();
    assert!(
        headers
            .to_ascii_lowercase()
            .contains("authorization: bearer secret")
    );
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["model"]["id"], "model-id");
    assert!(body["options"].get("maxTokens").is_none());
    assert_eq!(events.len(), 5);
    assert!(matches!(
        events.last(),
        Some(AssistantMessageEvent::Done { reason: DoneStopReason::Stop, message })
            if matches!(message.content.first(), Some(AssistantContentBlock::Text(text)) if text.text == "hello")
    ));
}
