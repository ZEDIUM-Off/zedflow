//! Pi-compatible, side-effect-free root facade for `@earendil-works/pi-ai`.
//!
//! The TypeScript package root re-exports core types, model registries, auth helpers,
//! faux provider helpers, diagnostics, stream helpers, JSON helpers, OAuth types, and
//! retry/validation utilities.  This Rust facade mirrors that root surface with the
//! canonical Rust types and keeps legacy duplicate/runtime-drift helpers under their
//! module paths instead of flattening them here.
//!
//! JS-only observability note: Pi can use Node `registerHooks` to observe exact dynamic
//! `import()` specifiers. Rust has static linking and no equivalent runtime module-load
//! hook; use provider-free side-effect checks instead (see `tests/lazy-module-load.rs`).

pub use crate::api::anthropic_messages::{
    AnthropicEffort, AnthropicOptions, AnthropicThinkingDisplay,
};
pub use crate::api::azure_openai_responses::AzureOpenAIResponsesOptions;
pub use crate::api::bedrock_converse_stream::{BedrockOptions, BedrockThinkingDisplay};
pub use crate::api::google_generative_ai::GoogleOptions;
pub use crate::api::google_shared::GoogleThinkingLevel;
pub use crate::api::google_vertex::GoogleVertexOptions;
pub use crate::api::mistral_conversations::MistralOptions;
pub use crate::api::openai_codex_responses::{
    OpenAICodexResponsesOptions, OpenAICodexWebSocketDebugStats,
};
pub use crate::api::openai_completions::OpenAICompletionsOptions;
pub use crate::api::openai_responses::OpenAIResponsesOptions;
pub use crate::auth::context::{DefaultProviderAuthContext, default_provider_auth_context};
pub use crate::auth::credential_store::InMemoryCredentialStore;
pub use crate::auth::helpers::{
    ApiKeyAuth, AuthCallbackError, LazyOAuth, LazyOAuthInput, OAuthAuth, env_api_key_auth,
    lazy_oauth,
};
pub use crate::auth::types::{
    ApiKeyCredential, AuthAbortSignal, AuthContext, AuthEvent, AuthFuture, AuthLoginCallbacks,
    AuthModel, AuthPrompt, AuthProvider, AuthResult, AuthSelectOption, BoxError, Credential,
    CredentialModify, CredentialStore, ModelAuth, OAuthCredential, ProviderAuth, ResolvedAuth,
};
pub use crate::images_models::{
    CreateImagesProviderOptions, ImagesModels, ImagesProvider, create_images_models,
    create_images_models_with_auth_context, create_images_models_with_auth_context_and_credentials,
    create_images_models_with_credentials, create_images_provider,
};
pub use crate::models::{
    CreateProviderOptions, ModelSource, Models, Provider, ProviderApi, RefreshModelSource,
    create_models, create_models_with_auth_context,
    create_models_with_auth_context_and_credentials, create_models_with_credentials,
    create_provider,
};
pub use crate::providers::faux::*;
pub use crate::session_resources::*;
pub use crate::types::*;
pub use crate::utils::diagnostics::{
    DiagnosticDetails, ThrownError, ThrownValue, append_assistant_message_diagnostic,
    create_assistant_message_diagnostic, extract_diagnostic_error, format_thrown_value,
};
pub use crate::utils::event_stream::*;
pub use crate::utils::json_parse::*;
pub use crate::utils::oauth::types::{
    OAuthAuthInfo, OAuthCredentials, OAuthDeviceCodeInfo, OAuthLoginCallbacks as OAuthUiCallbacks,
    OAuthPrompt, OAuthProviderId, OAuthProviderInterface, OAuthSelectOption, OAuthSelectPrompt,
};
#[allow(deprecated)]
pub use crate::utils::oauth::types::{OAuthProvider, OAuthProviderInfo};
pub use crate::utils::overflow::*;
pub use crate::utils::retry::*;
pub use crate::utils::typebox_helpers::*;
pub use crate::utils::validation::*;

/// Index entrypoint name from the source package.
pub const INDEX_ENTRYPOINT: &str = "@earendil-works/pi-ai";
