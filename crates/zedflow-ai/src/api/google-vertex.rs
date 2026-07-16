//! Google Vertex API ported from Pi.

#![allow(
    clippy::result_large_err,
    reason = "preserve the structured Pi provider error contract"
)]

use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::future::BoxFuture;
use serde_json::{Value, json};

use crate::api::google_shared::{
    Context as SharedContext, FunctionCallingConfigMode, GenerateContentChunk,
    GoogleAssistantMessageEventStream, GoogleStreamCollector, GoogleStreamFrame,
    Model as SharedModel, ModelInput, Tool, collect_google_stream, convert_messages, convert_tools,
    map_tool_choice,
};
use crate::types::{
    AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream as CanonicalEventStream,
    Context as CanonicalContext, DoneStopReason, ErrorStopReason, Model as CanonicalModel,
    ProviderResponse, ProviderStreams, SimpleStreamOptions as CanonicalSimpleOptions,
    StopReason as CanonicalStopReason, StreamOptions,
};
use crate::utils::error_body::{ProviderHttpErrorParts, ProviderServiceError};

const API_VERSION: &str = "v1";
const GCP_VERTEX_CREDENTIALS_MARKER: &str = "gcp-vertex-credentials";

/// Result type for the Google Vertex port.
pub type Result<T> = std::result::Result<T, GoogleVertexError>;

/// Errors returned by the Google Vertex port.
#[derive(Debug)]
#[non_exhaustive]
pub enum GoogleVertexError {
    /// Vertex project ID was not provided in options or environment.
    MissingProject,
    /// Vertex location was not provided in options or environment.
    MissingLocation,
    /// A provider hook rejected the request.
    Hook(crate::types::ProviderHookError),
    /// The HTTP transport failed.
    Http(reqwest::Error),
    /// Vertex returned a normalized provider service failure.
    Service(ProviderServiceError),
    /// A request header was invalid.
    InvalidHeader(String),
    /// Vertex returned malformed SSE framing or text.
    InvalidSse(String),
    /// Vertex returned malformed SSE JSON.
    InvalidResponse(serde_json::Error),
    /// Canonical message/event conversion failed.
    InvalidCanonicalEvent(serde_json::Error),
    /// Application Default Credentials could not produce an access token.
    AdcAuth(gcp_auth::Error),
    /// The request was aborted.
    Aborted,
}

impl PartialEq for GoogleVertexError {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::MissingProject, Self::MissingProject)
                | (Self::MissingLocation, Self::MissingLocation)
        )
    }
}

impl Eq for GoogleVertexError {}

impl fmt::Display for GoogleVertexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingProject => f.write_str(
                "Vertex AI requires a project ID. Set GOOGLE_CLOUD_PROJECT/GCLOUD_PROJECT or pass project in options.",
            ),
            Self::MissingLocation => f.write_str(
                "Vertex AI requires a location. Set GOOGLE_CLOUD_LOCATION or pass location in options.",
            ),
            Self::Hook(error) => error.fmt(f),
            Self::Http(error) => error.fmt(f),
            Self::Service(error) => error.fmt(f),
            Self::InvalidHeader(error) => f.write_str(error),
            Self::InvalidSse(error) => write!(f, "invalid Google Vertex SSE response: {error}"),
            Self::InvalidResponse(error) => write!(f, "invalid Google Vertex SSE response: {error}"),
            Self::InvalidCanonicalEvent(error) => {
                write!(f, "invalid canonical Google Vertex event: {error}")
            }
            Self::AdcAuth(error) => write!(f, "Google Vertex ADC authentication failed: {error}"),
            Self::Aborted => f.write_str("request aborted"),
        }
    }
}

impl StdError for GoogleVertexError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Hook(error) => Some(error),
            Self::Http(error) => Some(error),
            Self::Service(error) => Some(error),
            Self::InvalidResponse(error) | Self::InvalidCanonicalEvent(error) => Some(error),
            Self::AdcAuth(error) => Some(error),
            Self::MissingProject
            | Self::MissingLocation
            | Self::InvalidHeader(_)
            | Self::InvalidSse(_)
            | Self::Aborted => None,
        }
    }
}

/// HTTP headers supplied by a model or request options.
pub type ProviderHeaders = HashMap<String, String>;

/// Provider environment overrides used before process environment access.
pub type ProviderEnv = HashMap<String, String>;

/// Token budgets for Google thinking levels.
pub type ThinkingBudgets = HashMap<ClampedThinkingLevel, i32>;

/// Pi thinking level accepted by simple stream options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThinkingLevel {
    /// Minimal reasoning effort.
    Minimal,
    /// Low reasoning effort.
    Low,
    /// Medium reasoning effort.
    Medium,
    /// High reasoning effort.
    High,
    /// Extra-high reasoning effort; clamped to high for Google.
    XHigh,
}

/// Google thinking level values mirrored from `@google/genai`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GoogleThinkingLevel {
    /// Google's unspecified sentinel.
    ThinkingLevelUnspecified,
    /// Minimal thinking.
    Minimal,
    /// Low thinking.
    Low,
    /// Medium thinking.
    Medium,
    /// High thinking.
    High,
}

impl GoogleThinkingLevel {
    /// Returns the exact Google enum string used by Pi.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ThinkingLevelUnspecified => "THINKING_LEVEL_UNSPECIFIED",
            Self::Minimal => "MINIMAL",
            Self::Low => "LOW",
            Self::Medium => "MEDIUM",
            Self::High => "HIGH",
        }
    }
}

impl fmt::Display for GoogleThinkingLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Google thinking levels after Pi clamps unsupported `xhigh`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClampedThinkingLevel {
    /// Minimal reasoning effort.
    Minimal,
    /// Low reasoning effort.
    Low,
    /// Medium reasoning effort.
    Medium,
    /// High reasoning effort.
    High,
}

impl From<ThinkingLevel> for ClampedThinkingLevel {
    fn from(value: ThinkingLevel) -> Self {
        match value {
            ThinkingLevel::Minimal => Self::Minimal,
            ThinkingLevel::Low => Self::Low,
            ThinkingLevel::Medium => Self::Medium,
            ThinkingLevel::High | ThinkingLevel::XHigh => Self::High,
        }
    }
}

/// Google tool choice behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GoogleToolChoice {
    /// Let Google choose whether to call a tool.
    Auto,
    /// Disable tool use.
    None,
    /// Force some tool use.
    Any,
}

/// Minimal model shape consumed by this port row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    /// Model identifier from Pi.
    pub id: String,
    /// Provider identifier from Pi.
    pub provider: String,
    /// Optional provider base URL.
    pub base_url: Option<String>,
    /// Whether the model supports reasoning options.
    pub reasoning: bool,
    /// Provider default headers.
    pub headers: ProviderHeaders,
}

/// Minimal context shape consumed by this port row.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Context {
    /// Optional system prompt.
    pub system_prompt: Option<String>,
    /// Conversation messages.
    pub messages: Vec<crate::api::google_shared::Message>,
    /// Available tools.
    pub tools: Vec<Tool>,
}

/// Prepared Vertex request plus deterministic streamed fixture output.
#[derive(Debug, Clone, PartialEq)]
pub struct AssistantMessageEventStream {
    /// HTTP request that the reqwest fallback would send.
    pub request: PreparedGoogleVertexRequest,
    /// Collected stream events when deterministic chunks are supplied.
    pub collected: Option<GoogleAssistantMessageEventStream>,
}

/// Prepared reqwest fallback request.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedGoogleVertexRequest {
    /// Request URL.
    pub url: String,
    /// Request headers.
    pub headers: ProviderHeaders,
    /// JSON payload after `on_payload` mutation.
    pub payload: Value,
    /// Client/auth configuration used to build the request.
    client: ClientConfig,
}

/// Google thinking configuration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ThinkingConfig {
    /// Whether Google should include visible thought summaries.
    pub include_thoughts: Option<bool>,
    /// Level-based thinking configuration for Gemini 3 models.
    pub thinking_level: Option<GoogleThinkingLevel>,
    /// Token-budget thinking configuration for Gemini 2.x models.
    pub thinking_budget: Option<i32>,
}

/// Google-specific thinking options accepted by [`stream`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoogleThinkingOptions {
    /// Whether thinking is enabled.
    pub enabled: bool,
    /// Token budget; `-1` requests Google's dynamic budget and `0` disables thinking.
    pub budget_tokens: Option<i32>,
    /// Level-based thinking for models that support it.
    pub level: Option<GoogleThinkingLevel>,
}

/// Callback that can inspect or replace the raw Google payload.
pub type PayloadHook = Arc<
    dyn Fn(
            Value,
            Model,
        ) -> BoxFuture<
            'static,
            std::result::Result<Option<Value>, crate::types::ProviderHookError>,
        > + Send
        + Sync,
>;

/// Options specific to Pi's Google Vertex stream implementation.
#[derive(Clone, Default)]
pub struct GoogleVertexOptions {
    /// Sampling temperature.
    pub temperature: Option<f64>,
    /// Maximum output tokens requested by the caller.
    pub max_tokens: Option<u32>,
    /// Optional Vertex API key.
    pub api_key: Option<String>,
    /// Optional custom HTTP headers.
    pub headers: ProviderHeaders,
    /// Google tool choice behavior.
    pub tool_choice: Option<GoogleToolChoice>,
    /// Google thinking options.
    pub thinking: Option<GoogleThinkingOptions>,
    /// Vertex project ID.
    pub project: Option<String>,
    /// Vertex location.
    pub location: Option<String>,
    /// Provider environment overrides.
    pub env: ProviderEnv,
    /// Optional payload hook matching Pi's onPayload behavior.
    pub on_payload: Option<PayloadHook>,
    /// Deterministic response chunks used by local tests instead of live Google calls.
    pub response_chunks: Vec<GenerateContentChunk>,
    /// Timestamp used for fallback tool-call ids.
    pub id_timestamp_ms: Option<u64>,
}

/// Options accepted by [`stream_simple`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SimpleStreamOptions {
    /// Sampling temperature.
    pub temperature: Option<f64>,
    /// Maximum output tokens requested by the caller.
    pub max_tokens: Option<u32>,
    /// Optional Vertex API key.
    pub api_key: Option<String>,
    /// Optional custom HTTP headers.
    pub headers: ProviderHeaders,
    /// Provider environment overrides.
    pub env: ProviderEnv,
    /// Unified reasoning level passed to simple streams.
    pub reasoning: Option<ThinkingLevel>,
    /// Custom token budgets per Google thinking level.
    pub thinking_budgets: ThinkingBudgets,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HttpOptions {
    base_url: Option<String>,
    base_url_resource_scope: Option<&'static str>,
    api_version: Option<String>,
    headers: ProviderHeaders,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClientConfig {
    vertexai: bool,
    project: Option<String>,
    location: Option<String>,
    api_key: Option<String>,
    api_version: &'static str,
    key_filename: Option<String>,
    http_options: Option<HttpOptions>,
}

fn provider_env_value(name: &str, env: &ProviderEnv) -> Option<String> {
    env.get(name)
        .filter(|value| !value.is_empty())
        .cloned()
        .or_else(|| std::env::var(name).ok().filter(|value| !value.is_empty()))
}

fn resolve_custom_base_url(base_url: Option<&str>) -> Option<String> {
    let trimmed = base_url?.trim();
    if trimmed.is_empty() || trimmed.contains("{location}") {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn is_api_version_path_part(part: &str) -> bool {
    let Some(rest) = part.strip_prefix('v') else {
        return false;
    };
    let digits = rest.chars().take_while(|ch| ch.is_ascii_digit()).count();
    if digits == 0 {
        return false;
    }
    let rest = &rest[digits..];
    if rest.is_empty() {
        return true;
    }
    let Some(beta) = rest.strip_prefix("beta") else {
        return false;
    };
    beta.chars().all(|ch| ch.is_ascii_digit())
}

fn base_url_includes_api_version(base_url: &str) -> bool {
    base_url.split('/').any(is_api_version_path_part)
}

fn build_http_options(model: &Model, options_headers: &ProviderHeaders) -> Option<HttpOptions> {
    let base_url = resolve_custom_base_url(model.base_url.as_deref());
    let api_version = base_url
        .as_deref()
        .filter(|url| base_url_includes_api_version(url))
        .map(|_| String::new());
    let headers: ProviderHeaders = model
        .headers
        .iter()
        .chain(options_headers)
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();

    if base_url.is_none() && headers.is_empty() {
        None
    } else {
        Some(HttpOptions {
            base_url_resource_scope: base_url.as_ref().map(|_| "COLLECTION"),
            base_url,
            api_version,
            headers,
        })
    }
}

fn build_google_auth_options(env: &ProviderEnv) -> Option<String> {
    provider_env_value("GOOGLE_APPLICATION_CREDENTIALS", env)
}

fn is_placeholder_api_key(api_key: &str) -> bool {
    api_key
        .strip_prefix('<')
        .and_then(|inner| inner.strip_suffix('>'))
        .is_some_and(|inner| !inner.is_empty() && !inner.contains('>'))
}

fn resolve_api_key(options: Option<&GoogleVertexOptions>) -> Option<String> {
    let options = options?;
    let api_key = options
        .api_key
        .clone()
        .or_else(|| provider_env_value("GOOGLE_CLOUD_API_KEY", &options.env))?;
    let api_key = api_key.trim();
    if api_key.is_empty()
        || api_key == GCP_VERTEX_CREDENTIALS_MARKER
        || is_placeholder_api_key(api_key)
    {
        None
    } else {
        Some(api_key.to_string())
    }
}

fn resolve_project(options: &GoogleVertexOptions) -> Result<String> {
    options
        .project
        .as_ref()
        .filter(|value| !value.is_empty())
        .cloned()
        .or_else(|| provider_env_value("GOOGLE_CLOUD_PROJECT", &options.env))
        .or_else(|| provider_env_value("GCLOUD_PROJECT", &options.env))
        .ok_or(GoogleVertexError::MissingProject)
}

fn resolve_location(options: &GoogleVertexOptions) -> Result<String> {
    options
        .location
        .as_ref()
        .filter(|value| !value.is_empty())
        .cloned()
        .or_else(|| provider_env_value("GOOGLE_CLOUD_LOCATION", &options.env))
        .ok_or(GoogleVertexError::MissingLocation)
}

fn create_client(model: &Model, options: &GoogleVertexOptions) -> Result<ClientConfig> {
    Ok(ClientConfig {
        vertexai: true,
        project: Some(resolve_project(options)?),
        location: Some(resolve_location(options)?),
        api_key: None,
        api_version: API_VERSION,
        key_filename: build_google_auth_options(&options.env),
        http_options: build_http_options(model, &options.headers),
    })
}

fn create_client_with_api_key(
    model: &Model,
    api_key: String,
    options_headers: &ProviderHeaders,
) -> ClientConfig {
    ClientConfig {
        vertexai: true,
        project: None,
        location: None,
        api_key: Some(api_key),
        api_version: API_VERSION,
        key_filename: None,
        http_options: build_http_options(model, options_headers),
    }
}

fn contains_gemini3_family(id: &str, family: &str) -> bool {
    for (start, _) in id.match_indices("gemini-3") {
        let mut rest = &id[start + "gemini-3".len()..];
        if let Some(next) = rest.strip_prefix('.') {
            let digits = next.chars().take_while(|ch| ch.is_ascii_digit()).count();
            if digits == 0 {
                continue;
            }
            rest = &next[digits..];
        }
        if rest
            .strip_prefix('-')
            .is_some_and(|next| next.starts_with(family))
        {
            return true;
        }
    }
    false
}

fn is_gemini3_pro_model(model: &Model) -> bool {
    contains_gemini3_family(&model.id.to_lowercase(), "pro")
}

fn is_gemini3_flash_model(model: &Model) -> bool {
    let id = model.id.to_lowercase();
    contains_gemini3_family(&id, "flash")
        || id == "gemini-flash-latest"
        || id == "gemini-flash-lite-latest"
}

fn get_disabled_thinking_config(model: &Model) -> ThinkingConfig {
    if is_gemini3_pro_model(model) {
        return ThinkingConfig {
            thinking_level: Some(GoogleThinkingLevel::Low),
            ..ThinkingConfig::default()
        };
    }
    if is_gemini3_flash_model(model) {
        return ThinkingConfig {
            thinking_level: Some(GoogleThinkingLevel::Minimal),
            ..ThinkingConfig::default()
        };
    }

    ThinkingConfig {
        thinking_budget: Some(0),
        ..ThinkingConfig::default()
    }
}

fn get_gemini3_thinking_level(effort: ClampedThinkingLevel, model: &Model) -> GoogleThinkingLevel {
    if is_gemini3_pro_model(model) {
        return match effort {
            ClampedThinkingLevel::Minimal | ClampedThinkingLevel::Low => GoogleThinkingLevel::Low,
            ClampedThinkingLevel::Medium | ClampedThinkingLevel::High => GoogleThinkingLevel::High,
        };
    }
    match effort {
        ClampedThinkingLevel::Minimal => GoogleThinkingLevel::Minimal,
        ClampedThinkingLevel::Low => GoogleThinkingLevel::Low,
        ClampedThinkingLevel::Medium => GoogleThinkingLevel::Medium,
        ClampedThinkingLevel::High => GoogleThinkingLevel::High,
    }
}

fn get_google_budget(
    model: &Model,
    effort: ClampedThinkingLevel,
    custom_budgets: &ThinkingBudgets,
) -> i32 {
    if let Some(budget) = custom_budgets.get(&effort) {
        return *budget;
    }

    match (model.id.as_str(), effort) {
        (id, ClampedThinkingLevel::Minimal) if id.contains("2.5-pro") => 128,
        (id, ClampedThinkingLevel::Low) if id.contains("2.5-pro") => 2048,
        (id, ClampedThinkingLevel::Medium) if id.contains("2.5-pro") => 8192,
        (id, ClampedThinkingLevel::High) if id.contains("2.5-pro") => 32768,
        (id, ClampedThinkingLevel::Minimal) if id.contains("2.5-flash") => 128,
        (id, ClampedThinkingLevel::Low) if id.contains("2.5-flash") => 2048,
        (id, ClampedThinkingLevel::Medium) if id.contains("2.5-flash") => 8192,
        (id, ClampedThinkingLevel::High) if id.contains("2.5-flash") => 24576,
        _ => -1,
    }
}

fn thinking_config(model: &Model, options: &GoogleVertexOptions) -> Option<ThinkingConfig> {
    let thinking = options.thinking?;
    if thinking.enabled && model.reasoning {
        let mut config = ThinkingConfig {
            include_thoughts: Some(true),
            ..ThinkingConfig::default()
        };
        if let Some(level) = thinking.level {
            config.thinking_level = Some(level);
        } else if let Some(budget_tokens) = thinking.budget_tokens {
            config.thinking_budget = Some(budget_tokens);
        }
        Some(config)
    } else if model.reasoning && !thinking.enabled {
        Some(get_disabled_thinking_config(model))
    } else {
        None
    }
}

fn shared_model(model: &Model) -> SharedModel {
    SharedModel {
        id: model.id.clone(),
        api: "google-vertex".to_string(),
        provider: model.provider.clone(),
        input: vec![ModelInput::Text, ModelInput::Image],
    }
}

fn shared_context(context: &Context) -> SharedContext {
    SharedContext {
        messages: context.messages.clone(),
    }
}

fn tool_choice_string(choice: GoogleToolChoice) -> &'static str {
    match choice {
        GoogleToolChoice::Auto => "auto",
        GoogleToolChoice::None => "none",
        GoogleToolChoice::Any => "any",
    }
}

fn thinking_config_value(config: ThinkingConfig) -> Value {
    let mut value = serde_json::Map::new();
    if config.include_thoughts == Some(true) {
        value.insert("includeThoughts".to_string(), Value::Bool(true));
    }
    if let Some(level) = config.thinking_level {
        value.insert(
            "thinkingLevel".to_string(),
            Value::String(level.to_string()),
        );
    }
    if let Some(budget) = config.thinking_budget {
        value.insert("thinkingBudget".to_string(), json!(budget));
    }
    Value::Object(value)
}

fn build_params(model: &Model, context: &Context, options: &GoogleVertexOptions) -> Value {
    let mut config = serde_json::Map::new();
    if let Some(temperature) = options.temperature {
        config.insert("temperature".to_string(), json!(temperature));
    }
    if let Some(max_tokens) = options.max_tokens {
        config.insert("maxOutputTokens".to_string(), json!(max_tokens));
    }
    if let Some(system_prompt) = context
        .system_prompt
        .as_ref()
        .filter(|prompt| !prompt.is_empty())
    {
        config.insert("systemInstruction".to_string(), json!(system_prompt));
    }
    if let Some(tools) = convert_tools(&context.tools, false) {
        config.insert("tools".to_string(), json!(tools));
    }
    if !context.tools.is_empty()
        && let Some(choice) = options.tool_choice
    {
        let mode = match map_tool_choice(tool_choice_string(choice)) {
            FunctionCallingConfigMode::Auto => "AUTO",
            FunctionCallingConfigMode::None => "NONE",
            FunctionCallingConfigMode::Any => "ANY",
        };
        config.insert(
            "toolConfig".to_string(),
            json!({ "functionCallingConfig": { "mode": mode } }),
        );
    }
    if let Some(thinking) = thinking_config(model, options) {
        config.insert(
            "thinkingConfig".to_string(),
            thinking_config_value(thinking),
        );
    }

    json!({
        "model": model.id,
        "contents": convert_messages(&shared_model(model), &shared_context(context)),
        "config": Value::Object(config),
    })
}

fn vertex_url(model: &Model, client: &ClientConfig) -> String {
    if let Some(http_options) = &client.http_options
        && let Some(base_url) = &http_options.base_url
    {
        let api_version = if http_options.api_version.as_deref() == Some("") {
            ""
        } else {
            "/v1"
        };
        return format!(
            "{}{}/publishers/google/models/{}:streamGenerateContent?alt=sse",
            base_url.trim_end_matches('/'),
            api_version,
            model.id
        );
    }

    if client.api_key.is_some() {
        return format!(
            "https://aiplatform.googleapis.com/v1/publishers/google/models/{}:streamGenerateContent?alt=sse",
            model.id
        );
    }

    let project = client.project.as_deref().unwrap_or("");
    let location = client.location.as_deref().unwrap_or("global");
    format!(
        "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:streamGenerateContent?alt=sse",
        location, project, location, model.id
    )
}

fn build_request(
    model: &Model,
    context: &Context,
    options: &GoogleVertexOptions,
) -> Result<PreparedGoogleVertexRequest> {
    let client = if let Some(api_key) = resolve_api_key(Some(options)) {
        create_client_with_api_key(model, api_key, &options.headers)
    } else {
        create_client(model, options)?
    };
    let mut headers = model.headers.clone();
    headers.extend(options.headers.clone());
    if let Some(api_key) = &client.api_key {
        headers.insert("x-goog-api-key".to_string(), api_key.clone());
    }
    let mut payload = build_params(model, context, options);
    if let Some(on_payload) = &options.on_payload
        && let Some(next_payload) =
            futures::executor::block_on(on_payload(payload.clone(), model.clone()))
                .map_err(GoogleVertexError::Hook)?
    {
        payload = next_payload;
    }
    let url = vertex_url(model, &client);

    Ok(PreparedGoogleVertexRequest {
        url,
        headers,
        payload,
        client,
    })
}

/// Streams a Google Vertex request using a reqwest-compatible prepared request.
///
/// The current Rust port keeps the live network boundary out of deterministic tests; response
/// chunks supplied in options are collected with the same text/thinking/tool/usage mapping Pi uses.
///
/// # Errors
///
/// Returns [`GoogleVertexError::MissingProject`] or [`GoogleVertexError::MissingLocation`] when ADC
/// auth is selected without required Vertex settings.
pub fn stream(
    model: &Model,
    context: &Context,
    options: Option<&GoogleVertexOptions>,
) -> Result<AssistantMessageEventStream> {
    let default_options;
    let options = if let Some(options) = options {
        options
    } else {
        default_options = GoogleVertexOptions::default();
        &default_options
    };
    let request = build_request(model, context, options)?;
    let collected = (!options.response_chunks.is_empty()).then(|| {
        collect_google_stream(
            "google-vertex",
            model.provider.clone(),
            model.id.clone(),
            &options.response_chunks,
            options.id_timestamp_ms.unwrap_or_default(),
        )
    });

    Ok(AssistantMessageEventStream { request, collected })
}

/// Streams a Google Vertex request using simplified options.
///
/// # Errors
///
/// Returns [`GoogleVertexError::MissingProject`] or [`GoogleVertexError::MissingLocation`] when ADC
/// auth is selected without required Vertex settings, or a port placeholder until the Google GenAI
/// streaming client is selected for Rust.
pub fn stream_simple(
    model: &Model,
    context: &Context,
    options: Option<&SimpleStreamOptions>,
) -> Result<AssistantMessageEventStream> {
    let options = options.cloned().unwrap_or_default();
    let thinking = match options.reasoning {
        None => Some(GoogleThinkingOptions {
            enabled: false,
            budget_tokens: None,
            level: None,
        }),
        Some(reasoning) => {
            let effort = ClampedThinkingLevel::from(reasoning);
            if is_gemini3_pro_model(model) || is_gemini3_flash_model(model) {
                Some(GoogleThinkingOptions {
                    enabled: true,
                    budget_tokens: None,
                    level: Some(get_gemini3_thinking_level(effort, model)),
                })
            } else {
                Some(GoogleThinkingOptions {
                    enabled: true,
                    budget_tokens: Some(get_google_budget(
                        model,
                        effort,
                        &options.thinking_budgets,
                    )),
                    level: None,
                })
            }
        }
    };

    let stream_options = GoogleVertexOptions {
        temperature: options.temperature,
        max_tokens: options.max_tokens,
        api_key: options.api_key,
        headers: options.headers,
        env: options.env,
        thinking,
        ..GoogleVertexOptions::default()
    };

    stream(model, context, Some(&stream_options))
}

/// Returns the canonical Google Vertex request/SSE implementation.
#[must_use]
pub fn provider_streams() -> ProviderStreams {
    ProviderStreams {
        stream: Arc::new(stream_registered),
        stream_simple: Arc::new(stream_simple_registered),
    }
}

/// Starts a canonical Vertex request and returns immediately.
#[must_use]
pub fn stream_registered(
    model: &CanonicalModel,
    context: &CanonicalContext,
    options: Option<&StreamOptions>,
) -> CanonicalEventStream {
    let stream = CanonicalEventStream::new();
    let worker_stream = stream.clone();
    let model = model.clone();
    let context = context.clone();
    let options = options.cloned().unwrap_or_default();
    crate::utils::runtime::spawn_worker(async move {
        run_registered_worker(worker_stream, model, context, options).await;
    });
    stream
}

/// Starts Vertex using Pi's simple reasoning option mapping.
#[must_use]
pub fn stream_simple_registered(
    model: &CanonicalModel,
    context: &CanonicalContext,
    options: Option<&CanonicalSimpleOptions>,
) -> CanonicalEventStream {
    let mut stream_options = options
        .map(|options| options.stream.clone())
        .unwrap_or_default();
    if let Some(reasoning) = options.and_then(|options| options.reasoning) {
        stream_options.extra.insert(
            "reasoning".to_owned(),
            Value::String(
                match reasoning {
                    crate::types::ThinkingLevel::Minimal => "minimal",
                    crate::types::ThinkingLevel::Low => "low",
                    crate::types::ThinkingLevel::Medium => "medium",
                    crate::types::ThinkingLevel::High => "high",
                    crate::types::ThinkingLevel::XHigh => "xhigh",
                }
                .to_owned(),
            ),
        );
    }
    stream_registered(model, context, Some(&stream_options))
}

async fn run_registered_worker(
    stream: CanonicalEventStream,
    model: CanonicalModel,
    context: CanonicalContext,
    options: StreamOptions,
) {
    match execute_registered(&stream, &model, &context, &options).await {
        Ok(message) => emit_final_message(&stream, &model, message),
        Err(error) => emit_terminal_error(
            &stream,
            &model,
            error.to_string(),
            matches!(error, GoogleVertexError::Aborted),
        ),
    }
}

async fn execute_registered(
    stream: &CanonicalEventStream,
    model: &CanonicalModel,
    context: &CanonicalContext,
    options: &StreamOptions,
) -> Result<crate::api::google_shared::GoogleAssistantMessage> {
    check_abort(options.signal.as_ref())?;
    let local_model = Model {
        id: model.id.clone(),
        provider: model.provider.clone(),
        base_url: (!model.base_url.is_empty()).then(|| model.base_url.clone()),
        reasoning: model.reasoning,
        headers: model.headers.clone().unwrap_or_default(),
    };
    let local_context = canonical_context(context)?;
    let mut local_options = GoogleVertexOptions {
        temperature: options.temperature,
        max_tokens: options.max_tokens,
        api_key: options.api_key.clone(),
        headers: options
            .headers
            .clone()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(name, value)| value.map(|value| (name, value)))
            .collect(),
        tool_choice: options
            .extra
            .get("toolChoice")
            .and_then(Value::as_str)
            .map(|choice| match choice {
                "none" => GoogleToolChoice::None,
                "any" => GoogleToolChoice::Any,
                _ => GoogleToolChoice::Auto,
            }),
        thinking: registered_thinking(&local_model, options),
        project: options
            .extra
            .get("project")
            .and_then(Value::as_str)
            .map(str::to_owned),
        location: options
            .extra
            .get("location")
            .and_then(Value::as_str)
            .map(str::to_owned),
        env: options.env.clone().unwrap_or_default(),
        ..GoogleVertexOptions::default()
    };
    let mut payload = build_params(&local_model, &local_context, &local_options);
    if let Some(hook) = options.on_payload.as_ref()
        && let Some(next) = hook(payload.clone(), model.clone())
            .await
            .map_err(GoogleVertexError::Hook)?
    {
        payload = next;
    }
    local_options.on_payload = None;
    let mut request = build_request(&local_model, &local_context, &local_options)?;
    request.payload = rest_payload_from_sdk(payload);
    if request.request_uses_adc()
        && !request
            .headers
            .keys()
            .any(|name| name.eq_ignore_ascii_case("authorization"))
    {
        let token = resolve_adc_access_token(&local_options.env).await?;
        request
            .headers
            .insert("authorization".to_owned(), format!("Bearer {token}"));
    }

    let mut builder = reqwest::Client::builder();
    if let Some(timeout_ms) = options.timeout_ms {
        builder = builder.timeout(Duration::from_millis(timeout_ms));
    }
    let send = builder
        .build()
        .map_err(GoogleVertexError::Http)?
        .post(&request.url)
        .headers(to_header_map(&request.headers)?)
        .body(request.payload.to_string())
        .send();
    let mut response = await_or_abort(send, options.signal.clone()).await?;
    let status = response.status().as_u16();
    let response_headers = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.to_string(), value.to_owned()))
        })
        .collect::<HashMap<_, _>>();
    if let Some(hook) = options.on_response.as_ref() {
        hook(
            ProviderResponse {
                status,
                headers: response_headers.clone(),
            },
            model.clone(),
        )
        .await
        .map_err(GoogleVertexError::Hook)?;
    }
    if !(200..300).contains(&status) {
        let body = read_response_body(&mut response, options.signal.clone()).await?;
        let source = GoogleStatusError {
            status,
            body: body.clone(),
        };
        return Err(GoogleVertexError::Service(
            ProviderServiceError::with_source(
                ProviderHttpErrorParts::new("Google Vertex request failed")
                    .with_status(status)
                    .with_headers(response_headers)
                    .with_body(body),
                source,
            ),
        ));
    }

    let mut collector = GoogleStreamCollector::new(
        "google-vertex".to_owned(),
        model.provider.clone(),
        model.id.clone(),
        now_ms(),
    );
    emit_frames(
        stream,
        model,
        collector.take_frames(),
        options.signal.as_ref(),
    )?;
    let mut decoder = GoogleSseDecoder::default();
    loop {
        check_abort(options.signal.as_ref())?;
        let Some(chunk) = await_or_abort(response.chunk(), options.signal.clone()).await? else {
            break;
        };
        for decoded in decoder.push(&chunk)? {
            check_abort(options.signal.as_ref())?;
            collector.apply_chunk(&decoded);
            emit_frames(
                stream,
                model,
                collector.take_frames(),
                options.signal.as_ref(),
            )?;
            tokio::task::yield_now().await;
        }
    }
    for decoded in decoder.finish()? {
        check_abort(options.signal.as_ref())?;
        collector.apply_chunk(&decoded);
        emit_frames(
            stream,
            model,
            collector.take_frames(),
            options.signal.as_ref(),
        )?;
    }
    let (collected, frames) = collector.finish_incremental();
    emit_frames(stream, model, frames, options.signal.as_ref())?;
    check_abort(options.signal.as_ref())?;
    Ok(collected.message)
}

impl PreparedGoogleVertexRequest {
    fn request_uses_adc(&self) -> bool {
        self.client.api_key.is_none()
    }
}

async fn resolve_adc_access_token(env: &ProviderEnv) -> Result<String> {
    use gcp_auth::TokenProvider;

    if let Some(token) = provider_env_value("GOOGLE_OAUTH_ACCESS_TOKEN", env) {
        return Ok(token);
    }

    const CLOUD_PLATFORM_SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";
    let token = if let Some(path) = env
        .get("GOOGLE_APPLICATION_CREDENTIALS")
        .filter(|path| !path.is_empty())
    {
        gcp_auth::CustomServiceAccount::from_file(path)
            .map_err(GoogleVertexError::AdcAuth)?
            .token(&[CLOUD_PLATFORM_SCOPE])
            .await
            .map_err(GoogleVertexError::AdcAuth)?
    } else {
        gcp_auth::provider()
            .await
            .map_err(GoogleVertexError::AdcAuth)?
            .token(&[CLOUD_PLATFORM_SCOPE])
            .await
            .map_err(GoogleVertexError::AdcAuth)?
    };
    Ok(token.as_str().to_owned())
}

fn rest_payload_from_sdk(payload: Value) -> Value {
    let contents = payload
        .get("contents")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let config = payload.get("config").and_then(Value::as_object);
    let mut body = serde_json::Map::from_iter([("contents".to_owned(), contents)]);
    if let Some(config) = config {
        if let Some(system) = config.get("systemInstruction") {
            body.insert(
                "systemInstruction".to_owned(),
                json!({ "parts": [{ "text": system }] }),
            );
        }
        for key in ["tools", "toolConfig"] {
            if let Some(value) = config.get(key) {
                body.insert(key.to_owned(), value.clone());
            }
        }
        let generation = config
            .iter()
            .filter(|(key, _)| {
                !matches!(key.as_str(), "systemInstruction" | "tools" | "toolConfig")
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<serde_json::Map<_, _>>();
        if !generation.is_empty() {
            body.insert("generationConfig".to_owned(), Value::Object(generation));
        }
    }
    Value::Object(body)
}

fn canonical_context(context: &CanonicalContext) -> Result<Context> {
    let value = serde_json::to_value(context).map_err(GoogleVertexError::InvalidResponse)?;
    let messages = value["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(canonical_message)
        .collect();
    let tools = context
        .tools
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|tool| Tool {
            name: tool.name,
            description: tool.description,
            parameters: tool.parameters,
        })
        .collect();
    Ok(Context {
        system_prompt: context.system_prompt.clone(),
        messages,
        tools,
    })
}

fn canonical_message(message: &Value) -> Option<crate::api::google_shared::Message> {
    use crate::api::google_shared::{AssistantContent, Message, UserContent, UserContentPart};
    let role = message.get("role")?.as_str()?;
    let user_parts = |value: &Value| {
        value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|part| match part.get("type")?.as_str()? {
                "text" => Some(UserContentPart::Text {
                    text: part.get("text")?.as_str()?.to_owned(),
                }),
                "image" => Some(UserContentPart::Image {
                    data: part.get("data")?.as_str()?.to_owned(),
                    mime_type: part.get("mimeType")?.as_str()?.to_owned(),
                }),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    match role {
        "user" => Some(Message::User {
            content: message
                .get("content")
                .and_then(Value::as_str)
                .map(|text| UserContent::Text(text.to_owned()))
                .unwrap_or_else(|| UserContent::Parts(user_parts(&message["content"]))),
        }),
        "assistant" => Some(Message::Assistant {
            content: message["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|part| match part.get("type")?.as_str()? {
                    "text" => Some(AssistantContent::Text {
                        text: part.get("text")?.as_str()?.to_owned(),
                        text_signature: part
                            .get("textSignature")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                    }),
                    "thinking" => Some(AssistantContent::Thinking {
                        thinking: part.get("thinking")?.as_str()?.to_owned(),
                        thinking_signature: part
                            .get("thinkingSignature")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        redacted: part
                            .get("redacted")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    }),
                    "toolCall" => Some(AssistantContent::ToolCall {
                        id: part.get("id")?.as_str()?.to_owned(),
                        name: part.get("name")?.as_str()?.to_owned(),
                        arguments: part.get("arguments").cloned(),
                        thought_signature: part
                            .get("thoughtSignature")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                    }),
                    _ => None,
                })
                .collect(),
            api: message
                .get("api")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            provider: message
                .get("provider")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            model: message
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            stop_reason: match message.get("stopReason").and_then(Value::as_str) {
                Some("length") => crate::api::google_shared::StopReason::Length,
                Some("toolUse") => crate::api::google_shared::StopReason::ToolUse,
                Some("error") => crate::api::google_shared::StopReason::Error,
                Some("aborted") => crate::api::google_shared::StopReason::Aborted,
                _ => crate::api::google_shared::StopReason::Stop,
            },
        }),
        "toolResult" => Some(Message::ToolResult {
            tool_call_id: message.get("toolCallId")?.as_str()?.to_owned(),
            tool_name: message.get("toolName")?.as_str()?.to_owned(),
            content: user_parts(&message["content"]),
            is_error: message
                .get("isError")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }),
        _ => None,
    }
}

fn registered_thinking(model: &Model, options: &StreamOptions) -> Option<GoogleThinkingOptions> {
    let reasoning = options.extra.get("reasoning").and_then(Value::as_str);
    if !model.reasoning {
        return None;
    }
    let Some(reasoning) = reasoning else {
        return Some(GoogleThinkingOptions {
            enabled: false,
            budget_tokens: None,
            level: None,
        });
    };
    let effort = match reasoning {
        "minimal" => ClampedThinkingLevel::Minimal,
        "low" => ClampedThinkingLevel::Low,
        "medium" => ClampedThinkingLevel::Medium,
        _ => ClampedThinkingLevel::High,
    };
    if is_gemini3_pro_model(model) || is_gemini3_flash_model(model) {
        Some(GoogleThinkingOptions {
            enabled: true,
            budget_tokens: None,
            level: Some(get_gemini3_thinking_level(effort, model)),
        })
    } else {
        Some(GoogleThinkingOptions {
            enabled: true,
            budget_tokens: Some(get_google_budget(model, effort, &ThinkingBudgets::new())),
            level: None,
        })
    }
}

fn to_header_map(headers: &ProviderHeaders) -> Result<reqwest::header::HeaderMap> {
    let mut map = reqwest::header::HeaderMap::new();
    for (name, value) in headers {
        let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
            .map_err(|error| GoogleVertexError::InvalidHeader(error.to_string()))?;
        let value = reqwest::header::HeaderValue::from_str(value)
            .map_err(|error| GoogleVertexError::InvalidHeader(error.to_string()))?;
        map.insert(name, value);
    }
    map.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/json"),
    );
    Ok(map)
}

async fn await_or_abort<T>(
    future: impl std::future::Future<Output = std::result::Result<T, reqwest::Error>>,
    signal: Option<crate::types::AbortSignal>,
) -> Result<T> {
    if let Some(signal) = signal {
        match futures::future::select(Box::pin(future), Box::pin(wait_for_abort(signal))).await {
            futures::future::Either::Left((result, _)) => result.map_err(GoogleVertexError::Http),
            futures::future::Either::Right(((), _)) => Err(GoogleVertexError::Aborted),
        }
    } else {
        future.await.map_err(GoogleVertexError::Http)
    }
}

async fn wait_for_abort(signal: crate::types::AbortSignal) {
    while !signal.aborted() {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

fn check_abort(signal: Option<&crate::types::AbortSignal>) -> Result<()> {
    if signal.is_some_and(crate::types::AbortSignal::aborted) {
        Err(GoogleVertexError::Aborted)
    } else {
        Ok(())
    }
}

async fn read_response_body(
    response: &mut reqwest::Response,
    signal: Option<crate::types::AbortSignal>,
) -> Result<String> {
    let mut body = Vec::new();
    while let Some(chunk) = await_or_abort(response.chunk(), signal.clone()).await? {
        body.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

#[derive(Debug)]
struct GoogleStatusError {
    status: u16,
    body: String,
}
impl fmt::Display for GoogleStatusError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Google Vertex request failed with status {}: {}",
            self.status, self.body
        )
    }
}
impl StdError for GoogleStatusError {}

#[derive(Default)]
struct GoogleSseDecoder {
    pending: Vec<u8>,
}
impl GoogleSseDecoder {
    fn push(&mut self, bytes: &[u8]) -> Result<Vec<GenerateContentChunk>> {
        self.pending.extend_from_slice(bytes);
        self.decode(false)
    }
    fn finish(&mut self) -> Result<Vec<GenerateContentChunk>> {
        self.decode(true)
    }
    fn decode(&mut self, flush: bool) -> Result<Vec<GenerateContentChunk>> {
        let mut chunks = Vec::new();
        while let Some((end, delimiter_len)) = find_sse_delimiter(&self.pending) {
            let event = self.pending.drain(..end).collect::<Vec<_>>();
            self.pending.drain(..delimiter_len);
            if let Some(chunk) = decode_sse_event(&event)? {
                chunks.push(chunk);
            }
        }
        if flush && !self.pending.is_empty() {
            let event = std::mem::take(&mut self.pending);
            if let Some(chunk) = decode_sse_event(&event)? {
                chunks.push(chunk);
            }
        }
        Ok(chunks)
    }
}

fn find_sse_delimiter(bytes: &[u8]) -> Option<(usize, usize)> {
    bytes
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|index| (index, 2))
        .into_iter()
        .chain(
            bytes
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|index| (index, 4)),
        )
        .min_by_key(|(index, _)| *index)
}

fn decode_sse_event(event: &[u8]) -> Result<Option<GenerateContentChunk>> {
    let event = std::str::from_utf8(event).map_err(|error| {
        GoogleVertexError::InvalidSse(format!("invalid UTF-8 in Google SSE: {error}"))
    })?;
    let data = event
        .lines()
        .filter_map(|line| line.strip_prefix("data:").map(str::trim))
        .collect::<Vec<_>>()
        .join("\n");
    if data.is_empty() || data == "[DONE]" {
        Ok(None)
    } else {
        serde_json::from_str(&data)
            .map(Some)
            .map_err(GoogleVertexError::InvalidResponse)
    }
}

fn canonical_message_value(
    model: &CanonicalModel,
    message: &crate::api::google_shared::GoogleAssistantMessage,
) -> Value {
    let content = message.content.iter().map(|block| match block {
        crate::api::google_shared::GoogleContentBlock::Text { text, text_signature } => json!({ "type": "text", "text": text, "textSignature": text_signature }),
        crate::api::google_shared::GoogleContentBlock::Thinking { thinking, thinking_signature } => json!({ "type": "thinking", "thinking": thinking, "thinkingSignature": thinking_signature }),
        crate::api::google_shared::GoogleContentBlock::ToolCall { id, name, arguments, thought_signature } => json!({ "type": "toolCall", "id": id, "name": name, "arguments": arguments, "thoughtSignature": thought_signature }),
    }).collect::<Vec<_>>();
    let input_cost = model.cost.input * message.usage.input as f64 / 1_000_000.0;
    let output_cost = model.cost.output * message.usage.output as f64 / 1_000_000.0;
    let cache_read_cost = model.cost.cache_read * message.usage.cache_read as f64 / 1_000_000.0;
    let cache_write_cost = model.cost.cache_write * message.usage.cache_write as f64 / 1_000_000.0;
    json!({
        "role": "assistant", "content": content, "api": "google-vertex", "provider": model.provider,
        "model": model.id, "responseId": message.response_id,
        "usage": { "input": message.usage.input, "output": message.usage.output,
            "cacheRead": message.usage.cache_read, "cacheWrite": message.usage.cache_write,
            "reasoning": message.usage.reasoning, "totalTokens": message.usage.total_tokens,
            "cost": { "input": input_cost, "output": output_cost, "cacheRead": cache_read_cost,
                "cacheWrite": cache_write_cost, "total": input_cost + output_cost + cache_read_cost + cache_write_cost } },
        "stopReason": stop_reason_name(message.stop_reason), "timestamp": now_ms()
    })
}

fn stop_reason_name(reason: crate::api::google_shared::StopReason) -> &'static str {
    match reason {
        crate::api::google_shared::StopReason::Stop => "stop",
        crate::api::google_shared::StopReason::Length => "length",
        crate::api::google_shared::StopReason::ToolUse => "toolUse",
        crate::api::google_shared::StopReason::Error => "error",
        crate::api::google_shared::StopReason::Aborted => "aborted",
    }
}

fn emit_frames(
    stream: &CanonicalEventStream,
    model: &CanonicalModel,
    frames: Vec<GoogleStreamFrame>,
    signal: Option<&crate::types::AbortSignal>,
) -> Result<()> {
    for frame in frames {
        check_abort(signal)?;
        let partial = canonical_message_value(model, &frame.partial);
        let event = match frame.event {
            crate::api::google_shared::GoogleStreamEvent::Start => {
                json!({ "type": "start", "partial": partial })
            }
            crate::api::google_shared::GoogleStreamEvent::TextStart { content_index } => {
                json!({ "type": "text_start", "contentIndex": content_index, "partial": partial })
            }
            crate::api::google_shared::GoogleStreamEvent::TextDelta {
                content_index,
                delta,
            } => {
                json!({ "type": "text_delta", "contentIndex": content_index, "delta": delta, "partial": partial })
            }
            crate::api::google_shared::GoogleStreamEvent::TextEnd {
                content_index,
                content,
            } => {
                json!({ "type": "text_end", "contentIndex": content_index, "content": content, "partial": partial })
            }
            crate::api::google_shared::GoogleStreamEvent::ThinkingStart { content_index } => {
                json!({ "type": "thinking_start", "contentIndex": content_index, "partial": partial })
            }
            crate::api::google_shared::GoogleStreamEvent::ThinkingDelta {
                content_index,
                delta,
            } => {
                json!({ "type": "thinking_delta", "contentIndex": content_index, "delta": delta, "partial": partial })
            }
            crate::api::google_shared::GoogleStreamEvent::ThinkingEnd {
                content_index,
                content,
            } => {
                json!({ "type": "thinking_end", "contentIndex": content_index, "content": content, "partial": partial })
            }
            crate::api::google_shared::GoogleStreamEvent::ToolcallStart { content_index } => {
                json!({ "type": "toolcall_start", "contentIndex": content_index, "partial": partial })
            }
            crate::api::google_shared::GoogleStreamEvent::ToolcallDelta {
                content_index,
                delta,
            } => {
                json!({ "type": "toolcall_delta", "contentIndex": content_index, "delta": delta, "partial": partial })
            }
            crate::api::google_shared::GoogleStreamEvent::ToolcallEnd { content_index } => {
                json!({ "type": "toolcall_end", "contentIndex": content_index, "toolCall": partial["content"][content_index].clone(), "partial": partial })
            }
            crate::api::google_shared::GoogleStreamEvent::Done { .. } => continue,
        };
        stream.push(
            serde_json::from_value::<AssistantMessageEvent>(event)
                .map_err(GoogleVertexError::InvalidCanonicalEvent)?,
        );
    }
    Ok(())
}

fn emit_final_message(
    stream: &CanonicalEventStream,
    model: &CanonicalModel,
    message: crate::api::google_shared::GoogleAssistantMessage,
) {
    let message: AssistantMessage =
        serde_json::from_value(canonical_message_value(model, &message))
            .expect("shared Google messages must map to canonical messages");
    if message.stop_reason == CanonicalStopReason::Error {
        stream.push(AssistantMessageEvent::Error {
            reason: ErrorStopReason::Error,
            error: message,
        });
    } else {
        let reason = match message.stop_reason {
            CanonicalStopReason::Length => DoneStopReason::Length,
            CanonicalStopReason::ToolUse => DoneStopReason::ToolUse,
            _ => DoneStopReason::Stop,
        };
        stream.push(AssistantMessageEvent::Done { reason, message });
    }
}

fn emit_terminal_error(
    stream: &CanonicalEventStream,
    model: &CanonicalModel,
    error: String,
    aborted: bool,
) {
    let message: AssistantMessage = serde_json::from_value(json!({
        "role": "assistant", "content": [], "api": "google-vertex", "provider": model.provider,
        "model": model.id, "usage": { "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "totalTokens": 0,
            "cost": { "input": 0.0, "output": 0.0, "cacheRead": 0.0, "cacheWrite": 0.0, "total": 0.0 } },
        "stopReason": if aborted { "aborted" } else { "error" }, "errorMessage": error, "timestamp": now_ms()
    })).expect("canonical Google Vertex terminal message");
    stream.push(AssistantMessageEvent::Error {
        reason: if aborted {
            ErrorStopReason::Aborted
        } else {
            ErrorStopReason::Error
        },
        error: message,
    });
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::google_shared::{GoogleStreamEvent, StopReason};

    fn model(id: &str) -> Model {
        Model {
            id: id.to_string(),
            provider: "google-vertex".to_string(),
            base_url: None,
            reasoning: true,
            headers: HashMap::new(),
        }
    }

    fn model_with_base_url(base_url: &str) -> Model {
        Model {
            base_url: Some(base_url.to_string()),
            ..model("gemini-3-flash-preview")
        }
    }

    fn adc_options(api_key: Option<&str>) -> GoogleVertexOptions {
        GoogleVertexOptions {
            api_key: api_key.map(str::to_string),
            project: Some("test-project".to_string()),
            location: Some("us-central1".to_string()),
            ..GoogleVertexOptions::default()
        }
    }

    fn vertex_client_config(model: &Model, options: &GoogleVertexOptions) -> Result<ClientConfig> {
        if let Some(api_key) = resolve_api_key(Some(options)) {
            Ok(create_client_with_api_key(model, api_key, &options.headers))
        } else {
            create_client(model, options)
        }
    }

    #[test]
    fn vertex_api_key_placeholder_marker_uses_adc_client_config() {
        let client = vertex_client_config(
            &model("gemini-3-flash-preview"),
            &adc_options(Some("<authenticated>")),
        )
        .expect("placeholder api key should use ADC options");

        assert!(client.vertexai);
        assert_eq!(client.project.as_deref(), Some("test-project"));
        assert_eq!(client.location.as_deref(), Some("us-central1"));
        assert_eq!(client.api_version, "v1");
        assert!(client.api_key.is_none());
    }

    #[test]
    fn vertex_gcp_credentials_marker_uses_adc_client_config() {
        let client = vertex_client_config(
            &model("gemini-3-flash-preview"),
            &adc_options(Some("gcp-vertex-credentials")),
        )
        .expect("credentials marker should use ADC options");

        assert!(client.vertexai);
        assert_eq!(client.project.as_deref(), Some("test-project"));
        assert_eq!(client.location.as_deref(), Some("us-central1"));
        assert_eq!(client.api_version, "v1");
        assert!(client.api_key.is_none());
    }

    #[test]
    fn vertex_google_cloud_api_key_env_placeholder_still_uses_adc_client_config() {
        let options = GoogleVertexOptions {
            env: HashMap::from([(
                "GOOGLE_CLOUD_API_KEY".to_string(),
                "<authenticated>".to_string(),
            )]),
            ..adc_options(None)
        };
        let client = vertex_client_config(&model("gemini-3-flash-preview"), &options)
            .expect("placeholder env api key should not block ADC options");

        assert!(client.vertexai);
        assert_eq!(client.project.as_deref(), Some("test-project"));
        assert_eq!(client.location.as_deref(), Some("us-central1"));
        assert_eq!(client.api_version, "v1");
        assert!(client.api_key.is_none());
    }

    #[test]
    fn vertex_real_api_key_uses_api_key_client_config() {
        let options = GoogleVertexOptions {
            api_key: Some("AIzaSyExampleRealisticLookingApiKey123456".to_string()),
            ..GoogleVertexOptions::default()
        };
        let client = vertex_client_config(&model("gemini-3-flash-preview"), &options)
            .expect("real api key should build api-key client config");

        assert!(client.vertexai);
        assert_eq!(
            client.api_key.as_deref(),
            Some("AIzaSyExampleRealisticLookingApiKey123456")
        );
        assert_eq!(client.api_version, "v1");
        assert!(client.project.is_none());
        assert!(client.location.is_none());
    }

    #[test]
    fn vertex_generated_base_url_placeholders_are_not_forwarded() {
        let generated = model_with_base_url(
            "https://{location}-aiplatform.googleapis.com/v1/projects/test-project/locations/{location}",
        );
        let client = vertex_client_config(&generated, &adc_options(None))
            .expect("generated base URL placeholder should use ADC options");

        assert!(client.http_options.is_none());
    }

    #[test]
    fn vertex_custom_base_url_is_forwarded_to_adc_client_config() {
        let client = vertex_client_config(
            &model_with_base_url("https://proxy.example.com"),
            &adc_options(None),
        )
        .expect("custom base URL should use ADC options");
        let http_options = client
            .http_options
            .as_ref()
            .expect("custom base URL should create http options");

        assert!(client.vertexai);
        assert_eq!(client.project.as_deref(), Some("test-project"));
        assert_eq!(client.location.as_deref(), Some("us-central1"));
        assert_eq!(client.api_version, "v1");
        assert_eq!(
            http_options.base_url.as_deref(),
            Some("https://proxy.example.com")
        );
        assert_eq!(http_options.base_url_resource_scope, Some("COLLECTION"));
    }

    #[test]
    fn vertex_custom_base_url_is_forwarded_to_api_key_client_config() {
        let options = GoogleVertexOptions {
            api_key: Some("AIzaSyExampleRealisticLookingApiKey123456".to_string()),
            ..GoogleVertexOptions::default()
        };
        let client =
            vertex_client_config(&model_with_base_url("https://proxy.example.com"), &options)
                .expect("custom base URL should use api-key options");
        let http_options = client
            .http_options
            .as_ref()
            .expect("custom base URL should create http options");

        assert!(client.vertexai);
        assert_eq!(
            client.api_key.as_deref(),
            Some("AIzaSyExampleRealisticLookingApiKey123456")
        );
        assert_eq!(client.api_version, "v1");
        assert_eq!(
            http_options.base_url.as_deref(),
            Some("https://proxy.example.com")
        );
        assert_eq!(http_options.base_url_resource_scope, Some("COLLECTION"));
    }

    #[test]
    fn vertex_custom_base_url_with_api_version_disables_extra_api_version() {
        let client = vertex_client_config(
            &model_with_base_url(
                "https://proxy.example.com/v1/projects/test-project/locations/global",
            ),
            &adc_options(None),
        )
        .expect("versioned custom base URL should use ADC options");
        let http_options = client
            .http_options
            .as_ref()
            .expect("custom base URL should create http options");

        assert_eq!(
            http_options.base_url.as_deref(),
            Some("https://proxy.example.com/v1/projects/test-project/locations/global")
        );
        assert_eq!(http_options.base_url_resource_scope, Some("COLLECTION"));
        assert_eq!(http_options.api_version.as_deref(), Some(""));
    }

    #[test]
    fn api_key_marker_and_placeholders_use_adc() {
        let mut options = GoogleVertexOptions {
            api_key: Some(" gcp-vertex-credentials ".to_string()),
            ..GoogleVertexOptions::default()
        };
        assert_eq!(resolve_api_key(Some(&options)), None);

        options.api_key = Some("<GOOGLE_VERTEX_API_KEY>".to_string());
        assert_eq!(resolve_api_key(Some(&options)), None);

        options.api_key = Some(" real-key ".to_string());
        assert_eq!(resolve_api_key(Some(&options)).as_deref(), Some("real-key"));
    }

    #[test]
    fn project_location_and_credentials_resolve_from_env() {
        let options = GoogleVertexOptions {
            env: HashMap::from([
                ("GOOGLE_CLOUD_PROJECT".to_string(), "project-a".to_string()),
                (
                    "GOOGLE_CLOUD_LOCATION".to_string(),
                    "us-central1".to_string(),
                ),
                (
                    "GOOGLE_APPLICATION_CREDENTIALS".to_string(),
                    "/tmp/key.json".to_string(),
                ),
            ]),
            ..GoogleVertexOptions::default()
        };
        let client = create_client(&model("gemini-2.5-pro"), &options)
            .expect("env should satisfy ADC client setup");

        assert_eq!(client.project.as_deref(), Some("project-a"));
        assert_eq!(client.location.as_deref(), Some("us-central1"));
        assert_eq!(client.key_filename.as_deref(), Some("/tmp/key.json"));
    }

    #[test]
    fn vertex_budget_defaults_match_pi_source() {
        let custom = ThinkingBudgets::new();
        assert_eq!(
            get_google_budget(
                &model("gemini-2.5-pro"),
                ClampedThinkingLevel::High,
                &custom
            ),
            32768
        );
        assert_eq!(
            get_google_budget(
                &model("gemini-2.5-flash-lite"),
                ClampedThinkingLevel::Minimal,
                &custom
            ),
            128
        );
        assert_eq!(
            get_google_budget(
                &model("gemini-2.0-flash"),
                ClampedThinkingLevel::High,
                &custom
            ),
            -1
        );
    }

    #[test]
    fn stream_builds_api_key_request_and_applies_on_payload() {
        let options = GoogleVertexOptions {
            api_key: Some("real-key".to_string()),
            on_payload: Some(Arc::new(|mut payload, _model| {
                payload["config"]["maxOutputTokens"] = json!(9);
                Box::pin(async move { Ok(Some(payload)) })
            })),
            max_tokens: Some(3),
            ..GoogleVertexOptions::default()
        };
        let stream = stream(
            &model("gemini-2.5-pro"),
            &Context::default(),
            Some(&options),
        )
        .expect("request should be prepared");

        assert!(
            stream
                .request
                .url
                .contains("publishers/google/models/gemini-2.5-pro:streamGenerateContent")
        );
        assert_eq!(
            stream.request.headers.get("x-goog-api-key"),
            Some(&"real-key".to_string())
        );
        assert_eq!(
            stream.request.payload["config"]["maxOutputTokens"],
            json!(9)
        );
        assert!(stream.collected.is_none());
    }

    #[test]
    fn stream_collects_vertex_chunks_with_usage_and_tool_use() {
        let options = GoogleVertexOptions {
            api_key: Some("real-key".to_string()),
            id_timestamp_ms: Some(7),
            response_chunks: vec![GenerateContentChunk {
                response_id: Some("vertex-resp".to_string()),
                candidates: vec![crate::api::google_shared::Candidate {
                    content: Some(crate::api::google_shared::Content {
                        role: "model".to_string(),
                        parts: vec![crate::api::google_shared::Part {
                            function_call: Some(crate::api::google_shared::FunctionCall {
                                name: "search".to_string(),
                                args: json!({ "q": "rust" }),
                                id: Some("call-1".to_string()),
                            }),
                            ..crate::api::google_shared::Part::default()
                        }],
                    }),
                    finish_reason: Some("STOP".to_string()),
                }],
                usage_metadata: Some(crate::api::google_shared::UsageMetadata {
                    prompt_token_count: 8,
                    cached_content_token_count: 5,
                    candidates_token_count: 1,
                    thoughts_token_count: 2,
                    total_token_count: 11,
                }),
            }],
            ..GoogleVertexOptions::default()
        };
        let stream = stream(
            &model("gemini-2.5-pro"),
            &Context::default(),
            Some(&options),
        )
        .expect("request should be prepared");
        let collected = stream.collected.expect("fixture chunks should collect");

        assert_eq!(
            collected.message.response_id.as_deref(),
            Some("vertex-resp")
        );
        assert_eq!(collected.message.stop_reason, StopReason::ToolUse);
        assert_eq!(collected.message.usage.input, 3);
        assert_eq!(collected.message.usage.cache_read, 5);
        assert_eq!(collected.message.usage.output, 3);
        assert_eq!(
            collected.events.last(),
            Some(&GoogleStreamEvent::Done {
                reason: StopReason::ToolUse
            })
        );
    }

    #[test]
    fn captured_service_account_adc_resolves_bearer_and_dispatches() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        use base64::Engine;

        const PRIVATE_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQCraamItifFxHpA
p2d54ANGEhUP8LOoY3sQFFeiO8rmCT8Dg6L9DYXtnP455d7ewRhjCwfrATh+GfgM
5yVgoz4Z6K2mX+97ZAe+lDMPYlJDXnT9tPyYZoSchTkUDUrfkNDw+0J58s4VHzeT
pXzQILS1ucRyy+/XMgU8pM/wrGug8xCvMfjk/j7Dic8gSYP+NajltJDzujO4TGcg
Ne/3usVDOI1xlPC0RZWurD/8TsOv4MTngj5wFfNNIWAWPIAV2bQbsAJMJBqY3AyO
gfr2/k+VLzgb2MXhyPUHPZTBQiPW+mJQi64+ywmn7JeJ4dflRaMvJzYGkCldBaHw
EJom8k+tAgMBAAECggEABDKpQTzcyn4eVFkFMrnmup+Uvngxni0ZhXJKFyIJvQp6
7ZYatsHPBtuyai6T/7aQ51QM1JeKD6SJK5+5jZ1R1waYwhtVXRs9CVDN01GgHCBD
EzeMfBr+omqs1C3jKIh+ZXhxz1S/8Up7bPU/kkVKx6yOABW4gPeroymSIh3G4QEo
jQMrxb+MIC7UTDA3pZMkKkGONdVOQzqmfaQiqornopsZVSsYtLQLrPXnP3RTCRVl
7Hp6WSdxGobfgqKleo7nmgNpNB51uXXXudrGYSo4HRjnZ77kEi1bhvjRJP1uxMjn
mreE+uhkzYOlSV0VL1cd/rKvQ0SmLlI50XSLl2mGYQKBgQDu7Yi6iquDNPYBMAwj
pId8mAZxsilC4gWK9Z9zhlW0T6sAPhJLPzbilNHkuVIBFlI2aqjC7SxYYZFFHLBu
jJNgFQjrlkrB5xAD/MQ+Ea89owIieKbsf0XsOfKJacct0ZJXS/E4EoIEFgoN84Jj
iFiPZEJF7wTGmPDXKoTxqLUFNQKBgQC3qSRpwAi+eJLFUpHwQP3OSyw2pA5LMGi4
EpAOmQUElpseduk501zLgPVHbSn6/5GWAQMj2B6tlju5HobJaKzcu12u8ya6zPs3
Wf7G9DTeDrldoOoaSoStMgsQy6o2Q/ut7e80jLlbZXjR1xJT6qy+pfDoH9pEdyL3
Kfk6cZ7HmQKBgQDA3GJC2Y6KkaSF3ufdmYB4FSsWeY6O221H9u6nzOa/bpOE1ZXk
wXknOqOWsfS8xezE2iGxfssN6Gvf0sGj6rtHkpMpv55GmKI35b/urk27PiqJ8sQj
ILUrcrcRLp5FoOY0qytibKYgcD3bdxVoDHYYAQDx/HbpbCj0NfEsNFcyhQKBgBHV
nu+V8kNsufPnXLyT0xGhQx3bOHgcr06QnuSL/2y+ozmGGoe++pfYYfkZpKX3A1Ap
sQBeEDyTBiGn0Tblr0OP/jzq56vkE9EAMDlppWiazW1GHvWGnvOilGiBHno+h8YQ
ANZ9g9JYPC9ET0dO1o981bP0w+E6IG8X6FfAiMahAoGBAI1/TGm5TPSqW2bkjFsK
mXGDNsFgBxHDHxVRwRonBgTwx7IEGLX8m/B/gu5wbLVwuwratg+vmVFeHsGz676+
jLQtaBEZPQ2WM4zaLRAhtgoIKxYJBrAs48t/CuYizJMEO/bDDhw+BfAftVZtJ1Nv
DgxsymU6tx0SZiV+fFCzYMhY
-----END PRIVATE KEY-----"#;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ADC capture server");
        let base_url = format!("http://{}", listener.local_addr().expect("capture address"));
        let token_uri = format!("{base_url}/token");
        thread::spawn(move || {
            let (mut token_socket, _) = listener.accept().expect("accept token request");
            let mut token_request = [0_u8; 8192];
            let read = token_socket
                .read(&mut token_request)
                .expect("read token request");
            let token_request = String::from_utf8_lossy(&token_request[..read]);
            assert!(token_request.starts_with("POST /token "));
            let request_body = token_request
                .split_once("\r\n\r\n")
                .expect("token request body")
                .1;
            let assertion = url::form_urlencoded::parse(request_body.as_bytes())
                .find_map(|(name, value)| (name == "assertion").then(|| value.into_owned()))
                .expect("service-account JWT assertion");
            let claims = assertion.split('.').nth(1).expect("JWT claims");
            let claims: Value = serde_json::from_slice(
                &base64::engine::general_purpose::URL_SAFE
                    .decode(claims)
                    .expect("decode JWT claims"),
            )
            .expect("parse JWT claims");
            assert_eq!(
                claims["scope"],
                "https://www.googleapis.com/auth/cloud-platform"
            );
            let token_body = r#"{"access_token":"captured-service-account-token","expires_in":3600,"token_type":"Bearer"}"#;
            write!(
                token_socket,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                token_body.len(),
                token_body
            )
            .expect("write token response");

            let (mut vertex_socket, _) = listener.accept().expect("accept Vertex request");
            let mut vertex_request = [0_u8; 8192];
            let read = vertex_socket
                .read(&mut vertex_request)
                .expect("read Vertex request");
            let vertex_request = String::from_utf8_lossy(&vertex_request[..read]);
            assert!(vertex_request.starts_with("POST /v1/publishers/google/models/gemini-3-flash-preview:streamGenerateContent?alt=sse "));
            assert!(
                vertex_request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer captured-service-account-token")
            );
            let body = concat!(
                "data: {\"responseId\":\"service-account-response\",\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"ok\"}]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":1,\"candidatesTokenCount\":1,\"totalTokenCount\":2}}\n\n",
                "data: [DONE]\n\n",
            );
            write!(
                vertex_socket,
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\n\r\n{}",
                body.len(),
                body
            )
            .expect("write Vertex response");
        });

        let credentials_path = std::env::temp_dir().join(format!(
            "zedflow-vertex-service-account-{}-{}.json",
            std::process::id(),
            now_ms()
        ));
        std::fs::write(
            &credentials_path,
            serde_json::to_vec(&json!({
                "project_id": "test-project",
                "private_key": PRIVATE_KEY,
                "client_email": "captured@test-project.iam.gserviceaccount.com",
                "token_uri": token_uri,
            }))
            .expect("serialize credentials"),
        )
        .expect("write credentials");

        let canonical_model = CanonicalModel {
            id: "gemini-3-flash-preview".to_owned(),
            name: "Gemini 3 Flash".to_owned(),
            api: "google-vertex".to_owned(),
            provider: "google-vertex".to_owned(),
            base_url,
            ..CanonicalModel::default()
        };
        let options = StreamOptions {
            api_key: Some(GCP_VERTEX_CREDENTIALS_MARKER.to_owned()),
            env: Some(HashMap::from([(
                "GOOGLE_APPLICATION_CREDENTIALS".to_owned(),
                credentials_path.to_string_lossy().into_owned(),
            )])),
            extra: HashMap::from([
                ("project".to_owned(), json!("test-project")),
                ("location".to_owned(), json!("us-central1")),
            ]),
            ..StreamOptions::default()
        };
        let message = futures::executor::block_on(
            stream_registered(
                &canonical_model,
                &CanonicalContext::default(),
                Some(&options),
            )
            .result(),
        );
        assert_eq!(
            message.response_id.as_deref(),
            Some("service-account-response")
        );
        assert_eq!(message.stop_reason, CanonicalStopReason::Stop);
    }

    #[test]
    fn registered_stream_normalizes_vertex_http_errors() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture server");
        let base_url = format!("http://{}", listener.local_addr().expect("capture address"));
        thread::spawn(move || {
            let (mut socket, _) = listener.accept().expect("accept request");
            let mut request = [0_u8; 4096];
            assert!(socket.read(&mut request).expect("read request") > 0);
            let body = r#"{"error":{"message":"permission denied"}}"#;
            write!(
                socket,
                "HTTP/1.1 403 Forbidden\r\ncontent-length: {}\r\n\r\n{}",
                body.len(),
                body
            )
            .expect("write response");
        });
        let canonical_model = CanonicalModel {
            id: "gemini-3-flash-preview".to_owned(),
            name: "Gemini 3 Flash".to_owned(),
            api: "google-vertex".to_owned(),
            provider: "google-vertex".to_owned(),
            base_url,
            ..CanonicalModel::default()
        };
        let options = StreamOptions {
            api_key: Some("captured-key".to_owned()),
            ..StreamOptions::default()
        };

        let message = futures::executor::block_on(
            stream_registered(
                &canonical_model,
                &CanonicalContext::default(),
                Some(&options),
            )
            .result(),
        );
        assert_eq!(message.stop_reason, CanonicalStopReason::Error);
        assert!(
            message
                .error_message
                .as_deref()
                .is_some_and(|error| error.contains("permission denied") && error.contains("403"))
        );
    }

    #[test]
    fn registered_stream_emits_one_terminal_abort() {
        use futures::StreamExt;

        let controller = crate::utils::abort_signals::AbortController::new();
        controller.abort();
        let canonical_model = CanonicalModel {
            id: "gemini-3-flash-preview".to_owned(),
            name: "Gemini 3 Flash".to_owned(),
            api: "google-vertex".to_owned(),
            provider: "google-vertex".to_owned(),
            base_url: "http://127.0.0.1:1".to_owned(),
            ..CanonicalModel::default()
        };
        let options = StreamOptions {
            api_key: Some("captured-key".to_owned()),
            signal: Some(controller.signal()),
            ..StreamOptions::default()
        };
        let events = futures::executor::block_on(
            stream_registered(
                &canonical_model,
                &CanonicalContext::default(),
                Some(&options),
            )
            .collect::<Vec<_>>(),
        );
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            AssistantMessageEvent::Error {
                reason: ErrorStopReason::Aborted,
                error
            } if error.stop_reason == CanonicalStopReason::Aborted
        ));
    }
}
