use serde_json::{Value, json};

const REQUEST_BLOCKER: &str = "OpenAI Chat Completions request construction is not ported yet; keep ignored until stream_simple exposes/captures the real request params and client options without live provider calls.";

#[derive(Debug, Clone)]
struct Model {
    id: &'static str,
    provider: &'static str,
    api: &'static str,
    max_tokens: u32,
    context_window: Option<u32>,
}

#[derive(Debug, Clone, Default)]
struct StreamOptions {
    api_key: Option<&'static str>,
    max_tokens: Option<u32>,
    reasoning: Option<&'static str>,
    headers: Value,
    session_id: Option<&'static str>,
    env: Value,
}

#[derive(Debug, Clone, PartialEq)]
struct CapturedOpenAIRequest {
    params: Value,
    client_options: Value,
}

fn openai_model() -> Model {
    Model {
        id: "gpt-4o-mini",
        provider: "openai",
        api: "openai-completions",
        max_tokens: 4096,
        context_window: None,
    }
}

fn cloudflare_workers_model() -> Model {
    Model {
        id: "workers-ai/@cf/moonshotai/kimi-k2.6",
        provider: "cloudflare-ai-gateway",
        api: "openai-completions",
        max_tokens: 4096,
        context_window: None,
    }
}

fn cloudflare_byok_model() -> Model {
    Model {
        id: "gpt-5.1",
        provider: "cloudflare-ai-gateway",
        api: "openai-completions",
        max_tokens: 4096,
        context_window: None,
    }
}

fn with_context_window(mut model: Model, context_window: u32, max_tokens: u32) -> Model {
    model.context_window = Some(context_window);
    model.max_tokens = max_tokens;
    model
}

fn user_message(content: impl Into<Value>) -> Value {
    json!({ "role": "user", "content": content.into(), "timestamp": 0 })
}

fn capture_openai_completions_request(
    model: Model,
    context: Value,
    options: StreamOptions,
) -> CapturedOpenAIRequest {
    let _ = (
        model.id,
        model.provider,
        model.api,
        model.max_tokens,
        model.context_window,
        context,
        options.api_key,
        options.max_tokens,
        options.reasoning,
        options.headers,
        options.session_id,
        options.env,
    );
    panic!("{REQUEST_BLOCKER}");
}

#[test]
#[ignore = "OpenAI request params capture is not ported"]
fn omits_tools_field_when_context_tools_is_an_empty_array() {
    let captured = capture_openai_completions_request(
        openai_model(),
        json!({
            "messages": [user_message("hi")],
            "tools": [],
        }),
        StreamOptions {
            api_key: Some("test"),
            ..StreamOptions::default()
        },
    );

    assert!(!captured.params.as_object().unwrap().contains_key("tools"));
}

#[test]
#[ignore = "OpenAI request params capture is not ported"]
fn omits_tools_field_when_context_tools_is_undefined() {
    let captured = capture_openai_completions_request(
        openai_model(),
        json!({ "messages": [user_message("hi")] }),
        StreamOptions {
            api_key: Some("test"),
            ..StreamOptions::default()
        },
    );

    assert!(!captured.params.as_object().unwrap().contains_key("tools"));
}

#[test]
#[ignore = "OpenAI request params capture is not ported"]
fn sends_default_max_tokens() {
    let model = openai_model();
    let captured = capture_openai_completions_request(
        model.clone(),
        json!({ "messages": [user_message("hi")] }),
        StreamOptions {
            api_key: Some("test"),
            ..StreamOptions::default()
        },
    );

    assert_eq!(captured.params.get("max_tokens"), None);
    assert_eq!(
        captured.params.get("max_completion_tokens"),
        Some(&json!(model.max_tokens))
    );
}

#[test]
#[ignore = "OpenAI request params capture is not ported"]
fn sends_explicit_max_tokens() {
    let captured = capture_openai_completions_request(
        openai_model(),
        json!({ "messages": [user_message("hi")] }),
        StreamOptions {
            api_key: Some("test"),
            max_tokens: Some(1234),
            ..StreamOptions::default()
        },
    );

    assert_eq!(captured.params.get("max_tokens"), None);
    assert_eq!(
        captured.params.get("max_completion_tokens"),
        Some(&json!(1234))
    );
}

#[test]
#[ignore = "OpenAI request params capture is not ported"]
fn clamps_default_max_tokens_to_remaining_context() {
    let model = with_context_window(openai_model(), 10000, 8000);
    let captured = capture_openai_completions_request(
        model,
        json!({ "messages": [user_message("x".repeat(8000))] }),
        StreamOptions {
            api_key: Some("test"),
            ..StreamOptions::default()
        },
    );

    assert_eq!(captured.params.get("max_tokens"), None);
    assert_eq!(
        captured.params.get("max_completion_tokens"),
        Some(&json!(3904))
    );
}

#[test]
#[ignore = "OpenAI request params capture is not ported"]
fn clamps_explicit_max_tokens_to_remaining_context() {
    let model = with_context_window(openai_model(), 10000, 8000);
    let captured = capture_openai_completions_request(
        model,
        json!({ "messages": [user_message("x".repeat(8000))] }),
        StreamOptions {
            api_key: Some("test"),
            max_tokens: Some(7000),
            ..StreamOptions::default()
        },
    );

    assert_eq!(captured.params.get("max_tokens"), None);
    assert_eq!(
        captured.params.get("max_completion_tokens"),
        Some(&json!(3904))
    );
}

#[test]
#[ignore = "OpenAI request params/client options capture is not ported"]
fn uses_conservative_openai_compatible_fields_for_cloudflare_ai_gateway_compat_models() {
    let captured = capture_openai_completions_request(
        cloudflare_workers_model(),
        json!({
            "systemPrompt": "You are helpful.",
            "messages": [user_message("hi")],
        }),
        StreamOptions {
            max_tokens: Some(1234),
            reasoning: Some("high"),
            env: json!({
                "CLOUDFLARE_API_KEY": "cf-token",
                "CLOUDFLARE_ACCOUNT_ID": "account-id",
                "CLOUDFLARE_GATEWAY_ID": "gateway-id",
            }),
            ..StreamOptions::default()
        },
    );

    assert_eq!(captured.params["messages"][0]["role"], json!("system"));
    assert_eq!(captured.params.get("max_tokens"), Some(&json!(1234)));
    assert_eq!(captured.params.get("max_completion_tokens"), None);
    assert_eq!(captured.params.get("reasoning_effort"), None);
    assert_eq!(captured.params.get("store"), None);
    assert_eq!(
        captured.client_options.get("baseURL"),
        Some(&json!(
            "https://gateway.ai.cloudflare.com/v1/account-id/gateway-id/compat"
        ))
    );
    assert_eq!(
        captured.client_options["defaultHeaders"].get("Authorization"),
        Some(&Value::Null)
    );
    assert_eq!(
        captured.client_options["defaultHeaders"].get("cf-aig-authorization"),
        Some(&json!("Bearer cf-token"))
    );
}

#[test]
#[ignore = "OpenAI request client options capture is not ported"]
fn resolves_cloudflare_ai_gateway_base_url_through_provider_auth() {
    let captured = capture_openai_completions_request(
        cloudflare_workers_model(),
        json!({ "messages": [user_message("hi")] }),
        StreamOptions {
            env: json!({
                "CLOUDFLARE_API_KEY": "cf-token",
                "CLOUDFLARE_ACCOUNT_ID": "account-id",
                "CLOUDFLARE_GATEWAY_ID": "gateway-id",
            }),
            ..StreamOptions::default()
        },
    );

    assert_eq!(
        captured.client_options.get("baseURL"),
        Some(&json!(
            "https://gateway.ai.cloudflare.com/v1/account-id/gateway-id/compat"
        ))
    );
}

#[test]
#[ignore = "OpenAI request client options capture is not ported"]
fn preserves_inline_upstream_authorization_for_cloudflare_ai_gateway_byok_requests() {
    let captured = capture_openai_completions_request(
        cloudflare_byok_model(),
        json!({ "messages": [user_message("hi")] }),
        StreamOptions {
            headers: json!({ "Authorization": "Bearer upstream-token" }),
            env: json!({
                "CLOUDFLARE_API_KEY": "cf-token",
                "CLOUDFLARE_ACCOUNT_ID": "account-id",
                "CLOUDFLARE_GATEWAY_ID": "gateway-id",
            }),
            ..StreamOptions::default()
        },
    );

    assert_eq!(
        captured.client_options["defaultHeaders"].get("Authorization"),
        Some(&json!("Bearer upstream-token"))
    );
    assert_eq!(
        captured.client_options["defaultHeaders"].get("cf-aig-authorization"),
        Some(&json!("Bearer cf-token"))
    );
}

#[test]
#[ignore = "OpenAI request client options capture is not ported"]
fn sends_session_affinity_headers_for_workers_ai_through_cloudflare_ai_gateway() {
    let captured = capture_openai_completions_request(
        cloudflare_workers_model(),
        json!({ "messages": [user_message("hi")] }),
        StreamOptions {
            session_id: Some("session-1"),
            env: json!({
                "CLOUDFLARE_API_KEY": "cf-token",
                "CLOUDFLARE_ACCOUNT_ID": "account-id",
                "CLOUDFLARE_GATEWAY_ID": "gateway-id",
            }),
            ..StreamOptions::default()
        },
    );

    assert_eq!(
        captured.client_options["defaultHeaders"].get("session_id"),
        Some(&json!("session-1"))
    );
    assert_eq!(
        captured.client_options["defaultHeaders"].get("x-client-request-id"),
        Some(&json!("session-1"))
    );
    assert_eq!(
        captured.client_options["defaultHeaders"].get("x-session-affinity"),
        Some(&json!("session-1"))
    );
}

#[test]
#[ignore = "OpenAI request params capture is not ported"]
fn still_emits_tools_empty_array_for_anthropic_litellm_proxy_when_conversation_has_tool_history() {
    let captured = capture_openai_completions_request(
        openai_model(),
        json!({
            "messages": [
                user_message("use the tool"),
                {
                    "role": "assistant",
                    "content": [{
                        "type": "toolCall",
                        "id": "t1",
                        "name": "noop",
                        "arguments": {},
                    }],
                    "stopReason": "toolUse",
                    "usage": {
                        "input": 0,
                        "output": 0,
                        "cacheRead": 0,
                        "cacheWrite": 0,
                        "totalTokens": 0,
                        "cost": { "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0 },
                    },
                    "api": "openai-completions",
                    "provider": "openai",
                    "model": "gpt-4o-mini",
                    "timestamp": 0,
                },
                {
                    "role": "toolResult",
                    "toolCallId": "t1",
                    "toolName": "noop",
                    "content": [{ "type": "text", "text": "done" }],
                    "isError": false,
                    "timestamp": 0,
                },
            ],
            "tools": [],
        }),
        StreamOptions {
            api_key: Some("test"),
            ..StreamOptions::default()
        },
    );

    let tools = captured.params.get("tools").and_then(Value::as_array);
    assert!(matches!(tools, Some(values) if values.is_empty()));
}
