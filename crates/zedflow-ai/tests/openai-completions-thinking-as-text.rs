use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

use zedflow_ai::api::openai_completions::{
    AssistantChatContent, AssistantMessage, ChatCompletionMessage, ContentBlock, Context, Message,
    Model, ModelInput, OpenAICompletionsCompat, StopReason, convert_messages, get_compat,
};

fn model() -> Model {
    Model {
        id: "repro".into(),
        api: "openai-completions".into(),
        provider: "repro".into(),
        base_url: "http://127.0.0.1:1".into(),
        input: vec![ModelInput::Text],
        reasoning: true,
        thinking_level_map: HashMap::new(),
        headers: HashMap::new(),
        max_tokens: 100,
        context_window: None,
        compat: Some(OpenAICompletionsCompat {
            requires_thinking_as_text: Some(true),
            ..Default::default()
        }),
    }
}

fn replay(blocks: Vec<ContentBlock>) -> ChatCompletionMessage {
    let model = model();
    let context = Context {
        messages: vec![Message::Assistant(AssistantMessage {
            api: model.api.clone(),
            provider: model.provider.clone(),
            model: model.id.clone(),
            content: blocks,
            stop_reason: StopReason::Stop,
        })],
        ..Default::default()
    };
    convert_messages(&model, &context, &get_compat(&model)).remove(0)
}

#[test]
fn thinking_plus_text_replays_as_ordered_text_parts() {
    let message = replay(vec![
        ContentBlock::Thinking {
            thinking: "internal reasoning".into(),
            thinking_signature: None,
            redacted: false,
        },
        ContentBlock::Text {
            text: "visible answer".into(),
        },
    ]);
    let ChatCompletionMessage::Assistant {
        content: Some(AssistantChatContent::Parts(parts)),
        ..
    } = message
    else {
        panic!("parts")
    };
    assert_eq!(
        parts
            .iter()
            .map(|part| part.text.as_str())
            .collect::<Vec<_>>(),
        ["internal reasoning", "visible answer"]
    );
}

#[test]
fn thinking_only_replays_as_a_text_part() {
    let message = replay(vec![ContentBlock::Thinking {
        thinking: "internal reasoning".into(),
        thinking_signature: None,
        redacted: false,
    }]);
    let ChatCompletionMessage::Assistant {
        content: Some(AssistantChatContent::Parts(parts)),
        ..
    } = message
    else {
        panic!("parts")
    };
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].text, "internal reasoning");
}

#[tokio::test]
async fn registered_transport_serializes_thinking_as_text_parts() {
    use zedflow_ai::types::{
        AssistantContentBlock, AssistantMessageRole, ModelCompat,
        OpenAICompletionsCompat as CanonicalCompat, StreamOptions, TextContent, TextContentType,
        ThinkingContent, ThinkingContentType,
    };

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = [0; 8192];
        assert!(socket.read(&mut request).unwrap() > 0);
        let body = "data: {\"id\":\"thinking-id\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";
        write!(
            socket,
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
    });
    let registered = zedflow_ai::types::Model {
        id: "repro".into(),
        name: "Repro".into(),
        api: "openai-completions".into(),
        provider: "repro".into(),
        base_url: url,
        reasoning: true,
        max_tokens: 100,
        context_window: 4096,
        compat: Some(ModelCompat::OpenAICompletions(CanonicalCompat {
            requires_thinking_as_text: Some(true),
            ..Default::default()
        })),
        ..Default::default()
    };
    let assistant = zedflow_ai::types::AssistantMessage {
        role: AssistantMessageRole::Assistant,
        content: vec![
            AssistantContentBlock::Thinking(ThinkingContent {
                content_type: ThinkingContentType::Thinking,
                thinking: "internal reasoning".into(),
                thinking_signature: None,
                redacted: Some(false),
            }),
            AssistantContentBlock::Text(TextContent {
                content_type: TextContentType::Text,
                text: "visible answer".into(),
                text_signature: None,
            }),
        ],
        api: "openai-completions".into(),
        provider: "repro".into(),
        model: "repro".into(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Default::default(),
        stop_reason: zedflow_ai::types::StopReason::Stop,
        error_message: None,
        timestamp: 0,
    };
    let mut context = zedflow_ai::types::Context::default();
    context
        .messages
        .push(zedflow_ai::types::Message::Assistant(assistant));
    let captured = Arc::new(Mutex::new(None));
    let captured_hook = Arc::clone(&captured);
    let stream = zedflow_ai::api::openai_completions::stream_registered(
        &registered,
        &context,
        Some(&StreamOptions {
            api_key: Some("test".into()),
            on_payload: Some(Arc::new(move |payload, _| {
                *captured_hook.lock().unwrap() = Some(payload);
                Box::pin(async { Ok(None) })
            })),
            ..Default::default()
        }),
    );
    assert_eq!(
        stream.result().await.stop_reason,
        zedflow_ai::types::StopReason::Stop
    );
    let payload = captured.lock().unwrap().clone().unwrap();
    assert_eq!(
        payload["messages"][0]["content"],
        serde_json::json!([
            {"type":"text","text":"internal reasoning"},
            {"type":"text","text":"visible answer"}
        ])
    );
    server.join().unwrap();
}
