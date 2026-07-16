use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::json;
use zedflow_ai::api::openai_completions::{
    AssistantMessage, ChatCompletionContentPart, ChatCompletionMessage, ContentBlock, Context,
    Message, Model, ModelInput, StopReason, ToolCall, ToolResultMessage, UserChatContent,
    convert_messages, get_compat,
};

fn model() -> Model {
    Model {
        id: "gpt-4o-mini".into(),
        api: "openai-completions".into(),
        provider: "openai".into(),
        base_url: "https://api.openai.com/v1".into(),
        input: vec![ModelInput::Text, ModelInput::Image],
        reasoning: false,
        thinking_level_map: HashMap::new(),
        headers: HashMap::new(),
        max_tokens: 100,
        context_window: None,
        compat: None,
    }
}

fn call(id: &str) -> ContentBlock {
    ContentBlock::ToolCall(ToolCall {
        id: id.into(),
        name: "read".into(),
        arguments: json!({}),
        thought_signature: None,
    })
}
fn result(id: &str, image: bool) -> Message {
    let mut content = vec![ContentBlock::Text {
        text: if image {
            "Read image".into()
        } else {
            String::new()
        },
    }];
    if image {
        content.push(ContentBlock::Image {
            data: "ZmFrZQ==".into(),
            mime_type: "image/png".into(),
        });
    }
    Message::ToolResult(ToolResultMessage {
        tool_call_id: id.into(),
        tool_name: Some("read".into()),
        content,
    })
}

#[test]
fn consecutive_tool_result_images_are_batched_after_tool_messages() {
    let model = model();
    let context = Context {
        messages: vec![
            Message::Assistant(AssistantMessage {
                api: model.api.clone(),
                provider: model.provider.clone(),
                model: model.id.clone(),
                content: vec![call("one"), call("two")],
                stop_reason: StopReason::ToolUse,
            }),
            result("one", true),
            result("two", true),
        ],
        ..Default::default()
    };
    let messages = convert_messages(&model, &context, &get_compat(&model));
    assert_eq!(
        messages
            .iter()
            .map(ChatCompletionMessage::role)
            .collect::<Vec<_>>(),
        ["assistant", "tool", "tool", "user"]
    );
    let ChatCompletionMessage::User {
        content: UserChatContent::Parts(parts),
    } = messages.last().unwrap()
    else {
        panic!("image user")
    };
    assert_eq!(
        parts
            .iter()
            .filter(|part| matches!(part, ChatCompletionContentPart::ImageUrl { .. }))
            .count(),
        2
    );
}

#[test]
fn empty_tool_result_uses_no_output_placeholder() {
    let model = model();
    let context = Context {
        messages: vec![
            Message::Assistant(AssistantMessage {
                api: model.api.clone(),
                provider: model.provider.clone(),
                model: model.id.clone(),
                content: vec![call("one")],
                stop_reason: StopReason::ToolUse,
            }),
            result("one", false),
        ],
        ..Default::default()
    };
    let messages = convert_messages(&model, &context, &get_compat(&model));
    let content = messages
        .iter()
        .find_map(|message| match message {
            ChatCompletionMessage::Tool { content, .. } => Some(content),
            _ => None,
        })
        .unwrap();
    assert_eq!(content, "(no tool output)");
}

#[tokio::test]
async fn registered_transport_sends_batched_tool_result_images_on_the_wire() {
    use zedflow_ai::types::{
        AssistantContentBlock, AssistantMessageRole, ImageContent, ImageContentType, TextContent,
        TextContentType, ToolCall as CanonicalToolCall, ToolCallType, ToolResultContentBlock,
        ToolResultMessage as CanonicalToolResultMessage, ToolResultMessageRole,
    };

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = [0; 8192];
        assert!(socket.read(&mut request).unwrap() > 0);
        let body = "data: {\"id\":\"image-id\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n";
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
        input: vec![
            zedflow_ai::types::ModelInput::Text,
            zedflow_ai::types::ModelInput::Image,
        ],
        max_tokens: 100,
        context_window: 4096,
        ..Default::default()
    };
    let assistant = zedflow_ai::types::AssistantMessage {
        role: AssistantMessageRole::Assistant,
        content: ["one", "two"]
            .into_iter()
            .map(|id| {
                AssistantContentBlock::ToolCall(CanonicalToolCall {
                    content_type: ToolCallType::ToolCall,
                    id: id.into(),
                    name: "read".into(),
                    arguments: HashMap::new(),
                    thought_signature: None,
                })
            })
            .collect(),
        api: "openai-completions".into(),
        provider: "openai".into(),
        model: "gpt-4o-mini".into(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Default::default(),
        stop_reason: zedflow_ai::types::StopReason::ToolUse,
        error_message: None,
        timestamp: 0,
    };
    let tool_result = |id: &str| {
        zedflow_ai::types::Message::ToolResult(CanonicalToolResultMessage {
            role: ToolResultMessageRole::ToolResult,
            tool_call_id: id.into(),
            tool_name: "read".into(),
            content: vec![
                ToolResultContentBlock::Text(TextContent {
                    content_type: TextContentType::Text,
                    text: "Read image".into(),
                    text_signature: None,
                }),
                ToolResultContentBlock::Image(ImageContent {
                    content_type: ImageContentType::Image,
                    data: "ZmFrZQ==".into(),
                    mime_type: "image/png".into(),
                }),
            ],
            details: None,
            is_error: false,
            timestamp: 0,
        })
    };
    let mut context = zedflow_ai::types::Context::default();
    context
        .messages
        .push(zedflow_ai::types::Message::Assistant(assistant));
    context.messages.push(tool_result("one"));
    context.messages.push(tool_result("two"));

    let captured = Arc::new(Mutex::new(None));
    let captured_hook = Arc::clone(&captured);
    let stream = zedflow_ai::api::openai_completions::stream_registered(
        &registered,
        &context,
        Some(&zedflow_ai::types::StreamOptions {
            api_key: Some("test".into()),
            on_payload: Some(Arc::new(move |payload, _| {
                *captured_hook.lock().unwrap() = Some(payload);
                Box::pin(async { Ok(None) })
            })),
            ..Default::default()
        }),
    );
    let terminal = stream.result().await;
    assert_eq!(terminal.stop_reason, zedflow_ai::types::StopReason::Stop);
    assert_eq!(terminal.response_id.as_deref(), Some("image-id"));

    let payload = captured.lock().unwrap().clone().unwrap();
    assert_eq!(
        payload["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|message| message["role"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["assistant", "tool", "tool", "user"]
    );
    assert_eq!(payload["messages"][1]["content"], "Read image");
    assert_eq!(payload["messages"][2]["content"], "Read image");
    assert_eq!(
        payload["messages"][3]["content"],
        json!([
            {"type":"text","text":"Attached image(s) from tool result:"},
            {"type":"image_url","image_url":{"url":"data:image/png;base64,ZmFrZQ=="}},
            {"type":"image_url","image_url":{"url":"data:image/png;base64,ZmFrZQ=="}}
        ])
    );
    server.join().unwrap();
}
