//! Port of Pi `packages/ai/test/tool-call-id-normalization.test.ts`.
//!
//! The source test is live-provider parity for GitHub Copilot/OpenRouter/OpenAI Codex handoff.
//! Rust `compat::get_model`, OAuth token resolution, builtin provider dispatch, and live provider
//! stream implementations are still documented `request-capture blocker`s, so the parity cases stay
//! ignored until those seams are ported.

use serde_json::{Value, json};
use zedflow_ai::compat;

const BLOCKER: &str = "live tool-call ID normalization handoff needs compat::get_model, OAuth resolveApiKey, builtin provider dispatch, and live provider completeSimple/stream implementations";
const FAILING_TOOL_CALL_ID: &str = "call_pAYbIr76hXIjncD9UE4eGfnS|t5nnb2qYMFWGSsr13fhCd1CaCu3t3qONEPuOudu4HSVEtA8YJSL6FAZUxvoOoD792VIJWl91g87EdqsCWp9krVsdBysQoDaf9lMCLb8BS4EYi4gQd5kBQBYLlgD71PYwvf+TbMD9J9/5OMD42oxSRj8H+vRf78/l2Xla33LWz4nOgsddBlbvabICRs8GHt5C9PK5keFtzyi3lsyVKNlfduK3iphsZqs4MLv4zyGJnvZo/+QzShyk5xnMSQX/f98+aEoNflEApCdEOXipipgeiNWnpFSHbcwmMkZoJhURNu+JEz3xCh1mrXeYoN5o+trLL3IXJacSsLYXDrYTipZZbJFRPAucgbnjYBC+/ZzJOfkwCs+Gkw7EoZR7ZQgJ8ma+9586n4tT4cI8DEhBSZsWMjrCt8dxKg==";

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolCallResponse {
    stop_reason: &'static str,
    error_message: Option<String>,
    tool_call_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProviderResponse {
    stop_reason: &'static str,
    error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct PrefilledMessages {
    user_message: Value,
    assistant_message: Value,
    tool_result: Value,
    follow_up_user: Value,
}

fn echo_tool() -> Value {
    json!({
        "name": "echo",
        "description": "Echoes the message back",
        "parameters": {
            "type": "object",
            "properties": {
                "message": { "type": "string", "description": "Message to echo back" }
            },
            "required": ["message"],
        },
    })
}

fn assert_catalog_model(provider: &str, model: &str) {
    assert!(
        compat::get_model(provider, model).is_some(),
        "missing builtin model {provider}/{model}"
    );
}

fn generate_copilot_tool_call(_message: &str) -> ToolCallResponse {
    assert_catalog_model("github-copilot", "gpt-5.2-codex");
    panic!("{BLOCKER}");
}

fn complete_handoff(provider: &str, model: &str, _tool_call_id: &str) -> ProviderResponse {
    assert_catalog_model(provider, model);
    panic!("{BLOCKER}");
}

fn complete_prefilled_context(
    provider: &str,
    model: &str,
    _messages: &PrefilledMessages,
) -> ProviderResponse {
    assert_catalog_model(provider, model);
    panic!("{BLOCKER}");
}

fn build_prefilled_messages() -> PrefilledMessages {
    PrefilledMessages {
        user_message: json!({
            "role": "user",
            "content": "Use the echo tool to echo 'hello'",
            "timestamp": 1_u64,
        }),
        assistant_message: json!({
            "role": "assistant",
            "content": [{
                "type": "toolCall",
                "id": FAILING_TOOL_CALL_ID,
                "name": "echo",
                "arguments": { "message": "hello" },
            }],
            "api": "openai-responses",
            "provider": "github-copilot",
            "model": "gpt-5.2-codex",
            "usage": {
                "input": 100_u64,
                "output": 50_u64,
                "cacheRead": 0_u64,
                "cacheWrite": 0_u64,
                "totalTokens": 150_u64,
                "cost": { "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0 },
            },
            "stopReason": "toolUse",
            "timestamp": 2_u64,
        }),
        tool_result: json!({
            "role": "toolResult",
            "toolCallId": FAILING_TOOL_CALL_ID,
            "toolName": "echo",
            "content": [{ "type": "text", "text": "hello" }],
            "isError": false,
            "timestamp": 3_u64,
        }),
        follow_up_user: json!({
            "role": "user",
            "content": "Say hi",
            "timestamp": 4_u64,
        }),
    }
}

#[test]
fn tool_call_id_normalization_models_are_registered() {
    for (provider, model) in [
        ("github-copilot", "gpt-5.2-codex"),
        ("openrouter", "openai/gpt-5.2-codex"),
        ("openai-codex", "gpt-5.5"),
    ] {
        assert_eq!(
            compat::get_model(provider, model)
                .expect("builtin model should be registered")
                .provider,
            provider
        );
    }
    assert_eq!(echo_tool()["name"], "echo");
}

#[test]
#[ignore = "live provider parity test; see BLOCKER"]
fn github_copilot_to_openrouter_should_normalize_pipe_separated_ids() {
    let assistant_response = generate_copilot_tool_call("hello world");
    assert_eq!(assistant_response.stop_reason, "toolUse");
    assert!(assistant_response.error_message.is_none());
    assert!(assistant_response.tool_call_id.contains('|'));

    let openrouter_response = complete_handoff(
        "openrouter",
        "openai/gpt-5.2-codex",
        &assistant_response.tool_call_id,
    );
    assert_ne!(openrouter_response.stop_reason, "error");
    assert!(openrouter_response.error_message.is_none());
}

#[test]
#[ignore = "live provider parity test; see BLOCKER"]
fn github_copilot_to_openai_codex_should_normalize_pipe_separated_ids() {
    let assistant_response = generate_copilot_tool_call("test message");
    assert_eq!(assistant_response.stop_reason, "toolUse");
    assert!(assistant_response.error_message.is_none());

    let codex_response =
        complete_handoff("openai-codex", "gpt-5.5", &assistant_response.tool_call_id);
    assert_ne!(codex_response.stop_reason, "error");
    assert!(codex_response.error_message.is_none());
}

#[test]
#[ignore = "live provider parity test; see BLOCKER"]
fn openrouter_should_handle_prefilled_context_with_long_pipe_separated_ids() {
    let response = complete_prefilled_context(
        "openrouter",
        "openai/gpt-5.2-codex",
        &build_prefilled_messages(),
    );
    assert_ne!(response.stop_reason, "error");
    if let Some(error_message) = response.error_message {
        assert!(!error_message.contains("call_id"));
        assert!(!error_message.contains("too long"));
    }
}

#[test]
#[ignore = "live provider parity test; see BLOCKER"]
fn openai_codex_should_handle_prefilled_context_with_long_pipe_separated_ids() {
    let response =
        complete_prefilled_context("openai-codex", "gpt-5.5", &build_prefilled_messages());
    assert_ne!(response.stop_reason, "error");
    if let Some(error_message) = response.error_message {
        assert!(!error_message.contains("id"));
        assert!(!error_message.contains("additional characters"));
    }
}
