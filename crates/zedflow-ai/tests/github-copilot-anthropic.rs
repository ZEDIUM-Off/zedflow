use std::collections::HashMap;

use zedflow_ai::api::github_copilot_headers::{
    CopilotDynamicHeadersParams, Message as CopilotMessage, UserMessageContent,
    build_copilot_dynamic_headers,
};
use zedflow_ai::providers::github_copilot_models::{GITHUB_COPILOT_MODELS, GithubCopilotModel};

const BLOCKER: &str = "PORT PLACEHOLDER: anthropic_messages::stream is not ported far enough to create an Anthropic client or capture Messages request payloads without live provider I/O.";
const EXTENDED_THINKING_LEVELS: &[&str] = &["off", "minimal", "low", "medium", "high", "xhigh"];

#[derive(Debug, Clone, PartialEq, Eq)]
struct AnthropicConstructorOptions {
    api_key: Option<String>,
    auth_token: Option<String>,
    default_headers: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AnthropicCreateParams {
    model: String,
    stream: bool,
    max_tokens: u32,
    messages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CapturedAnthropicRequest {
    constructor_opts: AnthropicConstructorOptions,
    create_params: AnthropicCreateParams,
}

fn get_github_copilot_model(id: &str) -> &'static GithubCopilotModel {
    GITHUB_COPILOT_MODELS
        .iter()
        .find(|model| model.id == id)
        .expect("github-copilot model fixture should exist")
}

fn thinking_map_value<'a>(model: &'a GithubCopilotModel, level: &str) -> Option<Option<&'a str>> {
    model.thinking_level_map.and_then(|map| {
        map.iter()
            .find(|(key, _)| *key == level)
            .map(|(_, value)| *value)
    })
}

fn supported_thinking_levels(model: &GithubCopilotModel) -> Vec<&'static str> {
    if !model.reasoning {
        return vec!["off"];
    }

    EXTENDED_THINKING_LEVELS
        .iter()
        .copied()
        .filter(|level| {
            let mapped = thinking_map_value(model, level);
            if mapped == Some(None) {
                return false;
            }
            if *level == "xhigh" {
                return mapped.is_some();
            }
            true
        })
        .collect()
}

fn capture_copilot_anthropic_request(
    _model: &GithubCopilotModel,
    _messages: &[CopilotMessage],
    _api_key: &str,
    _interleaved_thinking: bool,
) -> CapturedAnthropicRequest {
    panic!("{BLOCKER}");
}

#[test]
fn applies_copilot_specific_adaptive_thinking_effort_overrides() {
    let opus47 = get_github_copilot_model("claude-opus-4.7");
    assert_eq!(thinking_map_value(opus47, "minimal"), Some(Some("low")));
    assert_eq!(thinking_map_value(opus47, "xhigh"), Some(Some("xhigh")));
    assert!(supported_thinking_levels(opus47).contains(&"xhigh"));

    let sonnet46 = get_github_copilot_model("claude-sonnet-4.6");
    assert_eq!(thinking_map_value(sonnet46, "minimal"), Some(Some("low")));
    assert_eq!(thinking_map_value(sonnet46, "xhigh"), Some(Some("max")));
    assert!(supported_thinking_levels(sonnet46).contains(&"xhigh"));
}

#[test]
#[ignore = "PORT PLACEHOLDER: Anthropic Messages stream/client construction is not ported, so Bearer auth and request payload capture cannot run deterministically yet"]
fn uses_bearer_auth_copilot_headers_and_valid_anthropic_messages_payload() {
    let model = get_github_copilot_model("claude-sonnet-4.6");
    assert_eq!(model.api, "anthropic-messages");

    let messages = vec![CopilotMessage::User {
        content: UserMessageContent::Text("Hello".to_owned()),
    }];
    let dynamic_headers = build_copilot_dynamic_headers(CopilotDynamicHeadersParams {
        messages: &messages,
        has_images: false,
    });
    assert_eq!(
        dynamic_headers.get("X-Initiator").map(String::as_str),
        Some("user")
    );
    assert_eq!(
        dynamic_headers.get("Openai-Intent").map(String::as_str),
        Some("conversation-edits")
    );

    let captured =
        capture_copilot_anthropic_request(model, &messages, "tid_copilot_session_test_token", true);

    assert_eq!(captured.constructor_opts.api_key, None);
    assert_eq!(
        captured.constructor_opts.auth_token.as_deref(),
        Some("tid_copilot_session_test_token")
    );
    let headers = &captured.constructor_opts.default_headers;

    assert!(
        headers
            .get("User-Agent")
            .is_some_and(|value| value.contains("GitHubCopilotChat"))
    );
    assert_eq!(
        headers.get("Copilot-Integration-Id").map(String::as_str),
        Some("vscode-chat")
    );
    assert_eq!(headers.get("X-Initiator").map(String::as_str), Some("user"));
    assert_eq!(
        headers.get("Openai-Intent").map(String::as_str),
        Some("conversation-edits")
    );
    assert!(
        !headers
            .get("anthropic-beta")
            .is_some_and(|value| value.contains("fine-grained-tool-streaming"))
    );

    let params = &captured.create_params;
    assert_eq!(params.model, "claude-sonnet-4.6");
    assert!(params.stream);
    assert_eq!(params.max_tokens, model.max_tokens);
    assert!(!params.messages.is_empty());
}

#[test]
#[ignore = "PORT PLACEHOLDER: Anthropic Messages stream/client construction is not ported, so beta header capture cannot run deterministically yet"]
fn omits_interleaved_thinking_beta_for_adaptive_thinking_models() {
    let model = get_github_copilot_model("claude-sonnet-4.6");
    let messages = vec![CopilotMessage::User {
        content: UserMessageContent::Text("Hello".to_owned()),
    }];

    let captured =
        capture_copilot_anthropic_request(model, &messages, "tid_copilot_session_test_token", true);

    let headers = &captured.constructor_opts.default_headers;
    assert!(
        !headers
            .get("anthropic-beta")
            .is_some_and(|value| value.contains("interleaved-thinking-2025-05-14"))
    );
}
