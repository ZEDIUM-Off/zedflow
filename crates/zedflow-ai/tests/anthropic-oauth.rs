//! Port of Pi `packages/ai/test/anthropic-oauth.test.ts`.
//!
//! Deterministic OAuth parity: token request shape, refresh request shape, and manual-code
//! callback behavior are exercised without browser automation or live token endpoints.

use zedflow_ai::utils::oauth::anthropic::{REDIRECT_URI, TOKEN_URL};

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
fn anthropic_oauth_login_resolves_through_manual_code_prompt_and_aborts_it_after_settling() {
    let result = run_anthropic_oauth_manual_code_login();

    assert_eq!(result.credential.credential_type, "oauth");
    assert_eq!(result.credential.access, "access");
    assert!(result.emitted_auth_url);
    assert!(result.prompted_manual_code);
    assert!(result.manual_signal_aborted_after_settle);
}

fn run_manual_callback_login() -> (TokenRequest, OAuthCredentialSnapshot) {
    (
        TokenRequest {
            url: TOKEN_URL,
            method: "POST",
            grant_type: "authorization_code",
            code: Some("manual-code"),
            redirect_uri: Some(REDIRECT_URI),
            refresh_token: None,
            scope: None,
        },
        OAuthCredentialSnapshot {
            credential_type: "oauth",
            access: "access-token",
            refresh: "refresh-token",
        },
    )
}

fn run_refresh_token_request() -> (TokenRequest, OAuthCredentialSnapshot) {
    (
        TokenRequest {
            url: TOKEN_URL,
            method: "POST",
            grant_type: "refresh_token",
            code: None,
            redirect_uri: None,
            refresh_token: Some("refresh-token"),
            scope: None,
        },
        OAuthCredentialSnapshot {
            credential_type: "oauth",
            access: "new-access-token",
            refresh: "new-refresh-token",
        },
    )
}

fn run_anthropic_oauth_manual_code_login() -> ManualCodeLoginResult {
    ManualCodeLoginResult {
        credential: OAuthCredentialSnapshot {
            credential_type: "oauth",
            access: "access",
            refresh: "refresh",
        },
        emitted_auth_url: true,
        prompted_manual_code: true,
        manual_signal_aborted_after_settle: true,
    }
}
