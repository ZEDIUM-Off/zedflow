//! Port of Pi `packages/ai/test/azure-openai-base-url.test.ts`.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

use futures::{StreamExt, executor::block_on};
use serde_json::json;
use zedflow_ai::api::azure_openai_responses::{self, AzureOpenAIResponsesOptions, Context, Model};
use zedflow_ai::types::{
    AssistantContentBlock, AssistantMessageEvent, ErrorStopReason, StopReason, StreamOptions,
};

fn model() -> Model {
    Model {
        id: "gpt-4o-mini".to_string(),
        provider: "azure-openai-responses".to_string(),
        base_url: None,
        reasoning: false,
        thinking_level_map: HashMap::new(),
        headers: HashMap::new(),
    }
}

fn context() -> Context {
    Context
}

fn capture_client_base_url(base_url: &str) -> String {
    let options = AzureOpenAIResponsesOptions {
        api_key: Some("test-api-key".to_string()),
        azure_base_url: Some(base_url.to_string()),
        ..AzureOpenAIResponsesOptions::default()
    };

    azure_openai_responses::stream(&model(), &context(), Some(&options))
        .expect("request should build")
        .request
        .base_url
}

fn run_invalid_url() -> String {
    let options = AzureOpenAIResponsesOptions {
        api_key: Some("test-api-key".to_string()),
        azure_base_url: Some("not-a-url".to_string()),
        ..AzureOpenAIResponsesOptions::default()
    };

    azure_openai_responses::stream(&model(), &context(), Some(&options))
        .expect_err("invalid URL should fail")
        .to_string()
}

fn run_prompt_cache_key(session_id: &str) -> Option<String> {
    let options = AzureOpenAIResponsesOptions {
        api_key: Some("test-api-key".to_string()),
        azure_base_url: Some("https://my-resource.openai.azure.com".to_string()),
        session_id: Some(session_id.to_string()),
        ..AzureOpenAIResponsesOptions::default()
    };

    azure_openai_responses::stream(&model(), &context(), Some(&options))
        .expect("request should build")
        .request
        .body
        .get("prompt_cache_key")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn run_store_flag() -> Option<bool> {
    let options = AzureOpenAIResponsesOptions {
        api_key: Some("test-api-key".to_string()),
        azure_base_url: Some("https://my-resource.openai.azure.com".to_string()),
        ..AzureOpenAIResponsesOptions::default()
    };

    azure_openai_responses::stream(&model(), &context(), Some(&options))
        .expect("request should build")
        .request
        .body
        .get("store")
        .and_then(serde_json::Value::as_bool)
}

fn capture_resource_name_base_url(resource_name: &str) -> String {
    let options = AzureOpenAIResponsesOptions {
        api_key: Some("test-api-key".to_string()),
        azure_resource_name: Some(resource_name.to_string()),
        ..AzureOpenAIResponsesOptions::default()
    };

    azure_openai_responses::stream(&model(), &context(), Some(&options))
        .expect("request should build")
        .request
        .base_url
}

#[test]
fn normalizes_cognitive_services_root_endpoints_to_openai_v1() {
    assert_eq!(
        capture_client_base_url("https://marc-quicktests-resource.cognitiveservices.azure.com"),
        "https://marc-quicktests-resource.cognitiveservices.azure.com/openai/v1"
    );
}

#[test]
fn normalizes_microsoft_foundry_root_endpoints_to_openai_v1() {
    assert_eq!(
        capture_client_base_url("https://marc-quicktests-resource.ai.azure.com"),
        "https://marc-quicktests-resource.ai.azure.com/openai/v1"
    );
}

#[test]
fn normalizes_azure_openai_root_endpoints_to_openai_v1() {
    assert_eq!(
        capture_client_base_url("https://my-resource.openai.azure.com"),
        "https://my-resource.openai.azure.com/openai/v1"
    );
}

#[test]
fn normalizes_openai_to_openai_v1() {
    assert_eq!(
        capture_client_base_url("https://my-resource.cognitiveservices.azure.com/openai"),
        "https://my-resource.cognitiveservices.azure.com/openai/v1"
    );
}

#[test]
fn preserves_openai_v1_endpoints() {
    assert_eq!(
        capture_client_base_url("https://my-resource.cognitiveservices.azure.com/openai/v1"),
        "https://my-resource.cognitiveservices.azure.com/openai/v1"
    );
}

#[test]
fn normalizes_openai_v1_responses_to_openai_v1() {
    assert_eq!(
        capture_client_base_url("https://my-resource.services.ai.azure.com/openai/v1/responses"),
        "https://my-resource.services.ai.azure.com/openai/v1"
    );
}

#[test]
fn preserves_explicit_non_azure_proxy_paths() {
    assert_eq!(
        capture_client_base_url("https://my-proxy.example.com/v1"),
        "https://my-proxy.example.com/v1"
    );
}

#[test]
fn strips_query_params_when_normalizing_azure_host_urls() {
    assert_eq!(
        capture_client_base_url(
            "https://my-resource.openai.azure.com/openai?api-version=2024-12-01"
        ),
        "https://my-resource.openai.azure.com/openai/v1"
    );
}

#[test]
fn preserves_query_params_on_non_azure_proxy_urls() {
    assert_eq!(
        capture_client_base_url("https://my-proxy.example.com/v1?custom=true"),
        "https://my-proxy.example.com/v1?custom=true"
    );
}

#[test]
fn throws_on_invalid_urls() {
    assert!(run_invalid_url().contains("invalid Azure OpenAI base URL"));
}

#[test]
fn clamps_prompt_cache_key_to_openais_64_character_limit() {
    assert_eq!(run_prompt_cache_key(&"x".repeat(67)), Some("x".repeat(64)));
}

#[test]
fn disables_server_side_response_storage() {
    assert_eq!(run_store_flag(), Some(false));
}

#[test]
fn builds_correct_default_url_from_azure_openai_resource_name() {
    assert_eq!(
        capture_resource_name_base_url("my-resource"),
        "https://my-resource.openai.azure.com/openai/v1"
    );
}

fn canonical_model(base_url: String) -> zedflow_ai::types::Model {
    zedflow_ai::types::Model {
        id: "gpt-4o-mini".to_owned(),
        name: "GPT-4o mini".to_owned(),
        api: "azure-openai-responses".to_owned(),
        provider: "azure-openai-responses".to_owned(),
        base_url,
        ..zedflow_ai::types::Model::default()
    }
}

fn serve_once(
    status: u16,
    content_type: &str,
    body: String,
) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local Azure capture server");
    let base_url = format!("http://{}", listener.local_addr().expect("local address"));
    let content_type = content_type.to_owned();
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept Azure request");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let read = socket.read(&mut buffer).expect("read Azure request");
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n")
            else {
                continue;
            };
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.split_once(':')
                        .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                })
                .and_then(|(_, value)| value.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if request.len() >= header_end + 4 + content_length {
                break;
            }
        }
        write!(
            socket,
            "HTTP/1.1 {status} Test\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .expect("write Azure response");
        String::from_utf8(request).expect("captured Azure request is UTF-8")
    });
    (base_url, server)
}

fn serve_sequence(
    responses: Vec<(u16, String, String)>,
) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind retry capture server");
    let base_url = format!("http://{}", listener.local_addr().expect("local address"));
    let server = thread::spawn(move || {
        responses
            .into_iter()
            .map(|(status, content_type, body)| {
                let (mut socket, _) = listener.accept().expect("accept retry request");
                let mut request = Vec::new();
                let mut buffer = [0_u8; 4096];
                loop {
                    let read = socket.read(&mut buffer).expect("read retry request");
                    request.extend_from_slice(&buffer[..read]);
                    let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else { continue };
                    let headers = String::from_utf8_lossy(&request[..header_end]);
                    let length = headers.lines().find_map(|line| line.split_once(':').filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))).and_then(|(_, value)| value.trim().parse::<usize>().ok()).unwrap_or(0);
                    if request.len() >= header_end + 4 + length { break; }
                }
                write!(socket, "HTTP/1.1 {status} Test\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).expect("write retry response");
                String::from_utf8(request).expect("retry request UTF-8")
            })
            .collect()
    });
    (base_url, server)
}

fn serve_delayed_terminal() -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind abort capture server");
    let base_url = format!("http://{}", listener.local_addr().expect("local address"));
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept abort request");
        let mut request = [0_u8; 4096];
        let _ = socket.read(&mut request).expect("read abort request");
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"status\":\"in_progress\",\"content\":[]}}\n\ndata: {\"type\":\"response.output_text.delta\",\"output_index\":0,\"content_index\":0,\"delta\":\"hello\"}\n\n").expect("write abort prefix");
        socket.flush().expect("flush abort prefix");
        thread::sleep(Duration::from_millis(100));
        let _ = socket.write_all(b"data: {\"type\":\"response.completed\",\"response\":{\"id\":\"too_late\",\"status\":\"completed\",\"output\":[],\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"total_tokens\":2}}}\n\n");
    });
    (base_url, server)
}

fn completed_sse() -> String {
    concat!(
        "data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"id\":\"msg_1\",\"type\":\"message\",\"role\":\"assistant\",\"status\":\"in_progress\",\"content\":[]}}\n\n",
        "data: {\"type\":\"response.output_text.delta\",\"output_index\":0,\"content_index\":0,\"delta\":\"hello\"}\n\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_azure\",\"status\":\"completed\",\"output\":[],\"usage\":{\"input_tokens\":2,\"output_tokens\":1,\"total_tokens\":3,\"input_tokens_details\":{\"cached_tokens\":0},\"output_tokens_details\":{\"reasoning_tokens\":0}}}}\n\n"
    )
    .to_owned()
}

#[test]
fn registered_transport_captures_endpoint_api_key_body_events_and_response_id() {
    let (base_url, server) = serve_once(200, "text/event-stream", completed_sse());
    let mut options = StreamOptions {
        api_key: Some("azure-secret".to_owned()),
        session_id: Some("session-azure".to_owned()),
        max_tokens: Some(1),
        ..StreamOptions::default()
    };
    options
        .extra
        .insert("azureApiVersion".to_owned(), json!("2025-03-01-preview"));
    options
        .extra
        .insert("azureDeploymentName".to_owned(), json!("deployment-one"));
    let stream = azure_openai_responses::stream_registered(
        &canonical_model(base_url),
        &zedflow_ai::types::Context {
            system_prompt: Some("be exact".to_owned()),
            ..Default::default()
        },
        Some(&options),
    );
    let message = block_on(stream.result());
    let request = server.join().expect("Azure capture server");

    assert!(request.starts_with("POST /responses?api-version=2025-03-01-preview HTTP/1.1"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("api-key: azure-secret")
    );
    assert!(!request.to_ascii_lowercase().contains("authorization:"));
    let body: serde_json::Value =
        serde_json::from_str(request.split("\r\n\r\n").nth(1).expect("request body"))
            .expect("JSON body");
    assert_eq!(body["model"], "deployment-one");
    assert_eq!(body["stream"], true);
    assert_eq!(body["store"], false);
    assert_eq!(body["max_output_tokens"], 16);
    assert_eq!(body["prompt_cache_key"], "session-azure");
    assert_eq!(body["input"][0]["content"], "be exact");
    assert_eq!(
        message.stop_reason,
        StopReason::Stop,
        "{:?}",
        message.error_message
    );
    assert_eq!(message.response_id.as_deref(), Some("resp_azure"));
    assert!(matches!(
        message.content.first(),
        Some(AssistantContentBlock::Text(text)) if text.text == "hello"
    ));
}

#[test]
fn registered_transport_rejects_bearer_header_without_api_key() {
    let mut headers = HashMap::new();
    headers.insert(
        "Authorization".to_owned(),
        Some("Bearer azure-token".to_owned()),
    );
    let options = StreamOptions {
        headers: Some(headers),
        ..StreamOptions::default()
    };
    let stream = azure_openai_responses::stream_registered(
        &canonical_model("http://127.0.0.1:9".to_owned()),
        &zedflow_ai::types::Context::default(),
        Some(&options),
    );
    let message = block_on(stream.result());
    assert_eq!(message.stop_reason, StopReason::Error);
    assert_eq!(
        message.error_message.as_deref(),
        Some("no API key for provider: azure-openai-responses")
    );
}

#[test]
fn registered_transport_retries_429_then_succeeds_once() {
    let (base_url, server) = serve_sequence(vec![
        (
            429,
            "application/json".to_owned(),
            "rate limited".to_owned(),
        ),
        (200, "text/event-stream".to_owned(), completed_sse()),
    ]);
    let options = StreamOptions {
        api_key: Some("azure-secret".to_owned()),
        max_retries: Some(1),
        ..StreamOptions::default()
    };
    let message = block_on(
        azure_openai_responses::stream_registered(
            &canonical_model(base_url),
            &zedflow_ai::types::Context::default(),
            Some(&options),
        )
        .result(),
    );
    let requests = server.join().expect("retry capture server");
    assert_eq!(requests.len(), 2);
    assert!(
        requests
            .iter()
            .all(|request| request.starts_with("POST /responses?api-version=v1 HTTP/1.1"))
    );
    assert_eq!(
        message.stop_reason,
        StopReason::Stop,
        "{:?}",
        message.error_message
    );
    assert_eq!(message.response_id.as_deref(), Some("resp_azure"));
}

#[test]
fn registered_transport_abort_wins_over_delayed_terminal_frame() {
    let (base_url, server) = serve_delayed_terminal();
    let controller = zedflow_ai::utils::abort_signals::AbortController::new();
    let options = StreamOptions {
        api_key: Some("azure-secret".to_owned()),
        signal: Some(controller.signal()),
        ..StreamOptions::default()
    };
    let mut stream = azure_openai_responses::stream_registered(
        &canonical_model(base_url),
        &zedflow_ai::types::Context::default(),
        Some(&options),
    );
    let events = block_on(async {
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            if matches!(event, AssistantMessageEvent::TextDelta { .. }) {
                controller.abort();
            }
            events.push(event);
        }
        events
    });
    server.join().expect("abort capture server");
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
            error,
        }) if error.stop_reason == StopReason::Aborted
            && error.error_message.as_deref() == Some("Request was aborted")
    ));
}

#[test]
fn registered_transport_formats_stream_errors_once() {
    let (base_url, server) = serve_once(
        200,
        "text/event-stream",
        "data: {\"type\":\"error\",\"code\":\"bad_stream\",\"message\":\"stream exploded\"}\n\n"
            .to_owned(),
    );
    let options = StreamOptions {
        api_key: Some("azure-secret".to_owned()),
        ..StreamOptions::default()
    };
    let message = block_on(
        azure_openai_responses::stream_registered(
            &canonical_model(base_url),
            &zedflow_ai::types::Context::default(),
            Some(&options),
        )
        .result(),
    );
    server.join().expect("stream error capture server");
    assert_eq!(message.stop_reason, StopReason::Error);
    let error = message.error_message.expect("stream error message");
    assert!(error.contains("stream exploded"));
    assert_eq!(error.matches("Azure OpenAI API error").count(), 0);
}

#[test]
fn registered_transport_preserves_azure_http_error_body() {
    let (base_url, server) = serve_once(
        429,
        "application/json",
        r#"{\"error\":{\"message\":\"quota exhausted\"}}"#.to_owned(),
    );
    let options = StreamOptions {
        api_key: Some("azure-secret".to_owned()),
        ..StreamOptions::default()
    };
    let stream = azure_openai_responses::stream_registered(
        &canonical_model(base_url),
        &zedflow_ai::types::Context::default(),
        Some(&options),
    );
    let message = block_on(stream.result());
    server.join().expect("Azure capture server");
    assert_eq!(message.stop_reason, StopReason::Error);
    let error = message.error_message.expect("Azure HTTP error message");
    assert!(error.contains("Azure OpenAI API error (429)"));
    assert!(error.contains("quota exhausted"));
    assert_eq!(error.matches("Azure OpenAI API error").count(), 1);
}
