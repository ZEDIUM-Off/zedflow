//! Port of Pi `packages/ai/test/openai-completions-response-model.test.ts`.
//!
//! The Pi test is deterministic and injects a fake OpenAI client. Rust exercises the same
//! observable chunk semantics through the deterministic stream chunk processor.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use futures::executor::block_on;
use serde_json::{Value, json};
use zedflow_ai::api::openai_completions::{
    self, Context, Message, Model, ModelInput, OpenAICompletionsOptions, UserMessageContent,
};
use zedflow_ai::types::{AssistantContentBlock, StopReason};

#[derive(Debug, Clone, PartialEq, Eq)]
struct AssistantResult {
    model: String,
    response_model: Option<String>,
    provider: String,
    stop_reason: &'static str,
}

fn open_router_auto() -> Model {
    Model {
        id: "openrouter/auto".to_owned(),
        api: "openai-completions".to_owned(),
        provider: "openrouter".to_owned(),
        base_url: "https://openrouter.ai/api/v1".to_owned(),
        input: vec![ModelInput::Text],
        reasoning: false,
        thinking_level_map: HashMap::new(),
        headers: HashMap::new(),
        max_tokens: 4096,
        context_window: None,
        compat: None,
    }
}

fn user_hi_context() -> Context {
    Context {
        messages: vec![Message::User {
            content: UserMessageContent::Text("hi".to_owned()),
        }],
        ..Context::default()
    }
}

fn run_openai_completions_chunks(
    model: &Model,
    context: &Context,
    chunks: Vec<Value>,
) -> AssistantResult {
    let options = OpenAICompletionsOptions {
        api_key: Some("test".to_owned()),
        ..OpenAICompletionsOptions::default()
    };
    openai_completions::stream(model, context, Some(&options))
        .expect("request envelope should still be buildable");
    let result = openai_completions::process_openai_completions_stream_chunks(
        model,
        chunks.into_iter().map(Some),
    );

    AssistantResult {
        model: result.message.model,
        response_model: result.message.response_model,
        provider: result.message.provider,
        stop_reason: match result.message.stop_reason {
            openai_completions::StopReason::Stop => "stop",
            openai_completions::StopReason::Length => "length",
            openai_completions::StopReason::ToolUse => "toolUse",
            openai_completions::StopReason::Aborted => "aborted",
            openai_completions::StopReason::Error => "error",
        },
    }
}

#[test]
fn surfaces_routed_chunk_model_on_response_model_without_changing_model() {
    let model = open_router_auto();
    let context = user_hi_context();
    let chunks = vec![
        json!({
            "id": "chatcmpl-1",
            "model": "anthropic/claude-opus-4.8",
            "choices": [{ "index": 0, "delta": { "content": "hi" } }],
        }),
        json!({
            "id": "chatcmpl-1",
            "model": "anthropic/claude-opus-4.8",
            "choices": [{ "index": 0, "delta": {}, "finish_reason": "stop" }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "prompt_tokens_details": { "cached_tokens": 0 },
                "completion_tokens_details": { "reasoning_tokens": 0 },
            },
        }),
    ];

    let message = run_openai_completions_chunks(&model, &context, chunks);

    assert_eq!(message.model, "openrouter/auto");
    assert_eq!(
        message.response_model.as_deref(),
        Some("anthropic/claude-opus-4.8")
    );
    assert_eq!(message.provider, "openrouter");
    assert_eq!(message.stop_reason, "stop");
}

#[test]
fn leaves_response_model_unset_when_chunks_echo_the_requested_id() {
    let model = open_router_auto();
    let context = user_hi_context();
    let chunks = vec![
        json!({
            "id": "chatcmpl-2",
            "model": "openrouter/auto",
            "choices": [{ "index": 0, "delta": { "content": "hi" } }],
        }),
        json!({
            "id": "chatcmpl-2",
            "model": "openrouter/auto",
            "choices": [{ "index": 0, "delta": {}, "finish_reason": "stop" }],
            "usage": {
                "prompt_tokens": 1,
                "completion_tokens": 1,
                "prompt_tokens_details": { "cached_tokens": 0 },
                "completion_tokens_details": { "reasoning_tokens": 0 },
            },
        }),
    ];

    let message = run_openai_completions_chunks(&model, &context, chunks);

    assert_eq!(message.model, "openrouter/auto");
    assert_eq!(message.response_model, None);
}

#[test]
fn ignores_empty_or_missing_chunk_model() {
    let model = open_router_auto();
    let context = user_hi_context();
    let chunks = vec![
        json!({
            "id": "chatcmpl-3",
            "choices": [{ "index": 0, "delta": { "content": "hi" } }],
        }),
        json!({
            "id": "chatcmpl-3",
            "model": "",
            "choices": [{ "index": 0, "delta": { "content": "!" } }],
        }),
        json!({
            "id": "chatcmpl-3",
            "choices": [{ "index": 0, "delta": {}, "finish_reason": "stop" }],
            "usage": {
                "prompt_tokens": 1,
                "completion_tokens": 2,
                "prompt_tokens_details": { "cached_tokens": 0 },
                "completion_tokens_details": { "reasoning_tokens": 0 },
            },
        }),
    ];

    let message = run_openai_completions_chunks(&model, &context, chunks);

    assert_eq!(message.model, "openrouter/auto");
    assert_eq!(message.response_model, None);
}

fn serve_openai_completions_sse(response_body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local OpenAI SSE server");
    let url = format!("http://{}", listener.local_addr().expect("addr"));
    thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept request");
        let mut request = [0_u8; 4096];
        let read = socket.read(&mut request).expect("read request");
        let request = String::from_utf8_lossy(&request[..read]);
        assert!(request.starts_with("POST /chat/completions "));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer test")
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

#[test]
fn live_http_sse_transport_preserves_response_id_usage_and_hooks() {
    let body = concat!(
        "data: {\"id\":\"chatcmpl-live\",\"model\":\"openrouter/auto\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hi\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl-live\",\"model\":\"openrouter/auto\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":2,\"completion_tokens\":1,\"total_tokens\":3}}\n\n",
        "data: [DONE]\n\n",
    );
    let mut model = open_router_auto();
    model.base_url = serve_openai_completions_sse(body);
    let payload_called = Arc::new(AtomicBool::new(false));
    let response_called = Arc::new(AtomicBool::new(false));
    let payload_flag = Arc::clone(&payload_called);
    let response_flag = Arc::clone(&response_called);
    let options = OpenAICompletionsOptions {
        api_key: Some("test".to_owned()),
        on_payload: Some(Arc::new(move |mut payload, _| {
            payload_flag.store(true, Ordering::SeqCst);
            payload["metadata"] = json!({ "hook": true });
            Box::pin(async move { Ok(Some(payload)) })
        })),
        on_response: Some(Arc::new(move |response, _| {
            assert_eq!(response.status, 200);
            response_flag.store(true, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        })),
        ..OpenAICompletionsOptions::default()
    };

    let stream = openai_completions::stream_live(&model, &user_hi_context(), Some(&options))
        .expect("live stream should start");
    let message = block_on(stream.result());

    assert!(payload_called.load(Ordering::SeqCst));
    assert!(response_called.load(Ordering::SeqCst));
    assert_eq!(message.response_id.as_deref(), Some("chatcmpl-live"));
    assert_eq!(message.usage.total_tokens, 3);
    assert_eq!(message.stop_reason, StopReason::Stop);
    assert!(matches!(
        message.content.first(),
        Some(AssistantContentBlock::Text(text)) if text.text == "hi"
    ));
}
