//! Port of Pi `packages/ai/test/provider-error-body-regression.test.ts`.

use std::collections::HashMap;

use serde_json::json;
use zedflow_ai::api::{bedrock_converse_stream as bedrock, openai_completions, openai_responses};

const STREAM_BLOCKER: &str =
    "provider streaming/catch paths need fake transport injection for full error-body assertions.";

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

fn drain_openai_completions_result(
    result: openai_completions::Result<openai_completions::OpenAICompletionsStream>,
) -> ProviderOutput {
    match result {
        Ok(_) => panic!("{STREAM_BLOCKER}"),
        Err(error) => panic!("unexpected early OpenAI Completions error: {error}"),
    }
}

fn drain_openai_responses_result(
    result: openai_responses::Result<openai_responses::AssistantMessageEventStream>,
) -> ProviderOutput {
    match result {
        Ok(_) => panic!("{STREAM_BLOCKER}"),
        Err(error) => panic!("unexpected early OpenAI Responses error: {error}"),
    }
}

fn drain_bedrock_result(
    result: zedflow_core::error::Result<bedrock::AssistantMessageEventStream>,
) -> ProviderOutput {
    match result {
        Err(zedflow_core::error::Error::PortPlaceholder(_)) | Ok(_) => panic!("{STREAM_BLOCKER}"),
        Err(error) => panic!("unexpected early Bedrock error: {error}"),
    }
}

#[test]
#[ignore = "provider streaming/catch paths need fake transport injection"]
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
#[ignore = "provider streaming/catch paths need fake transport injection"]
fn openai_completions_does_not_double_print_the_openrouter_metadata_raw_extra() {
    let _parsed_body = json!({
        "message": "Provider returned error",
        "code": 403,
        "metadata": { "raw": "upstream WAF blocked policy XYZ" },
    });

    let output = drain_openai_completions_result(openai_completions::stream(
        &openai_completions_model(),
        &openai_completions_context(),
        Some(&openai_completions_options()),
    ));

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
#[ignore = "provider streaming/catch paths need fake transport injection"]
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
#[ignore = "provider streaming/catch paths need fake transport injection"]
fn bedrock_body_blind_surfaces_the_gateway_body_instead_of_unknown_unknown_error() {
    let _send_error = json!({
        "name": "UnknownError",
        "$metadata": { "httpStatusCode": 403 },
        "$response": { "statusCode": 403, "body": "{\"message\":\"blocked by gateway WAF\"}" },
    });

    let output = drain_bedrock_result(bedrock::stream_simple(
        &bedrock_model(),
        &bedrock::Context,
        Some(&bedrock::BedrockOptions::default()),
    ));

    assert_eq!(output.stop_reason, Some("error"));
    let error_message = output.error_message.as_deref().unwrap_or_default();
    assert!(error_message.contains("403"));
    assert!(error_message.contains("blocked by gateway WAF"));
    assert!(!error_message.contains("Unknown: UnknownError"));
}
