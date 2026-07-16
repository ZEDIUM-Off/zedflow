use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

use zedflow_ai::api::openai_completions::{
    CacheRetention, Context, Model, ModelInput, OpenAICompletionsCompat, OpenAICompletionsOptions,
    build_request,
};

fn model(base_url: &str, compat: Option<OpenAICompletionsCompat>) -> Model {
    Model {
        id: "gpt-4o-mini".into(),
        api: "openai-completions".into(),
        provider: "openai".into(),
        base_url: base_url.into(),
        input: vec![ModelInput::Text],
        reasoning: false,
        thinking_level_map: HashMap::new(),
        headers: HashMap::new(),
        max_tokens: 100,
        context_window: None,
        compat,
    }
}

fn request(
    model: &Model,
    options: OpenAICompletionsOptions,
) -> zedflow_ai::api::openai_completions::OpenAICompletionsRequest {
    build_request(
        model,
        &Context::default(),
        Some(&OpenAICompletionsOptions {
            api_key: Some("test".into()),
            ..options
        }),
    )
    .expect("request")
}

#[test]
fn direct_openai_cache_fields_follow_retention_env_and_unicode_limit() {
    let direct = model("https://api.openai.com/v1", None);
    let short = request(
        &direct,
        OpenAICompletionsOptions {
            session_id: Some(format!("{}tail", "🦀".repeat(64))),
            ..Default::default()
        },
    );
    assert_eq!(short.body["prompt_cache_key"], "🦀".repeat(64));
    assert!(short.body.get("prompt_cache_retention").is_none());

    let mut env = HashMap::new();
    env.insert("PI_CACHE_RETENTION".into(), "long".into());
    let long = request(
        &direct,
        OpenAICompletionsOptions {
            session_id: Some("session-env".into()),
            env,
            ..Default::default()
        },
    );
    assert_eq!(long.body["prompt_cache_key"], "session-env");
    assert_eq!(long.body["prompt_cache_retention"], "24h");

    let none = request(
        &direct,
        OpenAICompletionsOptions {
            cache_retention: Some(CacheRetention::None),
            session_id: Some("session".into()),
            ..Default::default()
        },
    );
    assert!(none.body.get("prompt_cache_key").is_none());
    assert!(none.body.get("prompt_cache_retention").is_none());
}

#[test]
fn proxy_cache_compat_and_session_affinity_match_pi() {
    let incompatible = model(
        "https://proxy.example/v1",
        Some(OpenAICompletionsCompat {
            supports_long_cache_retention: Some(false),
            ..Default::default()
        }),
    );
    let omitted = request(
        &incompatible,
        OpenAICompletionsOptions {
            cache_retention: Some(CacheRetention::Long),
            session_id: Some("session".into()),
            ..Default::default()
        },
    );
    assert!(omitted.body.get("prompt_cache_key").is_none());
    assert!(omitted.body.get("prompt_cache_retention").is_none());

    let affinity = model(
        "https://proxy.example/v1",
        Some(OpenAICompletionsCompat {
            send_session_affinity_headers: Some(true),
            ..Default::default()
        }),
    );
    let mut headers = HashMap::new();
    headers.insert("x-client-request-id".into(), "override".into());
    let generated = request(
        &affinity,
        OpenAICompletionsOptions {
            session_id: Some("session-affinity".into()),
            headers,
            ..Default::default()
        },
    );
    assert_eq!(generated.headers["session_id"], "session-affinity");
    assert_eq!(generated.headers["x-session-affinity"], "session-affinity");
    assert_eq!(generated.headers["x-client-request-id"], "override");
}

#[tokio::test]
async fn registered_transport_sends_prompt_cache_fields_on_the_wire() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (body_tx, body_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0; 4096];
        loop {
            let read = socket.read(&mut buffer).unwrap();
            request.extend_from_slice(&buffer[..read]);
            let Some(headers_end) = request.windows(4).position(|window| window == b"\r\n\r\n")
            else {
                continue;
            };
            let headers = String::from_utf8_lossy(&request[..headers_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(str::trim)
                        .and_then(|value| value.parse::<usize>().ok())
                })
                .unwrap();
            if request.len() >= headers_end + 4 + content_length {
                body_tx
                    .send(
                        serde_json::from_slice::<serde_json::Value>(
                            &request[headers_end + 4..headers_end + 4 + content_length],
                        )
                        .unwrap(),
                    )
                    .unwrap();
                break;
            }
        }
        let body = "data: {\"id\":\"cache-id\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";
        write!(
            socket,
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
    });
    let registered = zedflow_ai::types::Model {
        id: "gpt-4o-mini".into(),
        name: "GPT".into(),
        api: "openai-completions".into(),
        provider: "openai".into(),
        base_url: url,
        max_tokens: 100,
        context_window: 4096,
        ..Default::default()
    };
    let stream = zedflow_ai::api::openai_completions::stream_registered(
        &registered,
        &Default::default(),
        Some(&zedflow_ai::types::StreamOptions {
            api_key: Some("test".into()),
            cache_retention: Some(zedflow_ai::types::CacheRetention::Long),
            session_id: Some("registered-session".into()),
            ..Default::default()
        }),
    );
    assert_eq!(
        stream.result().await.stop_reason,
        zedflow_ai::types::StopReason::Stop
    );
    let body = body_rx.recv().unwrap();
    assert_eq!(body["prompt_cache_key"], "registered-session");
    assert_eq!(body["prompt_cache_retention"], "24h");
    server.join().unwrap();
}
