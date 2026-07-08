//! OpenAI Codex (ChatGPT OAuth) flow ported from Pi's `packages/ai/src/utils/oauth/openai-codex.ts`.

#[cfg(test)]
use std::collections::BTreeMap;
use std::error::Error as StdError;
use std::fmt;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
use serde::Deserialize;
#[cfg(test)]
use serde_json::Value;
#[cfg(test)]
use url::Url;
use zedflow_core::{error::Result, placeholders};

use crate::auth::types::{
    AuthEvent, AuthFuture, AuthLoginCallbacks, AuthPrompt, AuthResult, AuthSelectOption, BoxError,
    ModelAuth, OAuthAuth, OAuthCredential,
};

/// OpenAI Codex OAuth client id used by Pi.
pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
/// OpenAI Codex OAuth authorization base URL.
pub const AUTH_BASE_URL: &str = "https://auth.openai.com";
/// OpenAI Codex OAuth authorization endpoint.
pub const AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
/// OpenAI Codex OAuth token endpoint.
pub const TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
/// Local redirect URI used by Pi's browser login flow.
pub const REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
/// Device-auth user-code endpoint.
pub const DEVICE_USER_CODE_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/usercode";
/// Device-auth token polling endpoint.
pub const DEVICE_TOKEN_URL: &str = "https://auth.openai.com/api/accounts/deviceauth/token";
/// Device-auth verification page shown to users.
pub const DEVICE_VERIFICATION_URI: &str = "https://auth.openai.com/codex/device";
/// Redirect URI used when exchanging a device-auth authorization code.
pub const DEVICE_REDIRECT_URI: &str = "https://auth.openai.com/deviceauth/callback";
/// Device-code timeout used by Pi, in seconds.
pub const DEVICE_CODE_TIMEOUT_SECONDS: u64 = 15 * 60;
/// Browser login method id shown by Pi.
pub const OPENAI_CODEX_BROWSER_LOGIN_METHOD: &str = "browser";
/// Device-code login method id shown by Pi.
pub const OPENAI_CODEX_DEVICE_CODE_LOGIN_METHOD: &str = "device_code";
/// OAuth scope requested by Pi's OpenAI Codex flow.
pub const SCOPE: &str = "openid profile email offline_access";
/// JWT claim path containing the ChatGPT account id.
pub const JWT_CLAIM_PATH: &str = "https://api.openai.com/auth";
/// OpenAI Codex OAuth display name used by auth resolution.
pub const OPENAI_CODEX_OAUTH_NAME: &str = "OpenAI (ChatGPT Plus/Pro)";
/// OpenAI Codex OAuth provider id used by Pi's OAuth registry.
pub const OPENAI_CODEX_OAUTH_PROVIDER_ID: &str = "openai-codex";
/// OpenAI Codex OAuth provider display name used by Pi's OAuth registry.
pub const OPENAI_CODEX_OAUTH_PROVIDER_NAME: &str = "ChatGPT Plus/Pro (Codex Subscription)";

const PLACEHOLDER_DEPENDENCY: &str = "Node.js node:crypto/node:http, Web Crypto PKCE, Fetch API, AbortSignal cancellation, and pollOAuthDeviceCodeFlow";
const PLACEHOLDER_BEHAVIOR: &str = "run OpenAI Codex browser and device-code OAuth login, start a localhost callback server on the configured callback host and port 1455, generate PKCE and state, poll device auth with pending/slow_down handling, exchange and refresh tokens at https://auth.openai.com/oauth/token, extract accountId from the access-token JWT, and preserve Pi cancellation and prompt behavior";

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct AuthorizationInput {
    code: Option<String>,
    state: Option<String>,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct OAuthToken {
    access: String,
    refresh: String,
    expires: i64,
}

#[cfg(test)]
#[derive(Debug, Deserialize)]
struct JwtPayload {
    #[serde(rename = "https://api.openai.com/auth")]
    auth: Option<JwtAuthClaim>,
}

#[cfg(test)]
#[derive(Debug, Deserialize)]
struct JwtAuthClaim {
    chatgpt_account_id: Option<String>,
}

/// OpenAI Codex OAuth errors handled before the HTTP placeholder boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenAiCodexOAuthError {
    /// The authorization code was missing after callback/manual parsing.
    MissingAuthorizationCode,
    /// The authorization state returned by the browser or prompt did not match the generated state.
    StateMismatch,
    /// The selected login method is not one of Pi's supported method ids.
    UnknownLoginMethod(String),
    /// The token did not contain a ChatGPT account id in Pi's expected JWT claim.
    MissingAccountId,
}

impl fmt::Display for OpenAiCodexOAuthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingAuthorizationCode => formatter.write_str("Missing authorization code"),
            Self::StateMismatch => formatter.write_str("State mismatch"),
            Self::UnknownLoginMethod(method) => {
                write!(formatter, "Unknown OpenAI Codex login method: {method}")
            }
            Self::MissingAccountId => formatter.write_str("Failed to extract accountId from token"),
        }
    }
}

impl StdError for OpenAiCodexOAuthError {}

/// Stateless OpenAI Codex OAuth auth handler.
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenAiCodexOAuth;

impl OAuthAuth for OpenAiCodexOAuth {
    fn name(&self) -> &str {
        OPENAI_CODEX_OAUTH_NAME
    }

    fn login<'a>(
        &'a self,
        callbacks: &'a dyn AuthLoginCallbacks,
    ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
        Box::pin(async move { login_with_auth_callbacks(callbacks).await })
    }

    fn refresh<'a>(
        &'a self,
        credential: &'a OAuthCredential,
    ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
        Box::pin(async move {
            refresh_openai_codex_token(&credential.refresh)
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

/// Provider-style OpenAI Codex OAuth descriptor matching Pi's `openaiCodexOAuthProvider` export.
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenAiCodexOAuthProvider;

impl OpenAiCodexOAuthProvider {
    /// Provider id used by Pi's OAuth registry.
    #[must_use]
    pub const fn id(self) -> &'static str {
        OPENAI_CODEX_OAUTH_PROVIDER_ID
    }

    /// Display name used by Pi's OAuth registry.
    #[must_use]
    pub const fn name(self) -> &'static str {
        OPENAI_CODEX_OAUTH_PROVIDER_NAME
    }

    /// Whether this OAuth flow uses a local callback server.
    #[must_use]
    pub const fn uses_callback_server(self) -> bool {
        true
    }

    /// Runs OpenAI Codex login.
    ///
    /// # Errors
    ///
    /// Returns prompt failures, unknown login-method errors, or a documented port placeholder until
    /// Rust replacements are selected for the browser/device OAuth HTTP flows.
    pub async fn login(self, callbacks: &dyn AuthLoginCallbacks) -> AuthResult<OAuthCredential> {
        login_with_auth_callbacks(callbacks).await
    }

    /// Refreshes an OpenAI Codex OAuth credential.
    ///
    /// # Errors
    ///
    /// Returns a documented port placeholder until a Rust token-exchange HTTP client is selected.
    pub async fn refresh_token(self, credentials: &OAuthCredential) -> Result<OAuthCredential> {
        refresh_openai_codex_token(&credentials.refresh).await
    }

    /// Converts OpenAI Codex OAuth credentials to the API key string Pi passes to requests.
    #[must_use]
    pub fn get_api_key<'a>(self, credentials: &'a OAuthCredential) -> &'a str {
        &credentials.access
    }
}

/// OpenAI Codex OAuth auth handler matching Pi's `openaiCodexOAuth` export.
pub const OPENAI_CODEX_OAUTH: OpenAiCodexOAuth = OpenAiCodexOAuth;
/// OpenAI Codex OAuth provider descriptor matching Pi's `openaiCodexOAuthProvider` export.
pub const OPENAI_CODEX_OAUTH_PROVIDER: OpenAiCodexOAuthProvider = OpenAiCodexOAuthProvider;

/// Login with OpenAI Codex OAuth using the Codex device-code flow.
///
/// PORT PLACEHOLDER:
/// Original dependency: Fetch API, AbortSignal cancellation, and `pollOAuthDeviceCodeFlow`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: POST `CLIENT_ID` to `DEVICE_USER_CODE_URL`, notify the user code, poll `DEVICE_TOKEN_URL` with pending/slow_down handling, exchange the authorization code with `DEVICE_REDIRECT_URI`, and return Pi OAuth credentials with `accountId`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until Rust HTTP and cancellation replacements are selected.
pub async fn login_openai_codex_device_code(
    _callbacks: &dyn AuthLoginCallbacks,
) -> Result<OAuthCredential> {
    placeholders::unsupported(PLACEHOLDER_DEPENDENCY, PLACEHOLDER_BEHAVIOR)
}

/// Login with OpenAI Codex OAuth using Pi's browser/manual-code flow.
///
/// PORT PLACEHOLDER:
/// Original dependency: Node.js `node:crypto`/`node:http`, Web Crypto PKCE, Fetch API, callback-page HTML, and prompt cancellation.
/// Reason: no Rust replacement selected yet.
/// Required behavior: generate state and PKCE, start a localhost server at `/auth/callback`, notify the auth URL, race browser callback with manual code input, validate state, exchange the code with `REDIRECT_URI`, close the server, and return Pi OAuth credentials with `accountId`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until Rust local-server, PKCE, HTTP, and cancellation
/// replacements are selected.
pub async fn login_openai_codex(_callbacks: &dyn AuthLoginCallbacks) -> Result<OAuthCredential> {
    placeholders::unsupported(PLACEHOLDER_DEPENDENCY, PLACEHOLDER_BEHAVIOR)
}

/// Refreshes an OpenAI Codex OAuth token.
///
/// PORT PLACEHOLDER:
/// Original dependency: Fetch API and OpenAI Codex OAuth token endpoint.
/// Reason: no Rust replacement selected yet.
/// Required behavior: POST form data `{ grant_type: "refresh_token", refresh_token, client_id }` to `TOKEN_URL`, parse `access_token`, `refresh_token`, and `expires_in`, extract `accountId` from the access-token JWT, and return Pi OAuth credentials.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until a Rust HTTP client and timeout policy are selected for
/// the token endpoint.
pub async fn refresh_openai_codex_token(_refresh_token: &str) -> Result<OAuthCredential> {
    placeholders::unsupported(PLACEHOLDER_DEPENDENCY, PLACEHOLDER_BEHAVIOR)
}

async fn login_with_auth_callbacks(
    callbacks: &dyn AuthLoginCallbacks,
) -> AuthResult<OAuthCredential> {
    let method = callbacks
        .prompt(AuthPrompt::Select {
            message: "Select OpenAI Codex login method:".to_owned(),
            options: vec![
                AuthSelectOption {
                    id: OPENAI_CODEX_BROWSER_LOGIN_METHOD.to_owned(),
                    label: "Browser login (default)".to_owned(),
                    description: None,
                },
                AuthSelectOption {
                    id: OPENAI_CODEX_DEVICE_CODE_LOGIN_METHOD.to_owned(),
                    label: "Device code login (headless)".to_owned(),
                    description: None,
                },
            ],
            signal: callbacks.signal(),
        })
        .await?;

    match method.as_str() {
        OPENAI_CODEX_DEVICE_CODE_LOGIN_METHOD => {
            callbacks.notify(AuthEvent::Progress {
                message: "Starting OpenAI Codex device code login".to_owned(),
            });
            login_openai_codex_device_code(callbacks)
                .await
                .map_err(|error| Box::new(error) as BoxError)
        }
        OPENAI_CODEX_BROWSER_LOGIN_METHOD => login_openai_codex(callbacks)
            .await
            .map_err(|error| Box::new(error) as BoxError),
        _ => Err(Box::new(OpenAiCodexOAuthError::UnknownLoginMethod(method)) as BoxError),
    }
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

    if let Ok(url) = Url::parse(value) {
        return AuthorizationInput {
            code: url
                .query_pairs()
                .find_map(|(key, value)| (key == "code").then(|| value.into_owned())),
            state: url
                .query_pairs()
                .find_map(|(key, value)| (key == "state").then(|| value.into_owned())),
        };
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

    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        match key.as_ref() {
            "code" => parsed.code = Some(value.into_owned()),
            "state" => parsed.state = Some(value.into_owned()),
            _ => {}
        }
    }

    parsed
}

#[cfg(test)]
fn get_account_id(access_token: &str) -> Option<String> {
    let mut parts = access_token.split('.');
    let _header = parts.next()?;
    let payload = parts.next()?;
    parts.next()?;
    if parts.next().is_some() {
        return None;
    }

    let payload = decode_base64url(payload)?;
    let payload: JwtPayload = serde_json::from_slice(&payload).ok()?;
    payload
        .auth
        .and_then(|auth| auth.chatgpt_account_id)
        .filter(|account_id| !account_id.is_empty())
}

#[cfg(test)]
fn credentials_from_token(
    token: OAuthToken,
) -> std::result::Result<OAuthCredential, OpenAiCodexOAuthError> {
    let account_id =
        get_account_id(&token.access).ok_or(OpenAiCodexOAuthError::MissingAccountId)?;
    let mut extra = BTreeMap::new();
    extra.insert("accountId".to_owned(), Value::String(account_id));
    Ok(OAuthCredential {
        refresh: token.refresh,
        access: token.access,
        expires: token.expires,
        extra,
    })
}

#[cfg(test)]
fn credentials_from_token_response(
    response_body: &str,
    now_millis: i64,
) -> serde_json::Result<std::result::Result<OAuthCredential, OpenAiCodexOAuthError>> {
    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
        refresh_token: String,
        expires_in: i64,
    }

    let token: TokenResponse = serde_json::from_str(response_body)?;
    Ok(credentials_from_token(OAuthToken {
        access: token.access_token,
        refresh: token.refresh_token,
        expires: now_millis + token.expires_in * 1000,
    }))
}

#[cfg(test)]
fn decode_base64url(input: &str) -> Option<Vec<u8>> {
    let mut bits = 0_u32;
    let mut bit_count = 0_u8;
    let mut out = Vec::with_capacity(input.len() * 3 / 4);

    for byte in input.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            _ => return None,
        };

        bits = (bits << 6) | u32::from(value);
        bit_count += 6;
        while bit_count >= 8 {
            bit_count -= 8;
            out.push(((bits >> bit_count) & 0xff) as u8);
        }
    }

    Some(out)
}

#[cfg(test)]
fn test_access_token(account_id: &str) -> String {
    let payload = format!(r#"{{"{JWT_CLAIM_PATH}":{{"chatgpt_account_id":"{account_id}"}}}}"#,);
    format!(
        "eyJhbGciOiJub25lIn0.{}.signature",
        base64url_no_pad(payload.as_bytes())
    )
}

#[cfg(test)]
fn base64url_no_pad(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0b0000_0011) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0b0000_1111) << 2) | (b2 >> 6)) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0b0011_1111) as usize] as char);
        }
    }
    out
}

#[cfg(test)]
fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    #[test]
    fn parses_authorization_input_like_pi() {
        assert_eq!(
            parse_authorization_input(" code123 "),
            AuthorizationInput {
                code: Some("code123".to_owned()),
                state: None,
            }
        );
        assert_eq!(
            parse_authorization_input("code123#state456"),
            AuthorizationInput {
                code: Some("code123".to_owned()),
                state: Some("state456".to_owned()),
            }
        );
        assert_eq!(
            parse_authorization_input("code=code+123&state=state456"),
            AuthorizationInput {
                code: Some("code 123".to_owned()),
                state: Some("state456".to_owned()),
            }
        );
        assert_eq!(
            parse_authorization_input(
                "http://localhost:1455/auth/callback?code=code+123&state=state456"
            ),
            AuthorizationInput {
                code: Some("code 123".to_owned()),
                state: Some("state456".to_owned()),
            }
        );
    }

    #[test]
    fn extracts_account_id_from_token_credentials() {
        let token = test_access_token("account-123");
        let credentials = credentials_from_token(OAuthToken {
            access: token.clone(),
            refresh: "refresh-token".to_owned(),
            expires: 1234,
        })
        .expect("account id");

        assert_eq!(credentials.access, token);
        assert_eq!(credentials.refresh, "refresh-token");
        assert_eq!(credentials.expires, 1234);
        assert_eq!(
            credentials.extra.get("accountId"),
            Some(&Value::String("account-123".to_owned()))
        );
    }

    #[test]
    fn token_response_preserves_pi_expiry_math() {
        let credentials = credentials_from_token_response(
            &format!(
                r#"{{"access_token":"{}","refresh_token":"refresh","expires_in":3600}}"#,
                test_access_token("account-456")
            ),
            1_000_000,
        )
        .expect("valid json")
        .expect("account id");

        assert_eq!(credentials.refresh, "refresh");
        assert_eq!(credentials.expires, 4_600_000);
        assert_eq!(
            credentials.extra.get("accountId"),
            Some(&Value::String("account-456".to_owned()))
        );
    }

    #[test]
    fn preserves_public_constants() {
        assert_eq!(CLIENT_ID, "app_EMoamEEZ73f0CkXaXp7hrann");
        assert_eq!(REDIRECT_URI, "http://localhost:1455/auth/callback");
        assert_eq!(DEVICE_CODE_TIMEOUT_SECONDS, 900);
        assert_eq!(OPENAI_CODEX_OAUTH_PROVIDER.id(), "openai-codex");
        assert!(OPENAI_CODEX_OAUTH_PROVIDER.uses_callback_server());
    }

    #[test]
    fn to_auth_returns_access_token_as_api_key() {
        let credential = OAuthCredential {
            refresh: "refresh".to_owned(),
            access: "access".to_owned(),
            expires: now_millis(),
            extra: BTreeMap::new(),
        };

        let auth = block_on(OPENAI_CODEX_OAUTH.to_auth(&credential)).expect("to auth");
        assert_eq!(auth.api_key.as_deref(), Some("access"));
    }
}
