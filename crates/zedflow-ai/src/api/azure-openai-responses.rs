//! Azure OpenAI Responses API ported from Pi.

#![allow(
    clippy::result_large_err,
    reason = "preserve partial streamed state in provider errors"
)]

use std::collections::{HashMap, HashSet};
use std::error::Error as StdError;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
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

/// Returns the canonical Azure OpenAI Responses production streams.
#[must_use]
pub fn provider_streams() -> crate::types::ProviderStreams {
    crate::types::ProviderStreams {
        stream: Arc::new(stream_registered),
        stream_simple: Arc::new(stream_simple_registered),
    }
}

/// Starts the canonical Azure OpenAI Responses production stream.
#[must_use]
pub fn stream_registered(
    model: &crate::types::Model,
    context: &crate::types::Context,
    options: Option<&crate::types::StreamOptions>,
) -> crate::types::AssistantMessageEventStream {
    let stream = crate::types::AssistantMessageEventStream::new();
    let worker_stream = stream.clone();
    let model = model.clone();
    let context = context.clone();
    let options = options.cloned().unwrap_or_default();
    crate::utils::runtime::spawn_worker(async move {
        run_registered_worker(worker_stream, model, context, options).await;
    });
    stream
}

/// Starts the canonical simple Azure OpenAI Responses stream.
#[must_use]
pub fn stream_simple_registered(
    model: &crate::types::Model,
    context: &crate::types::Context,
    options: Option<&crate::types::SimpleStreamOptions>,
) -> crate::types::AssistantMessageEventStream {
    let mut options = options
        .map(|options| options.stream.clone())
        .unwrap_or_default();
    if let Some(reasoning) = options
        .extra
        .remove("reasoning")
        .and_then(|value| value.as_str().map(str::to_owned))
    {
        options
            .extra
            .insert("reasoningEffort".to_owned(), Value::String(reasoning));
    }
    stream_registered(model, context, Some(&options))
}

#[derive(Debug)]
struct RegisteredLiveError {
    message: String,
    status: Option<u16>,
    body: Option<String>,
    partial: Option<crate::api::openai_responses_shared::AssistantMessage>,
}

impl RegisteredLiveError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: None,
            body: None,
            partial: None,
        }
    }

    fn http(status: u16, body: String) -> Self {
        Self {
            message: format!("HTTP {status}"),
            status: Some(status),
            body: Some(body),
            partial: None,
        }
    }

    fn with_partial(
        message: impl Into<String>,
        partial: &crate::api::openai_responses_shared::AssistantMessage,
    ) -> Self {
        Self {
            message: message.into(),
            status: None,
            body: None,
            partial: Some(partial.clone()),
        }
    }

    fn formatted_message(&self) -> String {
        let mut parts = crate::utils::error_body::ProviderHttpErrorParts::new(&self.message);
        if let Some(status) = self.status {
            parts = parts.with_status(status);
        }
        if let Some(body) = &self.body {
            parts = parts.with_body(body);
        }
        let normalized = crate::utils::error_body::normalize_provider_http_error(parts);
        crate::utils::error_body::format_provider_error(
            &normalized.normalized,
            Some("Azure OpenAI API error"),
        )
    }
}

async fn run_registered_worker(
    stream: crate::types::AssistantMessageEventStream,
    model: crate::types::Model,
    context: crate::types::Context,
    options: crate::types::StreamOptions,
) {
    let result: std::result::Result<_, RegisteredLiveError> = async {
        let (request, responses_model) = build_registered_request(&model, &context, &options)?;
        let mut body = request.body;
        if let Some(hook) = options.on_payload.as_ref()
            && let Some(next) = hook(body.clone(), model.clone())
                .await
                .map_err(|error| RegisteredLiveError::new(error.to_string()))?
        {
            body = next;
        }
        let message = execute_registered_request(
            &stream,
            &model,
            &responses_model,
            &request.base_url,
            &request.api_version,
            &request.headers,
            body,
            &options,
        )
        .await?;
        check_abort(options.signal.as_ref()).map_err(RegisteredLiveError::new)?;
        Ok(message)
    }
    .await;

    match result {
        Ok(message) => {
            let output = canonical_message(&message, &model, None);
            stream.push(crate::types::AssistantMessageEvent::Done {
                reason: canonical_done_reason(output.stop_reason),
                message: output,
            });
        }
        Err(error) => {
            let aborted = error.message == "Request was aborted";
            let error_message = error.formatted_message();
            let mut output = error.partial.as_ref().map_or_else(
                || empty_canonical_message(&model),
                |partial| canonical_message(partial, &model, None),
            );
            output.stop_reason = if aborted {
                crate::types::StopReason::Aborted
            } else {
                crate::types::StopReason::Error
            };
            output.error_message = Some(error_message);
            stream.push(crate::types::AssistantMessageEvent::Error {
                reason: if aborted {
                    crate::types::ErrorStopReason::Aborted
                } else {
                    crate::types::ErrorStopReason::Error
                },
                error: output,
            });
        }
    }
}

fn build_registered_request(
    model: &crate::types::Model,
    context: &crate::types::Context,
    options: &crate::types::StreamOptions,
) -> std::result::Result<
    (
        AzureOpenAIResponsesRequest,
        crate::api::openai_responses_shared::Model,
    ),
    RegisteredLiveError,
> {
    let headers = merged_registered_headers(model, options);
    if options.api_key.as_deref().is_none_or(str::is_empty) {
        return Err(RegisteredLiveError::new(format!(
            "no API key for provider: {}",
            model.provider
        )));
    }

    let local_model = Model {
        id: model.id.clone(),
        provider: model.provider.clone(),
        base_url: (!model.base_url.trim().is_empty()).then(|| model.base_url.clone()),
        reasoning: model.reasoning,
        thinking_level_map: registered_thinking_map(model),
        headers: HashMap::new(),
    };
    let local_options = registered_azure_options(options, headers);
    let deployment_name = resolve_deployment_name(&local_model, Some(&local_options));
    let config = resolve_azure_config(&local_model, Some(&local_options))
        .map_err(|error| RegisteredLiveError::new(error.to_string()))?;

    let responses_model = shared_registered_model(model);
    let shared_context = crate::api::openai_responses_shared::Context {
        system_prompt: context.system_prompt.clone(),
        messages: context
            .messages
            .iter()
            .filter_map(|message| serde_json::to_value(message).ok())
            .filter_map(|value| serde_json::from_value(canonical_to_shared_json(value)).ok())
            .collect(),
    };
    let allowed = HashSet::from([
        "openai".to_owned(),
        "openai-codex".to_owned(),
        "opencode".to_owned(),
        "azure-openai-responses".to_owned(),
    ]);
    let responses_context = crate::api::openai_responses::Context {
        system_prompt: None,
        messages: crate::api::openai_responses_shared::convert_responses_messages(
            &responses_model,
            &shared_context,
            &allowed,
            None,
        ),
        tools: context
            .tools
            .as_ref()
            .into_iter()
            .flatten()
            .map(|tool| crate::api::openai_responses::Tool {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            })
            .collect(),
        copilot_messages: Vec::new(),
    };
    let responses_request = crate::api::openai_responses::build_request(
        &crate::api::openai_responses::Model {
            id: deployment_name.clone(),
            api: model.api.clone(),
            provider: model.provider.clone(),
            base_url: config.base_url.clone(),
            reasoning: model.reasoning,
            thinking_level_map: registered_responses_thinking_map(model),
            headers: HashMap::new(),
            compat: None,
        },
        &responses_context,
        Some(&crate::api::openai_responses::OpenAIResponsesOptions {
            temperature: options.temperature,
            max_tokens: options.max_tokens,
            api_key: options.api_key.clone(),
            session_id: options.session_id.clone(),
            env: options.env.clone().unwrap_or_default(),
            reasoning_effort: registered_reasoning_effort(options),
            reasoning_summary: registered_reasoning_summary(options),
            timeout_ms: options.timeout_ms,
            max_retries: options.max_retries,
            ..crate::api::openai_responses::OpenAIResponsesOptions::default()
        }),
    )
    .map_err(|error| RegisteredLiveError::new(error.to_string()))?;

    Ok((
        AzureOpenAIResponsesRequest {
            base_url: config.base_url,
            api_version: config.api_version,
            deployment_name,
            headers: local_options.headers,
            body: responses_request.body,
            timeout_ms: options.timeout_ms,
            max_retries: options.max_retries.unwrap_or(0),
        },
        responses_model,
    ))
}

fn registered_azure_options(
    options: &crate::types::StreamOptions,
    headers: ProviderHeaders,
) -> AzureOpenAIResponsesOptions {
    AzureOpenAIResponsesOptions {
        api_key: options.api_key.clone(),
        headers,
        env: options.env.clone().unwrap_or_default(),
        azure_api_version: extra_string(options, "azureApiVersion"),
        azure_resource_name: extra_string(options, "azureResourceName"),
        azure_base_url: extra_string(options, "azureBaseUrl"),
        azure_deployment_name: extra_string(options, "azureDeploymentName"),
        ..AzureOpenAIResponsesOptions::default()
    }
}

fn extra_string(options: &crate::types::StreamOptions, name: &str) -> Option<String> {
    options
        .extra
        .get(name)
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn merged_registered_headers(
    model: &crate::types::Model,
    options: &crate::types::StreamOptions,
) -> ProviderHeaders {
    let mut headers = model.headers.clone().unwrap_or_default();
    if let Some(option_headers) = &options.headers {
        headers.extend(
            option_headers.iter().filter_map(|(name, value)| {
                value.as_ref().map(|value| (name.clone(), value.clone()))
            }),
        );
    }
    headers
}

fn registered_thinking_map(
    model: &crate::types::Model,
) -> HashMap<ModelThinkingLevel, Option<String>> {
    model
        .thinking_level_map
        .as_ref()
        .into_iter()
        .flatten()
        .map(|(level, value)| {
            let level = match level {
                crate::types::ModelThinkingLevel::Off => ModelThinkingLevel::Off,
                crate::types::ModelThinkingLevel::Minimal => ModelThinkingLevel::Minimal,
                crate::types::ModelThinkingLevel::Low => ModelThinkingLevel::Low,
                crate::types::ModelThinkingLevel::Medium => ModelThinkingLevel::Medium,
                crate::types::ModelThinkingLevel::High => ModelThinkingLevel::High,
                crate::types::ModelThinkingLevel::XHigh => ModelThinkingLevel::XHigh,
            };
            (level, value.clone())
        })
        .collect()
}

fn registered_responses_thinking_map(
    model: &crate::types::Model,
) -> HashMap<crate::api::openai_responses::ModelThinkingLevel, Option<String>> {
    model
        .thinking_level_map
        .as_ref()
        .into_iter()
        .flatten()
        .map(|(level, value)| {
            use crate::api::openai_responses::ModelThinkingLevel as Level;
            let level = match level {
                crate::types::ModelThinkingLevel::Off => Level::Off,
                crate::types::ModelThinkingLevel::Minimal => Level::Minimal,
                crate::types::ModelThinkingLevel::Low => Level::Low,
                crate::types::ModelThinkingLevel::Medium => Level::Medium,
                crate::types::ModelThinkingLevel::High => Level::High,
                crate::types::ModelThinkingLevel::XHigh => Level::XHigh,
            };
            (level, value.clone())
        })
        .collect()
}

fn registered_reasoning_effort(
    options: &crate::types::StreamOptions,
) -> Option<crate::api::openai_responses::ReasoningEffort> {
    use crate::api::openai_responses::ReasoningEffort;
    match extra_string(options, "reasoningEffort").as_deref()? {
        "minimal" => Some(ReasoningEffort::Minimal),
        "low" => Some(ReasoningEffort::Low),
        "medium" => Some(ReasoningEffort::Medium),
        "high" => Some(ReasoningEffort::High),
        "xhigh" => Some(ReasoningEffort::XHigh),
        _ => None,
    }
}

fn registered_reasoning_summary(
    options: &crate::types::StreamOptions,
) -> Option<crate::api::openai_responses::ReasoningSummary> {
    use crate::api::openai_responses::ReasoningSummary;
    match extra_string(options, "reasoningSummary").as_deref()? {
        "auto" => Some(ReasoningSummary::Auto),
        "detailed" => Some(ReasoningSummary::Detailed),
        "concise" => Some(ReasoningSummary::Concise),
        _ => None,
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "registered transport dependencies stay explicit"
)]
async fn execute_registered_request(
    stream: &crate::types::AssistantMessageEventStream,
    model: &crate::types::Model,
    responses_model: &crate::api::openai_responses_shared::Model,
    base_url: &str,
    api_version: &str,
    headers: &ProviderHeaders,
    body: Value,
    options: &crate::types::StreamOptions,
) -> std::result::Result<crate::api::openai_responses_shared::AssistantMessage, RegisteredLiveError>
{
    check_abort(options.signal.as_ref()).map_err(RegisteredLiveError::new)?;
    let mut url = reqwest::Url::parse(base_url)
        .map_err(|error| RegisteredLiveError::new(error.to_string()))?;
    url.set_path(&format!("{}/responses", url.path().trim_end_matches('/')));
    url.query_pairs_mut()
        .append_pair("api-version", api_version);
    let mut client = reqwest::Client::builder();
    if let Some(timeout_ms) = options.timeout_ms {
        client = client.timeout(Duration::from_millis(timeout_ms));
    }
    let client = client
        .build()
        .map_err(|error| RegisteredLiveError::new(error.to_string()))?;
    let request_headers =
        azure_headers(options.api_key.as_deref(), headers).map_err(RegisteredLiveError::new)?;
    let body =
        serde_json::to_vec(&body).map_err(|error| RegisteredLiveError::new(error.to_string()))?;
    let mut attempts = 0;
    let response = loop {
        let response = await_or_abort(
            client
                .post(url.clone())
                .headers(request_headers.clone())
                .body(body.clone())
                .send(),
            options.signal.clone(),
        )
        .await;
        match response {
            Ok(response)
                if is_retryable_azure_status(response.status().as_u16())
                    && attempts < options.max_retries.unwrap_or(0) =>
            {
                attempts += 1;
            }
            Ok(response) => break response,
            Err(error)
                if error != "Request was aborted"
                    && attempts < options.max_retries.unwrap_or(0) =>
            {
                attempts += 1;
            }
            Err(error) => return Err(RegisteredLiveError::new(error)),
        }
    };
    if let Some(hook) = options.on_response.as_ref() {
        hook(
            crate::types::ProviderResponse {
                status: response.status().as_u16(),
                headers: response
                    .headers()
                    .iter()
                    .filter_map(|(name, value)| {
                        value
                            .to_str()
                            .ok()
                            .map(|value| (name.to_string(), value.to_owned()))
                    })
                    .collect(),
            },
            model.clone(),
        )
        .await
        .map_err(|error| RegisteredLiveError::new(error.to_string()))?;
    }
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = await_or_abort(response.text(), options.signal.clone())
            .await
            .map_err(RegisteredLiveError::new)?;
        return Err(RegisteredLiveError::http(status, body));
    }

    let initial = empty_shared_message(model);
    stream.push(crate::types::AssistantMessageEvent::Start {
        partial: canonical_message(&initial, model, None),
    });
    let mut decoder = AzureSseDecoder::default();
    let mut bytes = response.bytes_stream();
    let mut processor = crate::api::openai_responses_shared::ResponsesStreamProcessor::default();
    let mut latest = initial;
    loop {
        let next = next_bytes_or_abort(&mut bytes, options.signal.clone())
            .await
            .map_err(|error| RegisteredLiveError::with_partial(error, &latest))?;
        let Some(bytes) = next else {
            return Err(RegisteredLiveError::with_partial(
                "Azure OpenAI Responses stream ended before a terminal response event",
                &latest,
            ));
        };
        for frame in decoder
            .push(bytes.as_ref())
            .map_err(|error| RegisteredLiveError::with_partial(error, &latest))?
        {
            check_abort(options.signal.as_ref())
                .map_err(|error| RegisteredLiveError::with_partial(error, &latest))?;
            if frame == "[DONE]" {
                return Err(RegisteredLiveError::with_partial(
                    "Azure OpenAI Responses stream ended before a terminal response event",
                    &latest,
                ));
            }
            let event = parse_response_event(&frame)
                .map_err(|error| RegisteredLiveError::with_partial(error, &latest))?;
            let mut generated = Vec::new();
            let terminal = processor
                .push(event, &mut latest, &mut generated, responses_model, None)
                .map_err(|error| RegisteredLiveError::with_partial(error.to_string(), &latest))?;
            for event in &generated {
                push_canonical_event(stream, event, model);
            }
            if terminal {
                check_abort(options.signal.as_ref())
                    .map_err(|error| RegisteredLiveError::with_partial(error, &latest))?;
                return Ok(latest);
            }
        }
    }
}

fn is_retryable_azure_status(status: u16) -> bool {
    matches!(status, 408 | 409 | 429 | 500..=599)
}

fn azure_headers(
    api_key: Option<&str>,
    headers: &ProviderHeaders,
) -> std::result::Result<HeaderMap, String> {
    let mut map = HeaderMap::new();
    map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let api_key = api_key
        .filter(|key| !key.is_empty())
        .ok_or_else(|| "missing Azure OpenAI API key".to_owned())?;
    map.insert(
        HeaderName::from_static("api-key"),
        HeaderValue::from_str(api_key).map_err(|error| error.to_string())?,
    );
    for (name, value) in headers {
        map.insert(
            HeaderName::from_bytes(name.as_bytes()).map_err(|error| error.to_string())?,
            HeaderValue::from_str(value).map_err(|error| error.to_string())?,
        );
    }
    Ok(map)
}

async fn await_or_abort<T>(
    future: impl std::future::Future<Output = std::result::Result<T, reqwest::Error>>,
    signal: Option<crate::types::AbortSignal>,
) -> std::result::Result<T, String> {
    if let Some(signal) = signal {
        match futures::future::select(Box::pin(future), Box::pin(wait_abort(signal))).await {
            futures::future::Either::Left((result, _)) => result.map_err(|error| error.to_string()),
            futures::future::Either::Right(((), _)) => Err("Request was aborted".to_owned()),
        }
    } else {
        future.await.map_err(|error| error.to_string())
    }
}

async fn next_bytes_or_abort<S, B>(
    stream: &mut S,
    signal: Option<crate::types::AbortSignal>,
) -> std::result::Result<Option<B>, String>
where
    S: futures::Stream<Item = std::result::Result<B, reqwest::Error>> + Unpin,
    B: AsRef<[u8]>,
{
    if let Some(signal) = signal {
        match futures::future::select(Box::pin(stream.next()), Box::pin(wait_abort(signal))).await {
            futures::future::Either::Left((result, _)) => {
                result.transpose().map_err(|error| error.to_string())
            }
            futures::future::Either::Right(((), _)) => Err("Request was aborted".to_owned()),
        }
    } else {
        stream
            .next()
            .await
            .transpose()
            .map_err(|error| error.to_string())
    }
}

async fn wait_abort(signal: crate::types::AbortSignal) {
    while !signal.aborted() {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

fn check_abort(signal: Option<&crate::types::AbortSignal>) -> std::result::Result<(), String> {
    if signal.is_some_and(|signal| signal.aborted()) {
        Err("Request was aborted".to_owned())
    } else {
        Ok(())
    }
}

#[derive(Default)]
struct AzureSseDecoder {
    buffer: Vec<u8>,
}

impl AzureSseDecoder {
    fn push(&mut self, bytes: &[u8]) -> std::result::Result<Vec<String>, String> {
        self.buffer.extend_from_slice(bytes);
        let mut frames = Vec::new();
        while let Some((end, separator_len)) = find_sse_separator(&self.buffer) {
            let frame = self.buffer.drain(..end + separator_len).collect::<Vec<_>>();
            let text = std::str::from_utf8(&frame[..end])
                .map_err(|error| format!("Azure OpenAI Responses stream UTF-8 error: {error}"))?;
            let data = text
                .lines()
                .filter_map(|line| {
                    line.trim_end_matches('\r')
                        .strip_prefix("data:")
                        .map(str::trim)
                })
                .collect::<Vec<_>>()
                .join("\n");
            if !data.is_empty() {
                frames.push(data);
            }
        }
        Ok(frames)
    }
}

fn find_sse_separator(buffer: &[u8]) -> Option<(usize, usize)> {
    buffer
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| (index, 4))
        .or_else(|| {
            buffer
                .windows(2)
                .position(|window| window == b"\n\n")
                .map(|index| (index, 2))
        })
}

fn parse_response_event(
    data: &str,
) -> std::result::Result<crate::api::openai_responses_shared::ResponseStreamEvent, String> {
    let mut value: Value = serde_json::from_str(data)
        .map_err(|error| format!("Azure OpenAI Responses stream JSON error: {error}"))?;
    if let Some(kind) = value
        .get("type")
        .and_then(Value::as_str)
        .map(|kind| kind.replace('.', "_"))
    {
        value["type"] = Value::String(kind);
    }
    serde_json::from_value(value)
        .map_err(|error| format!("Azure OpenAI Responses stream JSON error: {error}"))
}

fn shared_registered_model(
    model: &crate::types::Model,
) -> crate::api::openai_responses_shared::Model {
    crate::api::openai_responses_shared::Model {
        id: model.id.clone(),
        api: model.api.clone(),
        provider: model.provider.clone(),
        reasoning: model.reasoning,
        input: model
            .input
            .iter()
            .map(|input| match input {
                crate::types::ModelInput::Text => "text".to_owned(),
                crate::types::ModelInput::Image => "image".to_owned(),
            })
            .collect(),
        cost: crate::api::openai_responses_shared::ModelCost {
            input: model.cost.input,
            output: model.cost.output,
            cache_read: model.cost.cache_read,
            cache_write: model.cost.cache_write,
        },
        compat: None,
    }
}

fn empty_shared_message(
    model: &crate::types::Model,
) -> crate::api::openai_responses_shared::AssistantMessage {
    crate::api::openai_responses_shared::AssistantMessage {
        content: Vec::new(),
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        response_id: None,
        usage: crate::api::openai_responses_shared::Usage::default(),
        stop_reason: crate::api::openai_responses_shared::StopReason::Stop,
    }
}

fn push_canonical_event(
    stream: &crate::types::AssistantMessageEventStream,
    event: &crate::api::openai_responses_shared::AssistantMessageEvent,
    model: &crate::types::Model,
) {
    use crate::api::openai_responses_shared::AssistantMessageEvent as Shared;
    match event {
        Shared::ThinkingStart {
            content_index,
            partial,
        } => stream.push(crate::types::AssistantMessageEvent::ThinkingStart {
            content_index: *content_index,
            partial: canonical_message(partial, model, None),
        }),
        Shared::ThinkingDelta {
            content_index,
            delta,
            partial,
        } => stream.push(crate::types::AssistantMessageEvent::ThinkingDelta {
            content_index: *content_index,
            delta: delta.clone(),
            partial: canonical_message(partial, model, None),
        }),
        Shared::ThinkingEnd {
            content_index,
            content,
            partial,
        } => stream.push(crate::types::AssistantMessageEvent::ThinkingEnd {
            content_index: *content_index,
            content: content.clone(),
            partial: canonical_message(partial, model, None),
        }),
        Shared::TextStart {
            content_index,
            partial,
        } => stream.push(crate::types::AssistantMessageEvent::TextStart {
            content_index: *content_index,
            partial: canonical_message(partial, model, None),
        }),
        Shared::TextDelta {
            content_index,
            delta,
            partial,
        } => stream.push(crate::types::AssistantMessageEvent::TextDelta {
            content_index: *content_index,
            delta: delta.clone(),
            partial: canonical_message(partial, model, None),
        }),
        Shared::TextEnd {
            content_index,
            content,
            partial,
        } => stream.push(crate::types::AssistantMessageEvent::TextEnd {
            content_index: *content_index,
            content: content.clone(),
            partial: canonical_message(partial, model, None),
        }),
        Shared::ToolCallStart {
            content_index,
            partial,
        } => stream.push(crate::types::AssistantMessageEvent::ToolcallStart {
            content_index: *content_index,
            partial: canonical_message(partial, model, None),
        }),
        Shared::ToolCallDelta {
            content_index,
            delta,
            partial,
        } => stream.push(crate::types::AssistantMessageEvent::ToolcallDelta {
            content_index: *content_index,
            delta: delta.clone(),
            partial: canonical_message(partial, model, None),
        }),
        Shared::ToolCallEnd {
            content_index,
            tool_call,
            partial,
        } => stream.push(crate::types::AssistantMessageEvent::ToolcallEnd {
            content_index: *content_index,
            tool_call: canonical_tool_call(tool_call),
            partial: canonical_message(partial, model, None),
        }),
    }
}

fn canonical_message(
    message: &crate::api::openai_responses_shared::AssistantMessage,
    model: &crate::types::Model,
    error_message: Option<String>,
) -> crate::types::AssistantMessage {
    crate::types::AssistantMessage {
        role: crate::types::AssistantMessageRole::Assistant,
        content: message.content.iter().map(canonical_content).collect(),
        api: model.api.clone(),
        provider: message.provider.clone(),
        model: message.model.clone(),
        response_model: None,
        response_id: message.response_id.clone(),
        diagnostics: None,
        usage: crate::types::Usage {
            input: message.usage.input,
            output: message.usage.output,
            cache_read: message.usage.cache_read,
            cache_write: message.usage.cache_write,
            cache_write_1h: message.usage.cache_write_1h,
            reasoning: message.usage.reasoning,
            total_tokens: message.usage.total_tokens,
            ..crate::types::Usage::default()
        },
        stop_reason: canonical_stop_reason(message.stop_reason),
        error_message,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
    }
}

fn empty_canonical_message(model: &crate::types::Model) -> crate::types::AssistantMessage {
    let shared = empty_shared_message(model);
    canonical_message(&shared, model, None)
}

fn canonical_content(
    block: &crate::api::openai_responses_shared::AssistantContent,
) -> crate::types::AssistantContentBlock {
    match block {
        crate::api::openai_responses_shared::AssistantContent::Text(text) => {
            crate::types::AssistantContentBlock::Text(crate::types::TextContent {
                content_type: crate::types::TextContentType::Text,
                text: text.text.clone(),
                text_signature: text.text_signature.clone(),
            })
        }
        crate::api::openai_responses_shared::AssistantContent::Thinking(thinking) => {
            crate::types::AssistantContentBlock::Thinking(crate::types::ThinkingContent {
                content_type: crate::types::ThinkingContentType::Thinking,
                thinking: thinking.thinking.clone(),
                thinking_signature: thinking.thinking_signature.clone(),
                redacted: Some(thinking.redacted),
            })
        }
        crate::api::openai_responses_shared::AssistantContent::ToolCall(tool_call) => {
            crate::types::AssistantContentBlock::ToolCall(canonical_tool_call(tool_call))
        }
    }
}

fn canonical_tool_call(
    tool_call: &crate::api::openai_responses_shared::ToolCall,
) -> crate::types::ToolCall {
    crate::types::ToolCall {
        content_type: crate::types::ToolCallType::ToolCall,
        id: tool_call.id.clone(),
        name: tool_call.name.clone(),
        arguments: tool_call
            .arguments
            .as_object()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect(),
        thought_signature: tool_call.thought_signature.clone(),
    }
}

fn canonical_stop_reason(
    reason: crate::api::openai_responses_shared::StopReason,
) -> crate::types::StopReason {
    match reason {
        crate::api::openai_responses_shared::StopReason::Stop => crate::types::StopReason::Stop,
        crate::api::openai_responses_shared::StopReason::Length => crate::types::StopReason::Length,
        crate::api::openai_responses_shared::StopReason::ToolUse => {
            crate::types::StopReason::ToolUse
        }
        crate::api::openai_responses_shared::StopReason::Aborted => {
            crate::types::StopReason::Aborted
        }
        crate::api::openai_responses_shared::StopReason::Error => crate::types::StopReason::Error,
    }
}

fn canonical_done_reason(reason: crate::types::StopReason) -> crate::types::DoneStopReason {
    match reason {
        crate::types::StopReason::Length => crate::types::DoneStopReason::Length,
        crate::types::StopReason::ToolUse => crate::types::DoneStopReason::ToolUse,
        _ => crate::types::DoneStopReason::Stop,
    }
}

fn canonical_to_shared_json(value: Value) -> Value {
    match value {
        Value::Array(values) => {
            Value::Array(values.into_iter().map(canonical_to_shared_json).collect())
        }
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| {
                    let key = match key.as_str() {
                        "responseId" => "response_id",
                        "stopReason" => "stop_reason",
                        "toolCallId" => "tool_call_id",
                        "toolName" => "tool_name",
                        "isError" => "is_error",
                        "textSignature" => "text_signature",
                        "thinkingSignature" => "thinking_signature",
                        "mimeType" => "mime_type",
                        "thoughtSignature" => "thought_signature",
                        "cacheRead" => "cache_read",
                        "cacheWrite" => "cache_write",
                        "cacheWrite1h" => "cache_write_1h",
                        "totalTokens" => "total_tokens",
                        other => other,
                    }
                    .to_owned();
                    (key, canonical_to_shared_json(value))
                })
                .collect(),
        ),
        other => other,
    }
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
        let options = AzureOpenAIResponsesOptions {
            azure_resource_name: Some("my-resource".to_string()),
            ..AzureOpenAIResponsesOptions::default()
        };

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
