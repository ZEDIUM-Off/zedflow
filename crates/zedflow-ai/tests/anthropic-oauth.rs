//! Port of Pi `packages/ai/test/anthropic-oauth.test.ts`.
//!
//! The source test mocks `fetch` around Anthropic OAuth login/refresh. The Rust source row still
//! exposes those flows as PORT PLACEHOLDERs, so these parity tests are ignored until a Rust local
//! callback server, cancellable prompt signal, and token-exchange HTTP client are selected.

use zedflow_ai::utils::oauth::anthropic::{REDIRECT_URI, TOKEN_URL};

const BLOCKER: &str = "PORT PLACEHOLDER: Anthropic OAuth login/refresh still require a Rust local callback server, cancellable manual-code prompt signal, and injectable token-exchange HTTP client; no live calls are allowed";

#[derive(Debug, Clone, PartialEq, Eq)]
struct TokenRequest {
    url: &'static str,
    method: &'static str,
    grant_type: &'static str,
    code: Option<&'static str>,
    redirect_uri: Option<&'static str>,
    refresh_token: Option<&'static str>,
    scope: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OAuthCredentialSnapshot {
    credential_type: &'static str,
    access: &'static str,
    refresh: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManualCodeLoginResult {
    credential: OAuthCredentialSnapshot,
    emitted_auth_url: bool,
    prompted_manual_code: bool,
    manual_signal_aborted_after_settle: bool,
}

#[test]
#[ignore = "PORT PLACEHOLDER: loginAnthropic token exchange and localhost/manual callback flow are not implemented"]
fn keeps_the_localhost_redirect_uri_for_manual_callback_login() {
    let (request, credentials) = run_manual_callback_login();

    assert_eq!(request.url, TOKEN_URL);
    assert_eq!(request.method, "POST");
    assert_eq!(request.grant_type, "authorization_code");
    assert_eq!(request.code, Some("manual-code"));
    assert_eq!(request.redirect_uri, Some(REDIRECT_URI));
    assert_eq!(credentials.access, "access-token");
    assert_eq!(credentials.refresh, "refresh-token");
}

#[test]
#[ignore = "PORT PLACEHOLDER: refreshAnthropicToken token exchange HTTP client is not implemented"]
fn omits_scope_from_refresh_token_requests() {
    let (request, credentials) = run_refresh_token_request();

    assert_eq!(request.url, TOKEN_URL);
    assert_eq!(request.method, "POST");
    assert_eq!(request.grant_type, "refresh_token");
    assert_eq!(request.refresh_token, Some("refresh-token"));
    assert!(request.scope.is_none());
    assert_eq!(credentials.access, "new-access-token");
    assert_eq!(credentials.refresh, "new-refresh-token");
}

#[test]
#[ignore = "PORT PLACEHOLDER: anthropicOAuth.login manual_code prompt and prompt abort signal are not implemented"]
fn anthropic_oauth_login_resolves_through_manual_code_prompt_and_aborts_it_after_settling() {
    let result = run_anthropic_oauth_manual_code_login();

    assert_eq!(result.credential.credential_type, "oauth");
    assert_eq!(result.credential.access, "access");
    assert!(result.emitted_auth_url);
    assert!(result.prompted_manual_code);
    assert!(result.manual_signal_aborted_after_settle);
}

fn run_manual_callback_login() -> (TokenRequest, OAuthCredentialSnapshot) {
    panic!("{BLOCKER}")
}

fn run_refresh_token_request() -> (TokenRequest, OAuthCredentialSnapshot) {
    panic!("{BLOCKER}")
}

fn run_anthropic_oauth_manual_code_login() -> ManualCodeLoginResult {
    panic!("{BLOCKER}")
}
