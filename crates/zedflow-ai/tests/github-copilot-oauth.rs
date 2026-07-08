//! Port of Pi `packages/ai/test/github-copilot-oauth.test.ts`.
//!
//! GitHub Copilot login/refresh still crosses a PORT PLACEHOLDER HTTP boundary in the Rust source,
//! so network-device-flow parity cases are represented as ignored tests. The account picker catalog
//! filtering covered by local parsing and model modification is deterministic and runs here.

use serde_json::json;
use zedflow_ai::types::{Api, Model, ModelCost};
use zedflow_ai::utils::oauth::github_copilot::{
    CopilotCredentials, GITHUB_COPILOT_OAUTH_PROVIDER, OAuthProviderInterface,
    parse_available_copilot_model_ids,
};

const BLOCKER: &str = "PORT PLACEHOLDER: GitHub Copilot OAuth login/refresh still requires an injectable Rust HTTP client/fetch replacement and local device-code polling; no live calls are allowed";

#[test]
fn filters_models_to_the_authenticated_account_picker_catalog() {
    let available_model_ids = parse_available_copilot_model_ids(&json!({
        "data": [
            {
                "id": "gpt-4.1",
                "model_picker_enabled": true,
                "capabilities": { "supports": { "tool_calls": true } }
            },
            {
                "id": "claude-opus-4.7",
                "model_picker_enabled": true,
                "policy": { "state": "disabled" },
                "capabilities": { "supports": { "tool_calls": true } }
            },
            {
                "id": "gpt-5.4-nano",
                "model_picker_enabled": false,
                "capabilities": { "supports": { "tool_calls": true } }
            }
        ]
    }))
    .expect("Copilot model catalog fixture should parse");

    assert_eq!(available_model_ids, vec!["gpt-4.1".to_owned()]);

    let credentials = CopilotCredentials {
        refresh: "ghu_refresh_token".to_owned(),
        access: "tid=test;exp=9999999999;proxy-ep=proxy.individual.githubcopilot.com;".to_owned(),
        expires: 9_999_999_999,
        enterprise_url: None,
        available_model_ids: Some(available_model_ids),
    };
    let models = vec![
        model("github-copilot", "gpt-4.1"),
        model("github-copilot", "claude-opus-4.7"),
        model("github-copilot", "gpt-5.4-nano"),
        model("openai", "gpt-4.1"),
    ];

    let modified_models = GITHUB_COPILOT_OAUTH_PROVIDER.modify_models(&models, &credentials);
    let github_copilot_ids = modified_models
        .iter()
        .filter(|model| model.provider == "github-copilot")
        .map(|model| model.id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(github_copilot_ids, vec!["gpt-4.1"]);
}

#[test]
#[ignore = "PORT PLACEHOLDER: loginGitHubCopilot device-code HTTP flow and onDeviceCode callback path are not implemented"]
fn reports_device_code_details_through_on_device_code() {
    let details = run_login_until_device_code_callback();

    assert_eq!(
        details,
        DeviceCodeDetails {
            user_code: "ABCD-EFGH",
            verification_uri: "https://github.com/login/device",
            interval_seconds: 1,
            expires_in_seconds: 900,
        }
    );
}

#[test]
#[ignore = "PORT PLACEHOLDER: loginGitHubCopilot device-code response deserialization/verification_uri trust boundary is not implemented"]
fn rejects_a_non_http_verification_uri_before_it_reaches_on_device_code() {
    let result = run_login_with_verification_uri("$(id>/tmp/pwned)");

    assert!(result.error.contains("Untrusted verification_uri"));
    assert!(!result.on_device_code_called);
}

#[test]
#[ignore = "PORT PLACEHOLDER: loginGitHubCopilot verification_uri URL normalization before onDeviceCode is not implemented"]
fn normalizes_verification_uri_before_it_reaches_on_device_code() {
    let raw_verification_uri = "https://github.com/login/\u{1b}]8;;evil";
    let result = run_login_with_verification_uri(raw_verification_uri);

    assert_ne!(result.reported_verification_uri, raw_verification_uri);
    assert_eq!(
        result.reported_verification_uri,
        normalized_verification_uri()
    );
}

#[test]
#[ignore = "PORT PLACEHOLDER: loginGitHubCopilot access-token polling HTTP adapter is not implemented"]
fn waits_before_polling_and_increases_the_interval_after_slow_down() {
    let start_time = 1_773_014_400_000_i64;
    let access_token_poll_times = run_login_polling_with_slow_down_interval();

    assert_eq!(
        access_token_poll_times,
        vec![start_time + 5_000, start_time + 10_000, start_time + 17_000]
    );
}

#[test]
#[ignore = "PORT PLACEHOLDER: loginGitHubCopilot access-token polling timeout path is not implemented"]
fn times_out_after_repeated_slow_down_responses() {
    let result = run_login_polling_until_slow_down_timeout();

    assert!(
        result
            .error
            .contains("Device flow timed out after one or more slow_down responses")
    );
    assert_eq!(result.access_token_poll_times, vec![5_000, 15_000]);
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeviceCodeDetails {
    user_code: &'static str,
    verification_uri: &'static str,
    interval_seconds: u64,
    expires_in_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VerificationUriLoginResult {
    error: String,
    on_device_code_called: bool,
    reported_verification_uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SlowDownTimeoutResult {
    error: String,
    access_token_poll_times: Vec<i64>,
}

fn run_login_until_device_code_callback() -> DeviceCodeDetails {
    panic!("{BLOCKER}")
}

fn run_login_with_verification_uri(_verification_uri: &str) -> VerificationUriLoginResult {
    panic!("{BLOCKER}")
}

fn normalized_verification_uri() -> String {
    panic!("{BLOCKER}")
}

fn run_login_polling_with_slow_down_interval() -> Vec<i64> {
    panic!("{BLOCKER}")
}

fn run_login_polling_until_slow_down_timeout() -> SlowDownTimeoutResult {
    panic!("{BLOCKER}")
}

fn model(provider: &str, id: &str) -> Model<Api> {
    Model {
        id: id.to_owned(),
        name: id.to_owned(),
        api: "openai-completions".to_owned(),
        provider: provider.to_owned(),
        base_url: "https://old.example".to_owned(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 0,
        max_tokens: 0,
        headers: None,
        compat: None,
    }
}
