//! Auth helper constructors ported from Pi's `packages/ai/src/auth/helpers.ts`.

use std::collections::BTreeMap;
use std::error::Error as StdError;
use std::fmt;
use std::future::Future;
use std::sync::Arc;

use futures::future::{BoxFuture, FutureExt, Shared};
use futures::lock::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Result type used by auth helper callbacks.
pub type AuthResult<T> = std::result::Result<T, AuthCallbackError>;

/// Error returned by auth callbacks and lazy OAuth loading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthCallbackError {
    message: String,
}

impl AuthCallbackError {
    /// Creates an auth callback error from a message.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Human-readable error message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for AuthCallbackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl StdError for AuthCallbackError {}

/// Provider-scoped environment/config values.
pub type ProviderEnv = BTreeMap<String, String>;

/// Provider request headers; `None` suppresses a default header.
pub type ProviderHeaders = BTreeMap<String, Option<String>>;

/// Request auth for a model request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelAuth {
    /// API key or bearer-like token resolved for the request.
    pub api_key: Option<String>,
    /// Provider-specific headers resolved for the request.
    pub headers: Option<ProviderHeaders>,
    /// Provider-specific base URL resolved for the request.
    pub base_url: Option<String>,
}

impl ModelAuth {
    /// Creates request auth from an API key.
    #[must_use]
    pub fn api_key(api_key: impl Into<String>) -> Self {
        Self {
            api_key: Some(api_key.into()),
            headers: None,
            base_url: None,
        }
    }
}

/// Stored API-key credential.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApiKeyCredential {
    /// Stored credential key.
    pub key: Option<String>,
    /// Provider-scoped environment/config values.
    pub env: Option<ProviderEnv>,
}

/// Stored OAuth credential.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OAuthCredential {
    /// OAuth access token.
    pub access: String,
    /// OAuth refresh token.
    pub refresh: String,
    /// OAuth expiry timestamp, matching Pi's numeric `expires` field.
    pub expires: i64,
    /// Provider-specific credential fields preserved with the credential.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Result of resolving auth for a model.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedAuth {
    /// Request auth fields.
    pub auth: ModelAuth,
    /// Provider-scoped environment/config values resolved from credentials and ambient context.
    pub env: Option<ProviderEnv>,
    /// Human-readable source label for status UI.
    pub source: Option<String>,
}

/// Prompt kind shown during login.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthPromptKind {
    /// Plain text prompt.
    Text,
    /// Secret prompt.
    Secret,
    /// Selection prompt.
    Select,
    /// Manual OAuth code prompt.
    ManualCode,
}

/// Prompt shown to the user during login.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthPrompt {
    /// Prompt kind.
    pub kind: AuthPromptKind,
    /// Prompt message.
    pub message: String,
    /// Optional placeholder text.
    pub placeholder: Option<String>,
}

/// Login interaction callbacks serving API-key and OAuth flows.
pub trait AuthLoginCallbacks: Send + Sync {
    /// Prompts the user and returns the entered value.
    fn prompt<'a>(&'a self, prompt: AuthPrompt) -> BoxFuture<'a, AuthResult<String>>;

    /// Notifies the user about an auth event.
    fn notify(&self, _event: AuthEvent) {}
}

/// Auth flow notification event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthEvent {
    /// OAuth URL and optional instructions.
    AuthUrl {
        /// URL to open.
        url: String,
        /// Optional provider instructions.
        instructions: Option<String>,
    },
    /// OAuth device-code flow details.
    DeviceCode {
        /// User code to enter.
        user_code: String,
        /// Verification URI.
        verification_uri: String,
        /// Optional polling interval in seconds.
        interval_seconds: Option<u64>,
        /// Optional expiry in seconds.
        expires_in_seconds: Option<u64>,
    },
    /// Progress message.
    Progress {
        /// Human-readable progress message.
        message: String,
    },
}

/// Environment access for auth resolution.
pub trait AuthContext: Send + Sync {
    /// Reads an environment/config value by name.
    fn env<'a>(&'a self, name: &'a str) -> BoxFuture<'a, AuthResult<Option<String>>>;

    /// Checks whether a file exists. Browser contexts should return `false`.
    fn file_exists<'a>(&'a self, _path: &'a str) -> BoxFuture<'a, AuthResult<bool>> {
        async { Ok(false) }.boxed()
    }
}

/// API-key auth helper matching Pi's standard environment-variable resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiKeyAuth {
    name: String,
    env_vars: Vec<String>,
}

impl ApiKeyAuth {
    /// Display name, e.g. `Anthropic API key`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Environment variables checked after stored credentials.
    #[must_use]
    pub fn env_vars(&self) -> &[String] {
        &self.env_vars
    }

    /// Prompts for an API key and returns a stored API-key credential.
    ///
    /// # Errors
    ///
    /// Returns an error if the callback prompt rejects or is cancelled.
    pub async fn login<C>(&self, callbacks: &C) -> AuthResult<ApiKeyCredential>
    where
        C: AuthLoginCallbacks,
    {
        let key = callbacks
            .prompt(AuthPrompt {
                kind: AuthPromptKind::Secret,
                message: format!("Enter {}", self.name),
                placeholder: None,
            })
            .await?;

        Ok(ApiKeyCredential {
            key: Some(key),
            env: None,
        })
    }

    /// Resolves request auth from a stored credential, then from the first set environment variable.
    ///
    /// # Errors
    ///
    /// Returns an error if environment access fails.
    pub async fn resolve<C>(
        &self,
        ctx: &C,
        credential: Option<&ApiKeyCredential>,
    ) -> AuthResult<Option<ResolvedAuth>>
    where
        C: AuthContext,
    {
        if let Some(key) = credential
            .and_then(|credential| credential.key.as_deref())
            .filter(|key| !key.is_empty())
        {
            return Ok(Some(ResolvedAuth {
                auth: ModelAuth::api_key(key),
                env: None,
                source: Some("stored credential".to_owned()),
            }));
        }

        for env_var in &self.env_vars {
            if let Some(value) = ctx.env(env_var).await?.filter(|value| !value.is_empty()) {
                return Ok(Some(ResolvedAuth {
                    auth: ModelAuth::api_key(value),
                    env: None,
                    source: Some(env_var.clone()),
                }));
            }
        }

        Ok(None)
    }
}

/// Creates standard API-key auth where a stored credential wins, then the first set env var.
#[must_use]
pub fn env_api_key_auth<I, S>(name: impl Into<String>, env_vars: I) -> ApiKeyAuth
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    ApiKeyAuth {
        name: name.into(),
        env_vars: env_vars.into_iter().map(Into::into).collect(),
    }
}

/// OAuth auth implementation used by lazy OAuth wrappers.
pub trait OAuthAuth: Send + Sync {
    /// Display name, e.g. `Anthropic (Claude Pro/Max)`.
    fn name(&self) -> &str;

    /// Runs the login flow and returns a stored OAuth credential.
    fn login<'a>(
        &'a self,
        callbacks: &'a dyn AuthLoginCallbacks,
    ) -> BoxFuture<'a, AuthResult<OAuthCredential>>;

    /// Refreshes an OAuth credential.
    fn refresh<'a>(
        &'a self,
        credential: &'a OAuthCredential,
    ) -> BoxFuture<'a, AuthResult<OAuthCredential>>;

    /// Converts a valid OAuth credential to request auth without side effects.
    fn to_auth<'a>(
        &'a self,
        credential: &'a OAuthCredential,
    ) -> BoxFuture<'a, AuthResult<ModelAuth>>;
}

type OAuthLoader =
    Arc<dyn Fn() -> BoxFuture<'static, AuthResult<Arc<dyn OAuthAuth>>> + Send + Sync>;
type SharedOAuthLoad = Shared<BoxFuture<'static, AuthResult<Arc<dyn OAuthAuth>>>>;

/// Input for [`lazy_oauth`].
pub struct LazyOAuthInput<F> {
    /// Display name advertised before the OAuth implementation is loaded.
    pub name: String,
    /// Loader for the concrete OAuth implementation.
    pub load: F,
}

/// Lazy OAuth wrapper that loads the concrete implementation on first use.
pub struct LazyOAuth {
    name: String,
    load: OAuthLoader,
    promise: Mutex<Option<SharedOAuthLoad>>,
}

impl LazyOAuth {
    async fn loaded(&self) -> AuthResult<Arc<dyn OAuthAuth>> {
        let promise = {
            let mut promise = self.promise.lock().await;
            promise
                .get_or_insert_with(|| (self.load)().shared())
                .clone()
        };

        promise.await
    }
}

impl OAuthAuth for LazyOAuth {
    fn name(&self) -> &str {
        &self.name
    }

    fn login<'a>(
        &'a self,
        callbacks: &'a dyn AuthLoginCallbacks,
    ) -> BoxFuture<'a, AuthResult<OAuthCredential>> {
        async move { self.loaded().await?.login(callbacks).await }.boxed()
    }

    fn refresh<'a>(
        &'a self,
        credential: &'a OAuthCredential,
    ) -> BoxFuture<'a, AuthResult<OAuthCredential>> {
        async move { self.loaded().await?.refresh(credential).await }.boxed()
    }

    fn to_auth<'a>(
        &'a self,
        credential: &'a OAuthCredential,
    ) -> BoxFuture<'a, AuthResult<ModelAuth>> {
        async move { self.loaded().await?.to_auth(credential).await }.boxed()
    }
}

/// Wraps a dynamically loaded [`OAuthAuth`] so it loads on first `login`, `refresh`, or `to_auth` call.
#[must_use]
pub fn lazy_oauth<F, Fut>(input: LazyOAuthInput<F>) -> LazyOAuth
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = AuthResult<Arc<dyn OAuthAuth>>> + Send + 'static,
{
    LazyOAuth {
        name: input.name,
        load: Arc::new(move || (input.load)().boxed()),
        promise: Mutex::new(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    struct TestContext(BTreeMap<String, String>);

    impl AuthContext for TestContext {
        fn env<'a>(&'a self, name: &'a str) -> BoxFuture<'a, AuthResult<Option<String>>> {
            async move { Ok(self.0.get(name).cloned()) }.boxed()
        }
    }

    #[test]
    fn env_api_key_auth_prefers_stored_credential_then_first_set_env_var() {
        let auth = env_api_key_auth("Test key", ["EMPTY", "TEST_API_KEY"]);
        let ctx = TestContext(BTreeMap::from([
            ("EMPTY".to_owned(), String::new()),
            ("TEST_API_KEY".to_owned(), "from-env".to_owned()),
        ]));

        let stored = ApiKeyCredential {
            key: Some("from-store".to_owned()),
            env: None,
        };
        let resolved = block_on(auth.resolve(&ctx, Some(&stored)))
            .expect("stored resolution should not fail")
            .expect("stored credential should resolve");
        assert_eq!(resolved.auth.api_key.as_deref(), Some("from-store"));
        assert_eq!(resolved.source.as_deref(), Some("stored credential"));

        let resolved = block_on(auth.resolve(&ctx, None))
            .expect("env resolution should not fail")
            .expect("env key should resolve");
        assert_eq!(resolved.auth.api_key.as_deref(), Some("from-env"));
        assert_eq!(resolved.source.as_deref(), Some("TEST_API_KEY"));
    }
}
