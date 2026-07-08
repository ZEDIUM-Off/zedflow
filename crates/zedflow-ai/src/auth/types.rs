//! Auth type definitions ported from Pi's `packages/ai/src/auth/types.ts`.

use std::collections::BTreeMap;
use std::error::Error as StdError;
use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Boxed error accepted from app-owned auth callbacks and credential stores.
pub type BoxError = Box<dyn StdError + Send + Sync + 'static>;

/// Boxed future used by async auth traits.
pub type AuthFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Result type used by fallible auth callbacks and stores.
pub type AuthResult<T> = Result<T, BoxError>;

/// Provider-scoped environment/config values.
pub type ProviderEnv = BTreeMap<String, String>;

/// Provider HTTP headers; `None` mirrors Pi's `null` suppressing a default header.
pub type ProviderHeaders = BTreeMap<String, Option<String>>;

/// Request auth for a single model request.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelAuth {
    /// API key or bearer-like token resolved for the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Provider-specific headers resolved for the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<ProviderHeaders>,
    /// Provider-specific base URL resolved for the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

/// Stored API-key credential.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiKeyCredential {
    /// Stored API key, when configured directly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Provider-scoped environment/config values such as Cloudflare account ids.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<ProviderEnv>,
}

/// Stored OAuth credential.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OAuthCredential {
    /// Refresh token.
    pub refresh: String,
    /// Access token.
    pub access: String,
    /// Expiry timestamp, matching Pi's numeric `expires` field.
    pub expires: i64,
    /// Provider-specific OAuth fields preserved from Pi's open credential shape.
    #[serde(default, flatten, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, Value>,
}

/// One type-tagged credential per provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Credential {
    /// Stored API-key credential.
    #[serde(rename = "api_key")]
    ApiKey(ApiKeyCredential),
    /// Stored OAuth credential.
    #[serde(rename = "oauth")]
    OAuth(OAuthCredential),
}

/// Credential-store mutation callback.
pub type CredentialModify<'a> = Box<
    dyn FnOnce(Option<Credential>) -> AuthFuture<'a, AuthResult<Option<Credential>>> + Send + 'a,
>;

/// App-owned credential storage, keyed by provider id.
pub trait CredentialStore: Send + Sync {
    /// Reads the stored credential, if any.
    ///
    /// # Errors
    ///
    /// Returns storage failures from the implementation.
    fn read<'a>(&'a self, provider_id: &'a str) -> AuthFuture<'a, AuthResult<Option<Credential>>>;

    /// Runs a serialized read-modify-write for one provider id.
    ///
    /// The update callback receives the current credential and returns the new
    /// credential, or `None` to leave the entry unchanged.
    ///
    /// # Errors
    ///
    /// Returns storage failures or any error produced by the update callback.
    fn modify<'a>(
        &'a self,
        provider_id: &'a str,
        update: CredentialModify<'a>,
    ) -> AuthFuture<'a, AuthResult<Option<Credential>>>;

    /// Removes a credential, serialized against [`CredentialStore::modify`].
    ///
    /// # Errors
    ///
    /// Returns storage failures from the implementation.
    fn delete<'a>(&'a self, provider_id: &'a str) -> AuthFuture<'a, AuthResult<()>>;
}

/// Environment access for auth resolution.
pub trait AuthContext: Send + Sync {
    /// Reads an environment/config value by name.
    fn env<'a>(&'a self, name: &'a str) -> AuthFuture<'a, Option<String>>;

    /// Checks whether a file exists. Browser contexts should return `false`.
    fn file_exists<'a>(&'a self, path: &'a str) -> AuthFuture<'a, bool>;
}

/// PORT PLACEHOLDER:
/// Original dependency: DOM `AbortSignal`.
/// Reason: no Rust cancellation-token type has been selected for ported auth callbacks yet.
/// Required behavior: allow login flows and individual prompts to observe cancellation.
/// Replacement decision needed before production use.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AuthAbortSignal;

/// Prompt shown to the user during login.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthPrompt {
    /// Plain text prompt.
    Text {
        /// Prompt message.
        message: String,
        /// Optional placeholder text.
        placeholder: Option<String>,
        /// Optional per-prompt cancellation signal.
        signal: Option<AuthAbortSignal>,
    },
    /// Secret prompt.
    Secret {
        /// Prompt message.
        message: String,
        /// Optional placeholder text.
        placeholder: Option<String>,
        /// Optional per-prompt cancellation signal.
        signal: Option<AuthAbortSignal>,
    },
    /// Selection prompt.
    Select {
        /// Prompt message.
        message: String,
        /// Options displayed to the user.
        options: Vec<AuthSelectOption>,
        /// Optional per-prompt cancellation signal.
        signal: Option<AuthAbortSignal>,
    },
    /// Manual OAuth code prompt.
    ManualCode {
        /// Prompt message.
        message: String,
        /// Optional placeholder text.
        placeholder: Option<String>,
        /// Optional per-prompt cancellation signal.
        signal: Option<AuthAbortSignal>,
    },
}

/// Option shown by a selection auth prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSelectOption {
    /// Stable option id returned by the prompt callback.
    pub id: String,
    /// Human-readable option label.
    pub label: String,
    /// Optional option description.
    pub description: Option<String>,
}

/// Auth flow notification event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthEvent {
    /// OAuth URL and optional instructions.
    AuthUrl {
        /// URL to open.
        url: String,
        /// Optional provider instructions.
        #[serde(skip_serializing_if = "Option::is_none")]
        instructions: Option<String>,
    },
    /// OAuth device-code flow details.
    #[serde(rename_all = "camelCase")]
    DeviceCode {
        /// User code to enter.
        user_code: String,
        /// Verification URI.
        verification_uri: String,
        /// Optional polling interval in seconds.
        #[serde(skip_serializing_if = "Option::is_none")]
        interval_seconds: Option<u64>,
        /// Optional expiry in seconds.
        #[serde(skip_serializing_if = "Option::is_none")]
        expires_in_seconds: Option<u64>,
    },
    /// Progress message.
    Progress {
        /// Human-readable progress message.
        message: String,
    },
}

/// Login interaction callbacks serving both API-key and OAuth flows.
pub trait AuthLoginCallbacks: Send + Sync {
    /// Returns the cancellation signal for the whole login flow, if one exists.
    fn signal(&self) -> Option<AuthAbortSignal> {
        None
    }

    /// Prompts the user and returns the entered or selected string.
    ///
    /// # Errors
    ///
    /// Returns prompt cancellation, abort, or UI failures from the implementation.
    fn prompt<'a>(&'a self, prompt: AuthPrompt) -> AuthFuture<'a, AuthResult<String>>;

    /// Notifies the user about an auth event.
    fn notify(&self, event: AuthEvent);
}

/// Result of resolving auth for a model.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedAuth {
    /// Request auth material.
    pub auth: ModelAuth,
    /// Provider-scoped environment/config values resolved from credentials and ambient context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<ProviderEnv>,
    /// Human-readable label for status UI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Minimal model identity needed by auth resolution for chat and image models.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthModel {
    /// Provider identifier.
    pub provider: String,
    /// Provider API identifier.
    pub api: String,
    /// Model identifier.
    pub id: String,
    /// Optional provider base URL.
    pub base_url: Option<String>,
}

/// Input passed to API-key auth resolution.
pub struct ApiKeyResolveInput<'a> {
    /// Chat or image model being authenticated.
    pub model: &'a AuthModel,
    /// Environment and filesystem context.
    pub ctx: &'a dyn AuthContext,
    /// Stored API-key credential, when present.
    pub credential: Option<&'a ApiKeyCredential>,
}

/// API-key auth handler.
pub trait ApiKeyAuth: Send + Sync {
    /// Display name, e.g. `Anthropic API key`.
    fn name(&self) -> &str;

    /// Interactive setup for a stored API-key credential.
    ///
    /// Ambient-only providers can return `None`.
    ///
    /// # Errors
    ///
    /// Returns login, prompt, cancellation, or provider-specific failures.
    fn login<'a>(
        &'a self,
        _callbacks: &'a dyn AuthLoginCallbacks,
    ) -> AuthFuture<'a, AuthResult<Option<ApiKeyCredential>>> {
        Box::pin(async { Ok(None) })
    }

    /// Resolves request auth from stored credentials and ambient sources.
    ///
    /// # Errors
    ///
    /// Returns provider-specific auth resolution failures.
    fn resolve<'a>(
        &'a self,
        input: ApiKeyResolveInput<'a>,
    ) -> AuthFuture<'a, AuthResult<Option<ResolvedAuth>>>;
}

/// OAuth auth handler.
pub trait OAuthAuth: Send + Sync {
    /// Display name, e.g. `Anthropic (Claude Pro/Max)`.
    fn name(&self) -> &str;

    /// Runs the login flow and returns a stored OAuth credential.
    ///
    /// # Errors
    ///
    /// Returns login, prompt, cancellation, or provider-specific failures.
    fn login<'a>(
        &'a self,
        callbacks: &'a dyn AuthLoginCallbacks,
    ) -> AuthFuture<'a, AuthResult<OAuthCredential>>;

    /// Exchanges the refresh token for a fresh credential.
    ///
    /// # Errors
    ///
    /// Returns provider refresh failures such as invalid grants.
    fn refresh<'a>(
        &'a self,
        credential: &'a OAuthCredential,
    ) -> AuthFuture<'a, AuthResult<OAuthCredential>>;

    /// Derives request auth from a valid credential without side effects.
    ///
    /// # Errors
    ///
    /// Returns provider-specific auth derivation failures.
    fn to_auth<'a>(
        &'a self,
        credential: &'a OAuthCredential,
    ) -> AuthFuture<'a, AuthResult<ModelAuth>>;
}

/// Provider auth handlers. At least one of `api_key` or `oauth` should be present.
#[derive(Clone, Default)]
pub struct ProviderAuth {
    /// API-key auth handler.
    pub api_key: Option<std::sync::Arc<dyn ApiKeyAuth>>,
    /// OAuth auth handler.
    pub oauth: Option<std::sync::Arc<dyn OAuthAuth>>,
}
