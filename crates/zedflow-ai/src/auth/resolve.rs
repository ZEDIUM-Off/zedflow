//! Auth resolution shared by Pi chat and image model registries.

use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Boxed async future used by auth callbacks.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Boxed error type accepted from app-owned auth callbacks and stores.
pub type BoxError = Box<dyn StdError + Send + Sync + 'static>;

/// Provider-scoped environment/config values.
pub type ProviderEnv = HashMap<String, String>;

/// Provider HTTP headers; `None` mirrors Pi's `null` suppressing a default header.
pub type ProviderHeaders = HashMap<String, Option<String>>;

/// Error codes used by Pi model/auth resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ModelsErrorCode {
    /// Model source resolution failed.
    ModelSource,
    /// Model validation failed.
    ModelValidation,
    /// Provider execution failed.
    Provider,
    /// Streaming failed.
    Stream,
    /// API-key or credential-store auth failed.
    Auth,
    /// OAuth refresh or derivation failed.
    OAuth,
}

impl ModelsErrorCode {
    /// Returns the Pi string form for this error code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ModelSource => "model_source",
            Self::ModelValidation => "model_validation",
            Self::Provider => "provider",
            Self::Stream => "stream",
            Self::Auth => "auth",
            Self::OAuth => "oauth",
        }
    }
}

/// Error raised by model/auth resolution.
#[derive(Debug)]
pub struct ModelsError {
    code: ModelsErrorCode,
    message: String,
    source: Option<BoxError>,
}

impl ModelsError {
    /// Creates a model/auth error without a source cause.
    #[must_use]
    pub fn new(code: ModelsErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            source: None,
        }
    }

    /// Creates a model/auth error with a source cause.
    #[must_use]
    pub fn with_source(
        code: ModelsErrorCode,
        message: impl Into<String>,
        source: BoxError,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            source: Some(source),
        }
    }

    /// Returns the Pi error code.
    #[must_use]
    pub const fn code(&self) -> ModelsErrorCode {
        self.code
    }

    /// Returns the error message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ModelsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl StdError for ModelsError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_deref()
            .map(|error| error as &(dyn StdError + 'static))
    }
}

/// Result type for auth resolution.
pub type Result<T> = std::result::Result<T, ModelsError>;

/// Auth overrides supplied for a single request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthResolutionOverrides {
    /// Request-scoped API key override.
    pub api_key: Option<String>,
    /// Request-scoped provider environment overrides.
    pub env: Option<ProviderEnv>,
}

/// Model shape auth resolution receives: chat or image-generation models.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthModel {
    /// Provider identifier used for error messages and provider auth lookup.
    pub provider: String,
    /// Optional model identifier.
    pub id: String,
    /// Optional provider base URL.
    pub base_url: Option<String>,
}

/// Request auth for a single model request.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModelAuth {
    /// Resolved API key or bearer token.
    pub api_key: Option<String>,
    /// Resolved custom HTTP headers.
    pub headers: Option<ProviderHeaders>,
    /// Resolved provider base URL.
    pub base_url: Option<String>,
}

/// Stored api-key credential.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApiKeyCredential {
    /// Stored API key, if any.
    pub key: Option<String>,
    /// Stored provider-scoped environment/config values.
    pub env: Option<ProviderEnv>,
}

/// Stored OAuth credential.
#[derive(Debug, Clone, PartialEq)]
pub struct OAuthCredential {
    /// Refresh token.
    pub refresh: String,
    /// Access token.
    pub access: String,
    /// Expiry time in Unix epoch milliseconds.
    pub expires: u64,
    /// Provider-specific fields preserved from Pi's open credential shape.
    pub extra: HashMap<String, serde_json::Value>,
}

/// One type-tagged credential per provider.
#[derive(Debug, Clone, PartialEq)]
pub enum Credential {
    /// Stored api-key credential.
    ApiKey(ApiKeyCredential),
    /// Stored OAuth credential.
    OAuth(OAuthCredential),
}

/// Result of resolving auth for a model.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthResult {
    /// Request auth material.
    pub auth: ModelAuth,
    /// Provider-scoped environment/config values resolved from credentials and ambient context.
    pub env: Option<ProviderEnv>,
    /// Human-readable auth source label for status UI.
    pub source: Option<String>,
}

/// Environment and filesystem access for auth resolution.
#[derive(Clone)]
pub struct AuthContext {
    env: Arc<dyn Fn(String) -> BoxFuture<'static, Option<String>> + Send + Sync>,
    file_exists: Arc<dyn Fn(String) -> BoxFuture<'static, bool> + Send + Sync>,
}

impl AuthContext {
    /// Creates an auth context from async callbacks.
    #[must_use]
    pub fn new<Env, EnvFuture, FileExists, FileFuture>(env: Env, file_exists: FileExists) -> Self
    where
        Env: Fn(String) -> EnvFuture + Send + Sync + 'static,
        EnvFuture: Future<Output = Option<String>> + Send + 'static,
        FileExists: Fn(String) -> FileFuture + Send + Sync + 'static,
        FileFuture: Future<Output = bool> + Send + 'static,
    {
        Self {
            env: Arc::new(move |name| Box::pin(env(name))),
            file_exists: Arc::new(move |path| Box::pin(file_exists(path))),
        }
    }

    /// Reads an environment/config value.
    pub async fn env(&self, name: &str) -> Option<String> {
        (self.env)(name.to_owned()).await
    }

    /// Checks whether a file exists.
    pub async fn file_exists(&self, path: &str) -> bool {
        (self.file_exists)(path.to_owned()).await
    }
}

/// Input passed to API-key auth resolution.
#[derive(Clone)]
pub struct ApiKeyResolveInput {
    /// Model being authenticated.
    pub model: AuthModel,
    /// Auth environment/filesystem context.
    pub ctx: AuthContext,
    /// Stored or override credential, if any.
    pub credential: Option<ApiKeyCredential>,
}

type ApiKeyResolveFn = dyn Fn(ApiKeyResolveInput) -> BoxFuture<'static, std::result::Result<Option<AuthResult>, BoxError>>
    + Send
    + Sync;

/// Api-key auth handler.
#[derive(Clone)]
pub struct ApiKeyAuth {
    /// Display name, e.g. `Anthropic API key`.
    pub name: String,
    resolve: Arc<ApiKeyResolveFn>,
}

impl ApiKeyAuth {
    /// Creates an API-key auth handler.
    #[must_use]
    pub fn new<Resolve, ResolveFuture>(name: impl Into<String>, resolve: Resolve) -> Self
    where
        Resolve: Fn(ApiKeyResolveInput) -> ResolveFuture + Send + Sync + 'static,
        ResolveFuture:
            Future<Output = std::result::Result<Option<AuthResult>, BoxError>> + Send + 'static,
    {
        Self {
            name: name.into(),
            resolve: Arc::new(move |input| Box::pin(resolve(input))),
        }
    }

    /// Resolves API-key auth.
    ///
    /// # Errors
    ///
    /// Returns callback errors from the provider auth implementation.
    pub async fn resolve(
        &self,
        input: ApiKeyResolveInput,
    ) -> std::result::Result<Option<AuthResult>, BoxError> {
        (self.resolve)(input).await
    }
}

type OAuthRefreshFn = dyn Fn(OAuthCredential) -> BoxFuture<'static, std::result::Result<OAuthCredential, BoxError>>
    + Send
    + Sync;
type OAuthToAuthFn = dyn Fn(OAuthCredential) -> BoxFuture<'static, std::result::Result<ModelAuth, BoxError>>
    + Send
    + Sync;

/// OAuth auth handler.
#[derive(Clone)]
pub struct OAuthAuth {
    /// Display name, e.g. `Anthropic (Claude Pro/Max)`.
    pub name: String,
    refresh: Arc<OAuthRefreshFn>,
    to_auth: Arc<OAuthToAuthFn>,
}

impl OAuthAuth {
    /// Creates an OAuth auth handler.
    #[must_use]
    pub fn new<Refresh, RefreshFuture, ToAuth, ToAuthFuture>(
        name: impl Into<String>,
        refresh: Refresh,
        to_auth: ToAuth,
    ) -> Self
    where
        Refresh: Fn(OAuthCredential) -> RefreshFuture + Send + Sync + 'static,
        RefreshFuture:
            Future<Output = std::result::Result<OAuthCredential, BoxError>> + Send + 'static,
        ToAuth: Fn(OAuthCredential) -> ToAuthFuture + Send + Sync + 'static,
        ToAuthFuture: Future<Output = std::result::Result<ModelAuth, BoxError>> + Send + 'static,
    {
        Self {
            name: name.into(),
            refresh: Arc::new(move |credential| Box::pin(refresh(credential))),
            to_auth: Arc::new(move |credential| Box::pin(to_auth(credential))),
        }
    }

    /// Refreshes an OAuth credential.
    ///
    /// # Errors
    ///
    /// Returns callback errors from the provider OAuth implementation.
    pub async fn refresh(
        &self,
        credential: OAuthCredential,
    ) -> std::result::Result<OAuthCredential, BoxError> {
        (self.refresh)(credential).await
    }

    /// Derives request auth from an OAuth credential.
    ///
    /// # Errors
    ///
    /// Returns callback errors from the provider OAuth implementation.
    pub async fn to_auth(
        &self,
        credential: OAuthCredential,
    ) -> std::result::Result<ModelAuth, BoxError> {
        (self.to_auth)(credential).await
    }
}

/// Provider auth handlers.
#[derive(Clone, Default)]
pub struct ProviderAuth {
    /// API-key auth handler.
    pub api_key: Option<ApiKeyAuth>,
    /// OAuth auth handler.
    pub oauth: Option<OAuthAuth>,
}

/// Provider descriptor used by auth resolution.
#[derive(Clone)]
pub struct AuthProvider {
    /// Provider identifier.
    pub id: String,
    /// Provider auth handlers.
    pub auth: ProviderAuth,
}

/// Serialized credential storage keyed by provider id.
pub trait CredentialStore {
    /// Reads the stored credential, possibly expired.
    fn read<'a>(
        &'a self,
        provider_id: &'a str,
    ) -> BoxFuture<'a, std::result::Result<Option<Credential>, BoxError>>;

    /// Runs a serialized read-modify-write for a provider credential.
    fn modify<'a>(
        &'a self,
        provider_id: &'a str,
        update: CredentialModify<'a>,
    ) -> BoxFuture<'a, std::result::Result<Option<Credential>, BoxError>>;
}

/// Credential-store mutation callback.
pub type CredentialModify<'a> = Box<
    dyn FnOnce(
            Option<Credential>,
        ) -> BoxFuture<'a, std::result::Result<Option<Credential>, BoxError>>
        + Send
        + 'a,
>;

/// Auth resolution shared by the `Models` and `ImagesModels` collections.
///
/// A stored credential owns the provider: ambient/env is consulted only when
/// nothing is stored. No silent env fallback occurs after a failed refresh or
/// for a credential type without a matching handler.
///
/// # Errors
///
/// Returns [`ModelsError`] with code [`ModelsErrorCode::Auth`] for API-key or
/// credential-store failures, or [`ModelsErrorCode::OAuth`] for OAuth refresh
/// and auth-derivation failures.
pub async fn resolve_provider_auth(
    provider: &AuthProvider,
    model: &AuthModel,
    credentials: &impl CredentialStore,
    auth_context: &AuthContext,
    overrides: Option<&AuthResolutionOverrides>,
) -> Result<Option<AuthResult>> {
    let request_auth_context = overrides
        .and_then(|overrides| overrides.env.as_ref())
        .map_or_else(
            || auth_context.clone(),
            |env| overlay_env_auth_context(auth_context, env),
        );

    if let (Some(overrides), Some(api_key_auth)) = (overrides, provider.auth.api_key.as_ref()) {
        if let Some(api_key) = overrides.api_key.as_ref() {
            return resolve_api_key(
                &request_auth_context,
                api_key_auth,
                model,
                Some(ApiKeyCredential {
                    key: Some(api_key.clone()),
                    env: overrides.env.clone(),
                }),
            )
            .await;
        }
    }

    let stored = read_credential(credentials, &provider.id).await?;
    if let Some(stored) = stored {
        match stored {
            Credential::OAuth(stored) => {
                if let Some(oauth) = provider.auth.oauth.as_ref() {
                    return resolve_stored_oauth(credentials, &provider.id, oauth, stored).await;
                }
            }
            Credential::ApiKey(mut stored) => {
                if let Some(api_key_auth) = provider.auth.api_key.as_ref() {
                    if let Some(env) = overrides.and_then(|overrides| overrides.env.as_ref()) {
                        stored
                            .env
                            .get_or_insert_with(ProviderEnv::new)
                            .extend(env.clone());
                    }
                    return resolve_api_key(
                        &request_auth_context,
                        api_key_auth,
                        model,
                        Some(stored),
                    )
                    .await;
                }
            }
        }
        return Ok(None);
    }

    if let Some(api_key_auth) = provider.auth.api_key.as_ref() {
        resolve_api_key(&request_auth_context, api_key_auth, model, None).await
    } else {
        Ok(None)
    }
}

fn overlay_env_auth_context(base: &AuthContext, env: &ProviderEnv) -> AuthContext {
    let base_for_env = base.clone();
    let base_for_file_exists = base.clone();
    let env = env.clone();

    AuthContext::new(
        move |name| {
            let base = base_for_env.clone();
            let value = env.get(&name).filter(|value| !value.is_empty()).cloned();
            async move {
                match value {
                    Some(value) => Some(value),
                    None => base.env(&name).await,
                }
            }
        },
        move |path| {
            let base = base_for_file_exists.clone();
            async move { base.file_exists(&path).await }
        },
    )
}

async fn resolve_stored_oauth(
    credentials: &impl CredentialStore,
    provider_id: &str,
    oauth: &OAuthAuth,
    stored: OAuthCredential,
) -> Result<Option<AuthResult>> {
    let mut credential = stored;

    if now_millis() >= credential.expires {
        let provider_id_for_refresh = provider_id.to_owned();
        let oauth_for_refresh = oauth.clone();
        let post = credentials
            .modify(
                provider_id,
                Box::new(move |current| {
                    Box::pin(async move {
                        let Some(Credential::OAuth(current)) = current else {
                            return Ok(None);
                        };
                        if now_millis() < current.expires {
                            return Ok(None);
                        }
                        oauth_for_refresh
                            .refresh(current)
                            .await
                            .map(|credential| Some(Credential::OAuth(credential)))
                            .map_err(|error| {
                                Box::new(ModelsError::with_source(
                                    ModelsErrorCode::OAuth,
                                    format!("OAuth refresh failed for {provider_id_for_refresh}"),
                                    error,
                                )) as BoxError
                            })
                    })
                }),
            )
            .await
            .map_err(|error| match error.downcast::<ModelsError>() {
                Ok(error) => *error,
                Err(error) => ModelsError::with_source(
                    ModelsErrorCode::Auth,
                    format!("Credential store modify failed for {provider_id}"),
                    error,
                ),
            })?;

        let Some(Credential::OAuth(post)) = post else {
            return Ok(None);
        };
        credential = post;
    }

    oauth
        .to_auth(credential)
        .await
        .map(|auth| {
            Some(AuthResult {
                auth,
                env: None,
                source: Some("OAuth".to_owned()),
            })
        })
        .map_err(|error| {
            ModelsError::with_source(
                ModelsErrorCode::OAuth,
                format!("OAuth auth derivation failed for {provider_id}"),
                error,
            )
        })
}

async fn resolve_api_key(
    auth_context: &AuthContext,
    api_key: &ApiKeyAuth,
    model: &AuthModel,
    credential: Option<ApiKeyCredential>,
) -> Result<Option<AuthResult>> {
    api_key
        .resolve(ApiKeyResolveInput {
            model: model.clone(),
            ctx: auth_context.clone(),
            credential,
        })
        .await
        .map_err(|error| {
            ModelsError::with_source(
                ModelsErrorCode::Auth,
                format!("API key auth failed for provider {}", model.provider),
                error,
            )
        })
}

async fn read_credential(
    credentials: &impl CredentialStore,
    provider_id: &str,
) -> Result<Option<Credential>> {
    credentials.read(provider_id).await.map_err(|error| {
        ModelsError::with_source(
            ModelsErrorCode::Auth,
            format!("Credential store read failed for {provider_id}"),
            error,
        )
    })
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}
