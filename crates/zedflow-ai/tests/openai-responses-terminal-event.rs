//! Port of Pi `packages/ai/test/openai-responses-terminal-event.test.ts`.

use std::collections::HashMap;

use zedflow_ai::api::openai_responses::{self, Context, Model, OpenAIResponsesOptions};

fn model() -> Model {
    Model {
        id: "gpt-5-mini".to_owned(),
        api: "openai-responses".to_owned(),
        provider: "openai".to_owned(),
        base_url: "https://api.openai.com/v1".to_owned(),
        reasoning: true,
        thinking_level_map: HashMap::new(),
        headers: HashMap::new(),
        compat: None,
    }
}

fn context() -> Context {
    Context {
        system_prompt: Some(String::new()),
        messages: Vec::new(),
        tools: Vec::new(),
        copilot_messages: Vec::new(),
    }
}

#[test]
fn prepares_openai_responses_request_before_stream_processing() {
    let options = OpenAIResponsesOptions {
        api_key: Some("test".to_owned()),
        max_retries: Some(0),
        ..OpenAIResponsesOptions::default()
    };

    let stream = openai_responses::stream(&model(), &context(), Some(&options))
        .expect("request should be prepared");

    assert_eq!(
        stream.request.body.get("stream"),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(
        stream.request.body.get("store"),
        Some(&serde_json::Value::Bool(false))
    );
    assert_eq!(stream.request.max_retries, 0);
}
