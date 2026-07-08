//! Internal `genai` backend helpers for provider transports.
//!
//! `genai` stays behind this module; public Zedflow/Pi types do not expose it.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::time::Duration;

use genai::adapter::AdapterKind;
use genai::chat::{CacheControl, ChatOptions};
use genai::resolver::{AuthData, Endpoint};
use genai::{Client, ServiceTarget, WebConfig};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;

use crate::types::{CacheRetention, ProviderEnv, ProviderHeaders, StreamOptions};
use crate::utils::error_body::{
    NormalizedProviderHttpError, ProviderHttpErrorParts, format_provider_error,
    normalize_provider_http_error, truncate_error_text,
};
use crate::utils::headers::provider_headers_to_record;
use crate::utils::node_http_proxy::{ProxyUrlError, resolve_reqwest_proxy_for_target};

/// Genai adapters used by current provider-port units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GenAiAdapter {
    OpenAi,
    OpenAiResponses,
    Anthropic,
    Gemini,
    Vertex,
    Fireworks,
    Together,
    Groq,
    Kimi,
    Moonshot,
    Xai,
    DeepSeek,
    Zai,
    OpenRouter,
    GitHubCopilot,
    OpenCodeGo,
    BedrockApi,
    MiniMax,
}

impl GenAiAdapter {
    fn into_genai(self) -> AdapterKind {
        match self {
            Self::OpenAi => AdapterKind::OpenAI,
            Self::OpenAiResponses => AdapterKind::OpenAIResp,
            Self::Anthropic => AdapterKind::Anthropic,
            Self::Gemini => AdapterKind::Gemini,
            Self::Vertex => AdapterKind::Vertex,
            Self::Fireworks => AdapterKind::Fireworks,
            Self::Together => AdapterKind::Together,
            Self::Groq => AdapterKind::Groq,
            Self::Kimi => AdapterKind::Kimi,
            Self::Moonshot => AdapterKind::Moonshot,
            Self::Xai => AdapterKind::Xai,
            Self::DeepSeek => AdapterKind::DeepSeek,
            Self::Zai => AdapterKind::Zai,
            Self::OpenRouter => AdapterKind::OpenRouter,
            Self::GitHubCopilot => AdapterKind::GithubCopilot,
            Self::OpenCodeGo => AdapterKind::OpenCodeGo,
            Self::BedrockApi => AdapterKind::BedrockApi,
            Self::MiniMax => AdapterKind::MiniMax,
        }
    }

    fn namespace(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::OpenAiResponses => "openai_resp",
            Self::Anthropic => "anthropic",
            Self::Gemini => "gemini",
            Self::Vertex => "vertex",
            Self::Fireworks => "fireworks",
            Self::Together => "together",
            Self::Groq => "groq",
            Self::Kimi => "kimi",
            Self::Moonshot => "moonshot",
            Self::Xai => "xai",
            Self::DeepSeek => "deepseek",
            Self::Zai => "zai",
            Self::OpenRouter => "open_router",
            Self::GitHubCopilot => "github_copilot",
            Self::OpenCodeGo => "opencode_go",
            Self::BedrockApi => "bedrock_api",
            Self::MiniMax => "minimax",
        }
    }
}

/// Provider-level configuration for a genai-backed client.
#[derive(Debug, Clone, Default)]
pub(crate) struct GenAiClientConfig {
    pub adapter: Option<GenAiAdapter>,
    pub endpoint: Option<String>,
    pub api_key: Option<String>,
    pub proxy_target_url: Option<String>,
    pub request: GenAiRequestConfig,
}

/// Request options copied from Pi stream options that genai can safely consume.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct GenAiRequestConfig {
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub timeout_ms: Option<u64>,
    pub headers: Option<ProviderHeaders>,
    pub env: Option<ProviderEnv>,
    pub cache_retention: Option<CacheRetention>,
    pub extra_body: Option<Value>,
    pub has_payload_hook: bool,
    pub has_response_hook: bool,
}

impl GenAiRequestConfig {
    pub(crate) fn from_stream_options<TApi>(options: &StreamOptions<TApi>) -> Self {
        Self {
            temperature: options.temperature,
            max_tokens: options.max_tokens,
            timeout_ms: options.timeout_ms,
            headers: options.headers.clone(),
            env: options.env.clone(),
            cache_retention: options.cache_retention,
            extra_body: None,
            has_payload_hook: options.on_payload.is_some(),
            has_response_hook: options.on_response.is_some(),
        }
    }
}

/// Errors while building a genai client/config.
#[derive(Debug)]
pub(crate) enum GenAiBackendConfigError {
    Proxy(ProxyUrlError),
    InvalidHeaderName { name: String, message: String },
    InvalidHeaderValue { name: String, message: String },
}

impl fmt::Display for GenAiBackendConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Proxy(source) => write!(formatter, "{source}"),
            Self::InvalidHeaderName { name, message } => {
                write!(formatter, "Invalid header name {name:?}: {message}")
            }
            Self::InvalidHeaderValue { name, message } => {
                write!(formatter, "Invalid header value for {name:?}: {message}")
            }
        }
    }
}

impl Error for GenAiBackendConfigError {}

impl From<ProxyUrlError> for GenAiBackendConfigError {
    fn from(source: ProxyUrlError) -> Self {
        Self::Proxy(source)
    }
}

/// Maps a Pi/provider id to the closest genai adapter, when genai is suitable.
pub(crate) fn adapter_for_provider_id(provider_id: &str) -> Option<GenAiAdapter> {
    match provider_id {
        "openai" => Some(GenAiAdapter::OpenAi),
        "openai-responses" => Some(GenAiAdapter::OpenAiResponses),
        "anthropic" => Some(GenAiAdapter::Anthropic),
        "google" => Some(GenAiAdapter::Gemini),
        "google-vertex" => Some(GenAiAdapter::Vertex),
        "fireworks" => Some(GenAiAdapter::Fireworks),
        "together" => Some(GenAiAdapter::Together),
        "groq" => Some(GenAiAdapter::Groq),
        "kimi-coding" => Some(GenAiAdapter::Kimi),
        "moonshotai" | "moonshotai-cn" => Some(GenAiAdapter::Moonshot),
        "xai" => Some(GenAiAdapter::Xai),
        "deepseek" => Some(GenAiAdapter::DeepSeek),
        "zai" | "zai-coding-cn" => Some(GenAiAdapter::Zai),
        "openrouter" => Some(GenAiAdapter::OpenRouter),
        "github-copilot" => Some(GenAiAdapter::GitHubCopilot),
        "opencode-go" => Some(GenAiAdapter::OpenCodeGo),
        "amazon-bedrock" => Some(GenAiAdapter::BedrockApi),
        "minimax" | "minimax-cn" => Some(GenAiAdapter::MiniMax),
        _ => None,
    }
}

/// Returns a namespaced model for genai adapters that require or prefer namespacing.
pub(crate) fn namespaced_model(adapter: GenAiAdapter, model: &str) -> String {
    let namespace = adapter.namespace();
    if model
        .split_once("::")
        .is_some_and(|(prefix, _)| prefix == namespace)
    {
        model.to_string()
    } else {
        format!("{namespace}::{model}")
    }
}

/// Builds a genai client with Pi-compatible proxy resolution and request defaults.
pub(crate) fn build_client(config: &GenAiClientConfig) -> Result<Client, GenAiBackendConfigError> {
    let web_config = web_config(config)?;
    let mut builder = Client::builder().with_web_config(web_config);

    if let Some(adapter) = config.adapter {
        builder = builder.with_adapter_kind(adapter.into_genai());
    }

    if let Some(api_key) = config.api_key.clone() {
        builder = builder
            .with_auth_resolver_fn(move |_| Ok(Some(AuthData::from_single(api_key.clone()))));
    }

    if let Some(endpoint) = config.endpoint.clone() {
        builder = builder.with_service_target_resolver_fn(move |mut target: ServiceTarget| {
            target.endpoint = Endpoint::from_owned(endpoint.clone());
            Ok(target)
        });
    }

    Ok(builder.build())
}

/// Converts safe Pi stream options into genai chat options.
pub(crate) fn chat_options(config: &GenAiRequestConfig) -> ChatOptions {
    let mut options = ChatOptions::default();
    if let Some(temperature) = config.temperature {
        options = options.with_temperature(temperature);
    }
    if let Some(max_tokens) = config.max_tokens {
        options = options.with_max_tokens(max_tokens);
    }
    if let Some(headers) = provider_headers_to_record(config.headers.as_ref()) {
        options = options.with_extra_headers(headers);
    }
    if let Some(cache_control) = cache_control(config.cache_retention) {
        options = options.with_cache_control(cache_control);
    }
    if let Some(extra_body) = config.extra_body.clone() {
        options = options.with_extra_body(extra_body);
    }
    options
}

/// Normalized genai/HTTP error data for provider-specific display code.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GenAiBackendError {
    pub http: NormalizedProviderHttpError,
    pub display: String,
}

impl GenAiBackendError {
    pub(crate) fn from_genai(error: &genai::Error, prefix: Option<&str>) -> Self {
        let http = match error {
            genai::Error::WebAdapterCall { webc_error, .. }
            | genai::Error::WebModelCall { webc_error, .. } => http_error_from_webc(webc_error),
            genai::Error::HttpError { status, body, .. } => normalize_provider_http_error(
                ProviderHttpErrorParts::new(error.to_string())
                    .with_status(status.as_u16())
                    .with_body(body.clone()),
            ),
            genai::Error::ChatResponse { body, .. } => normalize_provider_http_error(
                ProviderHttpErrorParts::new(error.to_string()).with_body(body.to_string()),
            ),
            genai::Error::ChatResponseGeneration {
                response_body,
                cause,
                ..
            } => normalize_provider_http_error(
                ProviderHttpErrorParts::new(cause.clone()).with_body(response_body.to_string()),
            ),
            _ => normalize_provider_http_error(ProviderHttpErrorParts::new(error.to_string())),
        };
        let display = format_provider_error(&http.normalized, prefix);
        Self { http, display }
    }

    pub(crate) fn from_reqwest(error: &reqwest::Error, prefix: Option<&str>) -> Self {
        let mut parts = ProviderHttpErrorParts::new(error.to_string());
        if let Some(status) = error.status() {
            parts = parts.with_status(status.as_u16());
        }
        let http = normalize_provider_http_error(parts);
        let display = format_provider_error(&http.normalized, prefix);
        Self { http, display }
    }
}

fn web_config(config: &GenAiClientConfig) -> Result<WebConfig, GenAiBackendConfigError> {
    let mut web_config = WebConfig::default();

    if let Some(timeout_ms) = config.request.timeout_ms {
        web_config = web_config.with_timeout(Duration::from_millis(timeout_ms));
    }

    if let Some(headers) = provider_headers_to_record(config.request.headers.as_ref()) {
        web_config = web_config.with_default_headers(header_map(headers)?);
    }

    let target_url = config
        .proxy_target_url
        .as_deref()
        .or(config.endpoint.as_deref());
    if let Some(target_url) = target_url {
        if let Some(proxy) =
            resolve_reqwest_proxy_for_target(target_url, config.request.env.as_ref())?
        {
            web_config = web_config.with_proxy(proxy);
        }
    }

    Ok(web_config)
}

fn header_map(headers: HashMap<String, String>) -> Result<HeaderMap, GenAiBackendConfigError> {
    let mut map = HeaderMap::with_capacity(headers.len());
    for (name, value) in headers {
        let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|source| {
            GenAiBackendConfigError::InvalidHeaderName {
                name: name.clone(),
                message: source.to_string(),
            }
        })?;
        let header_value = HeaderValue::from_str(&value).map_err(|source| {
            GenAiBackendConfigError::InvalidHeaderValue {
                name,
                message: source.to_string(),
            }
        })?;
        map.insert(header_name, header_value);
    }
    Ok(map)
}

fn cache_control(retention: Option<CacheRetention>) -> Option<CacheControl> {
    match retention {
        Some(CacheRetention::Short) => Some(CacheControl::Ephemeral),
        Some(CacheRetention::Long) => Some(CacheControl::Ephemeral1h),
        Some(CacheRetention::None) | None => None,
    }
}

fn http_error_from_webc(error: &genai::webc::Error) -> NormalizedProviderHttpError {
    match error {
        genai::webc::Error::ResponseFailedStatus {
            status,
            body,
            headers,
        } => normalize_provider_http_error(
            ProviderHttpErrorParts::new(error.to_string())
                .with_status(status.as_u16())
                .with_body(body.clone())
                .with_headers(header_record(headers)),
        ),
        genai::webc::Error::ResponseFailedNotJson { body, .. }
        | genai::webc::Error::ResponseFailedInvalidJson { body, .. } => {
            normalize_provider_http_error(
                ProviderHttpErrorParts::new(error.to_string()).with_body(body.clone()),
            )
        }
        genai::webc::Error::Reqwest(error) => GenAiBackendError::from_reqwest(error, None).http,
        _ => normalize_provider_http_error(ProviderHttpErrorParts::new(error.to_string())),
    }
}

fn header_record(headers: &HeaderMap) -> HashMap<String, String> {
    headers
        .iter()
        .map(|(key, value)| {
            (
                key.as_str().to_string(),
                String::from_utf8_lossy(value.as_bytes()).into_owned(),
            )
        })
        .collect()
}

/// Truncates error text with Pi's shared provider cap.
pub(crate) fn truncate_provider_error_text(text: &str) -> String {
    truncate_error_text(
        text,
        crate::utils::error_body::MAX_PROVIDER_ERROR_BODY_CHARS,
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::types::StreamOptions;

    #[test]
    fn maps_provider_ids_without_migrating_provider_modules() {
        assert_eq!(
            adapter_for_provider_id("openai"),
            Some(GenAiAdapter::OpenAi)
        );
        assert_eq!(
            adapter_for_provider_id("openrouter"),
            Some(GenAiAdapter::OpenRouter)
        );
        assert_eq!(adapter_for_provider_id("mistral"), None);
    }

    #[test]
    fn namespaces_models_once() {
        assert_eq!(
            namespaced_model(GenAiAdapter::OpenRouter, "openai/gpt-4.1"),
            "open_router::openai/gpt-4.1"
        );
        assert_eq!(
            namespaced_model(GenAiAdapter::OpenRouter, "open_router::openai/gpt-4.1"),
            "open_router::openai/gpt-4.1"
        );
    }

    #[test]
    fn copies_safe_stream_options_and_marks_hooks() {
        let options = StreamOptions::<crate::types::Api> {
            temperature: Some(0.2),
            max_tokens: Some(128),
            timeout_ms: Some(5000),
            cache_retention: Some(CacheRetention::Long),
            headers: Some(HashMap::from([(
                "x-test".to_string(),
                Some("1".to_string()),
            )])),
            ..StreamOptions::default()
        };

        let request = GenAiRequestConfig::from_stream_options(&options);
        let chat = chat_options(&request);

        assert_eq!(request.temperature, Some(0.2));
        assert_eq!(request.max_tokens, Some(128));
        assert_eq!(request.timeout_ms, Some(5000));
        assert_eq!(chat.temperature, Some(0.2));
        assert_eq!(chat.max_tokens, Some(128));
        assert_eq!(chat.cache_control, Some(CacheControl::Ephemeral1h));
        assert!(!request.has_payload_hook);
        assert!(!request.has_response_hook);
    }

    #[test]
    fn truncates_error_text_with_pi_cap() {
        let text = "x".repeat(crate::utils::error_body::MAX_PROVIDER_ERROR_BODY_CHARS + 2);
        let truncated = truncate_provider_error_text(&text);

        assert!(truncated.ends_with("... [truncated 2 chars]"));
    }
}
