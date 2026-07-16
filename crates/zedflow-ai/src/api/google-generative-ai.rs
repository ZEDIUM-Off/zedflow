//! Google Generative AI API ported from Pi.

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

use crate::types::{
    AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream as CanonicalEventStream,
    Context as CanonicalContext, DoneStopReason, ErrorStopReason, Model as CanonicalModel,
    ProviderResponse, ProviderStreams, SimpleStreamOptions as CanonicalSimpleOptions,
    StopReason as CanonicalStopReason, StreamOptions,
};

use crate::api::google_shared::{
    Context as SharedContext, FunctionCallingConfigMode, GenerateContentChunk,
    GoogleAssistantMessageEventStream, GoogleStreamCollector, GoogleStreamFrame,
    Model as SharedModel, ModelInput, Tool, collect_google_stream, convert_messages, convert_tools,
    map_tool_choice,
};
use crate::utils::error_body::{ProviderHttpErrorParts, ProviderServiceError};

/// Result type for the Google Generative AI port.
pub type Result<T> = std::result::Result<T, GoogleGenerativeAiError>;

/// Errors returned by the Google Generative AI port.
#[derive(Debug)]
#[non_exhaustive]
pub enum GoogleGenerativeAiError {
    /// No API key was supplied for the model provider.
    MissingApiKey {
        /// Provider identifier from Pi.
        provider: String,
    },
    /// A provider hook rejected the request.
    Hook(crate::types::ProviderHookError),
    /// The HTTP transport failed.
    Http(reqwest::Error),
    /// Google returned a normalized provider service failure.
    Service(ProviderServiceError),
    /// A request header was invalid.
    InvalidHeader(String),
    /// Google returned malformed SSE framing or text.
    InvalidSse(String),
    /// Google returned malformed SSE JSON.
    InvalidResponse(serde_json::Error),
    /// Canonical message/event conversion failed.
    InvalidCanonicalEvent(serde_json::Error),
    /// The request was aborted.
    Aborted,
}

impl PartialEq for GoogleGenerativeAiError {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::MissingApiKey { provider: left }, Self::MissingApiKey { provider: right }) if left == right
        )
    }
}

impl Eq for GoogleGenerativeAiError {}

impl fmt::Display for GoogleGenerativeAiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApiKey { provider } => write!(f, "no API key for provider: {provider}"),
            Self::Hook(error) => error.fmt(f),
            Self::Http(error) => error.fmt(f),
            Self::Service(error) => error.fmt(f),
            Self::InvalidHeader(error) => f.write_str(error),
            Self::InvalidSse(error) => write!(f, "invalid Google SSE response: {error}"),
            Self::InvalidResponse(error) => write!(f, "invalid Google SSE response: {error}"),
            Self::InvalidCanonicalEvent(error) => {
                write!(f, "invalid canonical Google event: {error}")
            }
            Self::Aborted => f.write_str("request aborted"),
        }
    }
}

impl StdError for GoogleGenerativeAiError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Hook(error) => Some(error),
            Self::Http(error) => Some(error),
            Self::Service(error) => Some(error),
            Self::InvalidResponse(error) | Self::InvalidCanonicalEvent(error) => Some(error),
            Self::MissingApiKey { .. }
            | Self::InvalidHeader(_)
            | Self::InvalidSse(_)
            | Self::Aborted => None,
        }
    }
}

/// HTTP headers supplied by a model or request options.
pub type ProviderHeaders = HashMap<String, String>;

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

/// Prepared Google request plus deterministic streamed fixture output.
#[derive(Debug, Clone, PartialEq)]
pub struct AssistantMessageEventStream {
    /// HTTP request that the reqwest fallback would send.
    pub request: PreparedGoogleRequest,
    /// Collected stream events when deterministic chunks are supplied.
    pub collected: Option<GoogleAssistantMessageEventStream>,
}

/// Prepared reqwest fallback request.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedGoogleRequest {
    /// Request URL.
    pub url: String,
    /// Request headers.
    pub headers: ProviderHeaders,
    /// JSON payload after `on_payload` mutation.
    pub payload: Value,
}

/// Google thinking configuration.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ThinkingConfig {
    /// Whether Google should include visible thought summaries.
    pub include_thoughts: Option<bool>,
    /// Level-based thinking configuration for Gemini 3/Gemma 4 models.
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

/// Options specific to Pi's Google Generative AI stream implementation.
#[derive(Clone, Default)]
pub struct GoogleOptions {
    /// Sampling temperature.
    pub temperature: Option<f64>,
    /// Maximum output tokens requested by the caller.
    pub max_tokens: Option<u32>,
    /// API key for Google Generative AI.
    pub api_key: Option<String>,
    /// Optional custom HTTP headers.
    pub headers: ProviderHeaders,
    /// Google tool choice behavior.
    pub tool_choice: Option<GoogleToolChoice>,
    /// Google thinking options.
    pub thinking: Option<GoogleThinkingOptions>,
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
    /// API key for Google Generative AI.
    pub api_key: Option<String>,
    /// Optional custom HTTP headers.
    pub headers: ProviderHeaders,
    /// Unified reasoning level passed to simple streams.
    pub reasoning: Option<ThinkingLevel>,
    /// Custom token budgets per Google thinking level.
    pub thinking_budgets: ThinkingBudgets,
}

fn is_gemma4_model(model: &Model) -> bool {
    let id = model.id.to_lowercase();
    id.contains("gemma4") || id.contains("gemma-4")
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
    if is_gemini3_flash_model(model) || is_gemma4_model(model) {
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

fn get_thinking_level(effort: ClampedThinkingLevel, model: &Model) -> GoogleThinkingLevel {
    if is_gemini3_pro_model(model) {
        return match effort {
            ClampedThinkingLevel::Minimal | ClampedThinkingLevel::Low => GoogleThinkingLevel::Low,
            ClampedThinkingLevel::Medium | ClampedThinkingLevel::High => GoogleThinkingLevel::High,
        };
    }
    if is_gemma4_model(model) {
        return match effort {
            ClampedThinkingLevel::Minimal | ClampedThinkingLevel::Low => {
                GoogleThinkingLevel::Minimal
            }
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
        (id, ClampedThinkingLevel::Minimal) if id.contains("2.5-flash-lite") => 512,
        (id, ClampedThinkingLevel::Low) if id.contains("2.5-flash-lite") => 2048,
        (id, ClampedThinkingLevel::Medium) if id.contains("2.5-flash-lite") => 8192,
        (id, ClampedThinkingLevel::High) if id.contains("2.5-flash-lite") => 24576,
        (id, ClampedThinkingLevel::Minimal) if id.contains("2.5-flash") => 128,
        (id, ClampedThinkingLevel::Low) if id.contains("2.5-flash") => 2048,
        (id, ClampedThinkingLevel::Medium) if id.contains("2.5-flash") => 8192,
        (id, ClampedThinkingLevel::High) if id.contains("2.5-flash") => 24576,
        _ => -1,
    }
}

fn thinking_config(model: &Model, options: &GoogleOptions) -> Option<ThinkingConfig> {
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
        api: "google-generative-ai".to_string(),
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

fn build_params(model: &Model, context: &Context, options: &GoogleOptions) -> Value {
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

fn rest_payload_from_sdk(payload: Value) -> Value {
    let contents = payload
        .get("contents")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let config = payload.get("config").and_then(Value::as_object);
    let mut rest = serde_json::Map::new();
    rest.insert("contents".to_owned(), contents);

    if let Some(system) = config
        .and_then(|config| config.get("systemInstruction"))
        .and_then(Value::as_str)
    {
        rest.insert(
            "systemInstruction".to_owned(),
            json!({ "parts": [{ "text": system }] }),
        );
    }
    for field in ["tools", "toolConfig"] {
        if let Some(value) = config.and_then(|config| config.get(field)).cloned() {
            rest.insert(field.to_owned(), value);
        }
    }

    let mut generation = serde_json::Map::new();
    for field in ["temperature", "maxOutputTokens", "thinkingConfig"] {
        if let Some(value) = config.and_then(|config| config.get(field)).cloned() {
            generation.insert(field.to_owned(), value);
        }
    }
    if !generation.is_empty() {
        rest.insert("generationConfig".to_owned(), Value::Object(generation));
    }
    Value::Object(rest)
}

fn build_request(
    model: &Model,
    context: &Context,
    options: &GoogleOptions,
) -> Result<PreparedGoogleRequest> {
    let Some(api_key) = options.api_key.as_ref().filter(|key| !key.is_empty()) else {
        return Err(GoogleGenerativeAiError::MissingApiKey {
            provider: model.provider.clone(),
        });
    };
    let mut headers = model.headers.clone();
    headers.extend(options.headers.clone());
    headers.insert("x-goog-api-key".to_string(), api_key.clone());

    let base_url = model
        .base_url
        .as_deref()
        .unwrap_or("https://generativelanguage.googleapis.com/v1beta");
    let url = format!(
        "{}/models/{}:streamGenerateContent?alt=sse",
        base_url.trim_end_matches('/'),
        model.id
    );
    let mut payload = build_params(model, context, options);
    if let Some(on_payload) = &options.on_payload
        && let Some(next_payload) =
            futures::executor::block_on(on_payload(payload.clone(), model.clone()))
                .map_err(GoogleGenerativeAiError::Hook)?
    {
        payload = next_payload;
    }

    Ok(PreparedGoogleRequest {
        url,
        headers,
        payload,
    })
}

/// Streams a Google Generative AI request using a reqwest-compatible prepared request.
///
/// The current Rust port keeps the live network boundary out of deterministic tests; response
/// chunks supplied in options are collected with the same text/thinking/tool/usage mapping Pi uses.
///
/// # Errors
///
/// Returns [`GoogleGenerativeAiError::MissingApiKey`] when no API key is supplied.
pub fn stream(
    model: &Model,
    context: &Context,
    options: Option<&GoogleOptions>,
) -> Result<AssistantMessageEventStream> {
    let default_options;
    let options = if let Some(options) = options {
        options
    } else {
        default_options = GoogleOptions::default();
        &default_options
    };
    let request = build_request(model, context, options)?;
    let collected = (!options.response_chunks.is_empty()).then(|| {
        collect_google_stream(
            "google-generative-ai",
            model.provider.clone(),
            model.id.clone(),
            &options.response_chunks,
            options.id_timestamp_ms.unwrap_or_default(),
        )
    });

    Ok(AssistantMessageEventStream { request, collected })
}

/// Streams a Google Generative AI request using simplified options.
///
/// # Errors
///
/// Returns [`GoogleGenerativeAiError::MissingApiKey`] when no API key is supplied, or a port
/// placeholder until the Google GenAI streaming client is selected for Rust.
pub fn stream_simple(
    model: &Model,
    context: &Context,
    options: Option<&SimpleStreamOptions>,
) -> Result<AssistantMessageEventStream> {
    let options = options.cloned().unwrap_or_default();
    let Some(api_key) = options.api_key.clone() else {
        return Err(GoogleGenerativeAiError::MissingApiKey {
            provider: model.provider.clone(),
        });
    };

    let thinking = match options.reasoning {
        None => Some(GoogleThinkingOptions {
            enabled: false,
            budget_tokens: None,
            level: None,
        }),
        Some(reasoning) => {
            let effort = ClampedThinkingLevel::from(reasoning);
            if is_gemini3_pro_model(model)
                || is_gemini3_flash_model(model)
                || is_gemma4_model(model)
            {
                Some(GoogleThinkingOptions {
                    enabled: true,
                    budget_tokens: None,
                    level: Some(get_thinking_level(effort, model)),
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

    let stream_options = GoogleOptions {
        temperature: options.temperature,
        max_tokens: options.max_tokens,
        api_key: Some(api_key),
        headers: options.headers,
        thinking,
        ..GoogleOptions::default()
    };

    stream(model, context, Some(&stream_options))
}

/// Returns the canonical Google Generative AI request/SSE implementation.
#[must_use]
pub fn provider_streams() -> ProviderStreams {
    ProviderStreams {
        stream: Arc::new(stream_registered),
        stream_simple: Arc::new(stream_simple_registered),
    }
}

/// Starts a canonical Google request and returns immediately.
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

/// Starts Google using Pi's simple reasoning option mapping.
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
            matches!(error, GoogleGenerativeAiError::Aborted),
        ),
    }
}

async fn execute_registered(
    stream: &CanonicalEventStream,
    model: &CanonicalModel,
    context: &CanonicalContext,
    options: &StreamOptions,
) -> Result<crate::api::google_shared::GoogleAssistantMessage> {
    let api_key = options
        .api_key
        .as_deref()
        .filter(|key| !key.is_empty())
        .ok_or_else(|| GoogleGenerativeAiError::MissingApiKey {
            provider: model.provider.clone(),
        })?;
    check_abort(options.signal.as_ref())?;

    let local_model = Model {
        id: model.id.clone(),
        provider: model.provider.clone(),
        base_url: Some(model.base_url.clone()),
        reasoning: model.reasoning,
        headers: model.headers.clone().unwrap_or_default(),
    };
    let local_context = canonical_context(context)?;
    let google_options = GoogleOptions {
        temperature: options.temperature,
        max_tokens: options.max_tokens,
        api_key: Some(api_key.to_owned()),
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
        ..GoogleOptions::default()
    };
    let mut sdk_payload = build_params(&local_model, &local_context, &google_options);
    if let Some(hook) = options.on_payload.as_ref()
        && let Some(next) = hook(sdk_payload.clone(), model.clone())
            .await
            .map_err(GoogleGenerativeAiError::Hook)?
    {
        sdk_payload = next;
    }
    let payload = rest_payload_from_sdk(sdk_payload);
    let request = build_request(&local_model, &local_context, &google_options)?;
    let mut builder = reqwest::Client::builder();
    if let Some(timeout_ms) = options.timeout_ms {
        builder = builder.timeout(Duration::from_millis(timeout_ms));
    }
    let send = builder
        .build()
        .map_err(GoogleGenerativeAiError::Http)?
        .post(request.url)
        .headers(to_header_map(&request.headers)?)
        .body(payload.to_string())
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
        .map_err(GoogleGenerativeAiError::Hook)?;
    }
    if !(200..300).contains(&status) {
        let body = read_response_body(&mut response, options.signal.clone()).await?;
        let source = GoogleStatusError {
            status,
            body: body.clone(),
        };
        return Err(GoogleGenerativeAiError::Service(
            ProviderServiceError::with_source(
                ProviderHttpErrorParts::new("Google request failed")
                    .with_status(status)
                    .with_headers(response_headers)
                    .with_body(body),
                source,
            ),
        ));
    }

    let mut collector = GoogleStreamCollector::new(
        "google-generative-ai".to_owned(),
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
        let chunk = await_or_abort(response.chunk(), options.signal.clone()).await?;
        let Some(chunk) = chunk else { break };
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
            check_abort(options.signal.as_ref())?;
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
        tokio::task::yield_now().await;
        check_abort(options.signal.as_ref())?;
    }
    let (collected, frames) = collector.finish_incremental();
    emit_frames(stream, model, frames, options.signal.as_ref())?;
    check_abort(options.signal.as_ref())?;
    Ok(collected.message)
}

async fn await_or_abort<T>(
    future: impl std::future::Future<Output = std::result::Result<T, reqwest::Error>>,
    signal: Option<crate::types::AbortSignal>,
) -> Result<T> {
    if let Some(signal) = signal {
        match futures::future::select(Box::pin(future), Box::pin(wait_for_abort(signal))).await {
            futures::future::Either::Left((result, _)) => {
                result.map_err(GoogleGenerativeAiError::Http)
            }
            futures::future::Either::Right(((), _)) => Err(GoogleGenerativeAiError::Aborted),
        }
    } else {
        future.await.map_err(GoogleGenerativeAiError::Http)
    }
}

async fn wait_for_abort(signal: crate::types::AbortSignal) {
    while !signal.aborted() {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}

fn check_abort(signal: Option<&crate::types::AbortSignal>) -> Result<()> {
    if signal.is_some_and(crate::types::AbortSignal::aborted) {
        Err(GoogleGenerativeAiError::Aborted)
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
            "Google request failed with status {}: {}",
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
        GoogleGenerativeAiError::InvalidSse(format!("invalid UTF-8 in Google SSE: {error}"))
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
            .map_err(GoogleGenerativeAiError::InvalidResponse)
    }
}

fn registered_thinking(model: &Model, options: &StreamOptions) -> Option<GoogleThinkingOptions> {
    let reasoning = options.extra.get("reasoning").and_then(Value::as_str);
    let enabled = reasoning.is_some();
    if !model.reasoning {
        return None;
    }
    if !enabled {
        return Some(GoogleThinkingOptions {
            enabled: false,
            budget_tokens: None,
            level: None,
        });
    }
    let effort = match reasoning {
        Some("minimal") => ClampedThinkingLevel::Minimal,
        Some("low") => ClampedThinkingLevel::Low,
        Some("medium") => ClampedThinkingLevel::Medium,
        _ => ClampedThinkingLevel::High,
    };
    if is_gemini3_pro_model(model) || is_gemini3_flash_model(model) || is_gemma4_model(model) {
        Some(GoogleThinkingOptions {
            enabled: true,
            budget_tokens: None,
            level: Some(get_thinking_level(effort, model)),
        })
    } else {
        Some(GoogleThinkingOptions {
            enabled: true,
            budget_tokens: Some(get_google_budget(model, effort, &ThinkingBudgets::new())),
            level: None,
        })
    }
}

fn canonical_context(context: &CanonicalContext) -> Result<Context> {
    let value = serde_json::to_value(context).map_err(GoogleGenerativeAiError::InvalidResponse)?;
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
    let parts = |value: &Value| {
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
                .unwrap_or_else(|| UserContent::Parts(parts(&message["content"]))),
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
                        redacted: false,
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
            api: message.get("api")?.as_str()?.to_owned(),
            provider: message.get("provider")?.as_str()?.to_owned(),
            model: message.get("model")?.as_str()?.to_owned(),
            stop_reason: match message.get("stopReason").and_then(Value::as_str) {
                Some("error") => crate::api::google_shared::StopReason::Error,
                Some("aborted") => crate::api::google_shared::StopReason::Aborted,
                Some("toolUse") => crate::api::google_shared::StopReason::ToolUse,
                Some("length") => crate::api::google_shared::StopReason::Length,
                _ => crate::api::google_shared::StopReason::Stop,
            },
        }),
        "toolResult" => Some(Message::ToolResult {
            tool_call_id: message.get("toolCallId")?.as_str()?.to_owned(),
            tool_name: message.get("toolName")?.as_str()?.to_owned(),
            content: parts(&message["content"]),
            is_error: message
                .get("isError")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }),
        _ => None,
    }
}

fn to_header_map(headers: &ProviderHeaders) -> Result<reqwest::header::HeaderMap> {
    let mut map = reqwest::header::HeaderMap::new();
    for (name, value) in headers {
        let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
            .map_err(|error| GoogleGenerativeAiError::InvalidHeader(error.to_string()))?;
        let value = reqwest::header::HeaderValue::from_str(value)
            .map_err(|error| GoogleGenerativeAiError::InvalidHeader(error.to_string()))?;
        map.insert(name, value);
    }
    map.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/json"),
    );
    Ok(map)
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
        "role": "assistant", "content": content, "api": "google-generative-ai",
        "provider": model.provider, "model": model.id, "responseId": message.response_id,
        "usage": { "input": message.usage.input, "output": message.usage.output,
            "cacheRead": message.usage.cache_read, "cacheWrite": message.usage.cache_write,
            "reasoning": message.usage.reasoning, "totalTokens": message.usage.total_tokens,
            "cost": { "input": input_cost, "output": output_cost, "cacheRead": cache_read_cost,
                "cacheWrite": cache_write_cost,
                "total": input_cost + output_cost + cache_read_cost + cache_write_cost } },
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
                json!({ "type": "toolcall_end", "contentIndex": content_index,
                    "toolCall": partial["content"][content_index].clone(), "partial": partial })
            }
            crate::api::google_shared::GoogleStreamEvent::Done { .. } => continue,
        };
        stream.push(
            serde_json::from_value::<AssistantMessageEvent>(event)
                .map_err(GoogleGenerativeAiError::InvalidCanonicalEvent)?,
        );
        check_abort(signal)?;
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
        "role": "assistant", "content": [], "api": "google-generative-ai", "provider": model.provider,
        "model": model.id, "usage": { "input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "totalTokens": 0,
        "cost": { "input": 0.0, "output": 0.0, "cacheRead": 0.0, "cacheWrite": 0.0, "total": 0.0 } },
        "stopReason": if aborted { "aborted" } else { "error" }, "errorMessage": error, "timestamp": now_ms()
    })).expect("canonical Google terminal message");
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
            provider: "google".to_string(),
            base_url: None,
            reasoning: true,
            headers: HashMap::new(),
        }
    }

    #[test]
    fn disabled_thinking_config_matches_pi_model_families() {
        assert_eq!(
            get_disabled_thinking_config(&model("gemini-3.1-pro-preview")).thinking_level,
            Some(GoogleThinkingLevel::Low)
        );
        assert_eq!(
            get_disabled_thinking_config(&model("gemini-flash-lite-latest")).thinking_level,
            Some(GoogleThinkingLevel::Minimal)
        );
        assert_eq!(
            get_disabled_thinking_config(&model("gemma-4-preview")).thinking_level,
            Some(GoogleThinkingLevel::Minimal)
        );
        assert_eq!(
            get_disabled_thinking_config(&model("gemini-2.5-pro")).thinking_budget,
            Some(0)
        );
    }

    #[test]
    fn google_budget_defaults_match_pi_source() {
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
            512
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
    fn stream_simple_requires_api_key() {
        let err = stream_simple(&model("gemini-2.5-pro"), &Context::default(), None)
            .expect_err("missing api key should be reported");
        assert_eq!(
            err,
            GoogleGenerativeAiError::MissingApiKey {
                provider: "google".to_string()
            }
        );
    }

    #[test]
    fn stream_builds_payload_and_applies_on_payload() {
        let options = GoogleOptions {
            api_key: Some("key".to_string()),
            on_payload: Some(Arc::new(|mut payload, _model| {
                payload["config"]["temperature"] = json!(0.7);
                Box::pin(async move { Ok(Some(payload)) })
            })),
            ..GoogleOptions::default()
        };

        let stream = stream(
            &model("gemini-2.5-pro"),
            &Context::default(),
            Some(&options),
        )
        .expect("request should be prepared");

        assert_eq!(
            stream.request.url,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-pro:streamGenerateContent?alt=sse"
        );
        assert_eq!(
            stream.request.headers.get("x-goog-api-key"),
            Some(&"key".to_string())
        );
        assert_eq!(stream.request.payload["config"]["temperature"], json!(0.7));
        assert!(stream.collected.is_none());
    }

    #[test]
    fn stream_collects_google_chunks_with_tool_use_override() {
        let options = GoogleOptions {
            api_key: Some("key".to_string()),
            id_timestamp_ms: Some(42),
            response_chunks: vec![GenerateContentChunk {
                response_id: Some("resp-1".to_string()),
                candidates: vec![crate::api::google_shared::Candidate {
                    content: Some(crate::api::google_shared::Content {
                        role: "model".to_string(),
                        parts: vec![crate::api::google_shared::Part {
                            function_call: Some(crate::api::google_shared::FunctionCall {
                                name: "run".to_string(),
                                args: json!({ "command": "echo hi" }),
                                id: None,
                            }),
                            thought_signature: Some("sig".to_string()),
                            ..crate::api::google_shared::Part::default()
                        }],
                    }),
                    finish_reason: Some("STOP".to_string()),
                }],
                usage_metadata: Some(crate::api::google_shared::UsageMetadata {
                    prompt_token_count: 10,
                    cached_content_token_count: 3,
                    candidates_token_count: 4,
                    thoughts_token_count: 2,
                    total_token_count: 16,
                }),
            }],
            ..GoogleOptions::default()
        };

        let stream = stream(
            &model("gemini-2.5-pro"),
            &Context::default(),
            Some(&options),
        )
        .expect("request should be prepared");
        let collected = stream.collected.expect("fixture chunks should collect");

        assert_eq!(collected.message.response_id.as_deref(), Some("resp-1"));
        assert_eq!(collected.message.stop_reason, StopReason::ToolUse);
        assert_eq!(collected.message.usage.input, 7);
        assert_eq!(collected.message.usage.cache_read, 3);
        assert_eq!(collected.message.usage.output, 6);
        assert_eq!(
            collected.events.last(),
            Some(&GoogleStreamEvent::Done {
                reason: StopReason::ToolUse
            })
        );
        match &collected.message.content[0] {
            crate::api::google_shared::GoogleContentBlock::ToolCall { id, .. } => {
                assert_eq!(id, "run_42_1");
            }
            other => panic!("unexpected content: {other:?}"),
        }
    }
}
