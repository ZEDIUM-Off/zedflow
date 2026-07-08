//! Parity tests for Pi's `test/openai-completions-cache-control-format.test.ts`.
//!
//! OpenAI Completions request-payload construction and
//! provider payload capture are not ported yet. Keep these ignored until the
//! real `stream`/`buildParams` path exists in `zedflow_ai::api::openai_completions`.

use std::collections::HashMap;

use serde_json::{Value, json};
use zedflow_ai::api::openai_completions::{
    CacheControlFormat, CacheRetention, Context, Message, Model, ModelInput,
    OpenAICompletionsCompat, OpenAICompletionsOptions, ProviderHeaders, Tool, UserMessageContent,
};

const BLOCKER: &str = "OpenAI Completions buildParams/on_payload capture is not ported; stream is still a provider I/O placeholder, so cache-control payload markers cannot be observed locally.";

fn custom_qwen_model() -> Model {
    Model {
        id: "custom-qwen".to_owned(),
        api: "openai-completions".to_owned(),
        provider: "openrouter".to_owned(),
        base_url: "https://example.com/v1".to_owned(),
        input: vec![ModelInput::Text],
        reasoning: true,
        thinking_level_map: HashMap::new(),
        headers: ProviderHeaders::new(),
        compat: Some(OpenAICompletionsCompat {
            cache_control_format: Some(CacheControlFormat::Anthropic),
            ..OpenAICompletionsCompat::default()
        }),
    }
}

fn openrouter_anthropic_model() -> Model {
    Model {
        id: "anthropic/claude-sonnet-4".to_owned(),
        api: "openai-completions".to_owned(),
        provider: "openrouter".to_owned(),
        base_url: "https://openrouter.ai/api/v1".to_owned(),
        input: vec![ModelInput::Text],
        reasoning: true,
        thinking_level_map: HashMap::new(),
        headers: ProviderHeaders::new(),
        compat: None,
    }
}

fn capture_payload(_model: Model, options: Option<OpenAICompletionsOptions>) -> Value {
    let _context = Context {
        system_prompt: Some("System prompt".to_owned()),
        messages: vec![Message::User {
            content: UserMessageContent::Text("Hello".to_owned()),
        }],
        tools: vec![Tool {
            name: "read".to_owned(),
            description: "Read a file".to_owned(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }),
        }],
    };
    let _options = OpenAICompletionsOptions {
        api_key: Some("test-key".to_owned()),
        ..options.unwrap_or_default()
    };

    panic!("{BLOCKER}");
}

fn instruction_message(params: &Value) -> Option<&Value> {
    params.get("messages")?.as_array()?.iter().find(|message| {
        matches!(
            message.get("role").and_then(Value::as_str),
            Some("system" | "developer")
        )
    })
}

fn expect_anthropic_cache_markers(params: &Value) {
    let instruction = instruction_message(params).expect("instruction message should exist");
    let instruction_content = instruction
        .get("content")
        .and_then(Value::as_array)
        .expect("instruction content should be structured parts");
    assert_eq!(
        instruction_content
            .first()
            .and_then(|part| part.get("cache_control")),
        Some(&json!({ "type": "ephemeral" }))
    );

    let tools = params
        .get("tools")
        .and_then(Value::as_array)
        .expect("tools should exist");
    assert_eq!(tools.len(), 1);
    assert_eq!(
        tools.first().and_then(|tool| tool.get("cache_control")),
        Some(&json!({ "type": "ephemeral" }))
    );

    let last_message = params
        .get("messages")
        .and_then(Value::as_array)
        .and_then(|messages| messages.last())
        .expect("last message should exist");
    assert_eq!(last_message.get("role"), Some(&json!("user")));
    let last_content = last_message
        .get("content")
        .and_then(Value::as_array)
        .expect("last user content should be structured parts");
    assert_eq!(
        last_content
            .first()
            .and_then(|part| part.get("cache_control")),
        Some(&json!({ "type": "ephemeral" }))
    );
}

#[test]
#[ignore = "OpenAI Completions payload construction/on_payload capture is not ported"]
fn applies_anthropic_style_cache_markers_when_model_compat_enables_them() {
    let params = capture_payload(custom_qwen_model(), None);

    expect_anthropic_cache_markers(&params);
}

#[test]
#[ignore = "OpenAI Completions payload construction/on_payload capture is not ported"]
fn preserves_anthropic_style_cache_markers_for_openrouter_anthropic_models() {
    let params = capture_payload(openrouter_anthropic_model(), None);

    expect_anthropic_cache_markers(&params);
}

#[test]
#[ignore = "OpenAI Completions payload construction/on_payload capture is not ported"]
fn omits_anthropic_style_cache_markers_when_cache_retention_is_none() {
    let params = capture_payload(
        custom_qwen_model(),
        Some(OpenAICompletionsOptions {
            cache_retention: Some(CacheRetention::None),
            ..OpenAICompletionsOptions::default()
        }),
    );
    let instruction_content =
        instruction_message(&params).and_then(|message| message.get("content"));

    assert!(!instruction_content.is_some_and(Value::is_array));
    assert!(params.pointer("/tools/0/cache_control").is_none());

    let last_message = params
        .get("messages")
        .and_then(Value::as_array)
        .and_then(|messages| messages.last())
        .expect("last message should exist");
    assert!(last_message.get("content").is_some_and(Value::is_string));
}
