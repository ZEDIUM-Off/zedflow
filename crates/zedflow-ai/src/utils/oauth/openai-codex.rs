//! OpenAI Codex (ChatGPT OAuth) flow ported from Pi's `packages/ai/src/utils/oauth/openai-codex.ts`.

use std::collections::BTreeMap;
use std::error::Error as StdError;
use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::{Value, json};
use url::Url;

use crate::auth::types::{
    AuthEvent, AuthFuture, AuthLoginCallbacks, AuthPrompt, AuthResult, AuthSelectOption, BoxError,
    ModelAuth, OAuthAuth, OAuthCredential,
};
use crate::utils::abort_signals::{AbortController, combine_abort_signals};
use crate::utils::oauth::device_code::{
    OAuthDeviceCodePollOptions, OAuthDeviceCodePollResult, poll_oauth_device_code_flow,
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

const TOKEN_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
struct AuthorizationInput {
    code: Option<String>,
    state: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OAuthToken {
    access: String,
    refresh: String,
    expires: i64,
}

#[derive(Debug, Deserialize)]
struct JwtPayload {
    #[serde(rename = "https://api.openai.com/auth")]
    auth: Option<JwtAuthClaim>,
}

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
    /// The authorization server returned an HTTP or JSON failure.
    Http(String),
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
            Self::Http(message) => formatter.write_str(message),
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
        Box::pin(async move { refresh_openai_codex_token(&credential.refresh).await })
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
    /// Returns token-endpoint, JSON parsing, or account-id extraction failures.
    pub async fn refresh_token(self, credentials: &OAuthCredential) -> AuthResult<OAuthCredential> {
        refresh_openai_codex_token(&credentials.refresh).await
    }

    /// Converts OpenAI Codex OAuth credentials to the API key string Pi passes to requests.
    #[must_use]
    pub fn get_api_key(self, credentials: &OAuthCredential) -> &str {
        &credentials.access
    }
}

/// OpenAI Codex OAuth auth handler matching Pi's `openaiCodexOAuth` export.
pub const OPENAI_CODEX_OAUTH: OpenAiCodexOAuth = OpenAiCodexOAuth;
/// OpenAI Codex OAuth provider descriptor matching Pi's `openaiCodexOAuthProvider` export.
pub const OPENAI_CODEX_OAUTH_PROVIDER: OpenAiCodexOAuthProvider = OpenAiCodexOAuthProvider;

/// Login with OpenAI Codex OAuth using the Codex device-code flow.
///
/// # Errors
///
/// Returns device-code endpoint, polling, token-exchange, JSON parsing, or account-id failures.
pub async fn login_openai_codex_device_code(
    callbacks: &dyn AuthLoginCallbacks,
) -> AuthResult<OAuthCredential> {
    let device = start_openai_codex_device_auth().await?;
    callbacks.notify(AuthEvent::DeviceCode {
        user_code: device.user_code.clone(),
        verification_uri: DEVICE_VERIFICATION_URI.to_owned(),
        interval_seconds: Some(device.interval_seconds.max(0.0).floor() as u64),
        expires_in_seconds: Some(DEVICE_CODE_TIMEOUT_SECONDS),
    });
    let code = poll_openai_codex_device_auth(&device, callbacks.signal()).await?;
    exchange_authorization_code_for_credentials(
        &code.authorization_code,
        &code.code_verifier,
        DEVICE_REDIRECT_URI,
    )
    .await
}

/// Login with OpenAI Codex OAuth using Pi's browser/manual-code flow.
///
/// This Rust port uses the deterministic manual-code path: it emits Pi's browser URL, prompts for
/// an authorization code or redirect URL, validates state, and exchanges the code with `reqwest`.
/// Local browser callback automation remains a manual/live concern.
///
/// # Errors
///
/// Returns prompt, PKCE, state validation, token-endpoint, JSON parsing, or account-id failures.
pub async fn login_openai_codex(callbacks: &dyn AuthLoginCallbacks) -> AuthResult<OAuthCredential> {
    let flow = create_authorization_flow("pi").await?;
    callbacks.notify(AuthEvent::AuthUrl {
        url: flow.url,
        instructions: Some("A browser window should open. Complete login to finish.".to_owned()),
    });
    let prompt_controller = AbortController::new();
    let mut prompt_signal =
        combine_abort_signals(&[callbacks.signal(), Some(prompt_controller.signal())]);
    let input = callbacks
        .prompt(AuthPrompt::ManualCode {
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
        .is_some_and(|state| state != flow.state)
    {
        return Err(box_error(OpenAiCodexOAuthError::StateMismatch));
    }
    let code = parsed
        .code
        .ok_or_else(|| box_error(OpenAiCodexOAuthError::MissingAuthorizationCode))?;
    exchange_authorization_code_for_credentials(&code, &flow.verifier, REDIRECT_URI).await
}

/// Refreshes an OpenAI Codex OAuth token.
///
/// # Errors
///
/// Returns token-endpoint, JSON parsing, or account-id extraction failures.
pub async fn refresh_openai_codex_token(refresh_token: &str) -> AuthResult<OAuthCredential> {
    let token = post_openai_token_form(
        "refresh",
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", CLIENT_ID),
        ],
    )?;
    credentials_from_token(token).map_err(box_error)
}

#[derive(Debug, Clone)]
struct AuthorizationFlow {
    verifier: String,
    state: String,
    url: String,
}

#[derive(Debug, Clone, Deserialize)]
struct DeviceAuthResponse {
    device_auth_id: String,
    user_code: String,
    interval: Value,
}

#[derive(Debug, Clone)]
struct DeviceAuthInfo {
    device_auth_id: String,
    user_code: String,
    interval_seconds: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct DeviceTokenSuccessResponse {
    authorization_code: String,
    code_verifier: String,
}

#[derive(Debug, Clone)]
struct DeviceTokenSuccess {
    authorization_code: String,
    code_verifier: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
}

async fn create_authorization_flow(originator: &str) -> AuthResult<AuthorizationFlow> {
    let pkce = crate::utils::oauth::pkce::generate_pkce()
        .await
        .map_err(box_error)?;
    let state = create_state()?;
    let mut url = Url::parse(AUTHORIZE_URL).expect("OpenAI Codex authorize URL is valid");
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", CLIENT_ID)
        .append_pair("redirect_uri", REDIRECT_URI)
        .append_pair("scope", SCOPE)
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &state)
        .append_pair("id_token_add_organizations", "true")
        .append_pair("codex_cli_simplified_flow", "true")
        .append_pair("originator", originator);
    Ok(AuthorizationFlow {
        verifier: pkce.verifier,
        state,
        url: url.to_string(),
    })
}

fn create_state() -> AuthResult<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(box_error)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

async fn start_openai_codex_device_auth() -> AuthResult<DeviceAuthInfo> {
    let body = post_json(DEVICE_USER_CODE_URL, &json!({ "client_id": CLIENT_ID }))?;
    let response: DeviceAuthResponse = serde_json::from_str(&body).map_err(|error| {
        box_error(OpenAiCodexOAuthError::Http(format!(
            "Invalid OpenAI Codex device code response: {body}; details={error}"
        )))
    })?;
    let interval_seconds = interval_seconds(&response.interval).ok_or_else(|| {
        box_error(OpenAiCodexOAuthError::Http(format!(
            "Invalid OpenAI Codex device code response: {body}"
        )))
    })?;
    Ok(DeviceAuthInfo {
        device_auth_id: response.device_auth_id,
        user_code: response.user_code,
        interval_seconds,
    })
}

async fn poll_openai_codex_device_auth(
    device: &DeviceAuthInfo,
    signal: Option<crate::auth::types::AuthAbortSignal>,
) -> AuthResult<DeviceTokenSuccess> {
    let device_auth_id = device.device_auth_id.clone();
    let user_code = device.user_code.clone();
    poll_oauth_device_code_flow(OAuthDeviceCodePollOptions {
        interval_seconds: Some(device.interval_seconds),
        expires_in_seconds: Some(DEVICE_CODE_TIMEOUT_SECONDS as f64),
        wait_before_first_poll: false,
        signal,
        poll: move || {
            let device_auth_id = device_auth_id.clone();
            let user_code = user_code.clone();
            async move { poll_openai_codex_device_once(&device_auth_id, &user_code) }
        },
    })
    .await
    .map_err(box_error)
}

fn poll_openai_codex_device_once(
    device_auth_id: &str,
    user_code: &str,
) -> std::result::Result<OAuthDeviceCodePollResult<DeviceTokenSuccess>, OpenAiCodexOAuthError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(TOKEN_TIMEOUT)
        .build()
        .map_err(|error| OpenAiCodexOAuthError::Http(error.to_string()))?;
    let response = client
        .post(DEVICE_TOKEN_URL)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(&json!({ "device_auth_id": device_auth_id, "user_code": user_code }))
        .send()
        .map_err(|error| OpenAiCodexOAuthError::Http(error.to_string()))?;
    let status = response.status();
    let body = response
        .text()
        .map_err(|error| OpenAiCodexOAuthError::Http(error.to_string()))?;
    if status.is_success() {
        let json: DeviceTokenSuccessResponse = serde_json::from_str(&body).map_err(|_| {
            OpenAiCodexOAuthError::Http(format!(
                "Invalid OpenAI Codex device auth token response: {body}"
            ))
        })?;
        return Ok(OAuthDeviceCodePollResult::Complete {
            value: DeviceTokenSuccess {
                authorization_code: json.authorization_code,
                code_verifier: json.code_verifier,
            },
        });
    }
    if status.as_u16() == 403 || status.as_u16() == 404 {
        return Ok(OAuthDeviceCodePollResult::Pending);
    }
    let error_code = serde_json::from_str::<Value>(&body).ok().and_then(|json| {
        json.get("error").and_then(|error| {
            error.as_str().map(ToOwned::to_owned).or_else(|| {
                error
                    .get("code")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
        })
    });
    match error_code.as_deref() {
        Some("deviceauth_authorization_pending") => Ok(OAuthDeviceCodePollResult::Pending),
        Some("slow_down") => Ok(OAuthDeviceCodePollResult::SlowDown {
            interval_seconds: None,
        }),
        _ => Ok(OAuthDeviceCodePollResult::Failed {
            message: format!(
                "OpenAI Codex device auth failed with status {}{}",
                status.as_u16(),
                if body.is_empty() {
                    String::new()
                } else {
                    format!(": {body}")
                }
            ),
        }),
    }
}

async fn exchange_authorization_code_for_credentials(
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> AuthResult<OAuthCredential> {
    let token = post_openai_token_form(
        "exchange",
        &[
            ("grant_type", "authorization_code"),
            ("client_id", CLIENT_ID),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", verifier),
        ],
    )?;
    credentials_from_token(token).map_err(box_error)
}

fn post_openai_token_form(operation: &str, form: &[(&str, &str)]) -> AuthResult<OAuthToken> {
    let client = reqwest::blocking::Client::builder()
        .timeout(TOKEN_TIMEOUT)
        .build()
        .map_err(box_error)?;
    let response = client
        .post(TOKEN_URL)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(form_body(form))
        .send()
        .map_err(|error| {
            box_error(OpenAiCodexOAuthError::Http(format!(
                "OpenAI Codex token {operation} error: {error}"
            )))
        })?;
    read_token_response(response, operation)
}

fn read_token_response(
    response: reqwest::blocking::Response,
    operation: &str,
) -> AuthResult<OAuthToken> {
    let status = response.status();
    let status_text = status.canonical_reason().unwrap_or("");
    let body = response.text().map_err(box_error)?;
    if !status.is_success() {
        return Err(box_error(OpenAiCodexOAuthError::Http(format!(
            "OpenAI Codex token {operation} failed ({}): {}",
            status.as_u16(),
            if body.is_empty() { status_text } else { &body }
        ))));
    }
    let json: TokenResponse = serde_json::from_str(&body).map_err(|_| {
        box_error(OpenAiCodexOAuthError::Http(format!(
            "OpenAI Codex token {operation} response missing fields: {body}"
        )))
    })?;
    Ok(OAuthToken {
        access: json.access_token,
        refresh: json.refresh_token,
        expires: now_millis() + json.expires_in * 1000,
    })
}

fn post_json(url: &str, body: &Value) -> AuthResult<String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(TOKEN_TIMEOUT)
        .build()
        .map_err(box_error)?;
    let response = client
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(body)
        .send()
        .map_err(box_error)?;
    let status = response.status();
    let text = response.text().map_err(box_error)?;
    if !status.is_success() {
        if status.as_u16() == 404 {
            return Err(box_error(OpenAiCodexOAuthError::Http(
                "OpenAI Codex device code login is not enabled for this server. Use browser login or verify the server URL."
                    .to_owned(),
            )));
        }
        return Err(box_error(OpenAiCodexOAuthError::Http(format!(
            "OpenAI Codex device code request failed with status {}{}",
            status.as_u16(),
            if text.is_empty() {
                String::new()
            } else {
                format!(": {text}")
            }
        ))));
    }
    Ok(text)
}

fn form_body(form: &[(&str, &str)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer.extend_pairs(form.iter().copied());
    serializer.finish()
}

fn interval_seconds(value: &Value) -> Option<f64> {
    let seconds = value
        .as_f64()
        .or_else(|| value.as_str()?.trim().parse::<f64>().ok())?;
    (seconds.is_finite() && seconds >= 0.0).then_some(seconds)
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
            login_openai_codex_device_code(callbacks).await
        }
        OPENAI_CODEX_BROWSER_LOGIN_METHOD => login_openai_codex(callbacks).await,
        _ => Err(Box::new(OpenAiCodexOAuthError::UnknownLoginMethod(method)) as BoxError),
    }
}

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

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn box_error(error: impl StdError + Send + Sync + 'static) -> BoxError {
    Box::new(error)
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
