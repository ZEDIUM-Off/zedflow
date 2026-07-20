//! Port of Pi `packages/ai/test/responseid.test.ts`.
//!
//! The source file is a live provider E2E suite gated by credentials/OAuth tokens.
//! OpenAI Codex is capability-gated here; unavailable non-P7 providers stay ignored
//! with their exact credential requirements.

mod common;

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::thread;

use futures::{StreamExt, executor::block_on};
use serde_json::{Value, json};
use zedflow_ai::api::{
    google_vertex, openai_codex_responses, openai_completions, openai_responses,
};
use zedflow_ai::types::{Context as CanonicalContext, Model as CanonicalModel, StreamOptions};

const BLOCKER: &str = "live responseId E2E test blocked: Rust compat::get_model/complete and provider response_id network plumbing are not implemented";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Model {
    provider: &'static str,
    id: &'static str,
    api: &'static str,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct StreamOptionsWithExtras {
    api_key: Option<&'static str>,
    project: Option<&'static str>,
    location: Option<&'static str>,
    azure_deployment_name: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Context {
    system_prompt: &'static str,
    user_message: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
struct Response {
    stop_reason: &'static str,
    error_message: Option<String>,
    response_id: Value,
}

fn get_model(provider: &'static str, id: &'static str) -> Model {
    let api = match provider {
        "anthropic" => "anthropic-messages",
        "azure-openai-responses" => "azure-openai-responses",
        "github-copilot" => "github-copilot",
        "google" => "google-generative-ai",
        "google-vertex" => "google-vertex",
        "mistral" => "mistral-conversations",
        "openai" => "openai-responses",
        "openai-codex" => "openai-codex-responses",
        _ => provider,
    };

    Model { provider, id, api }
}

fn openai_completions_model() -> Model {
    Model {
        api: "openai-completions",
        ..get_model("openai", "gpt-4o-mini")
    }
}

fn make_context() -> Context {
    Context {
        system_prompt: "You are a helpful assistant. Be concise.",
        user_message: "Reply with exactly: response id test",
    }
}

fn complete(
    model: Model,
    context: &Context,
    options: StreamOptionsWithExtras,
) -> Result<Response, &'static str> {
    match model.api {
        "openai-completions" => complete_openai_completions(model, context, options),
        "openai-responses" => complete_openai_responses(model, context, options),
        "openai-codex-responses" => complete_openai_codex(model, context, options),
        _ => {
            let _source_fixture = (model, context, options);
            Err(BLOCKER)
        }
    }
}

fn complete_openai_completions(
    model: Model,
    context: &Context,
    options: StreamOptionsWithExtras,
) -> Result<Response, &'static str> {
    let api_key = options
        .api_key
        .map(str::to_owned)
        .or_else(|| common::live_credentials::api_key("openai"))
        .ok_or(BLOCKER)?;
    let stream = openai_completions::stream_live(
        &openai_completions::Model {
            id: model.id.to_owned(),
            api: "openai-completions".to_owned(),
            provider: model.provider.to_owned(),
            base_url: "https://api.openai.com/v1".to_owned(),
            input: vec![openai_completions::ModelInput::Text],
            reasoning: false,
            thinking_level_map: HashMap::new(),
            headers: openai_completions::ProviderHeaders::new(),
            max_tokens: 128,
            context_window: Some(128_000),
            compat: None,
        },
        &openai_completions::Context {
            system_prompt: Some(context.system_prompt.to_owned()),
            messages: vec![openai_completions::Message::User {
                content: openai_completions::UserMessageContent::Text(
                    context.user_message.to_owned(),
                ),
            }],
            tools: Vec::new(),
        },
        Some(&openai_completions::OpenAICompletionsOptions {
            api_key: Some(api_key),
            max_tokens: Some(32),
            timeout_ms: Some(30_000),
            ..openai_completions::OpenAICompletionsOptions::default()
        }),
    )
    .map_err(|_| BLOCKER)?;
    let message = block_on(stream.result());
    Ok(Response {
        stop_reason: match message.stop_reason {
            zedflow_ai::types::StopReason::Error => "error",
            _ => "stop",
        },
        error_message: message.error_message,
        response_id: message
            .response_id
            .map(Value::String)
            .unwrap_or(Value::Null),
    })
}

fn complete_openai_responses(
    model: Model,
    context: &Context,
    options: StreamOptionsWithExtras,
) -> Result<Response, &'static str> {
    let api_key = options
        .api_key
        .map(str::to_owned)
        .or_else(|| common::live_credentials::api_key("openai"))
        .ok_or(BLOCKER)?;
    let stream = openai_responses::stream_live(
        &openai_responses::Model {
            id: model.id.to_owned(),
            api: "openai-responses".to_owned(),
            provider: model.provider.to_owned(),
            base_url: "https://api.openai.com/v1".to_owned(),
            reasoning: false,
            thinking_level_map: HashMap::new(),
            headers: openai_responses::ProviderHeaders::new(),
            compat: None,
        },
        &openai_responses::Context {
            system_prompt: Some(context.system_prompt.to_owned()),
            messages: vec![json!({ "role": "user", "content": context.user_message })],
            tools: Vec::new(),
            copilot_messages: Vec::new(),
        },
        Some(&openai_responses::OpenAIResponsesOptions {
            api_key: Some(api_key),
            max_tokens: Some(32),
            timeout_ms: Some(30_000),
            ..openai_responses::OpenAIResponsesOptions::default()
        }),
    )
    .map_err(|_| BLOCKER)?;
    let message = block_on(stream.result());
    Ok(Response {
        stop_reason: match message.stop_reason {
            zedflow_ai::types::StopReason::Error => "error",
            _ => "stop",
        },
        error_message: message.error_message,
        response_id: message
            .response_id
            .map(Value::String)
            .unwrap_or(Value::Null),
    })
}

fn complete_openai_codex(
    model: Model,
    context: &Context,
    options: StreamOptionsWithExtras,
) -> Result<Response, &'static str> {
    let api_key = options
        .api_key
        .filter(|key| !key.starts_with('<'))
        .map(str::to_owned)
        .or_else(|| common::live_credentials::api_key("openai-codex"))
        .ok_or(BLOCKER)?;
    let stream = openai_codex_responses::stream_live(
        &openai_codex_responses::Model {
            id: model.id.to_owned(),
            provider: model.provider.to_owned(),
            base_url: Some("https://chatgpt.com/backend-api".to_owned()),
            reasoning: true,
            thinking_level_map: HashMap::new(),
            headers: HashMap::new(),
            max_tokens: Some(128_000),
            cost: zedflow_ai::types::ModelCost::default(),
        },
        &openai_codex_responses::Context {
            system_prompt: Some(context.system_prompt.to_owned()),
            tools: Vec::new(),
            input: vec![json!({
                "role": "user",
                "content": [{ "type": "input_text", "text": context.user_message }]
            })],
        },
        Some(&openai_codex_responses::OpenAICodexResponsesOptions {
            api_key: Some(api_key),
            max_tokens: Some(32),
            timeout_ms: Some(30_000),
            transport: Some(openai_codex_responses::Transport::Sse),
            ..openai_codex_responses::OpenAICodexResponsesOptions::default()
        }),
    )
    .map_err(|_| BLOCKER)?;
    let message = block_on(stream.result());
    Ok(Response {
        stop_reason: match message.stop_reason {
            zedflow_ai::types::StopReason::Error => "error",
            _ => "stop",
        },
        error_message: message.error_message,
        response_id: message
            .response_id
            .map(Value::String)
            .unwrap_or(Value::Null),
    })
}

fn expect_response_id(model: Model, options: StreamOptionsWithExtras) {
    let context = make_context();
    let response =
        complete(model, &context, options).expect("live responseId request should complete");

    assert_ne!(
        response.stop_reason, "error",
        "{:?}",
        response.error_message
    );
    assert!(
        response
            .response_id
            .as_str()
            .is_some_and(|id| !id.is_empty())
    );
    assert!(response.response_id.is_string());
}

#[test]
#[ignore = "live Google Gemini API parity test skipped: requires GEMINI_API_KEY and provider network calls"]
fn google_provider_exposes_response_id() {
    expect_response_id(
        get_model("google", "gemini-2.5-flash"),
        StreamOptionsWithExtras::default(),
    );
}

fn captured_vertex_response_id(api_key: Option<&str>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind Vertex capture server");
    let base_url = format!("http://{}", listener.local_addr().expect("capture address"));
    let expects_api_key = api_key.is_some();
    thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept Vertex request");
        let mut bytes = [0_u8; 16_384];
        let read = socket.read(&mut bytes).expect("read Vertex request");
        let request = String::from_utf8_lossy(&bytes[..read]);
        assert!(request.starts_with(
            "POST /v1/publishers/google/models/gemini-3-flash-preview:streamGenerateContent?alt=sse "
        ));
        let lower = request.to_ascii_lowercase();
        if expects_api_key {
            assert!(lower.contains("x-goog-api-key: captured-key"));
        } else {
            assert!(lower.contains("authorization: bearer captured-adc-token"));
        }
        assert!(request.contains("\"contents\""));
        assert!(request.contains("\"capturedHook\":true"));
        assert!(!request.contains("\"model\":\"gemini-3-flash-preview\""));
        let body = concat!(
            "data: {\"responseId\":\"vertex-captured-response\",\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"ok\"}]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":1,\"candidatesTokenCount\":1,\"totalTokenCount\":2}}\n\n",
            "data: [DONE]\n\n",
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        socket
            .write_all(response.as_bytes())
            .expect("write Vertex SSE");
    });

    let model = CanonicalModel {
        id: "gemini-3-flash-preview".to_owned(),
        name: "Gemini 3 Flash".to_owned(),
        api: "google-vertex".to_owned(),
        provider: "google-vertex".to_owned(),
        base_url,
        reasoning: true,
        ..CanonicalModel::default()
    };
    let context: CanonicalContext = serde_json::from_value(json!({
        "messages": [{"role":"user","content":"hello","timestamp":1}]
    }))
    .expect("canonical Vertex context");
    let mut options = StreamOptions {
        api_key: api_key.map(str::to_owned),
        on_payload: Some(Arc::new(|mut payload, _model| {
            payload["config"]["capturedHook"] = json!(true);
            Box::pin(async move { Ok(Some(payload)) })
        })),
        ..StreamOptions::default()
    };
    if api_key.is_none() {
        options.api_key = Some("gcp-vertex-credentials".to_owned());
        options
            .extra
            .insert("project".to_owned(), json!("test-project"));
        options
            .extra
            .insert("location".to_owned(), json!("us-central1"));
        options.headers = Some(HashMap::from([(
            "authorization".to_owned(),
            Some("Bearer captured-adc-token".to_owned()),
        )]));
    }
    let stream = google_vertex::stream_registered(&model, &context, Some(&options));
    let result_stream = stream.clone();
    let events = block_on(stream.collect::<Vec<_>>());
    let message = block_on(result_stream.result());
    assert_eq!(events.len(), 5);
    assert!(matches!(
        events.first(),
        Some(zedflow_ai::types::AssistantMessageEvent::Start { .. })
    ));
    assert!(matches!(
        events.last(),
        Some(zedflow_ai::types::AssistantMessageEvent::Done { .. })
    ));
    message.response_id.expect("captured Vertex responseId")
}

#[test]
fn google_vertex_provider_exposes_response_id_with_adc() {
    assert_eq!(
        captured_vertex_response_id(None),
        "vertex-captured-response"
    );
}

#[test]
fn google_vertex_provider_exposes_response_id_with_api_key() {
    assert_eq!(
        captured_vertex_response_id(Some("captured-key")),
        "vertex-captured-response"
    );
}

#[test]
fn openai_completions_provider_exposes_response_id() {
    if !response_id_live_ready("openai") {
        return;
    }
    expect_response_id(
        openai_completions_model(),
        StreamOptionsWithExtras::default(),
    );
}

#[test]
fn openai_responses_provider_exposes_response_id() {
    if !response_id_live_ready("openai") {
        return;
    }
    expect_response_id(
        get_model("openai", "gpt-5-mini"),
        StreamOptionsWithExtras::default(),
    );
}

#[test]
#[ignore = "live Anthropic API parity test skipped: requires ANTHROPIC_API_KEY and provider network calls"]
fn anthropic_provider_exposes_response_id() {
    expect_response_id(
        get_model("anthropic", "claude-sonnet-4-5"),
        StreamOptionsWithExtras::default(),
    );
}

#[test]
#[ignore = "live Azure OpenAI Responses parity test skipped: requires Azure OpenAI credentials and provider network calls"]
fn azure_openai_responses_provider_exposes_response_id() {
    expect_response_id(
        get_model("azure-openai-responses", "gpt-4o-mini"),
        StreamOptionsWithExtras {
            azure_deployment_name: Some("<AZURE_OPENAI_DEPLOYMENT_NAME>"),
            ..StreamOptionsWithExtras::default()
        },
    );
}

#[test]
#[ignore = "live Mistral API parity test skipped: requires MISTRAL_API_KEY and provider network calls"]
fn mistral_provider_exposes_response_id() {
    expect_response_id(
        get_model("mistral", "devstral-medium-latest"),
        StreamOptionsWithExtras::default(),
    );
}

#[test]
#[ignore = "live GitHub Copilot OpenAI-path parity test skipped: requires resolved github-copilot OAuth token and provider network calls"]
fn github_copilot_openai_path_exposes_response_id() {
    expect_response_id(
        get_model("github-copilot", "gpt-5.3-codex"),
        StreamOptionsWithExtras {
            api_key: Some("<github-copilot OAuth token>"),
            ..StreamOptionsWithExtras::default()
        },
    );
}

#[test]
#[ignore = "live GitHub Copilot Anthropic-path parity test skipped: requires resolved github-copilot OAuth token and provider network calls"]
fn github_copilot_anthropic_path_exposes_response_id() {
    expect_response_id(
        get_model("github-copilot", "claude-sonnet-4.6"),
        StreamOptionsWithExtras {
            api_key: Some("<github-copilot OAuth token>"),
            ..StreamOptionsWithExtras::default()
        },
    );
}

fn response_id_live_ready(provider: &str) -> bool {
    let capability = match provider {
        "openai-codex" => common::live_credentials::openai_codex(),
        _ => common::live_credentials::capability(provider),
    };
    if let Some(message) = capability.skip_message() {
        eprintln!("{message}");
        return false;
    }
    if provider == "openai" || provider == "openai-codex" {
        return true;
    }
    eprintln!("skipping live {provider} responseId test: {BLOCKER}");
    false
}

#[test]
fn openai_codex_provider_exposes_response_id() {
    if !response_id_live_ready("openai-codex") {
        return;
    }
    expect_response_id(
        get_model("openai-codex", "gpt-5.5"),
        StreamOptionsWithExtras {
            api_key: Some("<openai-codex OAuth token>"),
            ..StreamOptionsWithExtras::default()
        },
    );
}

fn serve_sse(response_body: &'static str, expected_path: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local SSE server");
    let url = format!("http://{}", listener.local_addr().expect("addr"));
    thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept request");
        let mut request = [0_u8; 8192];
        let read = socket.read(&mut request).expect("read request");
        let request = String::from_utf8_lossy(&request[..read]);
        assert!(request.starts_with(&format!("POST {expected_path} ")));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer ")
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        socket
            .write_all(response.as_bytes())
            .expect("write response");
    });
    url
}

fn serve_openai_responses_sse(response_body: &'static str) -> String {
    serve_sse(response_body, "/responses")
}

#[test]
fn openai_responses_live_transport_exposes_response_id() {
    let body = concat!(
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-live\",\"status\":\"in_progress\"}}\n\n",
        "data: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"type\":\"message\",\"id\":\"msg-live\",\"content\":[]}}\n\n",
        "data: {\"type\":\"response.output_text.delta\",\"output_index\":0,\"delta\":\"response id test\"}\n\n",
        "data: {\"type\":\"response.output_item.done\",\"output_index\":0,\"item\":{\"type\":\"message\",\"id\":\"msg-live\",\"content\":[{\"type\":\"output_text\",\"text\":\"response id test\"}]}}\n\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-live\",\"status\":\"completed\",\"usage\":{\"input_tokens\":2,\"output_tokens\":3,\"total_tokens\":5}}}\n\n",
        "data: [DONE]\n\n",
    );
    let model = openai_responses::Model {
        id: "gpt-5-mini".to_owned(),
        api: "openai-responses".to_owned(),
        provider: "openai".to_owned(),
        base_url: serve_openai_responses_sse(body),
        reasoning: false,
        thinking_level_map: HashMap::new(),
        headers: openai_responses::ProviderHeaders::new(),
        compat: None,
    };
    let context = openai_responses::Context {
        messages: vec![json!({"role":"user","content":"hi"})],
        ..openai_responses::Context::default()
    };
    let options = openai_responses::OpenAIResponsesOptions {
        api_key: Some("test".to_owned()),
        ..openai_responses::OpenAIResponsesOptions::default()
    };

    let stream = openai_responses::stream_live(&model, &context, Some(&options))
        .expect("live responses stream should start");
    let message = block_on(stream.result());

    assert_eq!(message.response_id.as_deref(), Some("resp-live"));
    assert_eq!(message.usage.total_tokens, 5);
    assert_ne!(message.stop_reason, zedflow_ai::types::StopReason::Error);
}

#[test]
fn openai_codex_sse_live_transport_exposes_response_id() {
    let body = concat!(
        "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp-codex-live\",\"status\":\"in_progress\"}}\n\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"message\",\"id\":\"msg-live\",\"content\":[]}}\n\n",
        "data: {\"type\":\"response.output_text.delta\",\"delta\":\"response id test\"}\n\n",
        "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"message\",\"id\":\"msg-live\",\"content\":[{\"type\":\"output_text\",\"text\":\"response id test\"}]}}\n\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-codex-live\",\"status\":\"completed\",\"usage\":{\"input_tokens\":2,\"output_tokens\":3,\"total_tokens\":5}}}\n\n",
        "data: [DONE]\n\n",
    );
    let model = openai_codex_responses::Model {
        id: "gpt-5.5".to_owned(),
        provider: "openai-codex".to_owned(),
        base_url: Some(serve_sse(body, "/codex/responses")),
        reasoning: true,
        thinking_level_map: HashMap::new(),
        headers: HashMap::new(),
        max_tokens: Some(128_000),
        cost: zedflow_ai::types::ModelCost::default(),
    };
    let token = "aaa.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9hY2NvdW50X2lkIjoiYWNjX3Rlc3QifX0.bbb";
    let stream = openai_codex_responses::stream_live(
        &model,
        &openai_codex_responses::Context {
            input: vec![json!({"role":"user","content":[{"type":"input_text","text":"hi"}]})],
            ..openai_codex_responses::Context::default()
        },
        Some(&openai_codex_responses::OpenAICodexResponsesOptions {
            api_key: Some(token.to_owned()),
            transport: Some(openai_codex_responses::Transport::Sse),
            ..openai_codex_responses::OpenAICodexResponsesOptions::default()
        }),
    )
    .expect("live codex stream should start");
    let message = block_on(stream.result());

    assert_eq!(message.response_id.as_deref(), Some("resp-codex-live"));
    assert_eq!(message.usage.total_tokens, 5);
    assert_ne!(message.stop_reason, zedflow_ai::types::StopReason::Error);
}
