//! OpenAI Responses API ported from Pi.

use std::collections::{HashMap, HashSet};
use std::error::Error as StdError;
use std::fmt;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::StreamExt;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::github_copilot_headers::{
    CopilotDynamicHeadersParams, Message as CopilotMessage, build_copilot_dynamic_headers,
    has_copilot_vision_input,
};

const OPENAI_RESPONSES_MIN_OUTPUT_TOKENS: u32 = 16;
const OPENAI_PROMPT_CACHE_KEY_MAX_LENGTH: usize = 64;

/// Result type for the OpenAI Responses port.
pub type Result<T> = std::result::Result<T, OpenAIResponsesError>;

/// Errors returned by the OpenAI Responses port.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum OpenAIResponsesError {
    /// No API key or authorization header was supplied for the model provider.
    MissingApiKey {
        /// Provider identifier from Pi.
        provider: String,
    },
    /// Provider transport failed before a stream could be produced.
    Transport(String),
}

impl fmt::Display for OpenAIResponsesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApiKey { provider } => write!(f, "no API key for provider: {provider}"),
            Self::Transport(error) => f.write_str(error),
        }
    }
}

impl StdError for OpenAIResponsesError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Transport(_) => None,
            Self::MissingApiKey { .. } => None,
        }
    }
}

/// HTTP headers supplied by a model or request options; `None` mirrors Pi's `null` value.
pub type ProviderHeaders = HashMap<String, Option<String>>;

/// Provider-scoped environment overrides.
pub type ProviderEnv = HashMap<String, String>;

/// Prompt cache retention preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum CacheRetention {
    /// Disable prompt caching.
    None,
    /// Use provider short retention.
    #[default]
    Short,
    /// Use provider long retention.
    Long,
}

/// OpenAI Responses reasoning effort accepted by Pi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
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

impl ReasoningEffort {
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

/// Pi thinking-level map key, including the `off` sentinel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelThinkingLevel {
    /// Disable reasoning.
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

impl From<ReasoningEffort> for ModelThinkingLevel {
    fn from(value: ReasoningEffort) -> Self {
        match value {
            ReasoningEffort::Minimal => Self::Minimal,
            ReasoningEffort::Low => Self::Low,
            ReasoningEffort::Medium => Self::Medium,
            ReasoningEffort::High => Self::High,
            ReasoningEffort::XHigh => Self::XHigh,
        }
    }
}

/// Reasoning summary preference accepted by OpenAI Responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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

/// Service tier values passed through to OpenAI Responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceTier {
    /// Let the provider choose the tier.
    Auto,
    /// Provider default tier.
    Default,
    /// Flex tier, priced at half cost in Pi.
    Flex,
    /// Priority tier, priced above default in Pi.
    Priority,
}

/// Compatibility settings for OpenAI Responses APIs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OpenAIResponsesCompat {
    /// Whether the provider supports the `developer` role instead of `system`.
    pub supports_developer_role: Option<bool>,
    /// Whether to send the OpenAI `session_id` cache-affinity header.
    pub send_session_id_header: Option<bool>,
    /// Whether the provider supports `prompt_cache_retention: "24h"`.
    pub supports_long_cache_retention: Option<bool>,
}

/// Resolved OpenAI Responses compatibility settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedOpenAIResponsesCompat {
    /// Whether the provider supports the `developer` role instead of `system`.
    pub supports_developer_role: bool,
    /// Whether to send the OpenAI `session_id` cache-affinity header.
    pub send_session_id_header: bool,
    /// Whether the provider supports `prompt_cache_retention: "24h"`.
    pub supports_long_cache_retention: bool,
}

/// Minimal model shape consumed by this port row.
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    /// Model identifier sent to the provider.
    pub id: String,
    /// API identifier from Pi.
    pub api: String,
    /// Provider identifier from Pi.
    pub provider: String,
    /// Provider base URL.
    pub base_url: String,
    /// Whether the model supports reasoning options.
    pub reasoning: bool,
    /// Provider/model-specific mappings for Pi thinking levels. `None` marks unsupported.
    pub thinking_level_map: HashMap<ModelThinkingLevel, Option<String>>,
    /// Default headers configured on the model.
    pub headers: ProviderHeaders,
    /// Optional OpenAI Responses provider overrides.
    pub compat: Option<OpenAIResponsesCompat>,
}

/// Minimal context shape consumed by this port row.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Context {
    /// Optional system prompt.
    pub system_prompt: Option<String>,
    /// Already-converted OpenAI Responses input items.
    pub messages: Vec<Value>,
    /// Available tool declarations.
    pub tools: Vec<Tool>,
    /// Pi messages used only for GitHub Copilot dynamic header inference.
    pub copilot_messages: Vec<CopilotMessage>,
}

/// Tool definition passed through to Responses as an OpenAI function tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tool {
    /// Tool name.
    pub name: String,
    /// Tool description.
    pub description: String,
    /// JSON Schema parameters generated by Pi's TypeBox tool schema.
    pub parameters: Value,
}

/// Usage and cost counters on a completed assistant message.
#[derive(Debug, Clone, PartialEq)]
pub struct Usage {
    /// Non-cached input tokens.
    pub input: f64,
    /// Output tokens.
    pub output: f64,
    /// Cached input tokens read.
    pub cache_read: f64,
    /// Cached input tokens written.
    pub cache_write: f64,
    /// Cost counters in provider currency units used by Pi.
    pub cost: UsageCost,
}

/// Cost counters for usage.
#[derive(Debug, Clone, PartialEq)]
pub struct UsageCost {
    /// Input-token cost.
    pub input: f64,
    /// Output-token cost.
    pub output: f64,
    /// Cache-read cost.
    pub cache_read: f64,
    /// Cache-write cost.
    pub cache_write: f64,
    /// Total cost.
    pub total: f64,
}

/// Provider HTTP response metadata exposed to response hooks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderResponse {
    /// HTTP status code.
    pub status: u16,
    /// HTTP response headers.
    pub headers: HashMap<String, String>,
}

/// Payload hook used by the OpenAI Responses transport.
pub type OpenAIResponsesPayloadHook = Arc<
    dyn Fn(
            Value,
            Model,
        ) -> futures::future::BoxFuture<
            'static,
            std::result::Result<Option<Value>, crate::types::ProviderHookError>,
        > + Send
        + Sync,
>;

/// Response hook used by the OpenAI Responses transport.
pub type OpenAIResponsesResponseHook = Arc<
    dyn Fn(
            ProviderResponse,
            Model,
        ) -> futures::future::BoxFuture<
            'static,
            std::result::Result<(), crate::types::ProviderHookError>,
        > + Send
        + Sync,
>;

/// OpenAI Responses-specific options.
#[derive(Clone, Default)]
pub struct OpenAIResponsesOptions {
    /// Sampling temperature.
    pub temperature: Option<f64>,
    /// Maximum output tokens requested by the caller.
    pub max_tokens: Option<u32>,
    /// API key for OpenAI-compatible providers.
    pub api_key: Option<String>,
    /// Optional session identifier used for prompt caching and request affinity.
    pub session_id: Option<String>,
    /// Optional custom HTTP headers.
    pub headers: ProviderHeaders,
    /// Provider-scoped environment values.
    pub env: ProviderEnv,
    /// Cancellation signal.
    pub signal: Option<crate::types::AbortSignal>,
    /// Optional callback for inspecting or replacing the JSON payload before it is sent.
    pub on_payload: Option<OpenAIResponsesPayloadHook>,
    /// Optional callback invoked after the HTTP response is received.
    pub on_response: Option<OpenAIResponsesResponseHook>,
    /// Prompt cache retention preference.
    pub cache_retention: Option<CacheRetention>,
    /// HTTP timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Maximum retry attempts.
    pub max_retries: Option<u32>,
    /// Reasoning effort requested by the caller.
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Reasoning summary preference requested by the caller.
    pub reasoning_summary: Option<ReasoningSummary>,
    /// Service tier preference.
    pub service_tier: Option<ServiceTier>,
}

impl fmt::Debug for OpenAIResponsesOptions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenAIResponsesOptions")
            .field("temperature", &self.temperature)
            .field("max_tokens", &self.max_tokens)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("session_id", &self.session_id)
            .field("headers", &self.headers)
            .field("env", &self.env)
            .field("signal", &self.signal)
            .field("on_payload", &self.on_payload.as_ref().map(|_| "<hook>"))
            .field("on_response", &self.on_response.as_ref().map(|_| "<hook>"))
            .field("cache_retention", &self.cache_retention)
            .field("timeout_ms", &self.timeout_ms)
            .field("max_retries", &self.max_retries)
            .field("reasoning_effort", &self.reasoning_effort)
            .field("reasoning_summary", &self.reasoning_summary)
            .field("service_tier", &self.service_tier)
            .finish()
    }
}

/// Options accepted by [`stream_simple`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SimpleStreamOptions {
    /// Sampling temperature.
    pub temperature: Option<f64>,
    /// Maximum output tokens requested by the caller.
    pub max_tokens: Option<u32>,
    /// API key for OpenAI-compatible providers.
    pub api_key: Option<String>,
    /// Optional session identifier.
    pub session_id: Option<String>,
    /// Optional custom HTTP headers.
    pub headers: ProviderHeaders,
    /// Provider-scoped environment values.
    pub env: ProviderEnv,
    /// Prompt cache retention preference.
    pub cache_retention: Option<CacheRetention>,
    /// HTTP timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Maximum retry attempts.
    pub max_retries: Option<u32>,
    /// Simplified thinking level requested by callers.
    pub reasoning: Option<ReasoningEffort>,
}

/// Prepared OpenAI Responses request plus Pi stream options.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OpenAIResponsesRequest {
    /// Provider base URL used by the OpenAI-compatible Responses endpoint.
    pub base_url: String,
    /// Headers sent with the request, after Pi default/session/Copilot/explicit merge.
    pub headers: ProviderHeaders,
    /// JSON body sent to `responses.create`.
    pub body: Value,
    /// Request timeout in milliseconds.
    pub timeout_ms: Option<u64>,
    /// Maximum retry attempts; Pi defaults this to zero.
    pub max_retries: u32,
}

/// Pi's event-stream handle for OpenAI Responses.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AssistantMessageEventStream {
    /// Request captured before provider I/O starts; deterministic tests assert Pi parity here.
    pub request: OpenAIResponsesRequest,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct ResponseCreateParamsStreaming {
    model: String,
    input: Vec<Value>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_cache_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_cache_retention: Option<&'static str>,
    store: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    service_tier: Option<ServiceTier>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ResponseTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<ReasoningOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    include: Option<Vec<&'static str>>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct ResponseTool {
    r#type: &'static str,
    name: String,
    description: String,
    parameters: Value,
    strict: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct ReasoningOptions {
    effort: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary: Option<String>,
}

/// Starts an OpenAI Responses stream by preparing the exact Pi request envelope.
///
/// Provider streaming is driven by a narrow HTTP fallback at the call boundary: hooks need the
/// mutable JSON body and raw response headers, so this path intentionally does not use `genai`.
///
/// # Errors
///
/// Returns [`OpenAIResponsesError::MissingApiKey`] when no API key or authorization header is
/// available.
pub fn stream(
    model: &Model,
    context: &Context,
    options: Option<&OpenAIResponsesOptions>,
) -> Result<AssistantMessageEventStream> {
    get_client_api_key(
        &model.provider,
        options.and_then(|options| options.api_key.as_deref()),
        options.map(|options| &options.headers),
    )?;

    Ok(AssistantMessageEventStream {
        request: build_request(model, context, options)?,
    })
}

/// Starts a live OpenAI Responses stream over HTTP/SSE.
pub fn stream_live(
    model: &Model,
    context: &Context,
    options: Option<&OpenAIResponsesOptions>,
) -> Result<crate::types::AssistantMessageEventStream> {
    let stream = crate::types::AssistantMessageEventStream::new();
    let worker_stream = stream.clone();
    let model = model.clone();
    let context = context.clone();
    let options = options.cloned().unwrap_or_default();
    let identity = crate::utils::runtime::StreamIdentity::new(
        model.api.clone(),
        model.provider.clone(),
        model.id.clone(),
    );
    crate::utils::runtime::spawn_stream_worker(stream.clone(), identity, async move {
        run_openai_responses_live_worker(worker_stream, model, context, options).await;
    });
    Ok(stream)
}

/// Starts an OpenAI Responses stream using Pi's simple stream option mapping.
///
/// # Errors
///
/// Returns [`OpenAIResponsesError::MissingApiKey`] when no API key or authorization header is
/// available, or propagates [`stream`] errors.
pub fn stream_simple(
    model: &Model,
    context: &Context,
    options: Option<&SimpleStreamOptions>,
) -> Result<AssistantMessageEventStream> {
    get_client_api_key(
        &model.provider,
        options.and_then(|options| options.api_key.as_deref()),
        options.map(|options| &options.headers),
    )?;
    let options = options.cloned().unwrap_or_default();
    let reasoning_effort = options
        .reasoning
        .filter(|effort| model.thinking_level_map.get(&(*effort).into()) != Some(&None));
    let stream_options = OpenAIResponsesOptions {
        temperature: options.temperature,
        max_tokens: options.max_tokens,
        api_key: options.api_key,
        session_id: options.session_id,
        headers: options.headers,
        env: options.env,
        cache_retention: options.cache_retention,
        timeout_ms: options.timeout_ms,
        max_retries: options.max_retries,
        reasoning_effort,
        ..OpenAIResponsesOptions::default()
    };

    stream(model, context, Some(&stream_options))
}

#[derive(Debug)]
struct OpenAIResponsesLiveError {
    message: String,
    partial: Option<crate::api::openai_responses_shared::AssistantMessage>,
}

impl OpenAIResponsesLiveError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            partial: None,
        }
    }

    fn with_partial(
        message: impl Into<String>,
        partial: &crate::api::openai_responses_shared::AssistantMessage,
    ) -> Self {
        Self {
            message: message.into(),
            partial: Some(partial.clone()),
        }
    }
}

async fn run_openai_responses_live_worker(
    stream: crate::types::AssistantMessageEventStream,
    model: Model,
    context: Context,
    options: OpenAIResponsesOptions,
) {
    let result = async {
        let api_key = get_client_api_key(
            &model.provider,
            options.api_key.as_deref(),
            Some(&options.headers),
        )
        .map_err(|error| OpenAIResponsesLiveError::new(error.to_string()))?;
        let mut request = build_request(&model, &context, Some(&options))
            .map_err(|error| OpenAIResponsesLiveError::new(error.to_string()))?;
        if let Some(on_payload) = options.on_payload.as_ref()
            && let Some(next_payload) = on_payload(request.body.clone(), model.clone())
                .await
                .map_err(|error| OpenAIResponsesLiveError::new(error.to_string()))?
        {
            request.body = next_payload;
        }
        execute_openai_responses_live(&stream, &model, &request, &api_key, &options).await
    }
    .await;

    match result {
        Ok(message) => {
            let output = canonical_message_from_responses(&message, &model, None);
            stream.push(crate::types::AssistantMessageEvent::Done {
                reason: canonical_done_reason(output.stop_reason),
                message: output,
            });
        }
        Err(error) => {
            let aborted = error.message == "Request was aborted";
            let mut output = error.partial.as_ref().map_or_else(
                || empty_canonical_message_for_responses(&model),
                |partial| canonical_message_from_responses(partial, &model, None),
            );
            output.stop_reason = if aborted {
                crate::types::StopReason::Aborted
            } else {
                crate::types::StopReason::Error
            };
            output.error_message = Some(error.message);
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

async fn execute_openai_responses_live(
    stream: &crate::types::AssistantMessageEventStream,
    model: &Model,
    request: &OpenAIResponsesRequest,
    api_key: &str,
    options: &OpenAIResponsesOptions,
) -> std::result::Result<
    crate::api::openai_responses_shared::AssistantMessage,
    OpenAIResponsesLiveError,
> {
    check_openai_responses_abort(options.signal.as_ref()).map_err(OpenAIResponsesLiveError::new)?;
    let client =
        build_openai_http_client(request.timeout_ms).map_err(OpenAIResponsesLiveError::new)?;
    let headers = openai_responses_headers(api_key, &request.headers)
        .map_err(OpenAIResponsesLiveError::new)?;
    let body = serde_json::to_vec(&request.body)
        .map_err(|error| OpenAIResponsesLiveError::new(error.to_string()))?;
    let mut attempts = 0;
    let response = loop {
        let response = await_openai_responses_or_abort(
            client
                .post(openai_responses_url(&request.base_url))
                .headers(headers.clone())
                .body(body.clone())
                .send(),
            options.signal.clone(),
        )
        .await;
        match response {
            Ok(response) => {
                if let Some(on_response) = options.on_response.as_ref() {
                    on_response(
                        provider_response_from_headers(
                            response.status().as_u16(),
                            response.headers(),
                        ),
                        model.clone(),
                    )
                    .await
                    .map_err(|error| OpenAIResponsesLiveError::new(error.to_string()))?;
                }
                if is_retryable_openai_responses_status(response.status().as_u16())
                    && attempts < request.max_retries
                {
                    let delay = crate::utils::retry::retry_after_delay(
                        response.headers(),
                        std::time::SystemTime::now(),
                    )
                    .unwrap_or_else(|| {
                        crate::utils::retry::retry_delay(
                            Duration::from_millis(500),
                            attempts,
                            Some(Duration::from_secs(8)),
                        )
                    });
                    if !crate::utils::retry::wait_or_abort(delay, options.signal.as_ref()).await {
                        return Err(OpenAIResponsesLiveError::new("Request was aborted"));
                    }
                    attempts += 1;
                    continue;
                }
                break response;
            }
            Err(error) if attempts < request.max_retries && error != "Request was aborted" => {
                let delay = crate::utils::retry::retry_delay(
                    Duration::from_millis(500),
                    attempts,
                    Some(Duration::from_secs(8)),
                );
                if !crate::utils::retry::wait_or_abort(delay, options.signal.as_ref()).await {
                    return Err(OpenAIResponsesLiveError::new("Request was aborted"));
                }
                attempts += 1;
            }
            Err(error) => return Err(OpenAIResponsesLiveError::new(error)),
        }
    };
    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = await_openai_responses_or_abort(response.text(), options.signal.clone())
            .await
            .map_err(OpenAIResponsesLiveError::new)?;
        return Err(OpenAIResponsesLiveError::new(format_openai_http_error(
            status,
            &body,
            Some("OpenAI API error"),
        )));
    }

    let shared_model = shared_model_from_responses(model);
    let initial_output = empty_shared_responses_message(model);
    stream.push(crate::types::AssistantMessageEvent::Start {
        partial: canonical_message_from_responses(&initial_output, model, None).into(),
    });

    let mut decoder = OpenAIResponsesSseDecoder::default();
    let mut bytes = response.bytes_stream();
    let mut processor = crate::api::openai_responses_shared::ResponsesStreamProcessor::default();
    let mut latest_output = initial_output;
    loop {
        let next = next_openai_responses_bytes_or_abort(&mut bytes, options.signal.clone())
            .await
            .map_err(|error| OpenAIResponsesLiveError::with_partial(error, &latest_output))?;
        let Some(bytes) = next else {
            return Err(OpenAIResponsesLiveError::with_partial(
                "OpenAI Responses stream ended before a terminal response event",
                &latest_output,
            ));
        };
        for frame in decoder
            .push(&bytes)
            .map_err(|error| OpenAIResponsesLiveError::with_partial(error, &latest_output))?
        {
            check_openai_responses_abort(options.signal.as_ref())
                .map_err(|error| OpenAIResponsesLiveError::with_partial(error, &latest_output))?;
            if frame == "[DONE]" {
                return Err(OpenAIResponsesLiveError::with_partial(
                    "OpenAI Responses stream ended before a terminal response event",
                    &latest_output,
                ));
            }
            let event = parse_openai_responses_event(&frame)
                .map_err(|error| OpenAIResponsesLiveError::with_partial(error, &latest_output))?;
            let mut generated = Vec::new();
            let terminal = processor
                .push(
                    event,
                    &mut latest_output,
                    &mut generated,
                    &shared_model,
                    None,
                )
                .map_err(|error| {
                    OpenAIResponsesLiveError::with_partial(error.to_string(), &latest_output)
                })?;
            for event in &generated {
                push_canonical_responses_event(stream, event, model);
            }
            if terminal {
                return Ok(latest_output);
            }
            tokio::task::yield_now().await;
        }
    }
}

fn empty_shared_responses_message(
    model: &Model,
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

fn is_retryable_openai_responses_status(status: u16) -> bool {
    matches!(status, 408 | 409 | 429 | 500..=599)
}

fn build_openai_http_client(
    timeout_ms: Option<u64>,
) -> std::result::Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder();
    if let Some(timeout_ms) = timeout_ms {
        builder = builder.timeout(Duration::from_millis(timeout_ms));
    }
    builder.build().map_err(|error| error.to_string())
}

async fn await_openai_responses_or_abort<T>(
    future: impl std::future::Future<Output = std::result::Result<T, reqwest::Error>>,
    signal: Option<crate::types::AbortSignal>,
) -> std::result::Result<T, String> {
    if let Some(signal) = signal {
        match futures::future::select(
            Box::pin(future),
            Box::pin(wait_openai_responses_abort(signal)),
        )
        .await
        {
            futures::future::Either::Left((result, _)) => result.map_err(|error| error.to_string()),
            futures::future::Either::Right(((), _)) => Err("Request was aborted".to_owned()),
        }
    } else {
        future.await.map_err(|error| error.to_string())
    }
}

async fn wait_openai_responses_abort(signal: crate::types::AbortSignal) {
    signal.cancelled().await;
}

fn check_openai_responses_abort(
    signal: Option<&crate::types::AbortSignal>,
) -> std::result::Result<(), String> {
    if signal.is_some_and(crate::types::AbortSignal::aborted) {
        Err("Request was aborted".to_owned())
    } else {
        Ok(())
    }
}

async fn next_openai_responses_bytes_or_abort<S, B>(
    stream: &mut S,
    signal: Option<crate::types::AbortSignal>,
) -> std::result::Result<Option<B>, String>
where
    S: futures::Stream<Item = std::result::Result<B, reqwest::Error>> + Unpin,
{
    if let Some(signal) = signal {
        match futures::future::select(
            Box::pin(stream.next()),
            Box::pin(wait_openai_responses_abort(signal)),
        )
        .await
        {
            futures::future::Either::Left((next, _)) => {
                next.transpose().map_err(|error| error.to_string())
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

#[derive(Default)]
struct OpenAIResponsesSseDecoder {
    buffer: Vec<u8>,
}

impl OpenAIResponsesSseDecoder {
    fn push(&mut self, bytes: &[u8]) -> std::result::Result<Vec<String>, String> {
        self.buffer.extend_from_slice(bytes);
        let mut frames = Vec::new();
        while let Some((end, separator_len)) = find_sse_separator(&self.buffer) {
            let frame = self.buffer.drain(..end + separator_len).collect::<Vec<_>>();
            let text = std::str::from_utf8(&frame[..end])
                .map_err(|error| format!("OpenAI Responses stream UTF-8 error: {error}"))?;
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

fn parse_openai_responses_event(
    data: &str,
) -> std::result::Result<crate::api::openai_responses_shared::ResponseStreamEvent, String> {
    let mut value: Value = serde_json::from_str(data)
        .map_err(|error| format!("OpenAI Responses stream JSON error: {error}"))?;
    if let Some(kind) = value
        .get("type")
        .and_then(Value::as_str)
        .map(|kind| kind.replace('.', "_"))
    {
        value["type"] = Value::String(kind);
    }
    serde_json::from_value(value)
        .map_err(|error| format!("OpenAI Responses stream JSON error: {error}"))
}

fn openai_responses_headers(
    api_key: &str,
    headers: &ProviderHeaders,
) -> std::result::Result<HeaderMap, String> {
    let mut map = HeaderMap::new();
    map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    if !has_header(Some(headers), "authorization")
        && !has_header(Some(headers), "cf-aig-authorization")
    {
        map.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {api_key}"))
                .map_err(|error| error.to_string())?,
        );
    }
    for (name, value) in headers {
        let Some(value) = value else { continue };
        map.insert(
            HeaderName::from_bytes(name.as_bytes()).map_err(|error| error.to_string())?,
            HeaderValue::from_str(value).map_err(|error| error.to_string())?,
        );
    }
    Ok(map)
}

fn openai_responses_url(base_url: &str) -> String {
    format!("{}/responses", base_url.trim_end_matches('/'))
}

fn provider_response_from_headers(
    status: u16,
    headers: &reqwest::header::HeaderMap,
) -> ProviderResponse {
    ProviderResponse {
        status,
        headers: headers
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.to_string(), value.to_string()))
            })
            .collect(),
    }
}

fn format_openai_http_error(status: u16, body: &str, prefix: Option<&str>) -> String {
    match prefix {
        Some(prefix) => format!("{prefix} ({status}): {body}"),
        None => format!("OpenAI API error ({status}): {body}"),
    }
}

fn shared_model_from_responses(model: &Model) -> crate::api::openai_responses_shared::Model {
    crate::api::openai_responses_shared::Model {
        id: model.id.clone(),
        api: model.api.clone(),
        provider: model.provider.clone(),
        reasoning: model.reasoning,
        input: vec!["text".to_string()],
        cost: crate::api::openai_responses_shared::ModelCost::default(),
        compat: None,
    }
}

fn push_canonical_responses_event(
    stream: &crate::types::AssistantMessageEventStream,
    event: &crate::api::openai_responses_shared::AssistantMessageEvent,
    model: &Model,
) {
    match event {
        crate::api::openai_responses_shared::AssistantMessageEvent::ThinkingStart {
            content_index,
            partial,
        } => stream.push(crate::types::AssistantMessageEvent::ThinkingStart {
            content_index: *content_index,
            partial: canonical_message_from_responses(partial, model, None).into(),
        }),
        crate::api::openai_responses_shared::AssistantMessageEvent::ThinkingDelta {
            content_index,
            delta,
            partial,
        } => stream.push(crate::types::AssistantMessageEvent::ThinkingDelta {
            content_index: *content_index,
            delta: delta.clone(),
            partial: canonical_message_from_responses(partial, model, None).into(),
        }),
        crate::api::openai_responses_shared::AssistantMessageEvent::ThinkingEnd {
            content_index,
            content,
            partial,
        } => stream.push(crate::types::AssistantMessageEvent::ThinkingEnd {
            content_index: *content_index,
            content: content.clone(),
            partial: canonical_message_from_responses(partial, model, None).into(),
        }),
        crate::api::openai_responses_shared::AssistantMessageEvent::TextStart {
            content_index,
            partial,
        } => stream.push(crate::types::AssistantMessageEvent::TextStart {
            content_index: *content_index,
            partial: canonical_message_from_responses(partial, model, None).into(),
        }),
        crate::api::openai_responses_shared::AssistantMessageEvent::TextDelta {
            content_index,
            delta,
            partial,
        } => stream.push(crate::types::AssistantMessageEvent::TextDelta {
            content_index: *content_index,
            delta: delta.clone(),
            partial: canonical_message_from_responses(partial, model, None).into(),
        }),
        crate::api::openai_responses_shared::AssistantMessageEvent::TextEnd {
            content_index,
            content,
            partial,
        } => stream.push(crate::types::AssistantMessageEvent::TextEnd {
            content_index: *content_index,
            content: content.clone(),
            partial: canonical_message_from_responses(partial, model, None).into(),
        }),
        crate::api::openai_responses_shared::AssistantMessageEvent::ToolCallStart {
            content_index,
            partial,
        } => stream.push(crate::types::AssistantMessageEvent::ToolcallStart {
            content_index: *content_index,
            partial: canonical_message_from_responses(partial, model, None).into(),
        }),
        crate::api::openai_responses_shared::AssistantMessageEvent::ToolCallDelta {
            content_index,
            delta,
            partial,
        } => stream.push(crate::types::AssistantMessageEvent::ToolcallDelta {
            content_index: *content_index,
            delta: delta.clone(),
            partial: canonical_message_from_responses(partial, model, None).into(),
        }),
        crate::api::openai_responses_shared::AssistantMessageEvent::ToolCallEnd {
            content_index,
            tool_call,
            partial,
        } => stream.push(crate::types::AssistantMessageEvent::ToolcallEnd {
            content_index: *content_index,
            tool_call: canonical_tool_call_from_responses(tool_call),
            partial: canonical_message_from_responses(partial, model, None).into(),
        }),
    }
}

fn canonical_message_from_responses(
    message: &crate::api::openai_responses_shared::AssistantMessage,
    model: &Model,
    error_message: Option<String>,
) -> crate::types::AssistantMessage {
    crate::types::AssistantMessage {
        role: crate::types::AssistantMessageRole::Assistant,
        content: message
            .content
            .iter()
            .map(canonical_content_from_responses)
            .collect(),
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
        timestamp: unix_timestamp_ms(),
    }
}

fn empty_canonical_message_for_responses(model: &Model) -> crate::types::AssistantMessage {
    crate::types::AssistantMessage {
        role: crate::types::AssistantMessageRole::Assistant,
        content: Vec::new(),
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: crate::types::Usage::default(),
        stop_reason: crate::types::StopReason::Stop,
        error_message: None,
        timestamp: unix_timestamp_ms(),
    }
}

fn canonical_content_from_responses(
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
            crate::types::AssistantContentBlock::ToolCall(canonical_tool_call_from_responses(
                tool_call,
            ))
        }
    }
}

fn canonical_tool_call_from_responses(
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

fn unix_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Returns the prompt-cache retention, defaulting from `PI_CACHE_RETENTION` when present.
#[must_use]
pub fn resolve_cache_retention(
    cache_retention: Option<CacheRetention>,
    env: &ProviderEnv,
) -> CacheRetention {
    if let Some(cache_retention) = cache_retention {
        return cache_retention;
    }
    if env.get("PI_CACHE_RETENTION").map(String::as_str) == Some("long") {
        CacheRetention::Long
    } else {
        CacheRetention::Short
    }
}

/// Resolves OpenAI Responses compatibility flags, matching Pi defaults.
#[must_use]
pub fn get_compat(model: &Model) -> ResolvedOpenAIResponsesCompat {
    let compat = model.compat.as_ref();
    ResolvedOpenAIResponsesCompat {
        supports_developer_role: compat
            .and_then(|compat| compat.supports_developer_role)
            .unwrap_or(true),
        send_session_id_header: compat
            .and_then(|compat| compat.send_session_id_header)
            .unwrap_or(true),
        supports_long_cache_retention: compat
            .and_then(|compat| compat.supports_long_cache_retention)
            .unwrap_or(true),
    }
}

/// Returns the provider prompt-cache retention value for OpenAI Responses.
#[must_use]
pub const fn prompt_cache_retention(
    compat: ResolvedOpenAIResponsesCompat,
    cache_retention: CacheRetention,
) -> Option<&'static str> {
    match (cache_retention, compat.supports_long_cache_retention) {
        (CacheRetention::Long, true) => Some("24h"),
        _ => None,
    }
}

/// Applies Pi's service-tier cost multiplier to usage.
pub fn apply_service_tier_pricing(
    usage: &mut Usage,
    service_tier: Option<ServiceTier>,
    model: &Model,
) {
    let multiplier = service_tier_cost_multiplier(model, service_tier);
    if multiplier == 1.0 {
        return;
    }
    usage.cost.input *= multiplier;
    usage.cost.output *= multiplier;
    usage.cost.cache_read *= multiplier;
    usage.cost.cache_write *= multiplier;
    usage.cost.total =
        usage.cost.input + usage.cost.output + usage.cost.cache_read + usage.cost.cache_write;
}

/// Builds the HTTP request envelope used by the OpenAI Responses fallback.
pub fn build_request(
    model: &Model,
    context: &Context,
    options: Option<&OpenAIResponsesOptions>,
) -> Result<OpenAIResponsesRequest> {
    let body = serde_json::to_value(build_params(model, context, options)).map_err(|error| {
        OpenAIResponsesError::Transport(format!(
            "failed to serialize OpenAI Responses request: {error}"
        ))
    })?;
    Ok(OpenAIResponsesRequest {
        base_url: model.base_url.clone(),
        headers: build_client_headers(model, context, options),
        body,
        timeout_ms: options.and_then(|options| options.timeout_ms),
        max_retries: options.and_then(|options| options.max_retries).unwrap_or(0),
    })
}

fn build_params(
    model: &Model,
    context: &Context,
    options: Option<&OpenAIResponsesOptions>,
) -> ResponseCreateParamsStreaming {
    let env = options.map(|options| &options.env).unwrap_or(&*EMPTY_ENV);
    let cache_retention =
        resolve_cache_retention(options.and_then(|options| options.cache_retention), env);
    let compat = get_compat(model);
    let tools = (!context.tools.is_empty()).then(|| {
        context
            .tools
            .iter()
            .map(|tool| ResponseTool {
                r#type: "function",
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
                strict: false,
            })
            .collect()
    });
    let reasoning = reasoning_options(model, options);

    ResponseCreateParamsStreaming {
        model: model.id.clone(),
        input: build_input(model, context, compat),
        stream: true,
        prompt_cache_key: prompt_cache_key(
            cache_retention,
            options.and_then(|options| options.session_id.as_deref()),
        ),
        prompt_cache_retention: prompt_cache_retention(compat, cache_retention),
        store: false,
        max_output_tokens: options
            .and_then(|options| options.max_tokens)
            .filter(|max_tokens| *max_tokens > 0)
            .map(|max_tokens| max_tokens.max(OPENAI_RESPONSES_MIN_OUTPUT_TOKENS)),
        temperature: options.and_then(|options| options.temperature),
        service_tier: options.and_then(|options| options.service_tier),
        tools,
        include: reasoning
            .as_ref()
            .and_then(|reasoning| reasoning.summary.as_ref())
            .map(|_| vec!["reasoning.encrypted_content"]),
        reasoning,
    }
}

static EMPTY_ENV: LazyLock<ProviderEnv> = LazyLock::new(ProviderEnv::new);

fn build_input(
    model: &Model,
    context: &Context,
    compat: ResolvedOpenAIResponsesCompat,
) -> Vec<Value> {
    let mut input =
        Vec::with_capacity(context.messages.len() + usize::from(context.system_prompt.is_some()));
    if let Some(system_prompt) = &context.system_prompt {
        let role = if model.reasoning && compat.supports_developer_role {
            "developer"
        } else {
            "system"
        };
        input.push(serde_json::json!({ "role": role, "content": system_prompt }));
    }
    input.extend(context.messages.iter().cloned());
    input
}

fn reasoning_options(
    model: &Model,
    options: Option<&OpenAIResponsesOptions>,
) -> Option<ReasoningOptions> {
    if !model.reasoning {
        return None;
    }
    if let Some(options) = options
        && (options.reasoning_effort.is_some() || options.reasoning_summary.is_some())
    {
        let effort = options
            .reasoning_effort
            .map(|effort| mapped_reasoning_effort(model, effort))
            .unwrap_or_else(|| Some("medium".to_string()))?;
        return Some(ReasoningOptions {
            effort,
            summary: Some(
                options
                    .reasoning_summary
                    .unwrap_or(ReasoningSummary::Auto)
                    .as_str()
                    .to_string(),
            ),
        });
    }
    if model.provider == "github-copilot" {
        return None;
    }
    match model.thinking_level_map.get(&ModelThinkingLevel::Off) {
        Some(None) => None,
        Some(Some(value)) => Some(ReasoningOptions {
            effort: value.clone(),
            summary: None,
        }),
        None => Some(ReasoningOptions {
            effort: "none".to_string(),
            summary: None,
        }),
    }
}

fn mapped_reasoning_effort(model: &Model, effort: ReasoningEffort) -> Option<String> {
    model
        .thinking_level_map
        .get(&effort.into())
        .cloned()
        .unwrap_or_else(|| Some(effort.as_str().to_string()))
}

fn build_client_headers(
    model: &Model,
    context: &Context,
    options: Option<&OpenAIResponsesOptions>,
) -> ProviderHeaders {
    let mut headers = model.headers.clone();
    if model.provider == "github-copilot" {
        let copilot_headers = build_copilot_dynamic_headers(CopilotDynamicHeadersParams {
            messages: &context.copilot_messages,
            has_images: has_copilot_vision_input(&context.copilot_messages),
        });
        headers.extend(
            copilot_headers
                .into_iter()
                .map(|(key, value)| (key, Some(value))),
        );
    }

    let cache_retention = resolve_cache_retention(
        options.and_then(|options| options.cache_retention),
        options.map(|options| &options.env).unwrap_or(&*EMPTY_ENV),
    );
    if cache_retention != CacheRetention::None
        && let Some(session_id) = options.and_then(|options| options.session_id.as_deref())
    {
        if get_compat(model).send_session_id_header {
            headers.insert("session_id".to_string(), Some(session_id.to_string()));
        }
        headers.insert(
            "x-client-request-id".to_string(),
            Some(session_id.to_string()),
        );
    }

    if let Some(options) = options {
        headers.extend(options.headers.clone());
    }
    headers
}

fn has_header(headers: Option<&ProviderHeaders>, name: &str) -> bool {
    let expected = name.to_ascii_lowercase();
    headers.is_some_and(|headers| {
        headers.iter().any(|(key, value)| {
            key.eq_ignore_ascii_case(&expected)
                && value.as_ref().is_some_and(|value| !value.trim().is_empty())
        })
    })
}

fn get_client_api_key(
    provider: &str,
    api_key: Option<&str>,
    headers: Option<&ProviderHeaders>,
) -> Result<String> {
    if let Some(api_key) = api_key.filter(|api_key| !api_key.is_empty()) {
        return Ok(api_key.to_string());
    }
    if has_header(headers, "authorization") || has_header(headers, "cf-aig-authorization") {
        return Ok("unused".to_string());
    }
    Err(OpenAIResponsesError::MissingApiKey {
        provider: provider.to_string(),
    })
}

/// Returns the OpenAI Responses prompt-cache key for a resolved retention preference.
#[must_use]
pub fn prompt_cache_key(
    cache_retention: CacheRetention,
    session_id: Option<&str>,
) -> Option<String> {
    if cache_retention == CacheRetention::None {
        return None;
    }
    clamp_openai_prompt_cache_key(session_id)
}

fn clamp_openai_prompt_cache_key(key: Option<&str>) -> Option<String> {
    let key = key?;
    Some(
        key.chars()
            .take(OPENAI_PROMPT_CACHE_KEY_MAX_LENGTH)
            .collect(),
    )
}

fn service_tier_cost_multiplier(model: &Model, service_tier: Option<ServiceTier>) -> f64 {
    match service_tier {
        Some(ServiceTier::Flex) => 0.5,
        Some(ServiceTier::Priority) if model.id == "gpt-5.5" => 2.5,
        Some(ServiceTier::Priority) => 2.0,
        _ => 1.0,
    }
}

/// Returns the canonical OpenAI Responses production streams.
#[must_use]
pub fn provider_streams() -> crate::types::ProviderStreams {
    crate::types::ProviderStreams {
        stream: Arc::new(stream_registered),
        stream_simple: Arc::new(stream_simple_registered),
    }
}

/// Starts the canonical OpenAI Responses production stream.
#[must_use]
pub fn stream_registered(
    model: &crate::types::Model,
    context: &crate::types::Context,
    options: Option<&crate::types::StreamOptions>,
) -> crate::types::AssistantMessageEventStream {
    let context = crate::api::transform_messages::transform_context(context, model, None);
    let local_model = registered_model(model);
    let local_context = registered_context(model, &context);
    let mut local_options = registered_options(model, options);
    if model.provider == "github-copilot" {
        local_options.headers.extend(
            super::github_copilot_headers::build_copilot_dynamic_headers_for_context(
                &context.messages,
            )
            .into_iter()
            .map(|(key, value)| (key, Some(value))),
        );
    }
    stream_live(&local_model, &local_context, Some(&local_options)).unwrap_or_else(|error| {
        let stream = crate::types::AssistantMessageEventStream::new();
        let mut output = empty_canonical_message_for_responses(&local_model);
        output.stop_reason = crate::types::StopReason::Error;
        output.error_message = Some(error.to_string());
        stream.push(crate::types::AssistantMessageEvent::Error {
            reason: crate::types::ErrorStopReason::Error,
            error: output,
        });
        stream
    })
}

/// Starts the canonical simple OpenAI Responses stream.
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

fn registered_model(model: &crate::types::Model) -> Model {
    Model {
        id: model.id.clone(),
        api: model.api.clone(),
        provider: model.provider.clone(),
        base_url: model.base_url.clone(),
        reasoning: model.reasoning,
        thinking_level_map: model
            .thinking_level_map
            .as_ref()
            .map(|map| {
                map.iter()
                    .map(|(level, value)| {
                        (
                            match level {
                                crate::types::ModelThinkingLevel::Off => ModelThinkingLevel::Off,
                                crate::types::ModelThinkingLevel::Minimal => {
                                    ModelThinkingLevel::Minimal
                                }
                                crate::types::ModelThinkingLevel::Low => ModelThinkingLevel::Low,
                                crate::types::ModelThinkingLevel::Medium => {
                                    ModelThinkingLevel::Medium
                                }
                                crate::types::ModelThinkingLevel::High => ModelThinkingLevel::High,
                                crate::types::ModelThinkingLevel::XHigh => {
                                    ModelThinkingLevel::XHigh
                                }
                            },
                            value.clone(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        headers: model
            .headers
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|(key, value)| (key, Some(value)))
            .collect(),
        compat: model.compat.as_ref().and_then(|compat| match compat {
            crate::types::ModelCompat::OpenAIResponses(compat) => Some(OpenAIResponsesCompat {
                supports_developer_role: compat.supports_developer_role,
                send_session_id_header: compat.send_session_id_header,
                supports_long_cache_retention: compat.supports_long_cache_retention,
            }),
            _ => None,
        }),
    }
}

fn registered_context(model: &crate::types::Model, context: &crate::types::Context) -> Context {
    let shared_model = crate::api::openai_responses_shared::Model {
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
        compat: model.compat.as_ref().and_then(|compat| match compat {
            crate::types::ModelCompat::OpenAIResponses(compat) => {
                Some(crate::api::openai_responses_shared::OpenAIResponsesCompat {
                    supports_developer_role: compat.supports_developer_role,
                })
            }
            _ => None,
        }),
    };
    let messages = context
        .messages
        .iter()
        .filter_map(|message| {
            let value = serde_json::to_value(message).ok()?;
            serde_json::from_value(canonical_message_to_shared_json(value)).ok()
        })
        .collect();
    let shared_context = crate::api::openai_responses_shared::Context {
        system_prompt: context.system_prompt.clone(),
        messages,
    };
    let allowed = HashSet::from([
        "openai".to_owned(),
        "openai-codex".to_owned(),
        "opencode".to_owned(),
    ]);
    Context {
        system_prompt: None,
        messages: crate::api::openai_responses_shared::convert_responses_messages(
            &shared_model,
            &shared_context,
            &allowed,
            None,
        ),
        tools: context
            .tools
            .as_ref()
            .into_iter()
            .flatten()
            .map(|tool| Tool {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            })
            .collect(),
        copilot_messages: Vec::new(),
    }
}

pub(crate) fn canonical_message_to_shared_json(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(canonical_message_to_shared_json)
                .collect(),
        ),
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
                    (key, canonical_message_to_shared_json(value))
                })
                .collect(),
        ),
        other => other,
    }
}

fn registered_options(
    model: &crate::types::Model,
    options: Option<&crate::types::StreamOptions>,
) -> OpenAIResponsesOptions {
    let options = options.cloned().unwrap_or_default();
    let canonical_model = model.clone();
    let payload_model = canonical_model.clone();
    let on_payload = options.on_payload.map(|hook| {
        Arc::new(move |payload, _model| hook(payload, payload_model.clone()))
            as OpenAIResponsesPayloadHook
    });
    let on_response = options.on_response.map(|hook| {
        Arc::new(move |response: ProviderResponse, _model| {
            hook(
                crate::types::ProviderResponse {
                    status: response.status,
                    headers: response.headers,
                },
                canonical_model.clone(),
            )
        }) as OpenAIResponsesResponseHook
    });
    let reasoning_effort = options
        .extra
        .get("reasoningEffort")
        .and_then(Value::as_str)
        .and_then(|effort| match effort {
            "minimal" => Some(ReasoningEffort::Minimal),
            "low" => Some(ReasoningEffort::Low),
            "medium" => Some(ReasoningEffort::Medium),
            "high" => Some(ReasoningEffort::High),
            "xhigh" => Some(ReasoningEffort::XHigh),
            _ => None,
        });
    OpenAIResponsesOptions {
        temperature: options.temperature,
        max_tokens: options.max_tokens,
        api_key: options.api_key,
        session_id: options.session_id,
        headers: options.headers.unwrap_or_default(),
        env: options.env.unwrap_or_default(),
        signal: options.signal,
        on_payload,
        on_response,
        cache_retention: options.cache_retention.map(|retention| match retention {
            crate::types::CacheRetention::None => CacheRetention::None,
            crate::types::CacheRetention::Short => CacheRetention::Short,
            crate::types::CacheRetention::Long => CacheRetention::Long,
        }),
        timeout_ms: options.timeout_ms,
        max_retries: options.max_retries,
        reasoning_effort,
        reasoning_summary: None,
        service_tier: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn model() -> Model {
        Model {
            id: "gpt-5".to_string(),
            api: "openai-responses".to_string(),
            provider: "openai".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            reasoning: true,
            thinking_level_map: HashMap::new(),
            headers: ProviderHeaders::new(),
            compat: None,
        }
    }

    #[test]
    fn resolves_api_key_from_authorization_header() {
        let mut headers = ProviderHeaders::new();
        headers.insert(
            "Authorization".to_string(),
            Some("Bearer token".to_string()),
        );

        assert_eq!(
            get_client_api_key("openai", None, Some(&headers)).unwrap(),
            "unused"
        );

        headers.clear();
        headers.insert("authorization".to_string(), Some("   ".to_string()));
        assert_eq!(
            get_client_api_key("openai", None, Some(&headers)).unwrap_err(),
            OpenAIResponsesError::MissingApiKey {
                provider: "openai".to_string()
            }
        );
    }

    #[test]
    fn builds_params_with_pi_defaults() {
        let mut model = model();
        model
            .thinking_level_map
            .insert(ModelThinkingLevel::Off, Some("disabled".to_string()));
        let context = Context {
            system_prompt: Some("be terse".to_string()),
            messages: vec![json!({"role":"user","content":[{"type":"input_text","text":"hi"}]})],
            tools: vec![Tool {
                name: "lookup".to_string(),
                description: "look up data".to_string(),
                parameters: json!({"type":"object"}),
            }],
            copilot_messages: Vec::new(),
        };
        let options = OpenAIResponsesOptions {
            max_tokens: Some(1),
            session_id: Some(
                "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789LONG".to_string(),
            ),
            ..OpenAIResponsesOptions::default()
        };

        let params = build_params(&model, &context, Some(&options));

        assert_eq!(params.model, "gpt-5");
        assert_eq!(params.input[0]["role"], "developer");
        assert_eq!(
            params.max_output_tokens,
            Some(OPENAI_RESPONSES_MIN_OUTPUT_TOKENS)
        );
        assert_eq!(params.prompt_cache_key.unwrap().chars().count(), 64);
        assert_eq!(params.reasoning.unwrap().effort, "disabled");
        assert!(!params.tools.unwrap()[0].strict);
    }

    #[test]
    fn long_cache_retention_respects_compat() {
        let mut env = ProviderEnv::new();
        env.insert("PI_CACHE_RETENTION".to_string(), "long".to_string());
        assert_eq!(resolve_cache_retention(None, &env), CacheRetention::Long);

        let mut model = model();
        model.compat = Some(OpenAIResponsesCompat {
            supports_long_cache_retention: Some(false),
            ..OpenAIResponsesCompat::default()
        });
        let params = build_params(
            &model,
            &Context::default(),
            Some(&OpenAIResponsesOptions {
                env,
                ..OpenAIResponsesOptions::default()
            }),
        );
        assert_eq!(params.prompt_cache_retention, None);
    }

    #[test]
    fn service_tier_pricing_matches_pi() {
        let mut usage = Usage {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            cost: UsageCost {
                input: 1.0,
                output: 2.0,
                cache_read: 3.0,
                cache_write: 4.0,
                total: 10.0,
            },
        };

        apply_service_tier_pricing(&mut usage, Some(ServiceTier::Flex), &model());

        assert_eq!(usage.cost.input, 0.5);
        assert_eq!(usage.cost.total, 5.0);
    }

    #[test]
    fn builds_copilot_session_headers_with_overrides() {
        let mut model = model();
        model.provider = "github-copilot".to_string();
        let mut override_headers = ProviderHeaders::new();
        override_headers.insert("X-Initiator".to_string(), Some("caller".to_string()));
        let headers = build_client_headers(
            &model,
            &Context {
                copilot_messages: vec![CopilotMessage::Assistant],
                ..Context::default()
            },
            Some(&OpenAIResponsesOptions {
                session_id: Some("s1".to_string()),
                headers: override_headers,
                ..OpenAIResponsesOptions::default()
            }),
        );

        assert_eq!(headers.get("session_id"), Some(&Some("s1".to_string())));
        assert_eq!(
            headers.get("x-client-request-id"),
            Some(&Some("s1".to_string()))
        );
        assert_eq!(
            headers.get("X-Initiator"),
            Some(&Some("caller".to_string()))
        );
    }

    fn model_for_openai_responses_copilot_provider_test(
        provider: &str,
        id: &str,
        off_mapping: Option<Option<&str>>,
    ) -> Model {
        let mut model = model();
        model.provider = provider.to_string();
        model.id = id.to_string();
        if provider == "opencode" {
            model.base_url = "https://proxy.example.com/v1".to_string();
        }
        match off_mapping {
            Some(value) => {
                model
                    .thinking_level_map
                    .insert(ModelThinkingLevel::Off, value.map(str::to_string));
            }
            None => {
                model.thinking_level_map.clear();
            }
        }
        model
    }

    fn default_openai_responses_context() -> Context {
        Context {
            system_prompt: Some("sys".to_string()),
            messages: vec![json!({"role":"user","content":"hi","timestamp":0})],
            ..Context::default()
        }
    }

    fn header_value<'a>(headers: &'a ProviderHeaders, name: &str) -> Option<&'a str> {
        headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .and_then(|(_, value)| value.as_deref())
    }

    fn capture_openai_responses_headers(
        model: &Model,
        options: &OpenAIResponsesOptions,
    ) -> (Option<String>, Option<String>) {
        let headers =
            build_client_headers(model, &default_openai_responses_context(), Some(options));
        (
            header_value(&headers, "session_id").map(str::to_string),
            header_value(&headers, "x-client-request-id").map(str::to_string),
        )
    }

    #[test]
    fn openai_responses_copilot_provider_omits_reasoning_when_no_reasoning_is_requested() {
        let model = model_for_openai_responses_copilot_provider_test(
            "github-copilot",
            "gpt-5-mini",
            Some(None),
        );

        let params = build_params(
            &model,
            &default_openai_responses_context(),
            Some(&OpenAIResponsesOptions::default()),
        );

        assert_eq!(params.reasoning, None);
    }

    #[test]
    fn openai_responses_copilot_provider_sends_none_reasoning_for_openai_models() {
        for model_id in [
            "gpt-5.1",
            "gpt-5.2",
            "gpt-5.3-codex",
            "gpt-5.4",
            "gpt-5.4-mini",
            "gpt-5.4-nano",
            "gpt-5.5",
        ] {
            let model = model_for_openai_responses_copilot_provider_test(
                "openai",
                model_id,
                Some(Some("none")),
            );

            let params = build_params(
                &model,
                &default_openai_responses_context(),
                Some(&OpenAIResponsesOptions::default()),
            );

            assert_eq!(
                params.reasoning,
                Some(ReasoningOptions {
                    effort: "none".to_string(),
                    summary: None,
                })
            );
        }
    }

    #[test]
    fn openai_responses_copilot_provider_omits_reasoning_when_off_is_unsupported() {
        for model_id in [
            "gpt-5",
            "gpt-5-mini",
            "gpt-5-nano",
            "gpt-5-pro",
            "gpt-5.2-pro",
            "gpt-5.4-pro",
            "gpt-5.5-pro",
        ] {
            let model =
                model_for_openai_responses_copilot_provider_test("openai", model_id, Some(None));

            let params = build_params(
                &model,
                &default_openai_responses_context(),
                Some(&OpenAIResponsesOptions::default()),
            );

            assert_eq!(params.reasoning, None);
        }
    }

    #[test]
    fn openai_responses_copilot_provider_sets_cache_affinity_headers_for_openai() {
        let captured = capture_openai_responses_headers(
            &model_for_openai_responses_copilot_provider_test(
                "openai",
                "gpt-5.4",
                Some(Some("none")),
            ),
            &OpenAIResponsesOptions {
                session_id: Some("session-123".to_string()),
                ..OpenAIResponsesOptions::default()
            },
        );

        assert_eq!(
            captured,
            (
                Some("session-123".to_string()),
                Some("session-123".to_string())
            )
        );
    }

    #[test]
    fn openai_responses_copilot_provider_clamps_prompt_cache_key() {
        let session_id = "x".repeat(67);
        let params = build_params(
            &model_for_openai_responses_copilot_provider_test(
                "openai",
                "gpt-5.4",
                Some(Some("none")),
            ),
            &default_openai_responses_context(),
            Some(&OpenAIResponsesOptions {
                session_id: Some(session_id),
                ..OpenAIResponsesOptions::default()
            }),
        );

        assert_eq!(params.prompt_cache_key, Some("x".repeat(64)));
    }

    #[test]
    fn openai_responses_copilot_provider_sets_cache_affinity_headers_for_proxy() {
        let captured = capture_openai_responses_headers(
            &model_for_openai_responses_copilot_provider_test(
                "opencode",
                "gpt-5.4",
                Some(Some("none")),
            ),
            &OpenAIResponsesOptions {
                session_id: Some("session-123".to_string()),
                ..OpenAIResponsesOptions::default()
            },
        );

        assert_eq!(
            captured,
            (
                Some("session-123".to_string()),
                Some("session-123".to_string())
            )
        );
    }

    #[test]
    fn openai_responses_copilot_provider_can_omit_session_id_header() {
        let mut model = model_for_openai_responses_copilot_provider_test(
            "opencode",
            "gpt-5.4",
            Some(Some("none")),
        );
        model.compat = Some(OpenAIResponsesCompat {
            send_session_id_header: Some(false),
            ..OpenAIResponsesCompat::default()
        });

        let captured = capture_openai_responses_headers(
            &model,
            &OpenAIResponsesOptions {
                session_id: Some("session-123".to_string()),
                ..OpenAIResponsesOptions::default()
            },
        );

        assert_eq!(captured, (None, Some("session-123".to_string())));
    }

    #[test]
    fn openai_responses_copilot_provider_explicit_headers_override_cache_affinity_headers() {
        let mut headers = ProviderHeaders::new();
        headers.insert(
            "session_id".to_string(),
            Some("override-session".to_string()),
        );
        headers.insert(
            "x-client-request-id".to_string(),
            Some("override-request".to_string()),
        );

        let captured = capture_openai_responses_headers(
            &model_for_openai_responses_copilot_provider_test(
                "openai",
                "gpt-5.4",
                Some(Some("none")),
            ),
            &OpenAIResponsesOptions {
                session_id: Some("session-123".to_string()),
                headers,
                ..OpenAIResponsesOptions::default()
            },
        );

        assert_eq!(
            captured,
            (
                Some("override-session".to_string()),
                Some("override-request".to_string())
            )
        );
    }

    #[test]
    fn openai_responses_copilot_provider_omits_cache_affinity_headers_when_cache_retention_is_none()
    {
        let captured = capture_openai_responses_headers(
            &model_for_openai_responses_copilot_provider_test(
                "openai",
                "gpt-5.4",
                Some(Some("none")),
            ),
            &OpenAIResponsesOptions {
                cache_retention: Some(CacheRetention::None),
                session_id: Some("session-123".to_string()),
                ..OpenAIResponsesOptions::default()
            },
        );

        assert_eq!(captured, (None, None));
    }

    #[test]
    fn openai_responses_copilot_provider_applies_service_tier_cost_multiplier() {
        for (model_id, service_tier, multiplier, input_cost, output_cost) in [
            ("gpt-5.4", ServiceTier::Priority, 2.0, 2.5, 15.0),
            ("gpt-5.5", ServiceTier::Priority, 2.5, 5.0, 30.0),
            ("gpt-5.5", ServiceTier::Flex, 0.5, 5.0, 30.0),
        ] {
            let model = model_for_openai_responses_copilot_provider_test(
                "openai",
                model_id,
                Some(Some("none")),
            );
            let mut usage = Usage {
                input: 1_000_000.0,
                output: 1_000_000.0,
                cache_read: 0.0,
                cache_write: 0.0,
                cost: UsageCost {
                    input: input_cost,
                    output: output_cost,
                    cache_read: 0.0,
                    cache_write: 0.0,
                    total: input_cost + output_cost,
                },
            };

            apply_service_tier_pricing(&mut usage, Some(service_tier), &model);

            assert_eq!(usage.cost.input, input_cost * multiplier);
            assert_eq!(usage.cost.output, output_cost * multiplier);
            assert_eq!(usage.cost.total, (input_cost + output_cost) * multiplier);
        }
    }

    #[test]
    fn openai_responses_stream_prepares_request_level_parity() {
        let stream = stream(
            &model_for_openai_responses_copilot_provider_test(
                "openai",
                "gpt-5.4",
                Some(Some("none")),
            ),
            &default_openai_responses_context(),
            Some(&OpenAIResponsesOptions {
                api_key: Some("test-key".to_string()),
                session_id: Some("session-123".to_string()),
                max_retries: Some(3),
                timeout_ms: Some(2500),
                ..OpenAIResponsesOptions::default()
            }),
        )
        .expect("request should be prepared");

        assert_eq!(stream.request.max_retries, 3);
        assert_eq!(stream.request.timeout_ms, Some(2500));
        assert_eq!(stream.request.body.get("store"), Some(&Value::Bool(false)));
        assert_eq!(
            stream.request.headers.get("session_id"),
            Some(&Some("session-123".to_string()))
        );
    }
}
