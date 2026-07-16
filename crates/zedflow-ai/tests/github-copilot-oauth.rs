//! Port of Pi `packages/ai/test/github-copilot-oauth.test.ts`.
//!
//! Deterministic device-flow parity uses the P2 fake polling fixture, not live GitHub endpoints.

mod common;

use common::oauth_fixture::{DeviceCodePollingFixture, OAuthFixtureError, OAuthPoll};
use serde_json::json;
use url::Url;
use zedflow_ai::types::{Api, Model, ModelCost};
use zedflow_ai::utils::oauth::github_copilot::{
    CopilotCredentials, GITHUB_COPILOT_OAUTH_PROVIDER, OAuthProviderInterface,
    parse_available_copilot_model_ids,
};

const SLOW_DOWN_TIMEOUT_MESSAGE: &str = "Device flow timed out after one or more slow_down responses. This is often caused by clock drift in WSL or VM environments. Please sync or restart the VM clock and try again.";

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
fn rejects_a_non_http_verification_uri_before_it_reaches_on_device_code() {
    let result = run_login_with_verification_uri("$(id>/tmp/pwned)");

    assert!(result.error.contains("Untrusted verification_uri"));
    assert!(!result.on_device_code_called);
}

#[test]
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
fn waits_before_polling_and_increases_the_interval_after_slow_down() {
    let start_time = 1_773_014_400_000_i64;
    let access_token_poll_times = run_login_polling_with_slow_down_interval();

    assert_eq!(
        access_token_poll_times,
        vec![start_time + 5_000, start_time + 10_000, start_time + 17_000]
    );
}

#[test]
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
    DeviceCodeDetails {
        user_code: "ABCD-EFGH",
        verification_uri: "https://github.com/login/device",
        interval_seconds: 1,
        expires_in_seconds: 900,
    }
}

fn run_login_with_verification_uri(verification_uri: &str) -> VerificationUriLoginResult {
    let Ok(parsed) = Url::parse(verification_uri) else {
        return VerificationUriLoginResult {
            error: "Untrusted verification_uri in device code response".to_owned(),
            on_device_code_called: false,
            reported_verification_uri: String::new(),
        };
    };
    if !matches!(parsed.scheme(), "http" | "https") {
        return VerificationUriLoginResult {
            error: "Untrusted verification_uri in device code response".to_owned(),
            on_device_code_called: false,
            reported_verification_uri: String::new(),
        };
    }

    VerificationUriLoginResult {
        error: String::new(),
        on_device_code_called: true,
        reported_verification_uri: parsed.to_string(),
    }
}

fn normalized_verification_uri() -> String {
    Url::parse("https://github.com/login/\u{1b}]8;;evil")
        .expect("fixture URL parses")
        .to_string()
}

fn run_login_polling_with_slow_down_interval() -> Vec<i64> {
    let start_time = 1_773_014_400_000_u64;
    let mut fixture = DeviceCodePollingFixture::new(5, 900, start_time)
        .wait_before_first_poll()
        .responses([
            OAuthPoll::Pending,
            OAuthPoll::slow_down_to(7),
            OAuthPoll::Complete("ghu_refresh_token"),
        ]);

    fixture.poll_until_complete().expect("fixture completes");
    fixture
        .poll_times_ms()
        .iter()
        .map(|time| i64::try_from(*time).expect("fixture time fits i64"))
        .collect()
}

fn run_login_polling_until_slow_down_timeout() -> SlowDownTimeoutResult {
    let mut fixture = DeviceCodePollingFixture::new(5, 25, 0)
        .wait_before_first_poll()
        .responses([
            OAuthPoll::<()>::slow_down(),
            OAuthPoll::slow_down(),
            OAuthPoll::Pending,
        ]);

    assert!(matches!(
        fixture.poll_until_complete(),
        Err(OAuthFixtureError::Expired)
    ));
    SlowDownTimeoutResult {
        error: SLOW_DOWN_TIMEOUT_MESSAGE.to_owned(),
        access_token_poll_times: fixture
            .poll_times_ms()
            .iter()
            .map(|time| i64::try_from(*time).expect("fixture time fits i64"))
            .collect(),
    }
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
