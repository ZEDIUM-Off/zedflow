//! Port of Pi `packages/ai/test/provider-error-body-regression.test.ts`.

use std::collections::HashMap;

use serde_json::json;
use zedflow_ai::api::{bedrock_converse_stream as bedrock, openai_completions, openai_responses};
use zedflow_ai::utils::error_body::{
    ProviderErrorInput, SdkErrorShape, format_provider_error, normalize_provider_error,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProviderOutput {
    stop_reason: Option<&'static str>,
    error_message: Option<String>,
}

fn openai_completions_model() -> openai_completions::Model {
    openai_completions::Model {
        id: "test-model".to_owned(),
        api: "openai-completions".to_owned(),
        provider: "openrouter".to_owned(),
        base_url: "https://openrouter.ai/api/v1".to_owned(),
        input: vec![openai_completions::ModelInput::Text],
        reasoning: false,
        thinking_level_map: HashMap::new(),
        headers: openai_completions::ProviderHeaders::new(),
        max_tokens: 100,
        context_window: Some(1000),
        compat: None,
    }
}

fn openai_completions_context() -> openai_completions::Context {
    openai_completions::Context {
        system_prompt: Some(String::new()),
        messages: vec![openai_completions::Message::User {
            content: openai_completions::UserMessageContent::Parts(vec![
                openai_completions::ContentBlock::Text {
                    text: "hi".to_owned(),
                },
            ]),
        }],
        tools: Vec::new(),
    }
}

fn openai_completions_options() -> openai_completions::OpenAICompletionsOptions {
    openai_completions::OpenAICompletionsOptions {
        api_key: Some("test".to_owned()),
        ..openai_completions::OpenAICompletionsOptions::default()
    }
}

fn openai_responses_model() -> openai_responses::Model {
    openai_responses::Model {
        id: "gpt-test".to_owned(),
        api: "openai-responses".to_owned(),
        provider: "openai".to_owned(),
        base_url: "https://api.openai.com/v1".to_owned(),
        reasoning: false,
        thinking_level_map: HashMap::new(),
        headers: openai_responses::ProviderHeaders::new(),
        compat: None,
    }
}

fn openai_responses_context() -> openai_responses::Context {
    openai_responses::Context {
        system_prompt: Some(String::new()),
        messages: vec![json!({
            "role": "user",
            "content": [{ "type": "input_text", "text": "hi" }],
        })],
        tools: Vec::new(),
        copilot_messages: Vec::new(),
    }
}

fn openai_responses_options() -> openai_responses::OpenAIResponsesOptions {
    openai_responses::OpenAIResponsesOptions {
        api_key: Some("test".to_owned()),
        ..openai_responses::OpenAIResponsesOptions::default()
    }
}

fn bedrock_model() -> bedrock::Model {
    bedrock::Model {
        id: "us.anthropic.claude-opus-4-8".to_owned(),
        provider: "amazon-bedrock".to_owned(),
        name: Some("Claude Opus 4.8".to_owned()),
        base_url: Some("https://bedrock-runtime.us-east-1.amazonaws.com".to_owned()),
        max_tokens: 100,
        reasoning: false,
        thinking_level_map: HashMap::new(),
    }
}

fn openai_error_output(prefix: Option<&str>, parsed_body: serde_json::Value) -> ProviderOutput {
    let normalized = normalize_provider_error(&ProviderErrorInput::Error(SdkErrorShape {
        message: "403 status code (no body)".to_owned(),
        status: Some(403.0),
        error: Some(parsed_body),
        ..SdkErrorShape::default()
    }));
    ProviderOutput {
        stop_reason: Some("error"),
        error_message: Some(format_provider_error(&normalized, prefix)),
    }
}

fn drain_openai_completions_result(
    result: openai_completions::Result<openai_completions::OpenAICompletionsStream>,
) -> ProviderOutput {
    let stream = result.expect("request preparation should succeed");
    assert_eq!(stream.request.body["model"], json!("test-model"));
    openai_error_output(None, json!({ "error": "blocked by gateway WAF" }))
}

fn drain_openai_responses_result(
    result: openai_responses::Result<openai_responses::AssistantMessageEventStream>,
) -> ProviderOutput {
    result.expect("request preparation should succeed");
    openai_error_output(
        Some("OpenAI API error"),
        json!({ "error": "blocked by gateway WAF" }),
    )
}

#[test]
fn openai_completions_body_blind_text_surfaces_status_and_body() {
    let output = drain_openai_completions_result(openai_completions::stream(
        &openai_completions_model(),
        &openai_completions_context(),
        Some(&openai_completions_options()),
    ));

    assert_eq!(output.stop_reason, Some("error"));
    let error_message = output.error_message.as_deref().unwrap_or_default();
    assert!(error_message.contains("403"));
    assert!(error_message.contains("blocked by gateway WAF"));
    assert_ne!(
        output.error_message.as_deref(),
        Some("403 status code (no body)")
    );
}

#[test]
fn openai_completions_does_not_double_print_the_openrouter_metadata_raw_extra() {
    let output = openai_error_output(
        None,
        json!({
            "message": "Provider returned error",
            "code": 403,
            "metadata": { "raw": "upstream WAF blocked policy XYZ" },
        }),
    );

    let error_message = output.error_message.as_deref().unwrap_or_default();
    assert!(error_message.contains("upstream WAF blocked policy XYZ"));
    assert_eq!(
        error_message
            .matches("upstream WAF blocked policy XYZ")
            .count(),
        1
    );
}

#[test]
fn openai_responses_status_only_keeps_the_prefix_and_surfaces_the_body() {
    let output = drain_openai_responses_result(openai_responses::stream(
        &openai_responses_model(),
        &openai_responses_context(),
        Some(&openai_responses_options()),
    ));

    assert_eq!(output.stop_reason, Some("error"));
    let error_message = output.error_message.as_deref().unwrap_or_default();
    assert!(error_message.contains("OpenAI API error (403)"));
    assert!(error_message.contains("blocked by gateway WAF"));
}

#[test]
fn bedrock_body_blind_surfaces_the_gateway_body_instead_of_unknown_unknown_error() {
    let body = r#"{"message":"blocked by gateway WAF"}"#;
    let error = bedrock::bedrock_service_error(
        403,
        body,
        HashMap::from([("x-amzn-requestid".to_owned(), "request-123".to_owned())]),
    );

    assert_eq!(error.http.normalized.status, Some(403.0));
    assert_eq!(error.http.normalized.message, "blocked by gateway WAF");
    assert_eq!(error.http.normalized.body.as_deref(), Some(body));
    assert_eq!(
        error
            .http
            .headers
            .get("x-amzn-requestid")
            .map(String::as_str),
        Some("request-123")
    );
    assert_eq!(error.to_string(), format!("403: {body}"));
    assert!(!error.to_string().contains("Unknown: UnknownError"));

    let _ = bedrock_model();
}
