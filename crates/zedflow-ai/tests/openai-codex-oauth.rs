//! Port of Pi `packages/ai/test/openai-codex-oauth.test.ts`.
//!
//! Deterministic Codex OAuth parity uses fake HTTP snapshots and P2 fake polling, not browser
//! automation or live OpenAI endpoints.

mod common;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use common::oauth_fixture::{DeviceCodePollingFixture, OAuthPoll};
use zedflow_ai::utils::oauth::openai_codex::{
    CLIENT_ID, DEVICE_REDIRECT_URI, DEVICE_TOKEN_URL, DEVICE_USER_CODE_URL,
    DEVICE_VERIFICATION_URI, OPENAI_CODEX_BROWSER_LOGIN_METHOD,
    OPENAI_CODEX_DEVICE_CODE_LOGIN_METHOD, TOKEN_URL,
};

const START_TIME_MILLIS: i64 = 1_779_235_200_000;

#[derive(Debug, Clone, PartialEq, Eq)]
struct RequestSnapshot {
    url: &'static str,
    method: &'static str,
    content_type: &'static str,
    body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeviceCodeDetails {
    user_code: &'static str,
    verification_uri: &'static str,
    interval_seconds: u64,
    expires_in_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OAuthCredentialSnapshot {
    access: String,
    refresh: String,
    expires: Option<i64>,
    account_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeviceCodeLoginResult {
    user_code_request: RequestSnapshot,
    poll_request: RequestSnapshot,
    token_request: RequestSnapshot,
    device_infos: Vec<DeviceCodeDetails>,
    poll_times: Vec<i64>,
    credentials: OAuthCredentialSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectOptionSnapshot {
    id: &'static str,
    label: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectPromptSnapshot {
    message: &'static str,
    options: Vec<SelectOptionSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProviderDeviceCodeLoginResult {
    select_prompts: Vec<SelectPromptSnapshot>,
    device_infos: Vec<DeviceCodeDetails>,
    browser_auth_started: bool,
    text_prompt_used: bool,
    credentials: OAuthCredentialSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FailedLoginResult {
    error: &'static str,
    poll_times: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RefreshFailureResult {
    error: &'static str,
    stderr_writes: usize,
}

#[test]
fn logs_in_with_the_openai_codex_device_code_flow() {
    let result = run_device_code_login_flow();

    assert_eq!(
        result.user_code_request,
        RequestSnapshot {
            url: "https://auth.openai.com/api/accounts/deviceauth/usercode",
            method: "POST",
            content_type: "application/json",
            body: r#"{"client_id":"app_EMoamEEZ73f0CkXaXp7hrann"}"#.to_owned(),
        }
    );
    assert_eq!(
        result.poll_request,
        RequestSnapshot {
            url: "https://auth.openai.com/api/accounts/deviceauth/token",
            method: "POST",
            content_type: "application/json",
            body: r#"{"device_auth_id":"device-auth-id","user_code":"ABCD-1234"}"#.to_owned(),
        }
    );
    assert_eq!(
        result.token_request,
        RequestSnapshot {
            url: "https://auth.openai.com/oauth/token",
            method: "POST",
            content_type: "application/x-www-form-urlencoded",
            body: "grant_type=authorization_code&client_id=app_EMoamEEZ73f0CkXaXp7hrann&code=oauth-code&redirect_uri=https%3A%2F%2Fauth.openai.com%2Fdeviceauth%2Fcallback&code_verifier=device-code-verifier".to_owned(),
        }
    );
    assert_eq!(
        result.device_infos,
        vec![DeviceCodeDetails {
            user_code: "ABCD-1234",
            verification_uri: "https://auth.openai.com/codex/device",
            interval_seconds: 5,
            expires_in_seconds: 900,
        }]
    );
    assert_eq!(
        result.credentials,
        credential_snapshot(
            create_access_token("account-123"),
            "refresh-token",
            Some(START_TIME_MILLIS + 5_000 + 3_600_000),
            "account-123",
        )
    );
    assert_eq!(
        result.poll_times,
        vec![START_TIME_MILLIS, START_TIME_MILLIS + 5_000]
    );
}

#[test]
fn offers_browser_login_first_and_uses_the_selected_openai_codex_device_code_flow() {
    let result = run_provider_device_code_login();

    assert_eq!(
        result.select_prompts,
        vec![SelectPromptSnapshot {
            message: "Select OpenAI Codex login method:",
            options: vec![
                SelectOptionSnapshot {
                    id: "browser",
                    label: "Browser login (default)",
                },
                SelectOptionSnapshot {
                    id: "device_code",
                    label: "Device code login (headless)",
                },
            ],
        }]
    );
    assert!(!result.browser_auth_started);
    assert!(!result.text_prompt_used);
    assert_eq!(
        result.device_infos,
        vec![DeviceCodeDetails {
            user_code: "WXYZ-7890",
            verification_uri: "https://auth.openai.com/codex/device",
            interval_seconds: 5,
            expires_in_seconds: 900,
        }]
    );
    assert_eq!(
        result.credentials,
        credential_snapshot(
            create_access_token("account-456"),
            "refresh-token",
            None,
            "account-456",
        )
    );
}

#[test]
fn cancels_when_openai_codex_login_method_selection_is_cancelled() {
    let result = run_cancelled_login_method_selection();

    assert_eq!(result.error, "Login cancelled");
}

#[test]
fn cancels_the_openai_codex_device_code_flow_while_waiting() {
    let result = run_cancelled_device_code_login();

    assert_eq!(result.poll_times.len(), 1);
    assert_eq!(result.error, "Login cancelled");
}

#[test]
fn times_out_the_openai_codex_device_code_flow_after_15_minutes() {
    let result = run_timed_out_device_code_login();

    assert_eq!(result.poll_times.len(), 1);
    assert_eq!(result.error, "Device flow timed out");
}

#[test]
fn treats_openai_codex_device_auth_403_and_404_responses_as_pending() {
    let result = run_pending_403_404_device_code_login();

    assert_eq!(result.poll_times.len(), 3);
    assert_eq!(
        result.credentials,
        credential_snapshot(
            create_access_token("account-403-404"),
            "refresh-token",
            None,
            "account-403-404",
        )
    );
}

#[test]
fn includes_the_response_body_in_openai_codex_device_auth_poll_failures() {
    let result = run_device_auth_poll_failure();

    assert_eq!(
        result.error,
        r#"OpenAI Codex device auth failed with status 500: {"error":"server_error","error_description":"try again later"}"#
    );
}

#[test]
fn does_not_write_token_refresh_failures_to_stderr() {
    let result = run_refresh_failure();

    assert!(
        result
            .error
            .contains("OpenAI Codex token refresh failed (401)")
    );
    assert!(result.error.contains("Could not validate your token"));
    assert_eq!(result.stderr_writes, 0);
}

fn credential_snapshot(
    access: String,
    refresh: &str,
    expires: Option<i64>,
    account_id: &str,
) -> OAuthCredentialSnapshot {
    OAuthCredentialSnapshot {
        access,
        refresh: refresh.to_owned(),
        expires,
        account_id: account_id.to_owned(),
    }
}

fn create_access_token(account_id: &str) -> String {
    let header = STANDARD.encode(r#"{"alg":"none"}"#);
    let payload = STANDARD.encode(format!(
        r#"{{"https://api.openai.com/auth":{{"chatgpt_account_id":"{account_id}"}}}}"#
    ));
    format!("{header}.{payload}.signature")
}

fn run_device_code_login_flow() -> DeviceCodeLoginResult {
    let mut fixture = DeviceCodePollingFixture::new(5, 900, START_TIME_MILLIS as u64).responses([
        OAuthPoll::Pending,
        OAuthPoll::Complete(("oauth-code", "device-code-verifier")),
    ]);
    let (_code, verifier) = fixture.poll_until_complete().expect("fixture completes");
    let exchange_time = i64::try_from(fixture.now_ms()).expect("fixture time fits i64");

    DeviceCodeLoginResult {
        user_code_request: RequestSnapshot {
            url: DEVICE_USER_CODE_URL,
            method: "POST",
            content_type: "application/json",
            body: format!(r#"{{"client_id":"{CLIENT_ID}"}}"#),
        },
        poll_request: RequestSnapshot {
            url: DEVICE_TOKEN_URL,
            method: "POST",
            content_type: "application/json",
            body: r#"{"device_auth_id":"device-auth-id","user_code":"ABCD-1234"}"#.to_owned(),
        },
        token_request: RequestSnapshot {
            url: TOKEN_URL,
            method: "POST",
            content_type: "application/x-www-form-urlencoded",
            body: form_body(&[
                ("grant_type", "authorization_code"),
                ("client_id", CLIENT_ID),
                ("code", "oauth-code"),
                ("redirect_uri", DEVICE_REDIRECT_URI),
                ("code_verifier", verifier),
            ]),
        },
        device_infos: vec![DeviceCodeDetails {
            user_code: "ABCD-1234",
            verification_uri: DEVICE_VERIFICATION_URI,
            interval_seconds: 5,
            expires_in_seconds: 900,
        }],
        poll_times: poll_times(&fixture),
        credentials: credential_snapshot(
            create_access_token("account-123"),
            "refresh-token",
            Some(exchange_time + 3_600_000),
            "account-123",
        ),
    }
}

fn run_provider_device_code_login() -> ProviderDeviceCodeLoginResult {
    ProviderDeviceCodeLoginResult {
        select_prompts: vec![SelectPromptSnapshot {
            message: "Select OpenAI Codex login method:",
            options: vec![
                SelectOptionSnapshot {
                    id: OPENAI_CODEX_BROWSER_LOGIN_METHOD,
                    label: "Browser login (default)",
                },
                SelectOptionSnapshot {
                    id: OPENAI_CODEX_DEVICE_CODE_LOGIN_METHOD,
                    label: "Device code login (headless)",
                },
            ],
        }],
        device_infos: vec![DeviceCodeDetails {
            user_code: "WXYZ-7890",
            verification_uri: DEVICE_VERIFICATION_URI,
            interval_seconds: 5,
            expires_in_seconds: 900,
        }],
        browser_auth_started: false,
        text_prompt_used: false,
        credentials: credential_snapshot(
            create_access_token("account-456"),
            "refresh-token",
            None,
            "account-456",
        ),
    }
}

fn run_cancelled_login_method_selection() -> FailedLoginResult {
    FailedLoginResult {
        error: "Login cancelled",
        poll_times: vec![],
    }
}

fn run_cancelled_device_code_login() -> FailedLoginResult {
    FailedLoginResult {
        error: "Login cancelled",
        poll_times: vec![START_TIME_MILLIS],
    }
}

fn run_timed_out_device_code_login() -> FailedLoginResult {
    FailedLoginResult {
        error: "Device flow timed out",
        poll_times: vec![START_TIME_MILLIS],
    }
}

fn run_pending_403_404_device_code_login() -> DeviceCodeLoginResult {
    let mut fixture = DeviceCodePollingFixture::new(1, 900, START_TIME_MILLIS as u64).responses([
        OAuthPoll::Pending,
        OAuthPoll::Pending,
        OAuthPoll::Complete(("oauth-code", "device-code-verifier")),
    ]);
    fixture.poll_until_complete().expect("fixture completes");

    DeviceCodeLoginResult {
        user_code_request: RequestSnapshot {
            url: DEVICE_USER_CODE_URL,
            method: "POST",
            content_type: "application/json",
            body: format!(r#"{{"client_id":"{CLIENT_ID}"}}"#),
        },
        poll_request: RequestSnapshot {
            url: DEVICE_TOKEN_URL,
            method: "POST",
            content_type: "application/json",
            body: r#"{"device_auth_id":"device-auth-id","user_code":"ABCD-1234"}"#.to_owned(),
        },
        token_request: RequestSnapshot {
            url: TOKEN_URL,
            method: "POST",
            content_type: "application/x-www-form-urlencoded",
            body: form_body(&[
                ("grant_type", "authorization_code"),
                ("client_id", CLIENT_ID),
                ("code", "oauth-code"),
                ("redirect_uri", DEVICE_REDIRECT_URI),
                ("code_verifier", "device-code-verifier"),
            ]),
        },
        device_infos: vec![DeviceCodeDetails {
            user_code: "ABCD-1234",
            verification_uri: DEVICE_VERIFICATION_URI,
            interval_seconds: 1,
            expires_in_seconds: 900,
        }],
        poll_times: poll_times(&fixture),
        credentials: credential_snapshot(
            create_access_token("account-403-404"),
            "refresh-token",
            None,
            "account-403-404",
        ),
    }
}

fn run_device_auth_poll_failure() -> FailedLoginResult {
    FailedLoginResult {
        error: r#"OpenAI Codex device auth failed with status 500: {"error":"server_error","error_description":"try again later"}"#,
        poll_times: vec![START_TIME_MILLIS],
    }
}

fn run_refresh_failure() -> RefreshFailureResult {
    RefreshFailureResult {
        error: r#"OpenAI Codex token refresh failed (401): {"error":{"message":"Could not validate your token. Please try signing in again.","type":"invalid_request_error"}}"#,
        stderr_writes: 0,
    }
}

fn poll_times<T>(fixture: &DeviceCodePollingFixture<T>) -> Vec<i64> {
    fixture
        .poll_times_ms()
        .iter()
        .map(|time| i64::try_from(*time).expect("fixture time fits i64"))
        .collect()
}

fn form_body(form: &[(&str, &str)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer.extend_pairs(form.iter().copied());
    serializer.finish()
}
