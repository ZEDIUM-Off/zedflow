use std::sync::{Arc, Mutex};

use zedflow_ai::compat::{ApiProvider, complete, register_api_provider, reset_api_providers};
use zedflow_ai::types::{
    AssistantContentBlock, AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream,
    AssistantMessageRole, Context, DoneStopReason, Model, StopReason, StreamOptions, TextContent,
    TextContentType, Usage,
};

fn output(model: &Model) -> AssistantMessage {
    AssistantMessage {
        role: AssistantMessageRole::Assistant,
        content: vec![AssistantContentBlock::Text(TextContent {
            content_type: TextContentType::Text,
            text: "ok".into(),
            text_signature: None,
        })],
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage::default(),
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp: 0,
    }
}

fn done_stream(model: &Model) -> AssistantMessageEventStream {
    let message = output(model);
    let stream = AssistantMessageEventStream::new();
    stream.push(AssistantMessageEvent::Done {
        reason: DoneStopReason::Stop,
        message,
    });
    stream
}

#[tokio::test]
async fn dispatches_unknown_provider_with_request_api_key() {
    reset_api_providers().expect("reset providers");

    let captured = Arc::new(Mutex::new(None));
    let captured_stream = Arc::clone(&captured);
    register_api_provider(
        ApiProvider {
            api: "openai-responses".into(),
            stream: Arc::new(move |model, _, options| {
                *captured_stream.lock().unwrap() = options.and_then(|value| value.api_key);
                Ok(done_stream(model))
            }),
            stream_simple: Arc::new(|model, _, _| Ok(done_stream(model))),
        },
        None,
    );

    let model = Model {
        id: "test-model".into(),
        name: "Test Model".into(),
        api: "openai-responses".into(),
        provider: "custom-openai".into(),
        base_url: "https://example.test/v1".into(),
        context_window: 128_000,
        max_tokens: 4_096,
        ..Model::default()
    };
    let message = complete(
        &model,
        &Context::default(),
        Some(StreamOptions {
            api_key: Some("request-key".into()),
            ..StreamOptions::default()
        }),
    )
    .await
    .expect("custom provider should complete");

    assert_eq!(message.content.len(), 1);
    assert_eq!(captured.lock().unwrap().as_deref(), Some("request-key"));
    reset_api_providers().expect("restore providers");
}
