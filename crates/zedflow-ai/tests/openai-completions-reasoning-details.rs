use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::{Value, json};
use zedflow_ai::api::openai_completions::{
    AssistantMessage, ContentBlock, Context, Message, Model, ModelInput, StopReason, Tool,
    build_request, get_compat, process_openai_completions_stream_chunks,
};

fn model() -> Model {
    Model {
        id: "google/gemini-test".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        input: vec![ModelInput::Text],
        reasoning: true,
        thinking_level_map: HashMap::new(),
        headers: HashMap::new(),
        max_tokens: 4096,
        context_window: Some(100_000),
        compat: None,
    }
}

#[test]
fn reasoning_detail_before_tool_call_is_preserved_and_replayed() {
    let model = model();
    let detail = json!({"type":"reasoning.encrypted","id":"call_1","data":"encrypted-signature"});
    let result = process_openai_completions_stream_chunks(
        &model,
        [
            Some(
                json!({"id":"chatcmpl-test","choices":[{"delta":{"reasoning_details":[detail.clone()]}}]}),
            ),
            Some(
                json!({"id":"chatcmpl-test","choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"read","arguments":"{\"path\":\"README.md\"}"}}]}}]}),
            ),
            Some(
                json!({"id":"chatcmpl-test","choices":[{"delta":{},"finish_reason":"tool_calls"}]}),
            ),
        ],
    );
    let ContentBlock::ToolCall(call) = result.message.content.first().expect("tool call") else {
        panic!("tool call")
    };
    assert_eq!(
        call.thought_signature.as_deref(),
        Some(detail.to_string().as_str())
    );

    let context = Context {
        messages: vec![Message::Assistant(AssistantMessage {
            api: model.api.clone(),
            provider: model.provider.clone(),
            model: model.id.clone(),
            content: result.message.content,
            stop_reason: StopReason::ToolUse,
        })],
        tools: vec![Tool {
            name: "read".into(),
            description: "Read".into(),
            parameters: json!({"type":"object"}),
        }],
        ..Default::default()
    };
    let request = build_request(&model, &context, None).expect("request");
    assert_eq!(
        request.body["messages"][0]["reasoning_details"],
        Value::Array(vec![detail])
    );
    assert!(!get_compat(&model).requires_thinking_as_text);
}

#[tokio::test]
async fn registered_transport_preserves_early_reasoning_detail_into_replay_payload() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        for turn in 0..2 {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0; 8192];
            assert!(socket.read(&mut request).unwrap() > 0);
            let body = if turn == 0 {
                concat!(
                    "data: {\"id\":\"reasoning-id\",\"choices\":[{\"delta\":{\"reasoning_details\":[{\"type\":\"reasoning.encrypted\",\"id\":\"call_1\",\"data\":\"encrypted-signature\"}]}}]}\n\n",
                    "data: {\"id\":\"reasoning-id\",\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"function\":{\"name\":\"read\",\"arguments\":\"{\\\"path\\\":\\\"README.md\\\"}\"}}]}}]}\n\n",
                    "data: {\"id\":\"reasoning-id\",\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
                    "data: [DONE]\n\n"
                )
            } else {
                "data: {\"id\":\"second-id\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n"
            };
            write!(socket, "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\n\r\n{}", body.len(), body).unwrap();
        }
    });
    let registered = zedflow_ai::types::Model {
        id: "google/gemini-test".into(),
        name: "Gemini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: url,
        reasoning: true,
        max_tokens: 4096,
        context_window: 100_000,
        ..Default::default()
    };
    let options = zedflow_ai::types::StreamOptions {
        api_key: Some("test".into()),
        ..Default::default()
    };
    let first = zedflow_ai::api::openai_completions::stream_registered(
        &registered,
        &Default::default(),
        Some(&options),
    )
    .result()
    .await;
    let Some(zedflow_ai::types::AssistantContentBlock::ToolCall(call)) = first.content.first()
    else {
        panic!("tool call")
    };
    assert_eq!(
        serde_json::from_str::<Value>(call.thought_signature.as_deref().unwrap()).unwrap(),
        json!({"type":"reasoning.encrypted","id":"call_1","data":"encrypted-signature"})
    );

    let mut replay = zedflow_ai::types::Context::default();
    replay
        .messages
        .push(zedflow_ai::types::Message::Assistant(first));
    let captured = Arc::new(Mutex::new(None));
    let captured_hook = Arc::clone(&captured);
    let second = zedflow_ai::api::openai_completions::stream_registered(
        &registered,
        &replay,
        Some(&zedflow_ai::types::StreamOptions {
            api_key: Some("test".into()),
            on_payload: Some(Arc::new(move |payload, _| {
                *captured_hook.lock().unwrap() = Some(payload);
                Box::pin(async { Ok(None) })
            })),
            ..Default::default()
        }),
    );
    assert_eq!(
        second.result().await.stop_reason,
        zedflow_ai::types::StopReason::Stop
    );
    assert_eq!(
        captured.lock().unwrap().as_ref().unwrap()["messages"][0]["reasoning_details"],
        json!([{"type":"reasoning.encrypted","id":"call_1","data":"encrypted-signature"}])
    );
    server.join().unwrap();
}
