//! Shared OAuth type definitions ported from Pi's `packages/ai/src/utils/oauth/types.ts`.

use crate::auth::types::{AuthFuture, AuthResult, OAuthCredential};
use crate::types::{Api, Model};
use crate::utils::abort_signals::AbortSignal;

/// Stored OAuth credentials with provider-specific extra fields preserved.
pub type OAuthCredentials = OAuthCredential;

/// OAuth provider identifier.
pub type OAuthProviderId = String;

/// Deprecated alias for [`OAuthProviderId`].
#[deprecated(note = "use OAuthProviderId instead")]
pub type OAuthProvider = OAuthProviderId;

/// Text prompt shown during an OAuth login flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthPrompt {
    /// Prompt message displayed to the user.
    pub message: String,
    /// Optional placeholder text.
    pub placeholder: Option<String>,
    /// Whether an empty response is accepted.
    pub allow_empty: Option<bool>,
}

/// Browser authorization URL and optional instructions for an OAuth flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthAuthInfo {
    /// URL to open for authorization.
    pub url: String,
    /// Optional provider-specific instructions.
    pub instructions: Option<String>,
}

/// Device-code details shown to the user during an OAuth flow.
#[derive(Debug, Clone, PartialEq)]
pub struct OAuthDeviceCodeInfo {
    /// User code to enter on the verification page.
    pub user_code: String,
    /// Verification page URL.
    pub verification_uri: String,
    /// Optional suggested polling interval, in seconds.
    pub interval_seconds: Option<f64>,
    /// Optional device-code expiry, in seconds.
    pub expires_in_seconds: Option<f64>,
}

/// One selectable OAuth login option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthSelectOption {
    /// Stable option id returned by the selector.
    pub id: String,
    /// Human-readable option label.
    pub label: String,
}

/// Selection prompt shown during an OAuth login flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthSelectPrompt {
    /// Prompt message displayed to the user.
    pub message: String,
    /// Options available for selection.
    pub options: Vec<OAuthSelectOption>,
}

/// UI callbacks used by provider OAuth login flows.
pub trait OAuthLoginCallbacks: Send + Sync {
    /// Notifies the UI about an authorization URL.
    fn on_auth(&self, info: OAuthAuthInfo);

    /// Notifies the UI about a device-code flow.
    fn on_device_code(&self, info: OAuthDeviceCodeInfo);

    /// Prompts the user for text input.
    ///
    /// # Errors
    ///
    /// Returns prompt, cancellation, or UI failures from the implementation.
    fn on_prompt<'a>(&'a self, prompt: OAuthPrompt) -> AuthFuture<'a, AuthResult<String>>;

    /// Reports progress to the UI.
    fn on_progress(&self, _message: &str) {}

    /// Prompts the user for manual OAuth code input, when supported.
    ///
    /// # Errors
    ///
    /// Returns prompt, cancellation, or UI failures from the implementation.
    fn on_manual_code_input<'a>(&'a self) -> AuthFuture<'a, AuthResult<Option<String>>> {
        Box::pin(async { Ok(None) })
    }

    /// Shows an interactive selector and returns the selected option id, or `None` on cancel.
    ///
    /// # Errors
    ///
    /// Returns prompt, cancellation, or UI failures from the implementation.
    fn on_select<'a>(
        &'a self,
        prompt: OAuthSelectPrompt,
    ) -> AuthFuture<'a, AuthResult<Option<String>>>;

    /// Returns the cancellation signal for the whole login flow, if one exists.
    fn signal(&self) -> Option<AbortSignal> {
        None
    }
}

/// Provider OAuth behavior contract.
pub trait OAuthProviderInterface: Send + Sync {
    /// Provider id.
    fn id(&self) -> &str;

    /// Display name.
    fn name(&self) -> &str;

    /// Runs the login flow and returns credentials to persist.
    ///
    /// # Errors
    ///
    /// Returns login, prompt, cancellation, or provider-specific failures.
    fn login<'a>(
        &'a self,
        callbacks: &'a dyn OAuthLoginCallbacks,
    ) -> AuthFuture<'a, AuthResult<OAuthCredentials>>;

    /// Whether login uses a local callback server and supports manual code input.
    fn uses_callback_server(&self) -> bool {
        false
    }

    /// Refreshes expired credentials and returns updated credentials to persist.
    ///
    /// # Errors
    ///
    /// Returns provider refresh failures.
    fn refresh_token<'a>(
        &'a self,
        credentials: &'a OAuthCredentials,
    ) -> AuthFuture<'a, AuthResult<OAuthCredentials>>;

    /// Converts credentials to the API key string for this provider.
    fn get_api_key<'a>(&self, credentials: &'a OAuthCredentials) -> &'a str;

    /// Optionally modifies models for this provider.
    fn modify_models(
        &self,
        models: &[Model<Api>],
        _credentials: &OAuthCredentials,
    ) -> Vec<Model<Api>> {
        models.to_vec()
    }
}

/// Deprecated compatibility shape for OAuth provider metadata.
#[deprecated(note = "use OAuthProviderInterface instead")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthProviderInfo {
    /// Provider id.
    pub id: OAuthProviderId,
    /// Display name.
    pub name: String,
    /// Whether the provider is available.
    pub available: bool,
}
