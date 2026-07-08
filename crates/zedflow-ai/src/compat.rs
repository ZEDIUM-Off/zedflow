//! Compatibility entrypoint preserving Pi's legacy `packages/ai/src/compat.ts` surface.
//!
//! This module ports the local registry and dispatch behavior that does not require
//! live provider calls. Builtin provider registration and generated catalog reads
//! remain documented placeholders until the provider catalog, faux provider, and
//! lazy stream modules are fully ported.

use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use crate::api::lazy::{
    Api, AssistantMessage, AssistantMessageEventStream, Context, Model, ProviderId,
};
use crate::api::simple_options::{ProviderEnv, SimpleStreamOptions, StreamOptions};
use zedflow_core::error::{Error as CoreError, PortPlaceholderError};
use zedflow_core::placeholders;

/// Result type returned by the compat registry and dispatch functions.
pub type Result<T> = std::result::Result<T, CompatError>;

/// Stream function shape used by the legacy compat API registry.
pub type ApiStreamFunction = Arc<
    dyn Fn(&Model, &Context, Option<StreamOptions>) -> Result<AssistantMessageEventStream>
        + Send
        + Sync
        + 'static,
>;

/// Simple stream function shape used by the legacy compat API registry.
pub type ApiStreamSimpleFunction = Arc<
    dyn Fn(&Model, &Context, Option<SimpleStreamOptions>) -> Result<AssistantMessageEventStream>
        + Send
        + Sync
        + 'static,
>;

/// Registered API provider exposed by Pi's compat registry.
#[derive(Clone)]
pub struct ApiProvider {
    /// API identifier handled by this provider.
    pub api: Api,
    /// Full provider stream function.
    pub stream: ApiStreamFunction,
    /// Simple provider stream function.
    pub stream_simple: ApiStreamSimpleFunction,
}

impl fmt::Debug for ApiProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiProvider")
            .field("api", &self.api)
            .field("stream", &"<stream>")
            .field("stream_simple", &"<stream_simple>")
            .finish()
    }
}

/// Placeholder registration returned by `register_faux_provider` in the TypeScript API.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/providers/faux.ts FauxProviderRegistration`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `create a faux provider, register its stream and streamSimple functions with a generated source id, expose its model/state helpers, and unregister those providers on demand`.
/// Replacement decision needed before production use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FauxProviderRegistration;

/// Options accepted by the faux provider registration placeholder.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/providers/faux.ts RegisterFauxProviderOptions`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `configure createFauxCore exactly as the TypeScript compat registerFauxProvider helper does`.
/// Replacement decision needed before production use.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegisterFauxProviderOptions;

/// Error type for legacy compat dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompatError {
    /// The requested model API does not match the registered provider API.
    MismatchedApi {
        /// API present on the model.
        actual: Api,
        /// API expected by the registered provider.
        expected: Api,
    },
    /// No provider was registered for the requested API.
    NoApiProvider {
        /// Requested API identifier.
        api: Api,
    },
    /// A provider stream ended without a final assistant message.
    MissingStreamResult,
    /// A documented port placeholder was reached.
    PortPlaceholder(PortPlaceholderError),
    /// A shared porting error without a richer compat variant.
    Porting(String),
}

impl fmt::Display for CompatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MismatchedApi { actual, expected } => {
                write!(f, "mismatched api: {actual} expected {expected}")
            }
            Self::NoApiProvider { api } => write!(f, "no API provider registered for api: {api}"),
            Self::MissingStreamResult => write!(f, "provider stream ended without a result"),
            Self::PortPlaceholder(error) => error.fmt(f),
            Self::Porting(error) => f.write_str(error),
        }
    }
}

impl StdError for CompatError {}

impl From<PortPlaceholderError> for CompatError {
    fn from(value: PortPlaceholderError) -> Self {
        Self::PortPlaceholder(value)
    }
}

impl From<CoreError> for CompatError {
    fn from(value: CoreError) -> Self {
        match value {
            CoreError::PortPlaceholder(placeholder) => Self::PortPlaceholder(placeholder),
            other => Self::Porting(other.to_string()),
        }
    }
}

#[derive(Clone)]
struct RegisteredApiProvider {
    provider: ApiProvider,
    source_id: Option<String>,
}

static API_PROVIDER_REGISTRY: OnceLock<Mutex<HashMap<Api, RegisteredApiProvider>>> =
    OnceLock::new();

fn registry() -> &'static Mutex<HashMap<Api, RegisteredApiProvider>> {
    API_PROVIDER_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn registry_lock() -> MutexGuard<'static, HashMap<Api, RegisteredApiProvider>> {
    registry()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

fn wrap_stream(api: Api, stream: ApiStreamFunction) -> ApiStreamFunction {
    Arc::new(move |model, context, options| {
        if model.api != api {
            return Err(CompatError::MismatchedApi {
                actual: model.api.clone(),
                expected: api.clone(),
            });
        }
        stream(model, context, options)
    })
}

fn wrap_stream_simple(api: Api, stream_simple: ApiStreamSimpleFunction) -> ApiStreamSimpleFunction {
    Arc::new(move |model, context, options| {
        if model.api != api {
            return Err(CompatError::MismatchedApi {
                actual: model.api.clone(),
                expected: api.clone(),
            });
        }
        stream_simple(model, context, options)
    })
}

/// Registers an API provider in the legacy compat registry.
pub fn register_api_provider(provider: ApiProvider, source_id: Option<String>) {
    let api = provider.api.clone();
    let wrapped = ApiProvider {
        api: provider.api.clone(),
        stream: wrap_stream(provider.api.clone(), provider.stream),
        stream_simple: wrap_stream_simple(provider.api, provider.stream_simple),
    };

    registry_lock().insert(
        api,
        RegisteredApiProvider {
            provider: wrapped,
            source_id,
        },
    );
}

/// Returns the provider registered for an API, if any.
#[must_use]
pub fn get_api_provider(api: &str) -> Option<ApiProvider> {
    registry_lock().get(api).map(|entry| entry.provider.clone())
}

/// Returns all registered API providers.
#[must_use]
pub fn get_api_providers() -> Vec<ApiProvider> {
    registry_lock()
        .values()
        .map(|entry| entry.provider.clone())
        .collect()
}

/// Unregisters providers that were registered with `source_id`.
pub fn unregister_api_providers(source_id: &str) {
    registry_lock().retain(|_, entry| entry.source_id.as_deref() != Some(source_id));
}

fn clear_api_providers() {
    registry_lock().clear();
}

/// Registers the faux provider helper from Pi's compat API.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/providers/faux.ts createFauxCore`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `create a faux core, register its api stream functions under a generated faux-provider source id, and return model/state helpers plus an unregister callback`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the faux provider core is ported.
pub fn register_faux_provider(
    _options: RegisterFauxProviderOptions,
) -> zedflow_core::error::Result<FauxProviderRegistration> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/providers/faux.ts createFauxCore",
        "create a faux core, register its api stream functions under a generated faux-provider source id, and return model/state helpers plus an unregister callback",
    )
}

/// Returns a builtin model from Pi's generated catalog.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/providers/all.ts getBuiltinModel`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `read the generated builtin model catalog and return the model matching provider and id`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the generated provider catalog is ported.
pub fn get_model(_provider: &str, _id: &str) -> zedflow_core::error::Result<Model> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/providers/all.ts getBuiltinModel",
        "read the generated builtin model catalog and return the model matching provider and id",
    )
}

/// Returns all builtin models from Pi's generated catalog.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/providers/all.ts getBuiltinModels`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `read and return all generated builtin models in Pi catalog order`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the generated provider catalog is ported.
pub fn get_models() -> zedflow_core::error::Result<Vec<Model>> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/providers/all.ts getBuiltinModels",
        "read and return all generated builtin models in Pi catalog order",
    )
}

/// Returns all builtin provider identifiers from Pi's generated catalog.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/providers/all.ts getBuiltinProviders`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `read and return all generated builtin providers in Pi catalog order`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the generated provider catalog is ported.
pub fn get_providers() -> zedflow_core::error::Result<Vec<ProviderId>> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/providers/all.ts getBuiltinProviders",
        "read and return all generated builtin providers in Pi catalog order",
    )
}

/// Registers Pi's builtin lazy API providers.
///
/// PORT PLACEHOLDER:
/// Original dependency: `references/pi/packages/ai/src/api/*.lazy.ts` and `references/pi/packages/ai/src/providers/all.ts builtinModels`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `register builtin API stream wrappers without clobbering overrides and remember the registered builtin provider instances for builtin model dispatch`.
/// Replacement decision needed before production use.
///
/// # Errors
///
/// Always returns a port placeholder until the builtin lazy API providers and catalog are ported.
pub fn register_built_in_api_providers() -> Result<()> {
    placeholders::unsupported(
        "references/pi/packages/ai/src/api/*.lazy.ts and references/pi/packages/ai/src/providers/all.ts builtinModels",
        "register builtin API stream wrappers without clobbering overrides and remember the registered builtin provider instances for builtin model dispatch",
    )
    .map_err(CompatError::from)
}

/// Clears all API providers and restores Pi's builtin providers.
///
/// # Errors
///
/// Returns a port placeholder until builtin provider registration is ported.
pub fn reset_api_providers() -> Result<()> {
    clear_api_providers();
    register_built_in_api_providers()
}

fn env_value(name: &str, env: &ProviderEnv) -> Option<String> {
    env.get(name)
        .filter(|value| !value.is_empty())
        .cloned()
        .or_else(|| std::env::var(name).ok().filter(|value| !value.is_empty()))
}

fn api_key_env_vars(provider: &str) -> &'static [&'static str] {
    match provider {
        "github-copilot" => &["COPILOT_GITHUB_TOKEN"],
        "anthropic" => &["ANTHROPIC_OAUTH_TOKEN", "ANTHROPIC_API_KEY"],
        "ant-ling" => &["ANT_LING_API_KEY"],
        "openai" => &["OPENAI_API_KEY"],
        "azure-openai-responses" => &["AZURE_OPENAI_API_KEY"],
        "nvidia" => &["NVIDIA_API_KEY"],
        "deepseek" => &["DEEPSEEK_API_KEY"],
        "google" => &["GEMINI_API_KEY"],
        "google-vertex" => &["GOOGLE_CLOUD_API_KEY"],
        "groq" => &["GROQ_API_KEY"],
        "cerebras" => &["CEREBRAS_API_KEY"],
        "xai" => &["XAI_API_KEY"],
        "openrouter" => &["OPENROUTER_API_KEY"],
        "vercel-ai-gateway" => &["AI_GATEWAY_API_KEY"],
        "zai" => &["ZAI_API_KEY"],
        "zai-coding-cn" => &["ZAI_CODING_CN_API_KEY"],
        "mistral" => &["MISTRAL_API_KEY"],
        "minimax" => &["MINIMAX_API_KEY"],
        "minimax-cn" => &["MINIMAX_CN_API_KEY"],
        "moonshotai" | "moonshotai-cn" => &["MOONSHOT_API_KEY"],
        "huggingface" => &["HF_TOKEN"],
        "fireworks" => &["FIREWORKS_API_KEY"],
        "together" => &["TOGETHER_API_KEY"],
        "opencode" | "opencode-go" => &["OPENCODE_API_KEY"],
        "kimi-coding" => &["KIMI_API_KEY"],
        "cloudflare-workers-ai" | "cloudflare-ai-gateway" => &["CLOUDFLARE_API_KEY"],
        "xiaomi" => &["XIAOMI_API_KEY"],
        "xiaomi-token-plan-cn" => &["XIAOMI_TOKEN_PLAN_CN_API_KEY"],
        "xiaomi-token-plan-ams" => &["XIAOMI_TOKEN_PLAN_AMS_API_KEY"],
        "xiaomi-token-plan-sgp" => &["XIAOMI_TOKEN_PLAN_SGP_API_KEY"],
        _ => &[],
    }
}

fn env_api_key(provider: &str, env: &ProviderEnv) -> Option<String> {
    api_key_env_vars(provider)
        .iter()
        .find_map(|name| env_value(name, env))
}

fn has_explicit_api_key(api_key: Option<&str>) -> bool {
    api_key.is_some_and(|api_key| !api_key.trim().is_empty())
}

fn with_env_api_key(model: &Model, options: Option<StreamOptions>) -> Option<StreamOptions> {
    if has_explicit_api_key(
        options
            .as_ref()
            .and_then(|options| options.api_key.as_deref()),
    ) {
        return options;
    }

    let env = options
        .as_ref()
        .map_or_else(ProviderEnv::new, |options| options.env.clone());
    let api_key = env_api_key(&model.provider, &env)?;
    let mut options = options.unwrap_or_default();
    options.api_key = Some(api_key);
    Some(options)
}

fn with_env_api_key_simple(
    model: &Model,
    options: Option<SimpleStreamOptions>,
) -> Option<SimpleStreamOptions> {
    if has_explicit_api_key(
        options
            .as_ref()
            .and_then(|options| options.stream.api_key.as_deref()),
    ) {
        return options;
    }

    let env = options
        .as_ref()
        .map_or_else(ProviderEnv::new, |options| options.stream.env.clone());
    let api_key = env_api_key(&model.provider, &env)?;
    let mut options = options.unwrap_or_default();
    options.stream.api_key = Some(api_key);
    Some(options)
}

fn resolve_api_provider(api: &str) -> Result<ApiProvider> {
    get_api_provider(api).ok_or_else(|| CompatError::NoApiProvider {
        api: api.to_owned(),
    })
}

/// Streams through the compat API registry.
///
/// This preserves the TypeScript fallback path for models whose API provider was
/// explicitly registered in the compat registry. Builtin model short-circuiting is
/// blocked until `providers/all.ts` is ported.
///
/// # Errors
///
/// Returns [`CompatError::NoApiProvider`] if no provider is registered, [`CompatError::MismatchedApi`]
/// if the registered provider is used with the wrong model API, or any error returned by the provider.
pub fn stream(
    model: &Model,
    context: &Context,
    options: Option<StreamOptions>,
) -> Result<AssistantMessageEventStream> {
    let provider = resolve_api_provider(&model.api)?;
    (provider.stream)(model, context, with_env_api_key(model, options))
}

/// Completes through the compat API registry and returns the final assistant message.
///
/// # Errors
///
/// Returns the same errors as [`stream`], or [`CompatError::MissingStreamResult`] if the provider
/// stream does not expose a final result.
pub fn complete(
    model: &Model,
    context: &Context,
    options: Option<StreamOptions>,
) -> Result<AssistantMessage> {
    stream(model, context, options)?
        .result()
        .cloned()
        .ok_or(CompatError::MissingStreamResult)
}

/// Streams through the compat API registry using simple stream options.
///
/// # Errors
///
/// Returns [`CompatError::NoApiProvider`] if no provider is registered, [`CompatError::MismatchedApi`]
/// if the registered provider is used with the wrong model API, or any error returned by the provider.
pub fn stream_simple(
    model: &Model,
    context: &Context,
    options: Option<SimpleStreamOptions>,
) -> Result<AssistantMessageEventStream> {
    let provider = resolve_api_provider(&model.api)?;
    (provider.stream_simple)(model, context, with_env_api_key_simple(model, options))
}

/// Completes through the compat API registry using simple stream options.
///
/// # Errors
///
/// Returns the same errors as [`stream_simple`], or [`CompatError::MissingStreamResult`] if the
/// provider stream does not expose a final result.
pub fn complete_simple(
    model: &Model,
    context: &Context,
    options: Option<SimpleStreamOptions>,
) -> Result<AssistantMessage> {
    stream_simple(model, context, options)?
        .result()
        .cloned()
        .ok_or(CompatError::MissingStreamResult)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::lazy::{AssistantContent, AssistantMessageEvent, StopReason, Usage};

    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn test_lock() -> MutexGuard<'static, ()> {
        TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    fn model() -> Model {
        Model::new("test-model", "openai-responses", "openai")
    }

    fn message(model: &Model) -> AssistantMessage {
        AssistantMessage {
            role: "assistant",
            content: vec![AssistantContent::Opaque("ok".to_owned())],
            api: model.api.clone(),
            provider: model.provider.clone(),
            model: model.id.clone(),
            usage: Usage::default(),
            stop_reason: StopReason::Stop,
            error_message: None,
            timestamp: 0,
        }
    }

    fn done_stream(model: &Model) -> AssistantMessageEventStream {
        let output = message(model);
        let mut stream = AssistantMessageEventStream::new();
        stream.push(AssistantMessageEvent::Done {
            reason: StopReason::Stop,
            message: output.clone(),
        });
        stream.end(Some(output));
        stream
    }

    #[test]
    fn complete_dispatches_unknown_providers_through_legacy_api_registry() {
        let _guard = test_lock();
        clear_api_providers();

        let captured = Arc::new(Mutex::new(None));
        let captured_stream = Arc::clone(&captured);
        let provider = ApiProvider {
            api: "openai-responses".to_owned(),
            stream: Arc::new(move |model, _, options| {
                *captured_stream
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner()) =
                    options.and_then(|options| options.api_key);
                Ok(done_stream(model))
            }),
            stream_simple: Arc::new(|model, _, _| Ok(done_stream(model))),
        };

        register_api_provider(provider, None);
        complete(
            &Model::new("test-model", "openai-responses", "custom-openai"),
            &Context,
            Some(StreamOptions {
                api_key: Some("request-key".to_owned()),
                ..StreamOptions::default()
            }),
        )
        .expect("registered provider should complete");

        assert_eq!(
            captured
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .as_deref(),
            Some("request-key")
        );
    }

    #[test]
    fn stream_injects_provider_env_api_key_when_option_key_is_absent() {
        let _guard = test_lock();
        clear_api_providers();

        let captured = Arc::new(Mutex::new(None));
        let captured_stream = Arc::clone(&captured);
        register_api_provider(
            ApiProvider {
                api: "openai-responses".to_owned(),
                stream: Arc::new(move |model, _, options| {
                    *captured_stream
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner()) =
                        options.and_then(|options| options.api_key);
                    Ok(done_stream(model))
                }),
                stream_simple: Arc::new(|model, _, _| Ok(done_stream(model))),
            },
            None,
        );

        let mut options = StreamOptions::default();
        options
            .env
            .insert("OPENAI_API_KEY".into(), "env-key".into());
        stream(&model(), &Context, Some(options)).expect("registered provider should stream");

        assert_eq!(
            captured
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .as_deref(),
            Some("env-key")
        );
    }

    #[test]
    fn unregister_api_providers_removes_matching_source_id() {
        let _guard = test_lock();
        clear_api_providers();

        register_api_provider(
            ApiProvider {
                api: "openai-responses".to_owned(),
                stream: Arc::new(|model, _, _| Ok(done_stream(model))),
                stream_simple: Arc::new(|model, _, _| Ok(done_stream(model))),
            },
            Some("source-a".to_owned()),
        );
        assert!(get_api_provider("openai-responses").is_some());

        unregister_api_providers("source-a");

        assert!(get_api_provider("openai-responses").is_none());
    }
}
