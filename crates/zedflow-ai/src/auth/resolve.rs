//! Auth resolution shared by Pi chat and image model registries.

use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
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
#[derive(Debug, Clone)]
pub struct ModelsError {
    code: ModelsErrorCode,
    message: String,
    source: Option<std::sync::Arc<dyn StdError + Send + Sync + 'static>>,
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
            source: Some(source.into()),
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
    pub env: Option<crate::auth::types::ProviderEnv>,
}

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
    provider: &crate::auth::types::AuthProvider,
    model: &crate::auth::types::AuthModel,
    credentials: &dyn crate::auth::types::CredentialStore,
    auth_context: &dyn crate::auth::types::AuthContext,
    overrides: Option<&AuthResolutionOverrides>,
) -> Result<Option<crate::auth::types::ResolvedAuth>> {
    use crate::auth::types::{ApiKeyCredential, Credential};

    let overlay_env = overrides.and_then(|overrides| overrides.env.as_ref());
    let request_auth_context = OverlayEnvAuthContext {
        base: auth_context,
        env: overlay_env,
    };

    if let (Some(overrides), Some(api_key_auth)) = (overrides, provider.auth.api_key.as_ref())
        && let Some(api_key) = overrides.api_key.as_ref()
    {
        return resolve_api_key(
            &request_auth_context,
            api_key_auth.as_ref(),
            model,
            Some(ApiKeyCredential {
                key: Some(api_key.clone()),
                env: overrides.env.clone(),
            }),
        )
        .await;
    }

    let stored = read_credential(credentials, &provider.id).await?;
    if let Some(stored) = stored {
        match stored {
            Credential::OAuth(stored) => {
                if let Some(oauth) = provider.auth.oauth.as_ref() {
                    return resolve_stored_oauth(credentials, &provider.id, oauth.clone(), stored)
                        .await;
                }
            }
            Credential::ApiKey(mut stored) => {
                if let Some(api_key_auth) = provider.auth.api_key.as_ref() {
                    if let Some(env) = overlay_env {
                        stored
                            .env
                            .get_or_insert_with(crate::auth::types::ProviderEnv::new)
                            .extend(env.clone());
                    }
                    return resolve_api_key(
                        &request_auth_context,
                        api_key_auth.as_ref(),
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
        resolve_api_key(&request_auth_context, api_key_auth.as_ref(), model, None).await
    } else {
        Ok(None)
    }
}

struct OverlayEnvAuthContext<'a> {
    base: &'a dyn crate::auth::types::AuthContext,
    env: Option<&'a crate::auth::types::ProviderEnv>,
}

impl crate::auth::types::AuthContext for OverlayEnvAuthContext<'_> {
    fn env<'a>(&'a self, name: &'a str) -> crate::auth::types::AuthFuture<'a, Option<String>> {
        Box::pin(async move {
            if let Some(value) = self
                .env
                .and_then(|env| env.get(name))
                .filter(|value| !value.is_empty())
                .cloned()
            {
                return Some(value);
            }
            self.base.env(name).await
        })
    }

    fn file_exists<'a>(&'a self, path: &'a str) -> crate::auth::types::AuthFuture<'a, bool> {
        self.base.file_exists(path)
    }
}

async fn resolve_stored_oauth(
    credentials: &dyn crate::auth::types::CredentialStore,
    provider_id: &str,
    oauth: std::sync::Arc<dyn crate::auth::types::OAuthAuth>,
    stored: crate::auth::types::OAuthCredential,
) -> Result<Option<crate::auth::types::ResolvedAuth>> {
    use crate::auth::types::Credential;

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
                            .refresh(&current)
                            .await
                            .map(|credential| Some(Credential::OAuth(credential)))
                            .map_err(|error| {
                                Box::new(ModelsError::with_source(
                                    ModelsErrorCode::OAuth,
                                    format!("OAuth refresh failed for {provider_id_for_refresh}"),
                                    error,
                                )) as crate::auth::types::BoxError
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
        .to_auth(&credential)
        .await
        .map(|auth| {
            Some(crate::auth::types::ResolvedAuth {
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
    auth_context: &dyn crate::auth::types::AuthContext,
    api_key: &dyn crate::auth::types::ApiKeyAuth,
    model: &crate::auth::types::AuthModel,
    credential: Option<crate::auth::types::ApiKeyCredential>,
) -> Result<Option<crate::auth::types::ResolvedAuth>> {
    api_key
        .resolve(crate::auth::types::ApiKeyResolveInput {
            model,
            ctx: auth_context,
            credential: credential.as_ref(),
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
    credentials: &dyn crate::auth::types::CredentialStore,
    provider_id: &str,
) -> Result<Option<crate::auth::types::Credential>> {
    credentials.read(provider_id).await.map_err(|error| {
        ModelsError::with_source(
            ModelsErrorCode::Auth,
            format!("Credential store read failed for {provider_id}"),
            error,
        )
    })
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
        })
}
