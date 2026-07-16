//! Compatibility entrypoint preserving Pi's legacy `packages/ai/src/compat.ts` surface.
//!
//! This module ports the local registry, builtin catalog reads, faux provider registration,
//! and dispatch behavior that does not require live provider calls.

use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use crate::types::{
    Api, AssistantMessage, AssistantMessageEventStream, Context, Model, ProviderEnv, ProviderId,
    ProviderStreams, SimpleStreamOptions, StreamOptions,
};
use zedflow_core::error::{Error as CoreError, PortPlaceholderError};

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

/// Registration returned by `register_faux_provider` in the TypeScript API.
#[derive(Clone)]
pub struct FauxProviderRegistration {
    /// API id registered for the faux provider.
    pub api: Api,
    /// Faux models exposed by the registration.
    pub models: Vec<Model>,
    /// Shared faux state.
    pub state: crate::providers::faux::FauxProviderState,
    source_id: String,
    core: crate::providers::faux::FauxCore,
}

impl fmt::Debug for FauxProviderRegistration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FauxProviderRegistration")
            .field("api", &self.api)
            .field("models", &self.models)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl FauxProviderRegistration {
    /// Returns the default faux model or the requested model id.
    #[must_use]
    pub fn get_model(&self, model_id: Option<&str>) -> Option<Model> {
        self.core.get_model(model_id)
    }

    /// Replaces pending faux responses.
    pub fn set_responses(&self, responses: Vec<crate::providers::faux::FauxResponseStep>) {
        self.core.set_responses(responses);
    }

    /// Appends pending faux responses.
    pub fn append_responses(&self, responses: Vec<crate::providers::faux::FauxResponseStep>) {
        self.core.append_responses(responses);
    }

    /// Returns pending faux response count.
    #[must_use]
    pub fn get_pending_response_count(&self) -> usize {
        self.core.get_pending_response_count()
    }

    /// Unregisters the faux API provider.
    pub fn unregister(&self) {
        unregister_api_providers(&self.source_id);
    }
}

/// Options accepted by the faux provider registration helper.
pub type RegisterFauxProviderOptions = crate::providers::faux::RegisterFauxProviderOptions;

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
static BUILTIN_API_PROVIDER_INSTANCES: OnceLock<Mutex<HashMap<Api, ApiProvider>>> = OnceLock::new();
static BUILTINS_REGISTERED: OnceLock<()> = OnceLock::new();
static COMPAT_MODELS: OnceLock<crate::models::Models> = OnceLock::new();

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
    ensure_builtins_registered();
    register_api_provider_inner(provider, source_id);
}

fn register_api_provider_inner(provider: ApiProvider, source_id: Option<String>) {
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
    ensure_builtins_registered();
    registry_lock().get(api).map(|entry| entry.provider.clone())
}

/// Returns all registered API providers.
#[must_use]
pub fn get_api_providers() -> Vec<ApiProvider> {
    ensure_builtins_registered();
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

fn builtin_instances_lock() -> MutexGuard<'static, HashMap<Api, ApiProvider>> {
    BUILTIN_API_PROVIDER_INSTANCES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

fn ensure_builtins_registered() {
    BUILTINS_REGISTERED.get_or_init(|| {
        let _ = register_built_in_api_providers();
    });
}

fn builtin_apis() -> Vec<(&'static str, ProviderStreams)> {
    vec![
        (
            "anthropic-messages",
            crate::api::anthropic_messages_lazy::anthropic_messages_api(),
        ),
        (
            "openai-completions",
            crate::api::openai_completions_lazy::open_ai_completions_api(),
        ),
        (
            "openai-responses",
            crate::api::openai_responses_lazy::open_ai_responses_api(),
        ),
        (
            "openai-codex-responses",
            crate::api::openai_codex_responses_lazy::open_ai_codex_responses_api(),
        ),
        (
            "azure-openai-responses",
            crate::api::azure_openai_responses_lazy::azure_open_ai_responses_api(),
        ),
        (
            "google-generative-ai",
            crate::api::google_generative_ai_lazy::google_generative_ai_api(),
        ),
        (
            "google-vertex",
            crate::api::google_vertex_lazy::google_vertex_api(),
        ),
        (
            "mistral-conversations",
            crate::api::mistral_conversations_lazy::mistral_conversations_api(),
        ),
        (
            "bedrock-converse-stream",
            crate::api::bedrock_converse_stream_lazy::bedrock_converse_stream_api(),
        ),
    ]
}

fn api_provider_from_streams(api: &str, streams: ProviderStreams) -> ApiProvider {
    ApiProvider {
        api: api.to_owned(),
        stream: Arc::new({
            let stream = Arc::clone(&streams.stream);
            move |model, context, options| Ok(stream(model, context, options.as_ref()))
        }),
        stream_simple: Arc::new(move |model, context, options| {
            Ok((streams.stream_simple)(model, context, options.as_ref()))
        }),
    }
}

/// Registers the faux provider helper from Pi's compat API.
pub fn register_faux_provider(options: RegisterFauxProviderOptions) -> FauxProviderRegistration {
    let core = crate::providers::faux::create_faux_core(options);
    let source_id = format!("faux-provider-{}", core.api);
    let api = core.api.clone();
    register_api_provider(
        ApiProvider {
            api: api.clone(),
            stream: Arc::new({
                let core = core.clone();
                move |model, context, options| {
                    Ok(core.stream_compat(model, context, options.as_ref()))
                }
            }),
            stream_simple: Arc::new({
                let core = core.clone();
                move |model, context, options| {
                    Ok(core.stream_compat(
                        model,
                        context,
                        options.as_ref().map(|options| &options.stream),
                    ))
                }
            }),
        },
        Some(source_id.clone()),
    );
    FauxProviderRegistration {
        api,
        models: core.models.clone(),
        state: core.state.clone(),
        source_id,
        core,
    }
}

/// Returns a builtin model from Pi's generated catalog.
pub fn get_model(provider: &str, id: &str) -> zedflow_core::error::Result<Model> {
    let Some(model) = crate::providers::all::get_builtin_model(provider, id) else {
        return Err(zedflow_core::error::Error::port_placeholder(
            PortPlaceholderError::new(
                "references/pi/packages/ai/src/providers/all.ts getBuiltinModel missing row",
                "return undefined for unknown builtin models once the compat API can represent absence",
            ),
        ));
    };
    Ok(model)
}

/// Returns all builtin models from Pi's generated catalog.
pub fn get_models() -> zedflow_core::error::Result<Vec<Model>> {
    Ok(crate::providers::all::get_builtin_providers()
        .into_iter()
        .flat_map(crate::providers::all::get_builtin_models)
        .collect())
}

/// Returns Pi's supported thinking levels for a model.
#[must_use]
pub fn get_supported_thinking_levels(model: &Model) -> Vec<&'static str> {
    let Some(levels) = thinking_level_map(&model.provider, &model.id) else {
        return if model_supports_reasoning(&model.provider, &model.id) {
            vec!["off", "minimal", "low", "medium", "high"]
        } else {
            vec!["off"]
        };
    };

    supported_thinking_levels_from_map(levels)
}

type ThinkingLevelMap = &'static [(&'static str, Option<&'static str>)];
const THINKING_XHIGH_MINIMAL_COMPAT: ThinkingLevelMap =
    &[("xhigh", Some("xhigh")), ("minimal", Some("low"))];
const THINKING_FABLE_COMPAT: ThinkingLevelMap = &[("off", None), ("xhigh", Some("xhigh"))];

fn supported_thinking_levels_from_map(map: ThinkingLevelMap) -> Vec<&'static str> {
    ["off", "minimal", "low", "medium", "high", "xhigh"]
        .into_iter()
        .filter(|level| {
            let mapped = map
                .iter()
                .find(|(name, _)| name == level)
                .map(|(_, value)| *value);
            if matches!(mapped, Some(None)) {
                return false;
            }
            if *level == "xhigh" {
                return mapped.is_some();
            }
            true
        })
        .collect()
}

fn model_supports_reasoning(provider: &str, id: &str) -> bool {
    match provider {
        "anthropic" => crate::providers::anthropic_models::ANTHROPIC_MODELS
            .iter()
            .find(|model| model.id == id)
            .is_some_and(|model| model.reasoning),
        "openai" => crate::providers::openai_models::OPENAI_MODELS
            .iter()
            .find(|model| model.id == id)
            .is_some_and(|model| model.reasoning),
        "openai-codex" => crate::providers::openai_codex_models::OPENAI_CODEX_MODELS
            .iter()
            .find(|model| model.id == id)
            .is_some_and(|model| model.reasoning),
        "deepseek" => crate::providers::deepseek_models::DEEPSEEK_MODELS
            .iter()
            .find(|model| model.id == id)
            .is_some_and(|model| model.reasoning),
        "opencode" => crate::providers::opencode_models::OPENCODE_MODELS
            .iter()
            .find(|model| model.id == id)
            .is_some_and(|model| model.reasoning),
        "opencode-go" => crate::providers::opencode_go_models::OPENCODE_GO_MODELS
            .iter()
            .find(|model| model.id == id)
            .is_some_and(|model| model.reasoning),
        "moonshotai" => crate::providers::moonshotai_models::MOONSHOTAI_MODELS
            .iter()
            .find(|model| model.id == id)
            .is_some_and(|model| model.reasoning),
        "moonshotai-cn" => crate::providers::moonshotai_cn_models::MOONSHOTAI_CN_MODELS
            .iter()
            .find(|model| model.id == id)
            .is_some_and(|model| model.reasoning),
        "openrouter" => crate::providers::openrouter_models::OPENROUTER_MODELS
            .iter()
            .find(|model| model.id == id)
            .is_some_and(|model| model.reasoning),
        "amazon-bedrock" => id.contains("claude") || id.contains("deepseek"),
        _ => false,
    }
}

fn thinking_level_map(provider: &str, id: &str) -> Option<ThinkingLevelMap> {
    match provider {
        "anthropic" => crate::providers::anthropic_models::ANTHROPIC_MODELS
            .iter()
            .find(|model| model.id == id)
            .and_then(|model| model.thinking_level_map),
        "openai" => crate::providers::openai_models::OPENAI_MODELS
            .iter()
            .find(|model| model.id == id)
            .and_then(|model| model.thinking_level_map),
        "openai-codex" => crate::providers::openai_codex_models::OPENAI_CODEX_MODELS
            .iter()
            .any(|model| model.id == id)
            .then_some(THINKING_XHIGH_MINIMAL_COMPAT),
        "deepseek" => crate::providers::deepseek_models::DEEPSEEK_MODELS
            .iter()
            .find(|model| model.id == id)
            .map(|model| model.thinking_level_map),
        "opencode" => crate::providers::opencode_models::OPENCODE_MODELS
            .iter()
            .find(|model| model.id == id)
            .and_then(|model| model.thinking_level_map),
        "opencode-go" => crate::providers::opencode_go_models::OPENCODE_GO_MODELS
            .iter()
            .find(|model| model.id == id)
            .and_then(|model| model.thinking_level_map),
        "moonshotai" => crate::providers::moonshotai_models::MOONSHOTAI_MODELS
            .iter()
            .find(|model| model.id == id)
            .and_then(|model| model.thinking_level_map),
        "moonshotai-cn" => crate::providers::moonshotai_cn_models::MOONSHOTAI_CN_MODELS
            .iter()
            .find(|model| model.id == id)
            .and_then(|model| model.thinking_level_map),
        "openrouter" => crate::providers::openrouter_models::OPENROUTER_MODELS
            .iter()
            .find(|model| model.id == id)
            .and_then(|model| model.thinking_level_map),
        "amazon-bedrock" if id.contains("claude-fable-5") => Some(THINKING_FABLE_COMPAT),
        _ => None,
    }
}

/// Returns all builtin provider identifiers from Pi's generated catalog.
pub fn get_providers() -> zedflow_core::error::Result<Vec<ProviderId>> {
    Ok(crate::providers::all::get_builtin_providers()
        .into_iter()
        .map(str::to_owned)
        .collect())
}

/// Registers Pi's builtin lazy API providers without clobbering existing overrides.
pub fn register_built_in_api_providers() -> Result<()> {
    for (api, streams) in builtin_apis() {
        if !registry_lock().contains_key(api) {
            let provider = api_provider_from_streams(api, streams.clone());
            register_api_provider_inner(provider, None);
        }
        if let Some(provider) = registry_lock().get(api).map(|entry| entry.provider.clone()) {
            builtin_instances_lock().insert(api.to_owned(), provider);
        }
    }
    Ok(())
}

/// Clears all API providers and restores Pi's builtin providers.
///
/// # Errors
///
/// Returns a port placeholder until builtin provider registration is ported.
pub fn reset_api_providers() -> Result<()> {
    clear_api_providers();
    builtin_instances_lock().clear();
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
        .and_then(|options| options.env.clone())
        .unwrap_or_default();
    let Some(api_key) = env_api_key(&model.provider, &env) else {
        return options;
    };
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
        .and_then(|options| options.stream.env.clone())
        .unwrap_or_default();
    let Some(api_key) = env_api_key(&model.provider, &env) else {
        return options;
    };
    let mut options = options.unwrap_or_default();
    options.stream.api_key = Some(api_key);
    Some(options)
}

fn compat_models() -> &'static crate::models::Models {
    COMPAT_MODELS.get_or_init(crate::providers::all::builtin_models)
}

fn same_api_provider(left: &ApiProvider, right: &ApiProvider) -> bool {
    left.api == right.api
        && Arc::ptr_eq(&left.stream, &right.stream)
        && Arc::ptr_eq(&left.stream_simple, &right.stream_simple)
}

fn should_use_builtin_models(model: &Model) -> bool {
    let Some(builtin) = compat_models().get_model(&model.provider, &model.id) else {
        return false;
    };
    if builtin.api != model.api {
        return false;
    }

    let Some(registered) = get_api_provider(&model.api) else {
        return false;
    };
    builtin_instances_lock()
        .get(&model.api)
        .is_some_and(|builtin_provider| same_api_provider(&registered, builtin_provider))
}

fn resolve_api_provider(api: &str) -> Result<ApiProvider> {
    get_api_provider(api).ok_or_else(|| CompatError::NoApiProvider {
        api: api.to_owned(),
    })
}

/// Streams through Pi's builtin `Models` collection for untouched builtin API providers,
/// otherwise through the compat API registry.
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
    if should_use_builtin_models(model) {
        return Ok(stream_builtin_models(model, context, options));
    }
    let provider = resolve_api_provider(&model.api)?;
    (provider.stream)(model, context, with_env_api_key(model, options))
}

/// Completes through the compat API registry and returns the final assistant message.
///
/// # Errors
///
/// Returns the same errors as [`stream`], or [`CompatError::MissingStreamResult`] if the provider
/// stream does not expose a final result.
pub async fn complete(
    model: &Model,
    context: &Context,
    options: Option<StreamOptions>,
) -> Result<AssistantMessage> {
    Ok(stream(model, context, options)?.result().await)
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
    if should_use_builtin_models(model) {
        return Ok(stream_builtin_models_simple(model, context, options));
    }
    let provider = resolve_api_provider(&model.api)?;
    (provider.stream_simple)(model, context, with_env_api_key_simple(model, options))
}

/// Completes through the compat API registry using simple stream options.
///
/// # Errors
///
/// Returns the same errors as [`stream_simple`], or [`CompatError::MissingStreamResult`] if the
/// provider stream does not expose a final result.
pub async fn complete_simple(
    model: &Model,
    context: &Context,
    options: Option<SimpleStreamOptions>,
) -> Result<AssistantMessage> {
    Ok(stream_simple(model, context, options)?.result().await)
}

fn stream_builtin_models(
    model: &Model,
    context: &Context,
    options: Option<StreamOptions>,
) -> AssistantMessageEventStream {
    compat_models().stream(model, context, options.as_ref())
}

fn stream_builtin_models_simple(
    model: &Model,
    context: &Context,
    options: Option<SimpleStreamOptions>,
) -> AssistantMessageEventStream {
    compat_models().stream_simple(model, context, options.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        AssistantContentBlock, AssistantMessageEvent, AssistantMessageRole, DoneStopReason,
        StopReason, TextContent, TextContentType, ThinkingLevel, Usage,
    };
    use futures::executor::block_on;

    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn test_lock() -> MutexGuard<'static, ()> {
        TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    fn model() -> Model {
        Model {
            id: "test-model".into(),
            name: "test-model".into(),
            api: "openai-responses".into(),
            provider: "openai".into(),
            ..Model::default()
        }
    }

    fn message(model: &Model) -> AssistantMessage {
        AssistantMessage {
            role: AssistantMessageRole::Assistant,
            content: vec![AssistantContentBlock::Text(TextContent {
                content_type: TextContentType::Text,
                text: "ok".to_owned(),
                text_signature: None,
            })],
            api: model.api.clone(),
            provider: model.provider.clone(),
            model: model.id.clone(),
            response_model: None,
            response_id: None,
            diagnostics: None,
            usage: Usage::default(),
            stop_reason: StopReason::Stop,
            error_message: None,
            timestamp: 0,
        }
    }

    fn done_stream(model: &Model) -> AssistantMessageEventStream {
        let output = message(model);
        let stream = AssistantMessageEventStream::new();
        stream.push(AssistantMessageEvent::Done {
            reason: DoneStopReason::Stop,
            message: output.clone(),
        });
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
        block_on(complete(
            &Model {
                id: "test-model".into(),
                api: "openai-responses".into(),
                provider: "custom-openai".into(),
                ..Model::default()
            },
            &Context::default(),
            Some(StreamOptions {
                api_key: Some("request-key".to_owned()),
                ..StreamOptions::default()
            }),
        ))
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

        let options = StreamOptions {
            env: Some(HashMap::from([("OPENAI_API_KEY".into(), "env-key".into())])),
            ..StreamOptions::default()
        };
        stream(&model(), &Context::default(), Some(options))
            .expect("registered provider should stream");

        assert_eq!(
            captured
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .as_deref(),
            Some("env-key")
        );
    }

    #[test]
    fn builtin_catalog_models_short_circuit_through_models_when_registry_is_unchanged() {
        let _guard = test_lock();
        reset_api_providers().expect("builtin providers reset");
        let model = get_model("openai", "gpt-4").expect("builtin model");

        let stream = super::stream(
            &model,
            &Context::default(),
            Some(StreamOptions {
                api_key: Some("request-key".to_owned()),
                ..StreamOptions::default()
            }),
        )
        .expect("builtin stream");

        let result = block_on(stream.result());
        assert_eq!(result.stop_reason, StopReason::Error);
    }

    #[test]
    fn builtin_registry_override_disables_short_circuit_and_receives_options() {
        let _guard = test_lock();
        reset_api_providers().expect("builtin providers reset");
        let model = get_model("openai", "gpt-4").expect("builtin model");
        let captured = Arc::new(Mutex::new(None));
        let captured_stream = Arc::clone(&captured);

        register_api_provider(
            ApiProvider {
                api: model.api.clone(),
                stream: Arc::new(move |model, _, options| {
                    *captured_stream
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner()) =
                        options.and_then(|options| options.api_key);
                    Ok(done_stream(model))
                }),
                stream_simple: Arc::new(|model, _, _| Ok(done_stream(model))),
            },
            Some("override".to_owned()),
        );

        block_on(complete(
            &model,
            &Context::default(),
            Some(StreamOptions {
                api_key: Some("request-key".to_owned()),
                ..StreamOptions::default()
            }),
        ))
        .expect("override provider should complete");

        assert_eq!(
            captured
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .as_deref(),
            Some("request-key")
        );
    }

    #[test]
    fn builtin_stream_wrappers_forward_option_presence() {
        let _guard = test_lock();
        let saw_stream_options = Arc::new(Mutex::new(false));
        let saw_simple_options = Arc::new(Mutex::new(false));
        let saw_stream_options_for_stream = Arc::clone(&saw_stream_options);
        let saw_simple_options_for_stream = Arc::clone(&saw_simple_options);
        let provider = api_provider_from_streams(
            "test-api",
            ProviderStreams {
                stream: Arc::new(move |model, _, options| {
                    *saw_stream_options_for_stream
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner()) = options.is_some();
                    done_stream(model)
                }),
                stream_simple: Arc::new(move |model, _, options| {
                    *saw_simple_options_for_stream
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner()) = options.is_some();
                    done_stream(model)
                }),
            },
        );
        let model = Model {
            id: "model".into(),
            api: "test-api".into(),
            provider: "provider".into(),
            ..Model::default()
        };

        (provider.stream)(&model, &Context::default(), Some(StreamOptions::default()))
            .expect("stream");
        (provider.stream_simple)(
            &model,
            &Context::default(),
            Some(SimpleStreamOptions::default()),
        )
        .expect("stream simple");

        assert!(
            *saw_stream_options
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
        );
        assert!(
            *saw_simple_options
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
        );
    }

    #[test]
    fn complete_simple_forwards_caller_options_to_custom_provider() {
        let _guard = test_lock();
        clear_api_providers();
        let captured = Arc::new(Mutex::new(None));
        let captured_simple = Arc::clone(&captured);
        register_api_provider(
            ApiProvider {
                api: "simple-api".to_owned(),
                stream: Arc::new(|model, _, _| Ok(done_stream(model))),
                stream_simple: Arc::new(move |model, _, options| {
                    *captured_simple
                        .lock()
                        .unwrap_or_else(|poison| poison.into_inner()) = options.map(|options| {
                        (
                            options.stream.api_key.clone(),
                            options.stream.session_id.clone(),
                            options.reasoning,
                        )
                    });
                    Ok(done_stream(model))
                }),
            },
            None,
        );
        let model = Model {
            id: "model".into(),
            api: "simple-api".into(),
            provider: "custom".into(),
            ..Model::default()
        };

        block_on(complete_simple(
            &model,
            &Context::default(),
            Some(SimpleStreamOptions {
                stream: StreamOptions {
                    api_key: Some("request-key".to_owned()),
                    session_id: Some("session-1".to_owned()),
                    ..StreamOptions::default()
                },
                reasoning: Some(ThinkingLevel::High),
                thinking_budgets: None,
            }),
        ))
        .expect("simple provider should complete");

        assert_eq!(
            *captured.lock().unwrap_or_else(|poison| poison.into_inner()),
            Some((
                Some("request-key".to_owned()),
                Some("session-1".to_owned()),
                Some(ThinkingLevel::High)
            ))
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
