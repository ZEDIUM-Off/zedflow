//! Google Vertex API ported from Pi.

use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;
use std::sync::Arc;

use serde_json::{Value, json};

use crate::api::google_shared::{
    Context as SharedContext, FunctionCallingConfigMode, GenerateContentChunk,
    GoogleAssistantMessageEventStream, Model as SharedModel, ModelInput, Tool,
    collect_google_stream, convert_messages, convert_tools, map_tool_choice,
};

const API_VERSION: &str = "v1";
const GCP_VERTEX_CREDENTIALS_MARKER: &str = "gcp-vertex-credentials";

/// Result type for the Google Vertex port.
pub type Result<T> = std::result::Result<T, GoogleVertexError>;

/// Errors returned by the Google Vertex port.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GoogleVertexError {
    /// Vertex project ID was not provided in options or environment.
    MissingProject,
    /// Vertex location was not provided in options or environment.
    MissingLocation,
}

impl fmt::Display for GoogleVertexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingProject => f.write_str(
                "Vertex AI requires a project ID. Set GOOGLE_CLOUD_PROJECT/GCLOUD_PROJECT or pass project in options.",
            ),
            Self::MissingLocation => f.write_str(
                "Vertex AI requires a location. Set GOOGLE_CLOUD_LOCATION or pass location in options.",
            ),
        }
    }
}

impl StdError for GoogleVertexError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::MissingProject | Self::MissingLocation => None,
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
pub type PayloadHook = Arc<dyn Fn(Value, &Model) -> Option<Value> + Send + Sync>;

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
    env.get(name).filter(|value| !value.is_empty()).cloned()
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
    let api_key = options?.api_key.as_deref()?.trim();
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
    if !context.tools.is_empty() {
        if let Some(choice) = options.tool_choice {
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
        return format!(
            "{}/publishers/google/models/{}:streamGenerateContent?alt=sse",
            base_url.trim_end_matches('/'),
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
        && let Some(next_payload) = on_payload(payload.clone(), model)
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
                Some(payload)
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
}
