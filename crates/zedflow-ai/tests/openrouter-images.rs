use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use zedflow_ai::api::openrouter_images::{
    ImagesContent, ImagesContext, ImagesModel, ImagesOptions, ImagesOutputModality,
    ImagesStopReason, ProviderHeaders, UsageCostRates,
};
use zedflow_ai::images::generate_images;
use zedflow_ai::utils::abort_signals::AbortController;

fn model(base_url: String) -> ImagesModel {
    ImagesModel {
        id: "google/gemini-3.1-flash-image-preview".into(),
        api: "openrouter-images".into(),
        provider: "openrouter".into(),
        base_url,
        headers: ProviderHeaders::from([(
            "HTTP-Referer".into(),
            Some("https://example.test".into()),
        )]),
        output: vec![ImagesOutputModality::Text, ImagesOutputModality::Image],
        cost: UsageCostRates {
            input: 0.015,
            output: 0.03,
            cache_read: 0.0,
            cache_write: 0.0,
        },
    }
}

#[test]
fn production_entrypoint_serializes_request_runs_hooks_and_parses_response() {
    let (base_url, captured, server) = captured_server(serde_json::json!({
        "id": "img-1",
        "usage": {
            "prompt_tokens": 12,
            "completion_tokens": 34,
            "prompt_tokens_details": { "cached_tokens": 0 }
        },
        "choices": [{
            "message": {
                "content": "Here is your image.",
                "images": [{ "image_url": "data:image/png;base64,ZmFrZS1wbmc=" }]
            }
        }]
    }));
    let payload_seen = Arc::new(Mutex::new(None));
    let response_seen = Arc::new(Mutex::new(None));
    let options = ImagesOptions {
        api_key: Some("test-key".into()),
        headers: ProviderHeaders::from([("X-Test".into(), Some("request".into()))]),
        on_payload: Some(Arc::new({
            let payload_seen = Arc::clone(&payload_seen);
            move |payload, _| {
                *payload_seen.lock().expect("payload capture") = Some(payload.clone());
                Box::pin(async { Ok(None) })
            }
        })),
        on_response: Some(Arc::new({
            let response_seen = Arc::clone(&response_seen);
            move |response, _| {
                *response_seen.lock().expect("response capture") = Some(response.status);
                Box::pin(async { Ok(()) })
            }
        })),
        ..ImagesOptions::default()
    };
    let context = ImagesContext {
        input: vec![
            ImagesContent::Text {
                text: "Generate a dog".into(),
            },
            ImagesContent::Image {
                mime_type: "image/png".into(),
                data: "aW5wdXQ=".into(),
            },
        ],
    };

    let output =
        futures::executor::block_on(generate_images(&model(base_url), &context, Some(&options)))
            .expect("builtin OpenRouter image API should be registered");
    server.join().expect("capture server");

    assert_eq!(output.stop_reason, ImagesStopReason::Stop);
    assert_eq!(output.response_id.as_deref(), Some("img-1"));
    assert_eq!(
        output.output,
        vec![
            ImagesContent::Text {
                text: "Here is your image.".into()
            },
            ImagesContent::Image {
                mime_type: "image/png".into(),
                data: "ZmFrZS1wbmc=".into()
            }
        ]
    );
    assert_eq!(
        output.usage.as_ref().map(|usage| usage.total_tokens),
        Some(46)
    );
    assert_eq!(*response_seen.lock().expect("response capture"), Some(200));

    let request = captured.lock().expect("request capture").clone();
    assert!(request.starts_with("POST /chat/completions HTTP/1.1\r\n"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("authorization: bearer test-key\r\n")
    );
    assert!(request.to_ascii_lowercase().contains("x-test: request\r\n"));
    let body: Value = serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(body["stream"], false);
    assert_eq!(body["modalities"], serde_json::json!(["image", "text"]));
    assert_eq!(body["messages"][0]["content"][0]["text"], "Generate a dog");
    assert_eq!(
        body["messages"][0]["content"][1]["image_url"]["url"],
        "data:image/png;base64,aW5wdXQ="
    );
    assert_eq!(
        payload_seen.lock().expect("payload capture").as_ref(),
        Some(&body)
    );
}

#[test]
fn production_entrypoint_returns_aborted_without_transport() {
    let controller = AbortController::new();
    controller.abort();
    let options = ImagesOptions {
        api_key: Some("test-key".into()),
        signal: Some(controller.signal()),
        ..ImagesOptions::default()
    };

    let output = futures::executor::block_on(generate_images(
        &model("http://127.0.0.1:1".into()),
        &ImagesContext::default(),
        Some(&options),
    ))
    .expect("registered provider returns an assistant result");

    assert_eq!(output.stop_reason, ImagesStopReason::Aborted);
    assert_eq!(output.error_message.as_deref(), Some("Request aborted"));
}

#[test]
fn production_entrypoint_returns_missing_key_error_without_transport() {
    let output = futures::executor::block_on(generate_images(
        &model("http://127.0.0.1:1".into()),
        &ImagesContext::default(),
        None,
    ))
    .expect("registered provider returns an assistant result");

    assert_eq!(output.stop_reason, ImagesStopReason::Error);
    assert_eq!(
        output.error_message.as_deref(),
        Some("No API key for provider: openrouter")
    );
}

fn captured_server(response: Value) -> (String, Arc<Mutex<String>>, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture server");
    let base_url = format!("http://{}", listener.local_addr().unwrap());
    let captured = Arc::new(Mutex::new(String::new()));
    let server_capture = Arc::clone(&captured);
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let read = stream.read(&mut buffer).expect("read request");
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
            if let Some(header_end) = find_bytes(&bytes, b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                if bytes.len() >= header_end + 4 + content_length {
                    break;
                }
            }
        }
        *server_capture.lock().expect("request capture") =
            String::from_utf8(bytes).expect("HTTP request is UTF-8");
        let body = response.to_string();
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nX-Test-Response: yes\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .expect("write response");
    });
    (base_url, captured, server)
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
