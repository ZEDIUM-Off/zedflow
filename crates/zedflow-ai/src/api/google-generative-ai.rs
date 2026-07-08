//! Google Generative AI API ported from Pi.

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

/// Result type for the Google Generative AI port.
pub type Result<T> = std::result::Result<T, GoogleGenerativeAiError>;

/// Errors returned by the Google Generative AI port.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GoogleGenerativeAiError {
    /// No API key was supplied for the model provider.
    MissingApiKey {
        /// Provider identifier from Pi.
        provider: String,
    },
}

impl fmt::Display for GoogleGenerativeAiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApiKey { provider } => write!(f, "no API key for provider: {provider}"),
        }
    }
}

impl StdError for GoogleGenerativeAiError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::MissingApiKey { .. } => None,
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
pub type PayloadHook = Arc<dyn Fn(Value, &Model) -> Option<Value> + Send + Sync>;

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
        && let Some(next_payload) = on_payload(payload.clone(), model)
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
                Some(payload)
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
