use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use futures::StreamExt;
use futures::executor::block_on;
use zedflow_ai::api::openai_completions::{
    Context, Model, ModelInput, OpenAICompletionsOptions, stream_live, stream_registered,
};
use zedflow_ai::types::{
    AssistantMessageEvent, ErrorStopReason, ModelCost, StopReason, StreamOptions,
};
use zedflow_ai::utils::abort_signals::AbortController;

fn serve(failures: usize) -> (String, thread::JoinHandle<usize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        for attempt in 0..=failures {
            let (mut socket, _) = listener.accept().expect("accept");
            let mut request = [0; 4096];
            assert!(socket.read(&mut request).expect("request") > 0);
            if attempt < failures {
                socket
                    .write_all(
                        b"HTTP/1.1 500 Internal Server Error\r\ncontent-length: 4\r\n\r\nnope",
                    )
                    .unwrap();
            } else {
                let body = "data: {\"id\":\"retry-ok\",\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: {\"id\":\"retry-ok\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";
                write!(socket, "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\n\r\n{}", body.len(), body).unwrap();
            }
        }
        failures + 1
    });
    (url, worker)
}

fn model(base_url: String) -> Model {
    Model {
        id: "test".into(),
        api: "openai-completions".into(),
        provider: "test".into(),
        base_url,
        input: vec![ModelInput::Text],
        reasoning: false,
        thinking_level_map: HashMap::new(),
        headers: HashMap::new(),
        max_tokens: 32,
        context_window: None,
        compat: None,
    }
}

fn registered_model(base_url: String) -> zedflow_ai::types::Model {
    zedflow_ai::types::Model {
        id: "test".into(),
        name: "Test".into(),
        api: "openai-completions".into(),
        provider: "test".into(),
        base_url,
        max_tokens: 32,
        context_window: 4096,
        cost: ModelCost {
            input: 1.0,
            output: 2.0,
            cache_read: 0.5,
            cache_write: 3.0,
        },
        ..Default::default()
    }
}

fn accept_request(listener: &TcpListener) -> TcpStream {
    let (mut socket, _) = listener.accept().expect("accept");
    let mut request = [0; 8192];
    assert!(socket.read(&mut request).expect("request") > 0);
    socket.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n").unwrap();
    socket
}

fn write_chunk(socket: &mut TcpStream, data: &str) {
    write!(socket, "{:X}\r\n{}\r\n", data.len(), data).unwrap();
    socket.flush().unwrap();
}

#[test]
fn production_transport_honors_explicit_retries() {
    let (url, server) = serve(2);
    let stream = stream_live(
        &model(url),
        &Context::default(),
        Some(&OpenAICompletionsOptions {
            api_key: Some("test".into()),
            max_retries: Some(2),
            ..Default::default()
        }),
    )
    .expect("stream");
    let result = block_on(stream.result());
    assert_eq!(result.response_id.as_deref(), Some("retry-ok"));
    assert_eq!(server.join().unwrap(), 3);
}

#[test]
fn request_envelope_disables_retries_by_default() {
    let request = zedflow_ai::api::openai_completions::build_request(
        &model("http://127.0.0.1:1".into()),
        &Context::default(),
        None,
    )
    .expect("request");
    assert_eq!(request.max_retries, 0);
    assert_eq!(request.client_options["maxRetries"], 0);
}

#[tokio::test]
async fn registered_done_settles_without_waiting_for_eof_and_maps_usage_cost() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (release_tx, release_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let mut socket = accept_request(&listener);
        write_chunk(
            &mut socket,
            concat!(
                "data: {\"id\":\"open-id\",\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\n",
                "data: {\"id\":\"open-id\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"total_tokens\":999,\"prompt_tokens_details\":{\"cached_tokens\":2,\"cache_write_tokens\":1},\"completion_tokens_details\":{\"reasoning_tokens\":3}}}\n\n",
                "data: [DONE]\n\n"
            ),
        );
        release_rx.recv_timeout(Duration::from_secs(2)).ok();
    });
    let stream = stream_registered(
        &registered_model(url),
        &Default::default(),
        Some(&StreamOptions {
            api_key: Some("test".into()),
            ..Default::default()
        }),
    );
    let message = tokio::time::timeout(Duration::from_millis(500), stream.result())
        .await
        .expect("must settle before EOF");
    assert_eq!(message.response_id.as_deref(), Some("open-id"));
    assert_eq!(message.stop_reason, StopReason::Stop);
    assert_eq!(message.usage.input, 7);
    assert_eq!(message.usage.output, 5);
    assert_eq!(message.usage.cache_read, 2);
    assert_eq!(message.usage.cache_write, 1);
    assert_eq!(message.usage.reasoning, Some(3));
    assert_eq!(message.usage.total_tokens, 15);
    assert!((message.usage.cost.total - 0.000_021).abs() < 1e-12);
    release_tx.send(()).ok();
    server.join().unwrap();
}

#[tokio::test]
async fn registered_abort_after_first_delta_is_one_terminal_with_partial_state() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut socket = accept_request(&listener);
        write_chunk(
            &mut socket,
            "data: {\"id\":\"abort-id\",\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n",
        );
        thread::sleep(Duration::from_secs(2));
    });
    let controller = AbortController::new();
    let mut stream = stream_registered(
        &registered_model(url),
        &Default::default(),
        Some(&StreamOptions {
            api_key: Some("test".into()),
            signal: Some(controller.signal()),
            ..Default::default()
        }),
    );
    loop {
        let event = tokio::time::timeout(Duration::from_millis(500), stream.next())
            .await
            .unwrap()
            .unwrap();
        if matches!(event, AssistantMessageEvent::TextDelta { .. }) {
            break;
        }
    }
    controller.abort();
    let terminal = tokio::time::timeout(Duration::from_millis(500), stream.next())
        .await
        .unwrap()
        .unwrap();
    let AssistantMessageEvent::Error {
        reason: ErrorStopReason::Aborted,
        error,
    } = terminal
    else {
        panic!("aborted terminal")
    };
    assert_eq!(error.response_id.as_deref(), Some("abort-id"));
    assert!(
        matches!(error.content.first(), Some(zedflow_ai::types::AssistantContentBlock::Text(text)) if text.text == "partial")
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(100), stream.next())
            .await
            .unwrap()
            .is_none()
    );
    drop(server);
}

#[tokio::test]
async fn registered_transport_drop_after_partial_is_one_error_preserving_partial() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let mut socket = accept_request(&listener);
        write_chunk(
            &mut socket,
            "data: {\"id\":\"drop-id\",\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n",
        );
    });
    let mut stream = stream_registered(
        &registered_model(url),
        &Default::default(),
        Some(&StreamOptions {
            api_key: Some("test".into()),
            ..Default::default()
        }),
    );
    let mut terminal = None;
    while let Some(event) = tokio::time::timeout(Duration::from_millis(500), stream.next())
        .await
        .unwrap()
    {
        if matches!(event, AssistantMessageEvent::Error { .. }) {
            terminal = Some(event);
            break;
        }
    }
    let Some(AssistantMessageEvent::Error {
        reason: ErrorStopReason::Error,
        error,
    }) = terminal
    else {
        panic!("error terminal")
    };
    assert_eq!(error.response_id.as_deref(), Some("drop-id"));
    assert!(
        matches!(error.content.first(), Some(zedflow_ai::types::AssistantContentBlock::Text(text)) if text.text == "partial")
    );
    assert!(!error.error_message.as_deref().unwrap().is_empty());
    assert!(stream.next().await.is_none());
    server.join().unwrap();
}

#[tokio::test]
async fn registered_content_filter_is_exact_error_with_partial_state() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (release_tx, release_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let mut socket = accept_request(&listener);
        write_chunk(
            &mut socket,
            concat!(
                "data: {\"id\":\"filter-id\",\"choices\":[{\"delta\":{\"content\":\"partial\"}}]}\n\n",
                "data: {\"id\":\"filter-id\",\"choices\":[{\"delta\":{},\"finish_reason\":\"content_filter\"}],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":1}}\n\n"
            ),
        );
        release_rx.recv_timeout(Duration::from_secs(2)).ok();
    });
    let message = tokio::time::timeout(
        Duration::from_millis(500),
        stream_registered(
            &registered_model(url),
            &Default::default(),
            Some(&StreamOptions {
                api_key: Some("test".into()),
                ..Default::default()
            }),
        )
        .result(),
    )
    .await
    .expect("content filter settles before EOF");
    assert_eq!(message.stop_reason, StopReason::Error);
    assert_eq!(
        message.error_message.as_deref(),
        Some("Provider finish_reason: content_filter")
    );
    assert_eq!(message.response_id.as_deref(), Some("filter-id"));
    assert_eq!(message.usage.total_tokens, 3);
    assert!(
        matches!(message.content.first(), Some(zedflow_ai::types::AssistantContentBlock::Text(text)) if text.text == "partial")
    );
    release_tx.send(()).ok();
    server.join().unwrap();
}
