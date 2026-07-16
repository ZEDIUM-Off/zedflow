//! Parity tests for Pi `openai-completions-empty-tools.test.ts`.

use std::collections::HashMap;

use serde_json::{Value, json};
use zedflow_ai::api::openai_completions::{
    AssistantMessage, ContentBlock, Context, Message, Model, ModelInput, OpenAICompletionsOptions,
    ProviderHeaders, StopReason, ToolCall, ToolResultMessage, UserMessageContent, build_request,
};

fn openai_model() -> Model {
    Model {
        id: "gpt-4o-mini".to_owned(),
        provider: "openai".to_owned(),
        api: "openai-completions".to_owned(),
        base_url: "https://api.openai.com/v1".to_owned(),
        input: vec![ModelInput::Text],
        reasoning: false,
        thinking_level_map: HashMap::new(),
        headers: ProviderHeaders::new(),
        max_tokens: 4096,
        context_window: None,
        compat: None,
    }
}

fn cloudflare_workers_model() -> Model {
    Model {
        id: "workers-ai/@cf/moonshotai/kimi-k2.6".to_owned(),
        provider: "cloudflare-ai-gateway".to_owned(),
        api: "openai-completions".to_owned(),
        base_url: "https://gateway.ai.cloudflare.com/v1".to_owned(),
        input: vec![ModelInput::Text],
        reasoning: true,
        thinking_level_map: HashMap::new(),
        headers: ProviderHeaders::new(),
        max_tokens: 4096,
        context_window: None,
        compat: None,
    }
}

fn cloudflare_byok_model() -> Model {
    Model {
        id: "gpt-5.1".to_owned(),
        ..cloudflare_workers_model()
    }
}

fn with_context_window(mut model: Model, context_window: u32, max_tokens: u32) -> Model {
    model.context_window = Some(context_window);
    model.max_tokens = max_tokens;
    model
}

fn user_context(content: impl Into<String>) -> Context {
    Context {
        messages: vec![Message::User {
            content: UserMessageContent::Text(content.into()),
        }],
        ..Context::default()
    }
}

fn capture(model: &Model, context: &Context, options: OpenAICompletionsOptions) -> Value {
    build_request(model, context, Some(&options))
        .expect("request should build")
        .body
}

fn capture_request(
    model: &Model,
    context: &Context,
    options: OpenAICompletionsOptions,
) -> zedflow_ai::api::openai_completions::OpenAICompletionsRequest {
    build_request(model, context, Some(&options)).expect("request should build")
}

#[test]
fn omits_tools_field_when_context_tools_is_an_empty_array() {
    let params = capture(
        &openai_model(),
        &Context {
            messages: user_context("hi").messages,
            tools: vec![],
            ..Context::default()
        },
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert!(!params.as_object().unwrap().contains_key("tools"));
}

#[test]
fn omits_tools_field_when_context_tools_is_undefined() {
    let params = capture(
        &openai_model(),
        &user_context("hi"),
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert!(!params.as_object().unwrap().contains_key("tools"));
}

#[test]
fn sends_default_max_tokens() {
    let model = openai_model();
    let params = capture(
        &model,
        &user_context("hi"),
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(params.get("max_tokens"), None);
    assert_eq!(params.get("max_completion_tokens"), Some(&json!(4096)));
}

#[test]
fn sends_explicit_max_tokens() {
    let params = capture(
        &openai_model(),
        &user_context("hi"),
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            max_tokens: Some(1234),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(params.get("max_tokens"), None);
    assert_eq!(params.get("max_completion_tokens"), Some(&json!(1234)));
}

#[test]
fn clamps_default_max_tokens_to_remaining_context() {
    let model = with_context_window(openai_model(), 10_000, 8000);
    let params = capture(
        &model,
        &user_context("x".repeat(8000)),
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(params.get("max_tokens"), None);
    assert_eq!(params.get("max_completion_tokens"), Some(&json!(3904)));
}

#[test]
fn clamps_explicit_max_tokens_to_remaining_context() {
    let model = with_context_window(openai_model(), 10_000, 8000);
    let params = capture(
        &model,
        &user_context("x".repeat(8000)),
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            max_tokens: Some(7000),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(params.get("max_tokens"), None);
    assert_eq!(params.get("max_completion_tokens"), Some(&json!(3904)));
}

#[test]
fn uses_conservative_openai_compatible_fields_for_cloudflare_ai_gateway_compat_models() {
    let request = capture_request(
        &cloudflare_workers_model(),
        &Context {
            system_prompt: Some("You are helpful.".to_owned()),
            messages: user_context("hi").messages,
            ..Context::default()
        },
        OpenAICompletionsOptions {
            max_tokens: Some(1234),
            reasoning_effort: Some(zedflow_ai::api::openai_completions::ReasoningEffort::High),
            env: HashMap::from([
                ("CLOUDFLARE_API_KEY".to_owned(), "cf-token".to_owned()),
                ("CLOUDFLARE_ACCOUNT_ID".to_owned(), "account-id".to_owned()),
                ("CLOUDFLARE_GATEWAY_ID".to_owned(), "gateway-id".to_owned()),
            ]),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(request.body["messages"][0]["role"], json!("system"));
    assert_eq!(request.body.get("max_tokens"), Some(&json!(1234)));
    assert_eq!(request.body.get("max_completion_tokens"), None);
    assert_eq!(request.body.get("reasoning_effort"), None);
    assert_eq!(request.body.get("store"), None);
    assert_eq!(
        request.client_options.get("baseURL"),
        Some(&json!(
            "https://gateway.ai.cloudflare.com/v1/account-id/gateway-id/compat"
        ))
    );
    assert_eq!(
        request.client_options["defaultHeaders"].get("Authorization"),
        Some(&Value::Null)
    );
    assert_eq!(
        request.client_options["defaultHeaders"].get("cf-aig-authorization"),
        Some(&json!("Bearer cf-token"))
    );
}

#[test]
fn resolves_cloudflare_ai_gateway_base_url_through_provider_auth() {
    let request = capture_request(
        &cloudflare_workers_model(),
        &user_context("hi"),
        OpenAICompletionsOptions {
            env: HashMap::from([
                ("CLOUDFLARE_API_KEY".to_owned(), "cf-token".to_owned()),
                ("CLOUDFLARE_ACCOUNT_ID".to_owned(), "account-id".to_owned()),
                ("CLOUDFLARE_GATEWAY_ID".to_owned(), "gateway-id".to_owned()),
            ]),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(
        request.client_options.get("baseURL"),
        Some(&json!(
            "https://gateway.ai.cloudflare.com/v1/account-id/gateway-id/compat"
        ))
    );
}

#[test]
fn preserves_inline_upstream_authorization_for_cloudflare_ai_gateway_byok_requests() {
    let request = capture_request(
        &cloudflare_byok_model(),
        &user_context("hi"),
        OpenAICompletionsOptions {
            headers: HashMap::from([(
                "Authorization".to_owned(),
                "Bearer upstream-token".to_owned(),
            )]),
            env: HashMap::from([
                ("CLOUDFLARE_API_KEY".to_owned(), "cf-token".to_owned()),
                ("CLOUDFLARE_ACCOUNT_ID".to_owned(), "account-id".to_owned()),
                ("CLOUDFLARE_GATEWAY_ID".to_owned(), "gateway-id".to_owned()),
            ]),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(
        request.client_options["defaultHeaders"].get("Authorization"),
        Some(&json!("Bearer upstream-token"))
    );
    assert_eq!(
        request.client_options["defaultHeaders"].get("cf-aig-authorization"),
        Some(&json!("Bearer cf-token"))
    );
}

#[test]
fn sends_session_affinity_headers_for_workers_ai_through_cloudflare_ai_gateway() {
    let request = capture_request(
        &cloudflare_workers_model(),
        &user_context("hi"),
        OpenAICompletionsOptions {
            session_id: Some("session-1".to_owned()),
            env: HashMap::from([
                ("CLOUDFLARE_API_KEY".to_owned(), "cf-token".to_owned()),
                ("CLOUDFLARE_ACCOUNT_ID".to_owned(), "account-id".to_owned()),
                ("CLOUDFLARE_GATEWAY_ID".to_owned(), "gateway-id".to_owned()),
            ]),
            ..OpenAICompletionsOptions::default()
        },
    );

    assert_eq!(
        request.client_options["defaultHeaders"].get("session_id"),
        Some(&json!("session-1"))
    );
    assert_eq!(
        request.client_options["defaultHeaders"].get("x-client-request-id"),
        Some(&json!("session-1"))
    );
    assert_eq!(
        request.client_options["defaultHeaders"].get("x-session-affinity"),
        Some(&json!("session-1"))
    );
}

#[test]
fn still_emits_tools_empty_array_for_anthropic_litellm_proxy_when_conversation_has_tool_history() {
    let context = Context {
        messages: vec![
            Message::User {
                content: UserMessageContent::Text("use the tool".to_owned()),
            },
            Message::Assistant(AssistantMessage {
                api: "openai-completions".to_owned(),
                provider: "openai".to_owned(),
                model: "gpt-4o-mini".to_owned(),
                content: vec![ContentBlock::ToolCall(ToolCall {
                    id: "t1".to_owned(),
                    name: "noop".to_owned(),
                    arguments: json!({}),
                    thought_signature: None,
                })],
                stop_reason: StopReason::ToolUse,
            }),
            Message::ToolResult(ToolResultMessage {
                tool_call_id: "t1".to_owned(),
                tool_name: Some("noop".to_owned()),
                content: vec![ContentBlock::Text {
                    text: "done".to_owned(),
                }],
            }),
        ],
        tools: vec![],
        ..Context::default()
    };
    let params = capture(
        &openai_model(),
        &context,
        OpenAICompletionsOptions {
            api_key: Some("test".to_owned()),
            ..OpenAICompletionsOptions::default()
        },
    );

    let tools = params.get("tools").and_then(Value::as_array);
    assert!(matches!(tools, Some(values) if values.is_empty()));
}
