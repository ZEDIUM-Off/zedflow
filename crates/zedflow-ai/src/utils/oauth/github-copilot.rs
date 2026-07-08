//! GitHub Copilot OAuth flow ported from Pi's `packages/ai/src/utils/oauth/github-copilot.ts`.

use std::collections::{BTreeMap, HashSet};
use std::error::Error as StdError;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;
use zedflow_core::placeholders;

use crate::auth::types::{
    AuthAbortSignal, AuthEvent, AuthFuture, AuthLoginCallbacks, AuthPrompt, AuthResult, BoxError,
    ModelAuth, OAuthAuth, OAuthCredential,
};
use crate::types::{Api, Model};

/// GitHub Copilot OAuth client id used by Pi.
pub const CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";

/// GitHub Copilot request headers used by Pi's OAuth/model policy calls.
pub const COPILOT_HEADERS: &[(&str, &str)] = &[
    ("User-Agent", "GitHubCopilotChat/0.35.0"),
    ("Editor-Version", "vscode/1.107.0"),
    ("Editor-Plugin-Version", "copilot-chat/0.35.0"),
    ("Copilot-Integration-Id", "vscode-chat"),
];

/// GitHub Copilot API version used when fetching available models.
pub const COPILOT_API_VERSION: &str = "2026-06-01";

const DEFAULT_COPILOT_BASE_URL: &str = "https://api.individual.githubcopilot.com";
const PLACEHOLDER_DEPENDENCY: &str = "fetch, AbortSignal.timeout, and pollOAuthDeviceCodeFlow from packages/ai/src/utils/oauth/device-code.ts";
const PLACEHOLDER_BEHAVIOR: &str = "perform GitHub device-code login, poll access-token responses with authorization_pending/slow_down handling, exchange GitHub access tokens for Copilot tokens, fetch selectable Copilot model ids, enable model policies, and honor AbortSignal cancellation without live provider calls in unit tests";

/// GitHub Copilot OAuth errors produced before the HTTP placeholder boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHubCopilotOAuthError {
    /// The enterprise URL/domain prompt could not be parsed as a trusted domain.
    InvalidEnterpriseDomain,
    /// The Copilot `/models` response was not the object shape Pi expects.
    InvalidCopilotModelsResponse,
}

impl fmt::Display for GitHubCopilotOAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEnterpriseDomain => f.write_str("invalid GitHub Enterprise URL/domain"),
            Self::InvalidCopilotModelsResponse => f.write_str("invalid Copilot models response"),
        }
    }
}

impl StdError for GitHubCopilotOAuthError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Urls {
    device_code_url: String,
    access_token_url: String,
    copilot_token_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    interval: Option<u64>,
    expires_in: u64,
}

/// OAuth credential shape returned by the GitHub Copilot flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotCredentials {
    /// Refresh token used to request a new Copilot token.
    pub refresh: String,
    /// Current Copilot access token.
    pub access: String,
    /// Expiry timestamp in milliseconds, with Pi's five-minute safety margin applied.
    pub expires: i64,
    /// Optional GitHub Enterprise domain.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enterprise_url: Option<String>,
    /// Account-selectable Copilot model ids. `None` represents older stored Pi credentials.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_model_ids: Option<Vec<String>>,
}

/// Device-code details shown to the user during GitHub Copilot login.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthDeviceCodeInfo {
    /// User code to enter on GitHub's verification page.
    pub user_code: String,
    /// Verification page URL.
    pub verification_uri: String,
    /// Suggested polling interval in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval_seconds: Option<u64>,
    /// Expiry time in seconds.
    pub expires_in_seconds: u64,
}

/// Text prompt used by the GitHub Copilot OAuth login flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthPrompt {
    /// Prompt message.
    pub message: String,
    /// Optional placeholder text.
    pub placeholder: Option<String>,
    /// Whether an empty answer is allowed.
    pub allow_empty: bool,
}

/// Callback contract used by the GitHub Copilot OAuth provider.
pub trait OAuthLoginCallbacks: Send + Sync {
    /// Reports device-code details to the UI.
    fn on_device_code(&self, info: OAuthDeviceCodeInfo);

    /// Prompts the user and returns the entered value.
    ///
    /// # Errors
    ///
    /// Returns prompt, cancellation, or UI failures from the implementation.
    fn on_prompt<'a>(&'a self, prompt: OAuthPrompt) -> AuthFuture<'a, AuthResult<String>>;

    /// Reports progress to the UI.
    fn on_progress(&self, _message: &str) {}

    /// Returns the cancellation signal for the whole login flow, if one exists.
    fn signal(&self) -> Option<AuthAbortSignal> {
        None
    }
}

/// Rust shape for Pi's `OAuthProviderInterface` used by GitHub Copilot.
pub trait OAuthProviderInterface: Send + Sync {
    /// Provider id.
    fn id(&self) -> &str;

    /// Display name.
    fn name(&self) -> &str;

    /// Runs provider login.
    ///
    /// # Errors
    ///
    /// Returns login, prompt, cancellation, or provider-specific failures.
    fn login<'a>(
        &'a self,
        callbacks: &'a dyn OAuthLoginCallbacks,
    ) -> AuthFuture<'a, AuthResult<CopilotCredentials>>;

    /// Refreshes provider credentials.
    ///
    /// # Errors
    ///
    /// Returns provider refresh failures.
    fn refresh_token<'a>(
        &'a self,
        credentials: &'a CopilotCredentials,
    ) -> AuthFuture<'a, AuthResult<CopilotCredentials>>;

    /// Returns the API key/access token from provider credentials.
    fn get_api_key<'a>(&self, credentials: &'a CopilotCredentials) -> &'a str;

    /// Applies per-credential Copilot base URL and available-model filtering.
    fn modify_models(
        &self,
        models: &[Model<Api>],
        credentials: &CopilotCredentials,
    ) -> Vec<Model<Api>>;
}

/// Normalizes a GitHub Enterprise URL or domain into a hostname.
#[must_use]
pub fn normalize_domain(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let candidate = if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    };
    Url::parse(&candidate)
        .ok()
        .and_then(|url| url.host_str().map(ToOwned::to_owned))
}

fn get_urls(domain: &str) -> Urls {
    Urls {
        device_code_url: format!("https://{domain}/login/device/code"),
        access_token_url: format!("https://{domain}/login/oauth/access_token"),
        copilot_token_url: format!("https://api.{domain}/copilot_internal/v2/token"),
    }
}

fn get_base_url_from_token(token: &str) -> Option<String> {
    let proxy_host = token
        .split(';')
        .find_map(|part| part.strip_prefix("proxy-ep="))?;
    let api_host = proxy_host
        .strip_prefix("proxy.")
        .map_or_else(|| proxy_host.to_owned(), |host| format!("api.{host}"));
    Some(format!("https://{api_host}"))
}

/// Returns the GitHub Copilot API base URL for a token or enterprise domain.
#[must_use]
pub fn get_github_copilot_base_url(token: Option<&str>, enterprise_domain: Option<&str>) -> String {
    if let Some(url_from_token) = token.and_then(get_base_url_from_token) {
        return url_from_token;
    }
    if let Some(domain) = enterprise_domain {
        return format!("https://copilot-api.{domain}");
    }
    DEFAULT_COPILOT_BASE_URL.to_owned()
}

fn as_object(value: &Value) -> Option<&serde_json::Map<String, Value>> {
    value.as_object()
}

fn is_selectable_copilot_model(item: &serde_json::Map<String, Value>) -> bool {
    let policy = item.get("policy").and_then(as_object);
    let capabilities = item.get("capabilities").and_then(as_object);
    let supports = capabilities
        .and_then(|capabilities| capabilities.get("supports"))
        .and_then(as_object);

    item.get("model_picker_enabled") == Some(&Value::Bool(true))
        && policy.and_then(|policy| policy.get("state").and_then(Value::as_str)) != Some("disabled")
        && supports.and_then(|supports| supports.get("tool_calls").and_then(Value::as_bool))
            != Some(false)
}

/// Parses selectable GitHub Copilot model ids from the Copilot `/models` response.
///
/// # Errors
///
/// Returns [`GitHubCopilotOAuthError::InvalidCopilotModelsResponse`] when `data` is not an array.
pub fn parse_available_copilot_model_ids(
    raw: &Value,
) -> Result<Vec<String>, GitHubCopilotOAuthError> {
    let Some(data) = raw.get("data").and_then(Value::as_array) else {
        return Err(GitHubCopilotOAuthError::InvalidCopilotModelsResponse);
    };

    Ok(data
        .iter()
        .filter_map(|raw_item| {
            let item = raw_item.as_object()?;
            let id = item.get("id")?.as_str()?;
            is_selectable_copilot_model(item).then(|| id.to_owned())
        })
        .collect())
}

async fn fetch_available_github_copilot_model_ids(
    _copilot_token: &str,
    _enterprise_domain: Option<&str>,
) -> AuthResult<Vec<String>> {
    unsupported_network()
}

async fn start_device_flow(domain: &str) -> AuthResult<DeviceCodeResponse> {
    let _urls = get_urls(domain);
    unsupported_network()
}

async fn poll_for_github_access_token(
    domain: &str,
    device: &DeviceCodeResponse,
    _signal: Option<AuthAbortSignal>,
) -> AuthResult<String> {
    let _urls = get_urls(domain);
    let _device = device;
    unsupported_network()
}

async fn refresh_github_copilot_access_token(
    refresh_token: &str,
    enterprise_domain: Option<&str>,
) -> AuthResult<CopilotCredentials> {
    let domain = enterprise_domain.unwrap_or("github.com");
    let _urls = get_urls(domain);
    let _refresh_token = refresh_token;
    unsupported_network()
}

/// Refreshes a GitHub Copilot token and account-selectable model list.
///
/// PORT PLACEHOLDER:
/// Original dependency: `fetch`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `exchange a GitHub access/refresh token for a Copilot token, subtract five minutes from expires_at, then fetch selectable models from the credential-specific Copilot API base URL`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Returns provider refresh failures or a port placeholder until the HTTP client replacement is selected.
pub async fn refresh_github_copilot_token(
    refresh_token: &str,
    enterprise_domain: Option<&str>,
) -> AuthResult<CopilotCredentials> {
    let mut credentials =
        refresh_github_copilot_access_token(refresh_token, enterprise_domain).await?;
    credentials.available_model_ids = Some(
        fetch_available_github_copilot_model_ids(&credentials.access, enterprise_domain).await?,
    );
    Ok(credentials)
}

async fn enable_all_github_copilot_models(
    token: &str,
    enterprise_domain: Option<&str>,
    callbacks: Option<&dyn OAuthLoginCallbacks>,
) {
    let _ = (token, enterprise_domain, callbacks);
}

/// Runs GitHub Copilot OAuth login using Pi's device-code flow.
///
/// PORT PLACEHOLDER:
/// Original dependency: `fetch`, DOM `AbortSignal`, and `pollOAuthDeviceCodeFlow`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `prompt for an optional GitHub Enterprise domain, start the GitHub device-code flow, notify the user with code and verification URI, poll until authorization completes, exchange for a Copilot token, enable known models, then fetch selectable model ids`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Returns invalid enterprise-domain input, prompt failures, cancellation/provider failures, or a port
/// placeholder until the HTTP/cancellation replacement is selected.
pub async fn login_github_copilot(
    callbacks: &dyn OAuthLoginCallbacks,
) -> AuthResult<CopilotCredentials> {
    let input = callbacks
        .on_prompt(OAuthPrompt {
            message: "GitHub Enterprise URL/domain (blank for github.com)".to_owned(),
            placeholder: Some("company.ghe.com".to_owned()),
            allow_empty: true,
        })
        .await?;

    let trimmed = input.trim();
    let enterprise_domain = normalize_domain(&input);
    if !trimmed.is_empty() && enterprise_domain.is_none() {
        return Err(Box::new(GitHubCopilotOAuthError::InvalidEnterpriseDomain));
    }
    let domain = enterprise_domain.as_deref().unwrap_or("github.com");

    let device = start_device_flow(domain).await?;
    callbacks.on_device_code(OAuthDeviceCodeInfo {
        user_code: device.user_code.clone(),
        verification_uri: device.verification_uri.clone(),
        interval_seconds: device.interval,
        expires_in_seconds: device.expires_in,
    });

    let github_access_token =
        poll_for_github_access_token(domain, &device, callbacks.signal()).await?;
    let mut credentials =
        refresh_github_copilot_access_token(&github_access_token, enterprise_domain.as_deref())
            .await?;

    callbacks.on_progress("Enabling models...");
    enable_all_github_copilot_models(&credentials.access, enterprise_domain.as_deref(), None).await;
    credentials.available_model_ids = Some(
        fetch_available_github_copilot_model_ids(&credentials.access, enterprise_domain.as_deref())
            .await?,
    );
    Ok(credentials)
}

fn copilot_enterprise_domain(credential: &OAuthCredential) -> Option<String> {
    credential
        .extra
        .get("enterpriseUrl")
        .and_then(Value::as_str)
        .and_then(normalize_domain)
}

fn to_oauth_credential(credentials: CopilotCredentials) -> OAuthCredential {
    let mut extra = BTreeMap::new();
    if let Some(enterprise_url) = credentials.enterprise_url {
        extra.insert("enterpriseUrl".to_owned(), Value::String(enterprise_url));
    }
    if let Some(model_ids) = credentials.available_model_ids {
        extra.insert(
            "availableModelIds".to_owned(),
            Value::Array(model_ids.into_iter().map(Value::String).collect()),
        );
    }
    OAuthCredential {
        refresh: credentials.refresh,
        access: credentials.access,
        expires: credentials.expires,
        extra,
    }
}

/// GitHub Copilot OAuth auth handler.
#[derive(Debug, Clone, Copy, Default)]
pub struct GitHubCopilotOAuth;

impl OAuthAuth for GitHubCopilotOAuth {
    fn name(&self) -> &str {
        "GitHub Copilot"
    }

    fn login<'a>(
        &'a self,
        callbacks: &'a dyn AuthLoginCallbacks,
    ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
        Box::pin(async move {
            let input = callbacks
                .prompt(AuthPrompt::Text {
                    message: "GitHub Enterprise URL/domain (blank for github.com)".to_owned(),
                    placeholder: Some("company.ghe.com".to_owned()),
                    signal: callbacks.signal(),
                })
                .await?;
            let trimmed = input.trim();
            let enterprise_domain = normalize_domain(&input);
            if !trimmed.is_empty() && enterprise_domain.is_none() {
                return Err(Box::new(GitHubCopilotOAuthError::InvalidEnterpriseDomain) as BoxError);
            }
            let domain = enterprise_domain.as_deref().unwrap_or("github.com");
            let device = start_device_flow(domain).await?;
            callbacks.notify(AuthEvent::DeviceCode {
                user_code: device.user_code.clone(),
                verification_uri: device.verification_uri.clone(),
                interval_seconds: device.interval,
                expires_in_seconds: Some(device.expires_in),
            });
            let github_access_token =
                poll_for_github_access_token(domain, &device, callbacks.signal()).await?;
            let mut credentials = refresh_github_copilot_access_token(
                &github_access_token,
                enterprise_domain.as_deref(),
            )
            .await?;
            callbacks.notify(AuthEvent::Progress {
                message: "Enabling models...".to_owned(),
            });
            enable_all_github_copilot_models(
                &credentials.access,
                enterprise_domain.as_deref(),
                None,
            )
            .await;
            credentials.available_model_ids = Some(
                fetch_available_github_copilot_model_ids(
                    &credentials.access,
                    enterprise_domain.as_deref(),
                )
                .await?,
            );
            Ok::<OAuthCredential, BoxError>(to_oauth_credential(credentials))
        })
    }

    fn refresh<'a>(
        &'a self,
        credential: &'a OAuthCredential,
    ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
        Box::pin(async move {
            let refreshed = refresh_github_copilot_token(
                &credential.refresh,
                copilot_enterprise_domain(credential).as_deref(),
            )
            .await?;
            Ok(to_oauth_credential(refreshed))
        })
    }

    fn to_auth<'a>(
        &'a self,
        credential: &'a OAuthCredential,
    ) -> AuthFuture<'a, AuthResult<ModelAuth>> {
        Box::pin(async move {
            Ok(ModelAuth {
                api_key: Some(credential.access.clone()),
                headers: None,
                base_url: Some(get_github_copilot_base_url(
                    Some(&credential.access),
                    copilot_enterprise_domain(credential).as_deref(),
                )),
            })
        })
    }
}

/// GitHub Copilot OAuth provider interface implementation.
#[derive(Debug, Clone, Copy, Default)]
pub struct GitHubCopilotOAuthProvider;

impl OAuthProviderInterface for GitHubCopilotOAuthProvider {
    fn id(&self) -> &str {
        "github-copilot"
    }

    fn name(&self) -> &str {
        "GitHub Copilot"
    }

    fn login<'a>(
        &'a self,
        callbacks: &'a dyn OAuthLoginCallbacks,
    ) -> AuthFuture<'a, AuthResult<CopilotCredentials>> {
        Box::pin(async move { login_github_copilot(callbacks).await })
    }

    fn refresh_token<'a>(
        &'a self,
        credentials: &'a CopilotCredentials,
    ) -> AuthFuture<'a, AuthResult<CopilotCredentials>> {
        Box::pin(async move {
            refresh_github_copilot_token(
                &credentials.refresh,
                credentials.enterprise_url.as_deref(),
            )
            .await
        })
    }

    fn get_api_key<'a>(&self, credentials: &'a CopilotCredentials) -> &'a str {
        &credentials.access
    }

    fn modify_models(
        &self,
        models: &[Model<Api>],
        credentials: &CopilotCredentials,
    ) -> Vec<Model<Api>> {
        let domain = credentials
            .enterprise_url
            .as_deref()
            .and_then(normalize_domain);
        let base_url = get_github_copilot_base_url(Some(&credentials.access), domain.as_deref());
        let available_model_ids = credentials
            .available_model_ids
            .as_ref()
            .map(|ids| ids.iter().map(String::as_str).collect::<HashSet<_>>());

        models
            .iter()
            .filter_map(|model| {
                if model.provider != "github-copilot" {
                    return Some(model.clone());
                }
                if available_model_ids
                    .as_ref()
                    .is_some_and(|ids| !ids.contains(model.id.as_str()))
                {
                    return None;
                }
                let mut model = model.clone();
                model.base_url = base_url.clone();
                Some(model)
            })
            .collect()
    }
}

/// Static GitHub Copilot OAuth auth handler.
pub static GITHUB_COPILOT_OAUTH: GitHubCopilotOAuth = GitHubCopilotOAuth;

/// Static GitHub Copilot OAuth provider interface.
pub static GITHUB_COPILOT_OAUTH_PROVIDER: GitHubCopilotOAuthProvider = GitHubCopilotOAuthProvider;

fn unsupported_network<T>() -> AuthResult<T> {
    Err(Box::new(placeholders::error(
        PLACEHOLDER_DEPENDENCY,
        PLACEHOLDER_BEHAVIOR,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use serde_json::json;

    #[test]
    fn normalizes_enterprise_domains_like_pi() {
        assert_eq!(
            normalize_domain(" company.ghe.com ").as_deref(),
            Some("company.ghe.com")
        );
        assert_eq!(
            normalize_domain("https://company.ghe.com/path").as_deref(),
            Some("company.ghe.com")
        );
        assert_eq!(normalize_domain(""), None);
        assert_eq!(normalize_domain("http://"), None);
    }

    #[test]
    fn resolves_copilot_base_url_from_token_enterprise_or_default() {
        assert_eq!(
            get_github_copilot_base_url(
                Some("tid=1;exp=2;proxy-ep=proxy.individual.githubcopilot.com;"),
                None,
            ),
            "https://api.individual.githubcopilot.com"
        );
        assert_eq!(
            get_github_copilot_base_url(None, Some("company.ghe.com")),
            "https://copilot-api.company.ghe.com"
        );
        assert_eq!(
            get_github_copilot_base_url(None, None),
            DEFAULT_COPILOT_BASE_URL
        );
    }

    #[test]
    fn parses_only_selectable_copilot_model_ids() {
        let raw = json!({
            "data": [
                { "id": "enabled", "model_picker_enabled": true, "policy": { "state": "enabled" }, "capabilities": { "supports": { "tool_calls": true } } },
                { "id": "no-tools", "model_picker_enabled": true, "capabilities": { "supports": { "tool_calls": false } } },
                { "id": "disabled", "model_picker_enabled": true, "policy": { "state": "disabled" } },
                { "id": "hidden", "model_picker_enabled": false }
            ]
        });

        assert_eq!(
            parse_available_copilot_model_ids(&raw).unwrap(),
            vec!["enabled"]
        );
        assert!(matches!(
            parse_available_copilot_model_ids(&json!({})),
            Err(GitHubCopilotOAuthError::InvalidCopilotModelsResponse)
        ));
    }

    #[test]
    fn provider_modifies_only_available_github_copilot_models() {
        let provider = GitHubCopilotOAuthProvider;
        let models = vec![
            model("github-copilot", "keep"),
            model("github-copilot", "drop"),
            model("openai", "other"),
        ];
        let credentials = CopilotCredentials {
            refresh: "refresh".to_owned(),
            access: "tid=1;proxy-ep=proxy.enterprise.githubcopilot.com;".to_owned(),
            expires: 0,
            enterprise_url: None,
            available_model_ids: Some(vec!["keep".to_owned()]),
        };

        let modified = provider.modify_models(&models, &credentials);

        assert_eq!(
            modified
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            vec!["keep", "other"]
        );
        assert_eq!(
            modified[0].base_url,
            "https://api.enterprise.githubcopilot.com"
        );
        assert_eq!(modified[1].base_url, "https://old.example");
    }

    #[test]
    fn oauth_to_auth_uses_token_proxy_endpoint() {
        let credential = OAuthCredential {
            refresh: "refresh".to_owned(),
            access: "tid=1;proxy-ep=proxy.individual.githubcopilot.com;".to_owned(),
            expires: 0,
            extra: BTreeMap::new(),
        };

        let auth = block_on(GitHubCopilotOAuth.to_auth(&credential)).unwrap();

        assert_eq!(auth.api_key.as_deref(), Some(credential.access.as_str()));
        assert_eq!(auth.base_url.as_deref(), Some(DEFAULT_COPILOT_BASE_URL));
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
            cost: crate::types::ModelCost {
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
}
