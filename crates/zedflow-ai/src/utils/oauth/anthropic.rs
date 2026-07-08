//! Anthropic OAuth flow ported from Pi's `packages/ai/src/utils/oauth/anthropic.ts`.

#[cfg(test)]
use std::collections::BTreeMap;
use std::error::Error as StdError;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
use serde::Deserialize;
#[cfg(test)]
use serde_json::Value;
use zedflow_core::{error::Result, placeholders};

use crate::auth::types::{
    AuthFuture, AuthLoginCallbacks, AuthResult, BoxError, ModelAuth, OAuthAuth, OAuthCredential,
};

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

const OAUTH_PLACEHOLDER_DEPENDENCY: &str = "Node.js node:http callback server, Fetch API AbortSignal.timeout, and Rust HTTP client for Anthropic OAuth token exchange";
const OAUTH_PLACEHOLDER_BEHAVIOR: &str = "run Anthropic authorization-code + PKCE login on localhost:53692/callback, accept manual code or redirect URL input, validate OAuth state, exchange authorization codes and refresh tokens at https://platform.claude.com/v1/oauth/token, and return Pi OAuth credentials with expires adjusted by five minutes";

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct AuthorizationInput {
    code: Option<String>,
    state: Option<String>,
}

#[cfg(test)]
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
        _callbacks: &'a dyn AuthLoginCallbacks,
    ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
        Box::pin(async { Err(oauth_placeholder_box()) })
    }

    fn refresh<'a>(
        &'a self,
        credential: &'a OAuthCredential,
    ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
        Box::pin(async move {
            refresh_anthropic_token(&credential.refresh)
                .await
                .map_err(|error| Box::new(error) as BoxError)
        })
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
    /// Returns a documented port placeholder until Rust replacements are selected for the local
    /// callback server and token-exchange HTTP client.
    pub async fn login(self, callbacks: &dyn AuthLoginCallbacks) -> AuthResult<OAuthCredential> {
        login_anthropic(callbacks)
            .await
            .map_err(|error| Box::new(error) as BoxError)
    }

    /// Refreshes an Anthropic OAuth credential.
    ///
    /// # Errors
    ///
    /// Returns a documented port placeholder until a Rust token-exchange HTTP client is selected.
    pub async fn refresh_token(self, credentials: &OAuthCredential) -> Result<OAuthCredential> {
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
/// PORT PLACEHOLDER:
/// Original dependency: Node.js `node:http`, Web Crypto PKCE, browser/manual prompt race, and HTTP token exchange.
/// Reason: no Rust replacement selected yet.
/// Required behavior: generate PKCE, start a localhost callback server on `127.0.0.1:53692`, notify the auth URL, accept either callback or manual redirect/code input, validate state, exchange the authorization code, and close the server.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until Rust replacements are selected for the local callback
/// server, PKCE generation, prompt race cancellation, and token exchange.
pub async fn login_anthropic(_callbacks: &dyn AuthLoginCallbacks) -> Result<OAuthCredential> {
    placeholders::unsupported(OAUTH_PLACEHOLDER_DEPENDENCY, OAUTH_PLACEHOLDER_BEHAVIOR)
}

/// Refreshes an Anthropic OAuth token.
///
/// PORT PLACEHOLDER:
/// Original dependency: Fetch API with `AbortSignal.timeout(30_000)` and Anthropic OAuth token endpoint.
/// Reason: no Rust replacement selected yet.
/// Required behavior: POST JSON `{ grant_type: "refresh_token", client_id, refresh_token }` to `TOKEN_URL`, parse `access_token`, `refresh_token`, and `expires_in`, and return credentials with expiry five minutes early.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until a Rust HTTP client and timeout policy are selected for
/// the token endpoint.
pub async fn refresh_anthropic_token(_refresh_token: &str) -> Result<OAuthCredential> {
    placeholders::unsupported(OAUTH_PLACEHOLDER_DEPENDENCY, OAUTH_PLACEHOLDER_BEHAVIOR)
}

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn oauth_placeholder_box() -> BoxError {
    Box::new(placeholders::error(
        OAUTH_PLACEHOLDER_DEPENDENCY,
        OAUTH_PLACEHOLDER_BEHAVIOR,
    )) as Box<dyn StdError + Send + Sync>
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
