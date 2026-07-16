//! Anthropic OAuth flow ported from Pi's `packages/ai/src/utils/oauth/anthropic.ts`.

use std::collections::BTreeMap;
use std::error::Error as StdError;
use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth::types::{
    AuthFuture, AuthLoginCallbacks, AuthResult, BoxError, ModelAuth, OAuthAuth, OAuthCredential,
};
use crate::utils::abort_signals::{AbortController, combine_abort_signals};

/// Anthropic OAuth client id decoded from Pi's base64 literal.
pub const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
/// Anthropic OAuth authorization endpoint.
pub const AUTHORIZE_URL: &str = "https://claude.ai/oauth/authorize";
/// Anthropic OAuth token endpoint.
pub const TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";
/// Default callback host used by Pi when `PI_OAUTH_CALLBACK_HOST` is unset.
pub const DEFAULT_CALLBACK_HOST: &str = "127.0.0.1";
/// Local callback port used by Pi.
pub const CALLBACK_PORT: u16 = 53692;
/// Local callback route used by Pi.
pub const CALLBACK_PATH: &str = "/callback";
/// Redirect URI sent to Anthropic by Pi.
pub const REDIRECT_URI: &str = "http://localhost:53692/callback";
/// OAuth scopes requested by Pi's Anthropic OAuth flow.
pub const SCOPES: &str = "org:create_api_key user:profile user:inference user:sessions:claude_code user:mcp_servers user:file_upload";
/// Anthropic OAuth provider id.
pub const ANTHROPIC_OAUTH_PROVIDER_ID: &str = "anthropic";
/// Anthropic OAuth display name.
pub const ANTHROPIC_OAUTH_NAME: &str = "Anthropic (Claude Pro/Max)";

const TOKEN_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnthropicOAuthError {
    MissingAuthorizationCode,
    MissingOAuthState,
    StateMismatch,
    Http(String),
}

impl fmt::Display for AnthropicOAuthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingAuthorizationCode => formatter.write_str("Missing authorization code"),
            Self::MissingOAuthState => formatter.write_str("Missing OAuth state"),
            Self::StateMismatch => formatter.write_str("OAuth state mismatch"),
            Self::Http(message) => formatter.write_str(message),
        }
    }
}

impl StdError for AnthropicOAuthError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AuthorizationInput {
    code: Option<String>,
    state: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
    #[allow(dead_code)]
    scope: Option<String>,
}

/// Stateless Anthropic OAuth auth handler.
#[derive(Debug, Clone, Copy, Default)]
pub struct AnthropicOAuth;

impl OAuthAuth for AnthropicOAuth {
    fn name(&self) -> &str {
        ANTHROPIC_OAUTH_NAME
    }

    fn login<'a>(
        &'a self,
        callbacks: &'a dyn AuthLoginCallbacks,
    ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
        Box::pin(async move { login_anthropic(callbacks).await })
    }

    fn refresh<'a>(
        &'a self,
        credential: &'a OAuthCredential,
    ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
        Box::pin(async move { refresh_anthropic_token(&credential.refresh).await })
    }

    fn to_auth<'a>(
        &'a self,
        credential: &'a OAuthCredential,
    ) -> AuthFuture<'a, AuthResult<ModelAuth>> {
        Box::pin(async move {
            Ok(ModelAuth {
                api_key: Some(credential.access.clone()),
                ..ModelAuth::default()
            })
        })
    }
}

/// Provider-style Anthropic OAuth descriptor matching Pi's `anthropicOAuthProvider` export.
#[derive(Debug, Clone, Copy, Default)]
pub struct AnthropicOAuthProvider;

impl AnthropicOAuthProvider {
    /// Provider id used by Pi's OAuth registry.
    #[must_use]
    pub const fn id(self) -> &'static str {
        ANTHROPIC_OAUTH_PROVIDER_ID
    }

    /// Display name used by Pi's OAuth registry.
    #[must_use]
    pub const fn name(self) -> &'static str {
        ANTHROPIC_OAUTH_NAME
    }

    /// Whether this OAuth flow uses a local callback server.
    #[must_use]
    pub const fn uses_callback_server(self) -> bool {
        true
    }

    /// Runs Anthropic login.
    ///
    /// # Errors
    ///
    /// Returns prompt, PKCE, state validation, token-endpoint, or JSON parsing failures.
    pub async fn login(self, callbacks: &dyn AuthLoginCallbacks) -> AuthResult<OAuthCredential> {
        login_anthropic(callbacks).await
    }

    /// Refreshes an Anthropic OAuth credential.
    ///
    /// # Errors
    ///
    /// Returns token-endpoint or JSON parsing failures.
    pub async fn refresh_token(self, credentials: &OAuthCredential) -> AuthResult<OAuthCredential> {
        refresh_anthropic_token(&credentials.refresh).await
    }

    /// Converts Anthropic OAuth credentials to the API key string Pi passes to requests.
    #[must_use]
    pub fn get_api_key(self, credentials: &OAuthCredential) -> &str {
        &credentials.access
    }
}

/// Anthropic OAuth auth handler matching Pi's `anthropicOAuth` export.
pub const ANTHROPIC_OAUTH: AnthropicOAuth = AnthropicOAuth;

/// Anthropic OAuth provider descriptor matching Pi's `anthropicOAuthProvider` export.
pub const ANTHROPIC_OAUTH_PROVIDER: AnthropicOAuthProvider = AnthropicOAuthProvider;

/// Login with Anthropic OAuth using authorization code + PKCE.
///
/// This Rust port uses the deterministic manual-code path: it emits Pi's browser URL, prompts for
/// an authorization code or redirect URL, validates state, and exchanges the code with `reqwest`.
/// Local browser callback automation remains a manual/live concern.
///
/// # Errors
///
/// Returns prompt, PKCE, state validation, token-endpoint, or JSON parsing failures.
pub async fn login_anthropic(callbacks: &dyn AuthLoginCallbacks) -> AuthResult<OAuthCredential> {
    let pkce = crate::utils::oauth::pkce::generate_pkce()
        .await
        .map_err(box_error)?;
    let auth_url = anthropic_authorize_url(&pkce.challenge, &pkce.verifier);
    callbacks.notify(crate::auth::types::AuthEvent::AuthUrl {
        url: auth_url,
        instructions: Some(
            "Complete login in your browser. If the browser is on another machine, paste the final redirect URL here."
                .to_owned(),
        ),
    });

    let prompt_controller = AbortController::new();
    let mut prompt_signal =
        combine_abort_signals(&[callbacks.signal(), Some(prompt_controller.signal())]);
    let input = callbacks
        .prompt(crate::auth::types::AuthPrompt::ManualCode {
            message: "Complete login in your browser, or paste the authorization code / redirect URL here:"
                .to_owned(),
            placeholder: Some(REDIRECT_URI.to_owned()),
            signal: prompt_signal.signal.clone(),
        })
        .await;
    prompt_controller.abort();
    prompt_signal.cleanup();
    let input = input?;
    let parsed = parse_authorization_input(&input);
    if parsed
        .state
        .as_deref()
        .is_some_and(|state| state != pkce.verifier)
    {
        return Err(box_error(AnthropicOAuthError::StateMismatch));
    }
    let code = parsed
        .code
        .ok_or_else(|| box_error(AnthropicOAuthError::MissingAuthorizationCode))?;
    let state = parsed.state.unwrap_or(pkce.verifier.clone());

    callbacks.notify(crate::auth::types::AuthEvent::Progress {
        message: "Exchanging authorization code for tokens...".to_owned(),
    });
    exchange_anthropic_authorization_code(&code, &state, &pkce.verifier, REDIRECT_URI).await
}

/// Refreshes an Anthropic OAuth token.
///
/// # Errors
///
/// Returns token-endpoint or JSON parsing failures.
pub async fn refresh_anthropic_token(refresh_token: &str) -> AuthResult<OAuthCredential> {
    let body = json!({
        "grant_type": "refresh_token",
        "client_id": CLIENT_ID,
        "refresh_token": refresh_token,
    });
    let response_body = post_json(TOKEN_URL, &body).map_err(|error| {
        box_error(AnthropicOAuthError::Http(format!(
            "Anthropic token refresh request failed. url={TOKEN_URL}; details={}",
            format_error_details(error.as_ref())
        )))
    })?;
    credentials_from_token_response(&response_body, now_millis()).map_err(|error| {
        box_error(AnthropicOAuthError::Http(format!(
            "Anthropic token refresh returned invalid JSON. url={TOKEN_URL}; body={response_body}; details={}",
            format_error_details(&error)
        )))
    })
}

async fn exchange_anthropic_authorization_code(
    code: &str,
    state: &str,
    verifier: &str,
    redirect_uri: &str,
) -> AuthResult<OAuthCredential> {
    let body = json!({
        "grant_type": "authorization_code",
        "client_id": CLIENT_ID,
        "code": code,
        "state": state,
        "redirect_uri": redirect_uri,
        "code_verifier": verifier,
    });
    let response_body = post_json(TOKEN_URL, &body).map_err(|error| {
        box_error(AnthropicOAuthError::Http(format!(
            "Token exchange request failed. url={TOKEN_URL}; redirect_uri={redirect_uri}; response_type=authorization_code; details={}",
            format_error_details(error.as_ref())
        )))
    })?;
    credentials_from_token_response(&response_body, now_millis()).map_err(|error| {
        box_error(AnthropicOAuthError::Http(format!(
            "Token exchange returned invalid JSON. url={TOKEN_URL}; body={response_body}; details={}",
            format_error_details(&error)
        )))
    })
}

fn anthropic_authorize_url(challenge: &str, state: &str) -> String {
    let mut url = url::Url::parse(AUTHORIZE_URL).expect("Anthropic authorize URL is valid");
    url.query_pairs_mut()
        .append_pair("code", "true")
        .append_pair("client_id", CLIENT_ID)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("scope", SCOPES)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", state);
    url.to_string()
}

fn post_json(url: &str, body: &Value) -> std::result::Result<String, BoxError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(TOKEN_TIMEOUT)
        .build()
        .map_err(box_error)?;
    let response = client
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .header(reqwest::header::ACCEPT, "application/json")
        .json(body)
        .send()
        .map_err(box_error)?;
    let status = response.status();
    let response_body = response.text().map_err(box_error)?;
    if !status.is_success() {
        return Err(box_error(AnthropicOAuthError::Http(format!(
            "HTTP request failed. status={}; url={url}; body={response_body}",
            status.as_u16()
        ))));
    }
    Ok(response_body)
}

fn parse_authorization_input(input: &str) -> AuthorizationInput {
    let value = input.trim();
    if value.is_empty() {
        return AuthorizationInput {
            code: None,
            state: None,
        };
    }

    if let Some(query) = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))
        .and_then(|_| value.split_once('?').map(|(_, query)| query))
    {
        return parse_query(query.split_once('#').map_or(query, |(query, _)| query));
    }

    if let Some((code, state)) = value.split_once('#') {
        return AuthorizationInput {
            code: Some(code.to_owned()),
            state: Some(state.to_owned()),
        };
    }

    if value.contains("code=") {
        return parse_query(value);
    }

    AuthorizationInput {
        code: Some(value.to_owned()),
        state: None,
    }
}

fn parse_query(query: &str) -> AuthorizationInput {
    let mut parsed = AuthorizationInput {
        code: None,
        state: None,
    };

    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        match key {
            "code" => parsed.code = Some(percent_decode(value)),
            "state" => parsed.state = Some(percent_decode(value)),
            _ => {}
        }
    }

    parsed
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                if let Ok(hex) = u8::from_str_radix(&value[i + 1..i + 3], 16) {
                    out.push(hex);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }

    String::from_utf8_lossy(&out).into_owned()
}

fn credentials_from_token_response(
    response_body: &str,
    now_millis: i64,
) -> serde_json::Result<OAuthCredential> {
    let token: TokenResponse = serde_json::from_str(response_body)?;
    Ok(OAuthCredential {
        refresh: token.refresh_token,
        access: token.access_token,
        expires: now_millis + token.expires_in * 1000 - 5 * 60 * 1000,
        extra: BTreeMap::<String, Value>::new(),
    })
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn format_error_details(error: &(dyn StdError + 'static)) -> String {
    let mut details = format!("{}: {}", std::any::type_name_of_val(error), error);
    if let Some(source) = error.source() {
        details.push_str("; cause=");
        details.push_str(&format_error_details(source));
    }
    details
}

fn box_error(error: impl StdError + Send + Sync + 'static) -> BoxError {
    Box::new(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_code() {
        assert_eq!(
            parse_authorization_input(" code123 "),
            AuthorizationInput {
                code: Some("code123".to_owned()),
                state: None,
            }
        );
    }

    #[test]
    fn parses_code_hash_state() {
        assert_eq!(
            parse_authorization_input("code123#state456"),
            AuthorizationInput {
                code: Some("code123".to_owned()),
                state: Some("state456".to_owned()),
            }
        );
    }

    #[test]
    fn parses_query_string_or_redirect_url() {
        let expected = AuthorizationInput {
            code: Some("code 123".to_owned()),
            state: Some("state456".to_owned()),
        };
        assert_eq!(
            parse_authorization_input("code=code+123&state=state456"),
            expected
        );
        assert_eq!(
            parse_authorization_input(
                "http://localhost:53692/callback?code=code+123&state=state456"
            ),
            expected
        );
    }

    #[test]
    fn converts_token_response_to_pi_credentials() {
        let credentials = credentials_from_token_response(
            r#"{"access_token":"access","refresh_token":"refresh","expires_in":3600}"#,
            1_000_000,
        )
        .expect("valid token response");

        assert_eq!(credentials.access, "access");
        assert_eq!(credentials.refresh, "refresh");
        assert_eq!(credentials.expires, 4_300_000);
    }

    #[test]
    fn preserves_public_constants() {
        assert_eq!(CLIENT_ID, "9d1c250a-e61b-44d9-88ed-5944d1962f5e");
        assert_eq!(REDIRECT_URI, "http://localhost:53692/callback");
        assert!(SCOPES.contains("user:inference"));
        assert!(ANTHROPIC_OAUTH_PROVIDER.uses_callback_server());
        assert_eq!(ANTHROPIC_OAUTH_PROVIDER.id(), "anthropic");
    }

    #[test]
    fn to_auth_returns_access_token_as_api_key() {
        let credential = OAuthCredential {
            refresh: "refresh".to_owned(),
            access: "access".to_owned(),
            expires: now_millis(),
            extra: BTreeMap::new(),
        };

        let auth =
            futures::executor::block_on(ANTHROPIC_OAUTH.to_auth(&credential)).expect("to auth");
        assert_eq!(auth.api_key.as_deref(), Some("access"));
    }
}
