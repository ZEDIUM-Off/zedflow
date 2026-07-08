//! Azure OpenAI Responses API ported from Pi.

use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const DEFAULT_AZURE_API_VERSION: &str = "v1";
const OPENAI_RESPONSES_MIN_OUTPUT_TOKENS: u32 = 16;

/// Result type for the Azure OpenAI Responses port.
pub type Result<T> = std::result::Result<T, AzureOpenAIResponsesError>;

/// Errors returned by the Azure OpenAI Responses port.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AzureOpenAIResponsesError {
    /// No API key was supplied for the model provider.
    MissingApiKey { provider: String },
    /// No Azure OpenAI base URL or resource name could be resolved.
    MissingBaseUrl,
    /// The supplied Azure OpenAI base URL is not a valid URL.
    InvalidBaseUrl { base_url: String },
    /// Provider transport failed before a stream could be produced.
    Transport(String),
}

impl fmt::Display for AzureOpenAIResponsesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApiKey { provider } => write!(f, "no API key for provider: {provider}"),
            Self::MissingBaseUrl => f.write_str(
                "Azure OpenAI base URL is required. Set AZURE_OPENAI_BASE_URL or AZURE_OPENAI_RESOURCE_NAME, or pass azureBaseUrl, azureResourceName, or model.baseUrl",
            ),
            Self::InvalidBaseUrl { base_url } => {
                write!(f, "invalid Azure OpenAI base URL: {base_url}")
            }
            Self::Transport(error) => f.write_str(error),
        }
    }
}

impl StdError for AzureOpenAIResponsesError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Transport(_)
            | Self::MissingApiKey { .. }
            | Self::MissingBaseUrl
            | Self::InvalidBaseUrl { .. } => None,
        }
    }
}

/// Provider-scoped environment overrides.
pub type ProviderEnv = HashMap<String, String>;

/// HTTP headers supplied by a model or request options.
pub type ProviderHeaders = HashMap<String, String>;

/// Pi thinking level accepted by Azure OpenAI Responses options.
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
    /// Extra-high reasoning effort.
    XHigh,
}

impl ThinkingLevel {
    fn as_str(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
        }
    }
}

/// Pi thinking level map key, including the `off` sentinel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelThinkingLevel {
    /// Disable reasoning when the provider supports an off mapping.
    Off,
    /// Minimal reasoning effort.
    Minimal,
    /// Low reasoning effort.
    Low,
    /// Medium reasoning effort.
    Medium,
    /// High reasoning effort.
    High,
    /// Extra-high reasoning effort.
    XHigh,
}

impl From<ThinkingLevel> for ModelThinkingLevel {
    fn from(value: ThinkingLevel) -> Self {
        match value {
            ThinkingLevel::Minimal => Self::Minimal,
            ThinkingLevel::Low => Self::Low,
            ThinkingLevel::Medium => Self::Medium,
            ThinkingLevel::High => Self::High,
            ThinkingLevel::XHigh => Self::XHigh,
        }
    }
}

/// Azure OpenAI reasoning summary preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningSummary {
    /// Let the provider choose the summary shape.
    Auto,
    /// Request a detailed reasoning summary.
    Detailed,
    /// Request a concise reasoning summary.
    Concise,
}

impl ReasoningSummary {
    fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Detailed => "detailed",
            Self::Concise => "concise",
        }
    }
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
    /// Provider/model-specific mappings for Pi thinking levels.
    pub thinking_level_map: HashMap<ModelThinkingLevel, Option<String>>,
    /// Default headers configured on the model.
    pub headers: ProviderHeaders,
}

/// Minimal context shape consumed by this port row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Context;

/// Prepared Azure OpenAI Responses request plus Pi stream options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AzureOpenAIResponsesRequest {
    /// Normalized Azure OpenAI base URL.
    pub base_url: String,
    /// Azure API version.
    pub api_version: String,
    /// Deployment name sent as the Responses model.
    pub deployment_name: String,
    /// Headers sent with the request.
    pub headers: ProviderHeaders,
    /// JSON body sent to `responses.create`.
    pub body: Value,
    /// Request timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Maximum retry attempts; Pi defaults this to zero.
    pub max_retries: u32,
}

/// Pi's event-stream handle for Azure OpenAI Responses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantMessageEventStream {
    /// Request captured before provider I/O starts; deterministic tests assert Pi parity here.
    pub request: AzureOpenAIResponsesRequest,
}

/// Azure OpenAI Responses-specific options.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AzureOpenAIResponsesOptions {
    /// Sampling temperature.
    pub temperature: Option<f64>,
    /// Maximum output tokens requested by the caller.
    pub max_tokens: Option<u32>,
    /// API key for Azure OpenAI.
    pub api_key: Option<String>,
    /// Optional session identifier used for prompt cache routing.
    pub session_id: Option<String>,
    /// Optional custom HTTP headers.
    pub headers: ProviderHeaders,
    /// Request timeout in milliseconds for SDKs that support it.
    pub timeout_ms: Option<u64>,
    /// Maximum retry attempts for SDKs that support it.
    pub max_retries: Option<u32>,
    /// Provider-scoped environment overrides.
    pub env: ProviderEnv,
    /// Reasoning effort.
    pub reasoning_effort: Option<ThinkingLevel>,
    /// Reasoning summary preference.
    pub reasoning_summary: Option<ReasoningSummary>,
    /// Azure OpenAI API version.
    pub azure_api_version: Option<String>,
    /// Azure OpenAI resource name.
    pub azure_resource_name: Option<String>,
    /// Azure OpenAI base URL.
    pub azure_base_url: Option<String>,
    /// Azure OpenAI deployment name.
    pub azure_deployment_name: Option<String>,
}

/// Options accepted by [`stream_simple`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SimpleStreamOptions {
    /// Sampling temperature.
    pub temperature: Option<f64>,
    /// Maximum output tokens requested by the caller.
    pub max_tokens: Option<u32>,
    /// API key for Azure OpenAI.
    pub api_key: Option<String>,
    /// Optional session identifier used for prompt cache routing.
    pub session_id: Option<String>,
    /// Optional custom HTTP headers.
    pub headers: ProviderHeaders,
    /// Request timeout in milliseconds for SDKs that support it.
    pub timeout_ms: Option<u64>,
    /// Maximum retry attempts for SDKs that support it.
    pub max_retries: Option<u32>,
    /// Provider-scoped environment overrides.
    pub env: ProviderEnv,
    /// Unified reasoning level passed to simple streams.
    pub reasoning: Option<ThinkingLevel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AzureConfig {
    base_url: String,
    api_version: String,
}

fn get_provider_env_value(name: &str, env: &ProviderEnv) -> Option<String> {
    env.get(name)
        .cloned()
        .or_else(|| std::env::var(name).ok())
        .filter(|value| !value.is_empty())
}

fn parse_deployment_name_map(value: Option<&str>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Some(value) = value else {
        return map;
    };

    for entry in value.split(',') {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((model_id, deployment_name)) = trimmed.split_once('=') else {
            continue;
        };
        if model_id.is_empty() || deployment_name.is_empty() {
            continue;
        }
        map.insert(
            model_id.trim().to_string(),
            deployment_name.trim().to_string(),
        );
    }

    map
}

fn resolve_deployment_name(model: &Model, options: Option<&AzureOpenAIResponsesOptions>) -> String {
    if let Some(deployment_name) =
        options.and_then(|options| options.azure_deployment_name.as_ref())
    {
        return deployment_name.clone();
    }

    let mapped_deployment = options
        .and_then(|options| {
            get_provider_env_value("AZURE_OPENAI_DEPLOYMENT_NAME_MAP", &options.env)
        })
        .and_then(|value| parse_deployment_name_map(Some(&value)).remove(&model.id));

    mapped_deployment.unwrap_or_else(|| model.id.clone())
}

fn normalize_azure_base_url(base_url: &str) -> Result<String> {
    let trimmed = base_url.trim().trim_end_matches('/');
    let Some(scheme_end) = trimmed.find("://") else {
        return Err(AzureOpenAIResponsesError::InvalidBaseUrl {
            base_url: base_url.to_string(),
        });
    };
    let host_start = scheme_end + 3;
    let after_scheme = &trimmed[host_start..];
    let host_end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    let host = &after_scheme[..host_end];
    if host.is_empty() {
        return Err(AzureOpenAIResponsesError::InvalidBaseUrl {
            base_url: base_url.to_string(),
        });
    }

    let suffix = &after_scheme[host_end..];
    let path = suffix.split(['?', '#']).next().unwrap_or_default();
    let normalized_path = path.trim_end_matches('/');
    let is_azure_host = host.ends_with(".openai.azure.com")
        || host.ends_with(".cognitiveservices.azure.com")
        || host.ends_with(".ai.azure.com");

    if is_azure_host
        && (normalized_path.is_empty()
            || normalized_path == "/openai"
            || normalized_path == "/openai/v1/responses")
    {
        return Ok(format!("{}{}/openai/v1", &trimmed[..host_start], host));
    }

    Ok(trimmed.to_string())
}

fn build_default_base_url(resource_name: &str) -> String {
    format!("https://{resource_name}.openai.azure.com/openai/v1")
}

fn resolve_azure_config(
    model: &Model,
    options: Option<&AzureOpenAIResponsesOptions>,
) -> Result<AzureConfig> {
    let empty_env = ProviderEnv::new();
    let env = options.map(|options| &options.env).unwrap_or(&empty_env);
    let api_version = options
        .and_then(|options| options.azure_api_version.clone())
        .or_else(|| get_provider_env_value("AZURE_OPENAI_API_VERSION", env))
        .unwrap_or_else(|| DEFAULT_AZURE_API_VERSION.to_string());

    let base_url = options
        .and_then(|options| options.azure_base_url.as_ref())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            get_provider_env_value("AZURE_OPENAI_BASE_URL", env)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        });
    let resource_name = options
        .and_then(|options| options.azure_resource_name.clone())
        .or_else(|| get_provider_env_value("AZURE_OPENAI_RESOURCE_NAME", env));

    let resolved_base_url = base_url
        .or_else(|| resource_name.as_deref().map(build_default_base_url))
        .or_else(|| model.base_url.clone())
        .ok_or(AzureOpenAIResponsesError::MissingBaseUrl)?;

    Ok(AzureConfig {
        base_url: normalize_azure_base_url(&resolved_base_url)?,
        api_version,
    })
}

fn clamp_max_output_tokens(value: Option<u32>) -> Option<u32> {
    value.map(|tokens| tokens.max(OPENAI_RESPONSES_MIN_OUTPUT_TOKENS))
}

fn mapped_reasoning_effort(model: &Model, effort: ThinkingLevel) -> String {
    model
        .thinking_level_map
        .get(&effort.into())
        .and_then(Clone::clone)
        .unwrap_or_else(|| effort.as_str().to_string())
}

fn off_reasoning_effort(model: &Model) -> Option<String> {
    match model.thinking_level_map.get(&ModelThinkingLevel::Off) {
        Some(None) => None,
        Some(Some(value)) => Some(value.clone()),
        None => Some("none".to_string()),
    }
}

/// Builds the HTTP request envelope used by the Azure OpenAI fallback.
pub fn build_request(
    model: &Model,
    _context: &Context,
    options: Option<&AzureOpenAIResponsesOptions>,
) -> Result<AzureOpenAIResponsesRequest> {
    let deployment_name = resolve_deployment_name(model, options);
    let azure_config = resolve_azure_config(model, options)?;
    let mut body = json!({
        "model": deployment_name,
        "input": [],
        "stream": true,
        "store": false,
    });
    if let Some(max_tokens) =
        options.and_then(|options| clamp_max_output_tokens(options.max_tokens))
    {
        body["max_output_tokens"] = json!(max_tokens);
    }
    if let Some(temperature) = options.and_then(|options| options.temperature) {
        body["temperature"] = json!(temperature);
    }
    if model.reasoning {
        let reasoning = options
            .and_then(|options| {
                options.reasoning_effort.map(|effort| {
                    let mut value = json!({ "effort": mapped_reasoning_effort(model, effort) });
                    if let Some(summary) = options.reasoning_summary.map(ReasoningSummary::as_str) {
                        value["summary"] = Value::String(summary.to_string());
                    }
                    value
                })
            })
            .or_else(|| off_reasoning_effort(model).map(|effort| json!({ "effort": effort })));
        if let Some(reasoning) = reasoning {
            body["reasoning"] = reasoning;
        }
    }
    if let Some(session_id) = options.and_then(|options| options.session_id.as_deref()) {
        body["prompt_cache_key"] = Value::String(clamp_openai_prompt_cache_key(session_id));
    }

    let headers = options
        .map(|options| options.headers.clone())
        .unwrap_or_default();
    Ok(AzureOpenAIResponsesRequest {
        base_url: azure_config.base_url,
        api_version: azure_config.api_version,
        deployment_name,
        headers,
        body,
        timeout_ms: options.and_then(|options| options.timeout_ms),
        max_retries: options.and_then(|options| options.max_retries).unwrap_or(0),
    })
}

fn clamp_openai_prompt_cache_key(key: &str) -> String {
    key.chars().take(64).collect()
}

/// Streams an Azure OpenAI Responses request by preparing the exact Pi request envelope.
///
/// Azure exact URL construction and hooks require raw request/response access, so this path uses a
/// narrow HTTP fallback boundary rather than `genai` normalization.
///
/// # Errors
///
/// Returns [`AzureOpenAIResponsesError::MissingApiKey`] when no API key is supplied or URL
/// resolution errors when Azure configuration is invalid.
pub fn stream(
    model: &Model,
    context: &Context,
    options: Option<&AzureOpenAIResponsesOptions>,
) -> Result<AssistantMessageEventStream> {
    if options
        .and_then(|options| options.api_key.as_deref())
        .is_none()
    {
        return Err(AzureOpenAIResponsesError::MissingApiKey {
            provider: model.provider.clone(),
        });
    }
    Ok(AssistantMessageEventStream {
        request: build_request(model, context, options)?,
    })
}

/// Streams an Azure OpenAI Responses request using simplified options.
///
/// # Errors
///
/// Returns [`AzureOpenAIResponsesError::MissingApiKey`] when no API key is supplied, or a port
/// placeholder until the OpenAI Responses streaming client is selected for Rust.
pub fn stream_simple(
    model: &Model,
    context: &Context,
    options: Option<&SimpleStreamOptions>,
) -> Result<AssistantMessageEventStream> {
    let options = options.cloned().unwrap_or_default();
    let Some(api_key) = options.api_key.clone() else {
        return Err(AzureOpenAIResponsesError::MissingApiKey {
            provider: model.provider.clone(),
        });
    };

    let stream_options = AzureOpenAIResponsesOptions {
        temperature: options.temperature,
        max_tokens: options.max_tokens,
        api_key: Some(api_key),
        session_id: options.session_id,
        headers: options.headers,
        timeout_ms: options.timeout_ms,
        max_retries: options.max_retries,
        env: options.env,
        reasoning_effort: options.reasoning,
        ..AzureOpenAIResponsesOptions::default()
    };

    stream(model, context, Some(&stream_options))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Model {
        Model {
            id: "gpt-5".to_string(),
            provider: "azure-openai-responses".to_string(),
            base_url: None,
            reasoning: true,
            thinking_level_map: HashMap::new(),
            headers: HashMap::new(),
        }
    }

    #[test]
    fn parses_deployment_name_map_like_pi() {
        let map = parse_deployment_name_map(Some(" gpt-5 = dep-a ,,bad,gpt-4= dep-b "));
        assert_eq!(map.get("gpt-5"), Some(&"dep-a".to_string()));
        assert_eq!(map.get("gpt-4"), Some(&"dep-b".to_string()));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn normalizes_azure_hosts_to_openai_v1() {
        let normalized =
            normalize_azure_base_url("https://example.openai.azure.com/openai/v1/responses?x=1")
                .expect("valid azure URL");
        assert_eq!(normalized, "https://example.openai.azure.com/openai/v1");
    }

    #[test]
    fn resolves_resource_name_to_default_base_url() {
        let mut options = AzureOpenAIResponsesOptions::default();
        options.azure_resource_name = Some("my-resource".to_string());

        let config = resolve_azure_config(&model(), Some(&options)).expect("resource name config");

        assert_eq!(
            config.base_url,
            "https://my-resource.openai.azure.com/openai/v1"
        );
        assert_eq!(config.api_version, "v1");
    }

    #[test]
    fn clamps_max_output_tokens_to_openai_minimum() {
        assert_eq!(clamp_max_output_tokens(Some(1)), Some(16));
        assert_eq!(clamp_max_output_tokens(Some(20)), Some(20));
        assert_eq!(clamp_max_output_tokens(None), None);
    }

    #[test]
    fn reasoning_mapping_matches_pi_defaults() {
        let mut model = model();
        model
            .thinking_level_map
            .insert(ModelThinkingLevel::Low, Some("medium".to_string()));
        model
            .thinking_level_map
            .insert(ModelThinkingLevel::Off, None);

        assert_eq!(
            mapped_reasoning_effort(&model, ThinkingLevel::Low),
            "medium"
        );
        assert_eq!(mapped_reasoning_effort(&model, ThinkingLevel::High), "high");
        assert_eq!(off_reasoning_effort(&model), None);
    }
}
