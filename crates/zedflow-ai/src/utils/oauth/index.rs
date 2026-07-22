//! OAuth credential management for AI providers.
//!
//! This module ports Pi's `packages/ai/src/utils/oauth/index.ts` entrypoint:
//! it re-exports provider modules and owns the mutable OAuth provider registry.

use std::error::Error as StdError;
use std::fmt;
use std::sync::{Arc, LazyLock, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::{SystemTime, UNIX_EPOCH};

pub use super::anthropic::{ANTHROPIC_OAUTH_PROVIDER, login_anthropic, refresh_anthropic_token};
pub use super::device_code::*;
pub use super::github_copilot::{
    GITHUB_COPILOT_OAUTH_PROVIDER, get_github_copilot_base_url, login_github_copilot,
    normalize_domain, refresh_github_copilot_token,
};
pub use super::openai_codex::{
    OPENAI_CODEX_BROWSER_LOGIN_METHOD, OPENAI_CODEX_DEVICE_CODE_LOGIN_METHOD,
    OPENAI_CODEX_OAUTH_PROVIDER, login_openai_codex, login_openai_codex_device_code,
    refresh_openai_codex_token,
};
#[allow(deprecated)]
pub use super::types::{
    OAuthAuthInfo, OAuthDeviceCodeInfo, OAuthLoginCallbacks, OAuthPrompt, OAuthProvider,
    OAuthSelectOption, OAuthSelectPrompt,
};

use super::anthropic::{
    ANTHROPIC_OAUTH as ANTHROPIC_AUTH, ANTHROPIC_OAUTH_NAME as ANTHROPIC_PROVIDER_NAME,
    ANTHROPIC_OAUTH_PROVIDER_ID as ANTHROPIC_PROVIDER_ID,
};
use super::github_copilot::GITHUB_COPILOT_OAUTH as GITHUB_COPILOT_AUTH;
use super::openai_codex::{
    OPENAI_CODEX_OAUTH as OPENAI_CODEX_AUTH,
    OPENAI_CODEX_OAUTH_PROVIDER_ID as OPENAI_CODEX_PROVIDER_ID,
    OPENAI_CODEX_OAUTH_PROVIDER_NAME as OPENAI_CODEX_PROVIDER_NAME,
};
use crate::{
    auth::types::{AuthFuture, AuthLoginCallbacks, AuthResult, OAuthAuth, OAuthCredential},
    types::Model,
};

/// OAuth credential shape stored by Pi OAuth providers.
pub type OAuthCredentials = OAuthCredential;

/// OAuth provider identifier.
pub type OAuthProviderId = String;

type OAuthProviderRegistry = Vec<(String, Arc<dyn OAuthProviderInterface>)>;

static OAUTH_PROVIDER_REGISTRY: LazyLock<RwLock<OAuthProviderRegistry>> =
    LazyLock::new(|| RwLock::new(built_in_oauth_providers()));

/// Rust shape for Pi's `OAuthProviderInterface` registry contract.
pub trait OAuthProviderInterface: Send + Sync {
    /// Provider id.
    fn id(&self) -> &str;

    /// Provider display name.
    fn name(&self) -> &str;

    /// Whether login uses a local callback server and supports manual code input.
    fn uses_callback_server(&self) -> bool {
        false
    }

    /// Runs the provider login flow and returns credentials to persist.
    ///
    /// # Errors
    ///
    /// Returns login, prompt, cancellation, or provider-specific failures.
    fn login<'a>(
        &'a self,
        callbacks: &'a dyn AuthLoginCallbacks,
    ) -> AuthFuture<'a, AuthResult<OAuthCredential>>;

    /// Refreshes expired credentials and returns updated credentials to persist.
    ///
    /// # Errors
    ///
    /// Returns provider refresh failures.
    fn refresh_token<'a>(
        &'a self,
        credentials: &'a OAuthCredential,
    ) -> AuthFuture<'a, AuthResult<OAuthCredential>>;

    /// Converts credentials to an API key string for the provider.
    fn get_api_key(&self, credentials: &OAuthCredential) -> String;

    /// Optionally modifies models for this provider.
    fn modify_models(&self, models: &[Model], _credentials: &OAuthCredential) -> Vec<Model> {
        models.to_vec()
    }
}

/// Deprecated OAuth provider info shape returned by Pi's compatibility helper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthProviderInfo {
    /// Provider id.
    pub id: OAuthProviderId,
    /// Provider display name.
    pub name: String,
    /// Whether the provider is available.
    pub available: bool,
}

/// API key resolved from OAuth credentials, paired with any refreshed credentials.
#[derive(Debug, Clone, PartialEq)]
pub struct OAuthApiKey {
    /// Updated credentials. This is the original credential unless refresh was required.
    pub new_credentials: OAuthCredential,
    /// API key string derived from the credentials.
    pub api_key: String,
}

/// Errors returned by the OAuth provider registry helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OAuthRegistryError {
    /// No provider is registered for the requested id.
    UnknownProvider(String),
    /// Refreshing an expired OAuth token failed.
    RefreshFailed(String),
}

impl fmt::Display for OAuthRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownProvider(provider_id) => {
                write!(formatter, "Unknown OAuth provider: {provider_id}")
            }
            Self::RefreshFailed(provider_id) => {
                write!(formatter, "Failed to refresh OAuth token for {provider_id}")
            }
        }
    }
}

impl StdError for OAuthRegistryError {}

/// Gets an OAuth provider by id.
#[must_use]
pub fn get_oauth_provider(id: &str) -> Option<Arc<dyn OAuthProviderInterface>> {
    registry_read()
        .iter()
        .find(|(provider_id, _)| provider_id == id)
        .map(|(_, provider)| Arc::clone(provider))
}

/// Registers or replaces a custom OAuth provider.
pub fn register_oauth_provider(provider: Arc<dyn OAuthProviderInterface>) {
    let id = provider.id().to_owned();
    let mut registry = registry_write();
    if let Some((_, existing)) = registry
        .iter_mut()
        .find(|(provider_id, _)| provider_id == &id)
    {
        *existing = provider;
    } else {
        registry.push((id, provider));
    }
}

/// Unregisters an OAuth provider.
///
/// If the provider is built-in, this restores the built-in implementation. Custom providers are
/// removed completely.
pub fn unregister_oauth_provider(id: &str) {
    let mut registry = registry_write();
    if let Some(built_in) = built_in_oauth_provider(id) {
        if let Some((_, existing)) = registry
            .iter_mut()
            .find(|(provider_id, _)| provider_id == id)
        {
            *existing = built_in;
        } else {
            registry.push((id.to_owned(), built_in));
        }
        return;
    }

    registry.retain(|(provider_id, _)| provider_id != id);
}

/// Resets OAuth providers to Pi's built-in provider set.
pub fn reset_oauth_providers() {
    *registry_write() = built_in_oauth_providers();
}

/// Returns all registered OAuth providers in registry insertion order.
#[must_use]
pub fn get_oauth_providers() -> Vec<Arc<dyn OAuthProviderInterface>> {
    registry_read()
        .iter()
        .map(|(_, provider)| Arc::clone(provider))
        .collect()
}

/// Returns deprecated OAuth provider info records for all registered providers.
#[deprecated(note = "use get_oauth_providers() which returns OAuthProviderInterface values")]
#[must_use]
pub fn get_oauth_provider_info_list() -> Vec<OAuthProviderInfo> {
    get_oauth_providers()
        .into_iter()
        .map(|provider| OAuthProviderInfo {
            id: provider.id().to_owned(),
            name: provider.name().to_owned(),
            available: true,
        })
        .collect()
}

/// Refreshes a token for any OAuth provider.
///
/// # Errors
///
/// Returns [`OAuthRegistryError::UnknownProvider`] when no provider is registered for `provider_id`,
/// or the selected provider's refresh failure.
pub async fn refresh_oauth_token(
    provider_id: &str,
    credentials: &OAuthCredential,
) -> AuthResult<OAuthCredential> {
    let provider = get_oauth_provider(provider_id)
        .ok_or_else(|| Box::new(OAuthRegistryError::UnknownProvider(provider_id.to_owned())))?;
    provider.refresh_token(credentials).await
}

/// Gets an API key from OAuth credentials and refreshes expired credentials first.
///
/// Returns `Ok(None)` when no credentials exist for `provider_id`, matching Pi's `null` return.
///
/// # Errors
///
/// Returns [`OAuthRegistryError::UnknownProvider`] when no provider is registered for `provider_id`,
/// or [`OAuthRegistryError::RefreshFailed`] when refreshing expired credentials fails.
pub async fn get_oauth_api_key(
    provider_id: &str,
    credentials: &std::collections::BTreeMap<String, OAuthCredential>,
) -> AuthResult<Option<OAuthApiKey>> {
    let provider = get_oauth_provider(provider_id)
        .ok_or_else(|| Box::new(OAuthRegistryError::UnknownProvider(provider_id.to_owned())))?;
    let Some(mut credential) = credentials.get(provider_id).cloned() else {
        return Ok(None);
    };

    if now_millis() >= credential.expires {
        credential = provider
            .refresh_token(&credential)
            .await
            .map_err(|_| Box::new(OAuthRegistryError::RefreshFailed(provider_id.to_owned())))?;
    }

    Ok(Some(OAuthApiKey {
        api_key: provider.get_api_key(&credential),
        new_credentials: credential,
    }))
}

#[derive(Debug, Clone, Copy, Default)]
struct AnthropicRegistryProvider;

impl OAuthProviderInterface for AnthropicRegistryProvider {
    fn id(&self) -> &str {
        ANTHROPIC_PROVIDER_ID
    }

    fn name(&self) -> &str {
        ANTHROPIC_PROVIDER_NAME
    }

    fn uses_callback_server(&self) -> bool {
        true
    }

    fn login<'a>(
        &'a self,
        callbacks: &'a dyn AuthLoginCallbacks,
    ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
        ANTHROPIC_AUTH.login(callbacks)
    }

    fn refresh_token<'a>(
        &'a self,
        credentials: &'a OAuthCredential,
    ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
        ANTHROPIC_AUTH.refresh(credentials)
    }

    fn get_api_key(&self, credentials: &OAuthCredential) -> String {
        credentials.access.clone()
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct GitHubCopilotRegistryProvider;

impl OAuthProviderInterface for GitHubCopilotRegistryProvider {
    fn id(&self) -> &str {
        "github-copilot"
    }

    fn name(&self) -> &str {
        "GitHub Copilot"
    }

    fn login<'a>(
        &'a self,
        callbacks: &'a dyn AuthLoginCallbacks,
    ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
        GITHUB_COPILOT_AUTH.login(callbacks)
    }

    fn refresh_token<'a>(
        &'a self,
        credentials: &'a OAuthCredential,
    ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
        GITHUB_COPILOT_AUTH.refresh(credentials)
    }

    fn get_api_key(&self, credentials: &OAuthCredential) -> String {
        credentials.access.clone()
    }

    fn modify_models(&self, models: &[Model], credentials: &OAuthCredential) -> Vec<Model> {
        let domain = credentials
            .extra
            .get("enterpriseUrl")
            .and_then(serde_json::Value::as_str)
            .and_then(normalize_domain);
        let base_url = get_github_copilot_base_url(Some(&credentials.access), domain.as_deref());
        let available_model_ids = credentials
            .extra
            .get("availableModelIds")
            .and_then(serde_json::Value::as_array)
            .map(|ids| {
                ids.iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<std::collections::HashSet<_>>()
            });

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

#[derive(Debug, Clone, Copy, Default)]
struct OpenAiCodexRegistryProvider;

impl OAuthProviderInterface for OpenAiCodexRegistryProvider {
    fn id(&self) -> &str {
        OPENAI_CODEX_PROVIDER_ID
    }

    fn name(&self) -> &str {
        OPENAI_CODEX_PROVIDER_NAME
    }

    fn uses_callback_server(&self) -> bool {
        true
    }

    fn login<'a>(
        &'a self,
        callbacks: &'a dyn AuthLoginCallbacks,
    ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
        OPENAI_CODEX_AUTH.login(callbacks)
    }

    fn refresh_token<'a>(
        &'a self,
        credentials: &'a OAuthCredential,
    ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
        OPENAI_CODEX_AUTH.refresh(credentials)
    }

    fn get_api_key(&self, credentials: &OAuthCredential) -> String {
        credentials.access.clone()
    }
}

fn built_in_oauth_providers() -> Vec<(String, Arc<dyn OAuthProviderInterface>)> {
    vec![
        (
            ANTHROPIC_PROVIDER_ID.to_owned(),
            Arc::new(AnthropicRegistryProvider),
        ),
        (
            "github-copilot".to_owned(),
            Arc::new(GitHubCopilotRegistryProvider),
        ),
        (
            OPENAI_CODEX_PROVIDER_ID.to_owned(),
            Arc::new(OpenAiCodexRegistryProvider),
        ),
    ]
}

fn built_in_oauth_provider(id: &str) -> Option<Arc<dyn OAuthProviderInterface>> {
    match id {
        ANTHROPIC_PROVIDER_ID => Some(Arc::new(AnthropicRegistryProvider)),
        "github-copilot" => Some(Arc::new(GitHubCopilotRegistryProvider)),
        OPENAI_CODEX_PROVIDER_ID => Some(Arc::new(OpenAiCodexRegistryProvider)),
        _ => None,
    }
}

fn registry_read() -> RwLockReadGuard<'static, Vec<(String, Arc<dyn OAuthProviderInterface>)>> {
    OAUTH_PROVIDER_REGISTRY
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn registry_write() -> RwLockWriteGuard<'static, Vec<(String, Arc<dyn OAuthProviderInterface>)>> {
    OAUTH_PROVIDER_REGISTRY
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Mutex, MutexGuard};

    use futures::executor::block_on;

    use super::*;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[derive(Debug, Clone, Copy)]
    struct StaticProvider {
        id: &'static str,
        name: &'static str,
    }

    impl OAuthProviderInterface for StaticProvider {
        fn id(&self) -> &str {
            self.id
        }

        fn name(&self) -> &str {
            self.name
        }

        fn login<'a>(
            &'a self,
            _callbacks: &'a dyn AuthLoginCallbacks,
        ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
            Box::pin(async { unreachable!("login is not used by registry tests") })
        }

        fn refresh_token<'a>(
            &'a self,
            credentials: &'a OAuthCredential,
        ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
            Box::pin(async move {
                let mut refreshed = credentials.clone();
                refreshed.access = "refreshed".to_owned();
                refreshed.expires = i64::MAX;
                Ok(refreshed)
            })
        }

        fn get_api_key(&self, credentials: &OAuthCredential) -> String {
            credentials.access.clone()
        }
    }

    #[test]
    fn returns_built_in_providers_in_pi_order() {
        let _guard = locked_registry();
        let ids = get_oauth_providers()
            .into_iter()
            .map(|provider| provider.id().to_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            ids,
            vec![
                "anthropic".to_owned(),
                "github-copilot".to_owned(),
                "openai-codex".to_owned()
            ]
        );
    }

    #[test]
    fn registers_custom_provider_and_unregisters_it() {
        let _guard = locked_registry();
        register_oauth_provider(Arc::new(StaticProvider {
            id: "custom",
            name: "Custom",
        }));

        assert_eq!(get_oauth_provider("custom").unwrap().name(), "Custom");

        unregister_oauth_provider("custom");

        assert!(get_oauth_provider("custom").is_none());
    }

    #[test]
    fn unregistering_built_in_restores_original_provider() {
        let _guard = locked_registry();
        register_oauth_provider(Arc::new(StaticProvider {
            id: "anthropic",
            name: "Replacement",
        }));
        assert_eq!(
            get_oauth_provider("anthropic").unwrap().name(),
            "Replacement"
        );

        unregister_oauth_provider("anthropic");

        assert_eq!(
            get_oauth_provider("anthropic").unwrap().name(),
            ANTHROPIC_PROVIDER_NAME
        );
    }

    #[test]
    fn get_oauth_api_key_refreshes_expired_credentials() {
        let _guard = locked_registry();
        register_oauth_provider(Arc::new(StaticProvider {
            id: "custom",
            name: "Custom",
        }));
        let mut credentials = BTreeMap::new();
        credentials.insert(
            "custom".to_owned(),
            OAuthCredential {
                refresh: "refresh".to_owned(),
                access: "expired".to_owned(),
                expires: 0,
                extra: BTreeMap::new(),
            },
        );

        let result = block_on(get_oauth_api_key("custom", &credentials))
            .unwrap()
            .expect("credentials exist");

        assert_eq!(result.api_key, "refreshed");
        assert_eq!(result.new_credentials.access, "refreshed");
    }

    fn locked_registry() -> MutexGuard<'static, ()> {
        let guard = TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        reset_oauth_providers();
        guard
    }
}
