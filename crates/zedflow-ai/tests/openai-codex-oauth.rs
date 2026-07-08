//! Port of Pi `packages/ai/test/openai-codex-oauth.test.ts`.
//!
//! The Pi test mocks `fetch`, fake timers, and `AbortController` around OpenAI Codex OAuth.
//! The Rust source row still exposes browser login, device-code polling, token exchange, refresh,
//! and cancellation as PORT PLACEHOLDERs, so these parity tests are ignored until an injectable
//! HTTP client, timer, and cancellation surface are selected.

use base64::{Engine as _, engine::general_purpose::STANDARD};

const BLOCKER: &str = "PORT PLACEHOLDER: OpenAI Codex OAuth login/refresh still require an injectable Rust HTTP client/fetch replacement, fakeable timers, AbortSignal-equivalent cancellation, and device-code polling; no live calls are allowed";
const START_TIME_MILLIS: i64 = 1_779_235_200_000;

#[derive(Debug, Clone, PartialEq, Eq)]
struct RequestSnapshot {
    url: &'static str,
    method: &'static str,
    content_type: &'static str,
    body: &'static str,
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
#[ignore = "PORT PLACEHOLDER: loginOpenAICodexDeviceCode HTTP polling, token exchange, timers, and callback delivery are not implemented"]
fn logs_in_with_the_openai_codex_device_code_flow() {
    let result = run_device_code_login_flow();

    assert_eq!(
        result.user_code_request,
        RequestSnapshot {
            url: "https://auth.openai.com/api/accounts/deviceauth/usercode",
            method: "POST",
            content_type: "application/json",
            body: r#"{"client_id":"app_EMoamEEZ73f0CkXaXp7hrann"}"#,
        }
    );
    assert_eq!(
        result.poll_request,
        RequestSnapshot {
            url: "https://auth.openai.com/api/accounts/deviceauth/token",
            method: "POST",
            content_type: "application/json",
            body: r#"{"device_auth_id":"device-auth-id","user_code":"ABCD-1234"}"#,
        }
    );
    assert_eq!(
        result.token_request,
        RequestSnapshot {
            url: "https://auth.openai.com/oauth/token",
            method: "POST",
            content_type: "application/x-www-form-urlencoded",
            body: "grant_type=authorization_code&client_id=app_EMoamEEZ73f0CkXaXp7hrann&code=oauth-code&redirect_uri=https%3A%2F%2Fauth.openai.com%2Fdeviceauth%2Fcallback&code_verifier=device-code-verifier",
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
#[ignore = "PORT PLACEHOLDER: openaiCodexOAuthProvider.login device-code selection still falls through to unimplemented device-code HTTP flow"]
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
#[ignore = "PORT PLACEHOLDER: Rust auth callback surface cannot yet model cancelled selection as Pi's undefined onSelect result"]
fn cancels_when_openai_codex_login_method_selection_is_cancelled() {
    let result = run_cancelled_login_method_selection();

    assert_eq!(result.error, "Login cancelled");
}

#[test]
#[ignore = "PORT PLACEHOLDER: loginOpenAICodexDeviceCode cancellation while waiting is not implemented"]
fn cancels_the_openai_codex_device_code_flow_while_waiting() {
    let result = run_cancelled_device_code_login();

    assert_eq!(result.poll_times.len(), 1);
    assert_eq!(result.error, "Login cancelled");
}

#[test]
#[ignore = "PORT PLACEHOLDER: loginOpenAICodexDeviceCode timeout path is not implemented"]
fn times_out_the_openai_codex_device_code_flow_after_15_minutes() {
    let result = run_timed_out_device_code_login();

    assert_eq!(result.poll_times.len(), 1);
    assert_eq!(result.error, "Device flow timed out");
}

#[test]
#[ignore = "PORT PLACEHOLDER: loginOpenAICodexDeviceCode 403/404 pending polling behavior is not implemented"]
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
#[ignore = "PORT PLACEHOLDER: loginOpenAICodexDeviceCode device-auth error-body passthrough is not implemented"]
fn includes_the_response_body_in_openai_codex_device_auth_poll_failures() {
    let result = run_device_auth_poll_failure();

    assert_eq!(
        result.error,
        r#"OpenAI Codex device auth failed with status 500: {"error":"server_error","error_description":"try again later"}"#
    );
}

#[test]
#[ignore = "PORT PLACEHOLDER: refreshOpenAICodexToken HTTP refresh failure path is not implemented"]
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
    panic!("{BLOCKER}")
}

fn run_provider_device_code_login() -> ProviderDeviceCodeLoginResult {
    panic!("{BLOCKER}")
}

fn run_cancelled_login_method_selection() -> FailedLoginResult {
    panic!("{BLOCKER}")
}

fn run_cancelled_device_code_login() -> FailedLoginResult {
    panic!("{BLOCKER}")
}

fn run_timed_out_device_code_login() -> FailedLoginResult {
    panic!("{BLOCKER}")
}

fn run_pending_403_404_device_code_login() -> DeviceCodeLoginResult {
    panic!("{BLOCKER}")
}

fn run_device_auth_poll_failure() -> FailedLoginResult {
    panic!("{BLOCKER}")
}

fn run_refresh_failure() -> RefreshFailureResult {
    panic!("{BLOCKER}")
}
