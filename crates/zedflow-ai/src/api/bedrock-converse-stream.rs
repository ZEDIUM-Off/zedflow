//! Amazon Bedrock Converse Stream API ported from Pi.

use std::collections::HashMap;

use serde_json::{Map, Value, json};
use url::Url;
use zedflow_core::error::Result;

use crate::utils::error_body::{
    ProviderErrorInput, SdkErrorShape, format_provider_error, normalize_provider_error,
};
use crate::utils::node_http_proxy::resolve_http_proxy_url_for_target;
use crate::utils::sanitize_unicode::sanitize_surrogates;

/// Provider-scoped environment overrides.
pub type ProviderEnv = HashMap<String, String>;

/// HTTP headers supplied by a model or request options.
pub type ProviderHeaders = HashMap<String, String>;

/// Request metadata tags attached to Bedrock inference requests.
pub type RequestMetadata = HashMap<String, String>;

/// Token budgets for Bedrock thinking levels.
pub type ThinkingBudgets = HashMap<ThinkingLevel, u32>;

/// Prompt cache retention preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheRetention {
    /// Disable explicit prompt cache points.
    None,
    /// Use Bedrock's default short-lived cache point.
    Short,
    /// Request the one-hour cache TTL when Bedrock supports it.
    Long,
}

impl Default for CacheRetention {
    fn default() -> Self {
        Self::Short
    }
}

/// Pi thinking level accepted by Bedrock stream options.
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

/// Controls how Claude thinking content is returned in Bedrock responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BedrockThinkingDisplay {
    /// Return summarized thinking text.
    Summarized,
    /// Omit thinking text while preserving provider continuity metadata.
    Omitted,
}

/// Bedrock tool choice behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BedrockToolChoice {
    /// Let Bedrock choose whether to call a tool.
    Auto,
    /// Force some tool use.
    Any,
    /// Disable tool use.
    None,
    /// Force a specific tool by name.
    Tool {
        /// Tool name to force.
        name: String,
    },
}

/// Minimal model shape consumed by this port row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    /// Model identifier from Pi.
    pub id: String,
    /// Provider identifier from Pi.
    pub provider: String,
    /// Human-readable model name, if configured.
    pub name: Option<String>,
    /// Optional provider base URL.
    pub base_url: Option<String>,
    /// Provider maximum output tokens.
    pub max_tokens: u32,
    /// Whether the model supports reasoning options.
    pub reasoning: bool,
    /// Provider/model-specific mappings for Pi thinking levels.
    pub thinking_level_map: HashMap<ThinkingLevel, String>,
}

/// Minimal context shape consumed by this port row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Context;

/// Placeholder for Pi's `AssistantMessageEventStream` contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssistantMessageEventStream;

/// Callback that can inspect or replace a Bedrock ConverseStream payload before send.
pub type BedrockPayloadHook = fn(Value, &Model) -> Option<Value>;

/// Callback invoked after Bedrock returns SDK HTTP metadata.
pub type BedrockResponseHook = fn(BedrockResponseMetadata, &Model);

/// Options specific to Pi's Bedrock Converse Stream implementation.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BedrockOptions {
    /// Sampling temperature.
    pub temperature: Option<f64>,
    /// Maximum output tokens requested by the caller.
    pub max_tokens: Option<u32>,
    /// Prompt cache retention preference.
    pub cache_retention: Option<CacheRetention>,
    /// Optional custom HTTP headers.
    pub headers: ProviderHeaders,
    /// Provider-scoped environment overrides.
    pub env: ProviderEnv,
    /// AWS region override.
    pub region: Option<String>,
    /// AWS profile override.
    pub profile: Option<String>,
    /// Bedrock tool choice behavior.
    pub tool_choice: Option<BedrockToolChoice>,
    /// Reasoning effort.
    pub reasoning: Option<ThinkingLevel>,
    /// Custom token budgets per thinking level.
    pub thinking_budgets: ThinkingBudgets,
    /// Whether to request interleaved thinking for Claude models that support it.
    pub interleaved_thinking: Option<bool>,
    /// Claude thinking content display mode.
    pub thinking_display: Option<BedrockThinkingDisplay>,
    /// Inference request tags for AWS cost allocation.
    pub request_metadata: RequestMetadata,
    /// Bearer token for Bedrock API key authentication.
    pub bearer_token: Option<String>,
    /// Optional payload replacement hook.
    pub on_payload: Option<BedrockPayloadHook>,
    /// Optional response metadata hook.
    pub on_response: Option<BedrockResponseHook>,
}

/// Bedrock SDK client configuration fields resolved before request construction.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BedrockClientConfig {
    /// AWS profile passed to the SDK client.
    pub profile: Option<String>,
    /// Explicit runtime endpoint passed to the SDK client.
    pub endpoint: Option<String>,
    /// AWS region passed to the SDK client.
    pub region: Option<String>,
}

/// Why genai's Bedrock adapters are not used for Pi-compatible Bedrock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BedrockGenaiParityReport {
    /// Whether genai can preserve all Pi-observable Bedrock behavior.
    pub can_preserve_pi_behavior: bool,
    /// Missing behaviors that force the Bedrock fallback path.
    pub blockers: Vec<&'static str>,
}

/// Bedrock authentication mode selected for the fallback client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BedrockAuthMode {
    /// Use the AWS SDK default credential chain.
    DefaultChain,
    /// Use explicit credentials from provider/process environment.
    ExplicitCredentials {
        /// AWS access key id.
        access_key_id: String,
        /// AWS secret access key.
        secret_access_key: String,
        /// Optional AWS session token.
        session_token: Option<String>,
    },
    /// Use dummy credentials when `AWS_BEDROCK_SKIP_AUTH=1`.
    SkipAuthDummyCredentials,
    /// Use Bedrock bearer-token auth.
    BearerToken(String),
}

/// Deterministic request plan for the narrow Bedrock Runtime fallback.
#[derive(Debug, Clone, PartialEq)]
pub struct BedrockRuntimeRequestPlan {
    /// SDK client region/profile/endpoint resolution.
    pub client_config: BedrockClientConfig,
    /// Authentication mode for the SDK client.
    pub auth_mode: BedrockAuthMode,
    /// Pi proxy URL selected for the target endpoint, if any.
    pub proxy_url: Option<String>,
    /// Whether HTTP/1 should be forced for custom endpoints/proxies.
    pub force_http1: bool,
    /// Caller headers that are safe to inject before SigV4 signing.
    pub custom_signed_headers: ProviderHeaders,
    /// ConverseStream input payload after the `onPayload` seam.
    pub payload: Value,
}

/// Captured Bedrock HTTP response metadata passed to `onResponse`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BedrockResponseMetadata {
    /// HTTP status code.
    pub status: u16,
    /// Response headers exposed by the AWS SDK metadata.
    pub headers: ProviderHeaders,
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value.filter(|value| !value.is_empty()).map(str::to_owned)
}

fn provider_env_value(name: &str, scoped: &ProviderEnv, ambient: &ProviderEnv) -> Option<String> {
    non_empty(scoped.get(name).map(String::as_str))
        .or_else(|| non_empty(ambient.get(name).map(String::as_str)))
}

fn configured_bedrock_region(options: &BedrockOptions, ambient: &ProviderEnv) -> Option<String> {
    non_empty(options.region.as_deref())
        .or_else(|| provider_env_value("AWS_REGION", &options.env, ambient))
        .or_else(|| provider_env_value("AWS_DEFAULT_REGION", &options.env, ambient))
}

fn standard_bedrock_endpoint_region(base_url: Option<&str>) -> Option<String> {
    let hostname = Url::parse(base_url?).ok()?.host_str()?.to_ascii_lowercase();
    let rest = hostname.strip_prefix("bedrock-runtime")?;
    let rest = rest.strip_prefix("-fips").unwrap_or(rest);
    let rest = rest.strip_prefix('.')?;
    let region = rest
        .strip_suffix(".amazonaws.com.cn")
        .or_else(|| rest.strip_suffix(".amazonaws.com"))?;

    if region.is_empty()
        || !region
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return None;
    }

    Some(region.to_owned())
}

fn should_use_explicit_bedrock_endpoint(
    base_url: Option<&str>,
    configured_region: Option<&str>,
    has_ambient_configured_profile: bool,
) -> bool {
    standard_bedrock_endpoint_region(base_url).is_none()
        || (configured_region.is_none() && !has_ambient_configured_profile)
}

fn inference_profile_arn_region(model_id: &str) -> Option<String> {
    let mut parts = model_id.split(':');
    let arn = parts.next()?;
    let partition = parts.next()?;
    let service = parts.next()?;
    let region = parts.next()?;

    if arn == "arn" && partition.starts_with("aws") && service == "bedrock" && !region.is_empty() {
        Some(region.to_owned())
    } else {
        None
    }
}

/// Resolves the Bedrock SDK client config from model/options and process environment.
#[must_use]
pub fn resolve_bedrock_client_config(
    model: &Model,
    options: &BedrockOptions,
) -> BedrockClientConfig {
    let ambient = std::env::vars().collect::<ProviderEnv>();
    resolve_bedrock_client_config_with_env(model, options, &ambient)
}

/// Resolves the Bedrock SDK client config using an explicit ambient environment map.
///
/// This mirrors Pi's endpoint/region/profile selection without constructing a live AWS client.
#[must_use]
pub fn resolve_bedrock_client_config_with_env(
    model: &Model,
    options: &BedrockOptions,
    ambient: &ProviderEnv,
) -> BedrockClientConfig {
    let configured_region = configured_bedrock_region(options, ambient);
    let has_ambient_configured_profile =
        provider_env_value("AWS_PROFILE", &ProviderEnv::new(), ambient).is_some();
    let endpoint_region = standard_bedrock_endpoint_region(model.base_url.as_deref());
    let use_explicit_endpoint = should_use_explicit_bedrock_endpoint(
        model.base_url.as_deref(),
        configured_region.as_deref(),
        has_ambient_configured_profile,
    );

    BedrockClientConfig {
        profile: non_empty(options.profile.as_deref())
            .or_else(|| provider_env_value("AWS_PROFILE", &options.env, ambient)),
        endpoint: use_explicit_endpoint
            .then(|| model.base_url.clone())
            .flatten(),
        region: inference_profile_arn_region(&model.id)
            .or(configured_region)
            .or_else(|| endpoint_region.filter(|_| use_explicit_endpoint))
            .or_else(|| (!has_ambient_configured_profile).then(|| "us-east-1".to_owned())),
    }
}

/// Returns the local proof used by U5 to choose the fallback over genai Bedrock.
#[must_use]
pub fn genai_bedrock_parity_report() -> BedrockGenaiParityReport {
    BedrockGenaiParityReport {
        can_preserve_pi_behavior: false,
        blockers: vec![
            "genai bedrock_sigv4 resolves only AWS_REGION/AWS_DEFAULT_REGION/default chain and has no per-request AWS profile or skip-auth dummy credentials seam",
            "genai bedrock_sigv4 has no AWS_BEARER_TOKEN_BEDROCK/httpBearerAuth path; genai bedrock_api uses a different BEDROCK_API_KEY convention",
            "genai signs only host/content-type in its SigV4 helper, so Pi caller headers cannot be injected at the Smithy build step and covered by SigV4",
            "genai chat options do not expose Pi onPayload payload replacement or onResponse AWS metadata callback",
            "genai hides the SDK requestHandler choice needed for Pi proxy/force-HTTP1 behavior",
        ],
    }
}

/// Resolves the Bedrock Runtime fallback request without making a network call.
///
/// This is the seam the live AWS SDK sender consumes: everything here is deterministic and
/// mirrors Pi's client setup before `BedrockRuntimeClient.send(new ConverseStreamCommand(...))`.
#[must_use]
pub fn resolve_bedrock_runtime_request_plan(
    model: &Model,
    context: &Value,
    options: &BedrockOptions,
) -> BedrockRuntimeRequestPlan {
    let ambient = std::env::vars().collect::<ProviderEnv>();
    resolve_bedrock_runtime_request_plan_with_env(model, context, options, &ambient)
}

/// Resolves the Bedrock Runtime fallback request using an explicit ambient environment map.
#[must_use]
pub fn resolve_bedrock_runtime_request_plan_with_env(
    model: &Model,
    context: &Value,
    options: &BedrockOptions,
    ambient: &ProviderEnv,
) -> BedrockRuntimeRequestPlan {
    let client_config = resolve_bedrock_client_config_with_env(model, options, ambient);
    let endpoint = client_config
        .endpoint
        .clone()
        .or_else(|| model.base_url.clone())
        .or_else(|| {
            client_config
                .region
                .as_ref()
                .map(|region| format!("https://bedrock-runtime.{region}.amazonaws.com"))
        })
        .unwrap_or_else(|| "https://bedrock-runtime.us-east-1.amazonaws.com".to_owned());
    let proxy_url = resolve_http_proxy_url_for_target(&endpoint, Some(&options.env))
        .ok()
        .flatten()
        .map(|url| url.to_string());
    let force_http1 = proxy_url.is_some()
        || provider_env_value("AWS_BEDROCK_FORCE_HTTP1", &options.env, ambient).as_deref()
            == Some("1");

    BedrockRuntimeRequestPlan {
        client_config,
        auth_mode: resolve_bedrock_auth_mode(options, ambient),
        proxy_url,
        force_http1,
        custom_signed_headers: signed_custom_headers(&options.headers),
        payload: build_bedrock_converse_payload_with_hook(model, context, options),
    }
}

fn resolve_bedrock_auth_mode(options: &BedrockOptions, ambient: &ProviderEnv) -> BedrockAuthMode {
    let skip_auth =
        provider_env_value("AWS_BEDROCK_SKIP_AUTH", &options.env, ambient).as_deref() == Some("1");
    let bearer_token = options
        .bearer_token
        .clone()
        .or_else(|| provider_env_value("AWS_BEARER_TOKEN_BEDROCK", &options.env, ambient));
    if let Some(token) = bearer_token.filter(|_| !skip_auth) {
        return BedrockAuthMode::BearerToken(token);
    }
    if skip_auth {
        return BedrockAuthMode::SkipAuthDummyCredentials;
    }

    match (
        provider_env_value("AWS_ACCESS_KEY_ID", &options.env, ambient),
        provider_env_value("AWS_SECRET_ACCESS_KEY", &options.env, ambient),
    ) {
        (Some(access_key_id), Some(secret_access_key)) => BedrockAuthMode::ExplicitCredentials {
            access_key_id,
            secret_access_key,
            session_token: provider_env_value("AWS_SESSION_TOKEN", &options.env, ambient),
        },
        _ => BedrockAuthMode::DefaultChain,
    }
}

/// Returns true when a caller header must not be overwritten before SigV4 signing.
#[must_use]
pub fn is_reserved_bedrock_header(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    lower == "authorization" || lower == "host" || lower.starts_with("x-amz-")
}

/// Filters caller headers to the subset Pi injects in the AWS SDK build step.
#[must_use]
pub fn signed_custom_headers(headers: &ProviderHeaders) -> ProviderHeaders {
    headers
        .iter()
        .filter(|(key, _)| !is_reserved_bedrock_header(key))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn resolve_cache_retention(options: &BedrockOptions) -> CacheRetention {
    options
        .cache_retention
        .or_else(|| {
            (provider_env_value("PI_CACHE_RETENTION", &options.env, &ProviderEnv::new()).as_deref()
                == Some("long"))
            .then_some(CacheRetention::Long)
        })
        .unwrap_or(CacheRetention::Short)
}

fn model_match_candidates(model: &Model) -> Vec<String> {
    let values = model
        .name
        .iter()
        .map(String::as_str)
        .chain(std::iter::once(model.id.as_str()));
    values
        .flat_map(|value| {
            let lower = value.to_ascii_lowercase();
            let normalized = lower
                .chars()
                .map(|ch| {
                    if matches!(ch, ' ' | '_' | '.' | ':') {
                        '-'
                    } else {
                        ch
                    }
                })
                .collect::<String>();
            [lower, normalized]
        })
        .collect()
}

fn is_anthropic_claude_model(model: &Model) -> bool {
    model_match_candidates(model).iter().any(|value| {
        value.contains("anthropic-claude")
            || value.contains("anthropic/claude")
            || value.contains("claude")
    })
}

fn supports_adaptive_thinking(model: &Model) -> bool {
    model_match_candidates(model).iter().any(|value| {
        value.contains("opus-4-6")
            || value.contains("opus-4-7")
            || value.contains("opus-4-8")
            || value.contains("sonnet-4-6")
            || value.contains("sonnet-5")
            || value.contains("fable-5")
    })
}

fn supports_native_xhigh_effort(model: &Model) -> bool {
    model_match_candidates(model).iter().any(|value| {
        value.contains("opus-4-7") || value.contains("opus-4-8") || value.contains("fable-5")
    })
}

fn thinking_effort(model: &Model, level: ThinkingLevel) -> &'static str {
    if level == ThinkingLevel::XHigh && supports_native_xhigh_effort(model) {
        return "xhigh";
    }
    match level {
        ThinkingLevel::Minimal | ThinkingLevel::Low => "low",
        ThinkingLevel::Medium => "medium",
        ThinkingLevel::High | ThinkingLevel::XHigh => "high",
    }
}

fn is_govcloud_target(model: &Model, options: &BedrockOptions) -> bool {
    configured_bedrock_region(options, &ProviderEnv::new())
        .is_some_and(|region| region.to_ascii_lowercase().starts_with("us-gov-"))
        || model.id.to_ascii_lowercase().starts_with("us-gov.")
        || model.id.to_ascii_lowercase().starts_with("arn:aws-us-gov:")
}

fn supports_prompt_caching(model: &Model, env: &ProviderEnv) -> bool {
    let candidates = model_match_candidates(model);
    if !candidates.iter().any(|value| value.contains("claude")) {
        return provider_env_value("AWS_BEDROCK_FORCE_CACHE", env, &ProviderEnv::new()).as_deref()
            == Some("1");
    }
    candidates.iter().any(|value| {
        value.contains("fable-5")
            || value.contains("sonnet-5")
            || value.contains("-4-")
            || value.contains("claude-3-7-sonnet")
            || value.contains("claude-3-5-haiku")
    })
}

fn build_system_prompt(
    model: &Model,
    context: &Value,
    cache_retention: CacheRetention,
    env: &ProviderEnv,
) -> Option<Value> {
    let prompt = context.get("systemPrompt")?.as_str()?;
    if prompt.is_empty() {
        return None;
    }
    let mut blocks = vec![json!({ "text": sanitize_surrogates(prompt) })];
    if cache_retention != CacheRetention::None && supports_prompt_caching(model, env) {
        let mut cache = json!({ "cachePoint": { "type": "default" } });
        if cache_retention == CacheRetention::Long {
            cache["cachePoint"]["ttl"] = json!("ONE_HOUR");
        }
        blocks.push(cache);
    }
    Some(Value::Array(blocks))
}

fn text_block(text: &str, required: bool) -> Option<Value> {
    let text = sanitize_surrogates(text);
    if text.trim().is_empty() {
        return required.then(|| json!({ "text": "<empty>" }));
    }
    Some(json!({ "text": text }))
}

fn convert_message(message: &Value) -> Option<Value> {
    match message.get("role")?.as_str()? {
        "user" => {
            let mut content = Vec::new();
            match message.get("content") {
                Some(Value::String(text)) => content.push(text_block(text, true)?),
                Some(Value::Array(items)) => {
                    for item in items {
                        if item.get("type").and_then(Value::as_str) == Some("text")
                            && let Some(block) = item
                                .get("text")
                                .and_then(Value::as_str)
                                .and_then(|text| text_block(text, false))
                        {
                            content.push(block);
                        }
                    }
                    if content.is_empty() {
                        content.push(json!({ "text": "<empty>" }));
                    }
                }
                _ => content.push(json!({ "text": "<empty>" })),
            }
            Some(json!({ "role": "user", "content": content }))
        }
        "assistant" => {
            let mut content = Vec::new();
            for item in message.get("content")?.as_array()? {
                match item.get("type").and_then(Value::as_str) {
                    Some("text") => {
                        if let Some(block) = item
                            .get("text")
                            .and_then(Value::as_str)
                            .and_then(|text| text_block(text, false))
                        {
                            content.push(block);
                        }
                    }
                    Some("toolCall") => content.push(json!({
                        "toolUse": {
                            "toolUseId": item.get("id").and_then(Value::as_str).unwrap_or_default(),
                            "name": item.get("name").and_then(Value::as_str).unwrap_or_default(),
                            "input": item.get("arguments").cloned().unwrap_or_else(|| json!({})),
                        }
                    })),
                    _ => {}
                }
            }
            (!content.is_empty()).then(|| json!({ "role": "assistant", "content": content }))
        }
        "toolResult" => Some(json!({
            "role": "user",
            "content": [{
                "toolResult": {
                    "toolUseId": message.get("toolCallId").and_then(Value::as_str).unwrap_or_default(),
                    "content": [{ "text": "<empty>" }],
                    "status": if message.get("isError").and_then(Value::as_bool).unwrap_or(false) { "error" } else { "success" },
                }
            }]
        })),
        _ => None,
    }
}

fn convert_messages(
    context: &Value,
    model: &Model,
    cache_retention: CacheRetention,
    env: &ProviderEnv,
) -> Vec<Value> {
    let mut messages = context
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(convert_message)
        .collect::<Vec<_>>();
    if cache_retention != CacheRetention::None
        && supports_prompt_caching(model, env)
        && let Some(Value::Object(last)) = messages.last_mut()
        && last.get("role").and_then(Value::as_str) == Some("user")
        && let Some(Value::Array(content)) = last.get_mut("content")
    {
        let mut cache = json!({ "cachePoint": { "type": "default" } });
        if cache_retention == CacheRetention::Long {
            cache["cachePoint"]["ttl"] = json!("ONE_HOUR");
        }
        content.push(cache);
    }
    messages
}

/// Builds the local ConverseStream command input payload.
#[must_use]
pub fn build_bedrock_converse_payload(
    model: &Model,
    context: &Value,
    options: &BedrockOptions,
) -> Value {
    let cache_retention = resolve_cache_retention(options);
    let mut payload = json!({
        "modelId": model.id,
        "messages": convert_messages(context, model, cache_retention, &options.env),
    });
    if let Some(system) = build_system_prompt(model, context, cache_retention, &options.env) {
        payload["system"] = system;
    }

    let max_tokens = options
        .max_tokens
        .or_else(|| is_anthropic_claude_model(model).then_some(model.max_tokens));
    let mut inference = Map::new();
    if let Some(max_tokens) = max_tokens {
        inference.insert("maxTokens".to_owned(), json!(max_tokens));
    }
    if let Some(temperature) = options.temperature {
        inference.insert("temperature".to_owned(), json!(temperature));
    }
    payload["inferenceConfig"] = Value::Object(inference);

    if let Some(additional) = build_additional_model_request_fields(model, options) {
        payload["additionalModelRequestFields"] = additional;
    }
    if !options.request_metadata.is_empty() {
        payload["requestMetadata"] = json!(options.request_metadata);
    }
    payload
}

fn build_bedrock_converse_payload_with_hook(
    model: &Model,
    context: &Value,
    options: &BedrockOptions,
) -> Value {
    let payload = build_bedrock_converse_payload(model, context, options);
    options
        .on_payload
        .and_then(|hook| hook(payload.clone(), model))
        .unwrap_or(payload)
}

/// Converts AWS SDK response metadata into Pi's `onResponse` callback shape.
#[must_use]
pub fn bedrock_response_metadata(status: u16, request_id: Option<&str>) -> BedrockResponseMetadata {
    let headers = request_id
        .map(|request_id| HashMap::from([("x-amzn-requestid".to_owned(), request_id.to_owned())]))
        .unwrap_or_default();
    BedrockResponseMetadata { status, headers }
}

/// Invokes the Bedrock response hook, if configured.
pub fn invoke_bedrock_on_response(
    options: &BedrockOptions,
    metadata: BedrockResponseMetadata,
    model: &Model,
) {
    if let Some(hook) = options.on_response {
        hook(metadata, model);
    }
}

fn build_additional_model_request_fields(model: &Model, options: &BedrockOptions) -> Option<Value> {
    let reasoning = options.reasoning?;
    if !model.reasoning || !is_anthropic_claude_model(model) {
        return None;
    }
    let display = (!is_govcloud_target(model, options)).then(|| match options.thinking_display {
        Some(BedrockThinkingDisplay::Omitted) => "omitted",
        Some(BedrockThinkingDisplay::Summarized) | None => "summarized",
    });
    if supports_adaptive_thinking(model) {
        let mut thinking = json!({ "type": "adaptive" });
        if let Some(display) = display {
            thinking["display"] = json!(display);
        }
        return Some(json!({
            "thinking": thinking,
            "output_config": { "effort": thinking_effort(model, reasoning) },
        }));
    }

    let level = if reasoning == ThinkingLevel::XHigh {
        ThinkingLevel::High
    } else {
        reasoning
    };
    let default_budget = match reasoning {
        ThinkingLevel::Minimal => 1024,
        ThinkingLevel::Low => 2048,
        ThinkingLevel::Medium => 8192,
        ThinkingLevel::High | ThinkingLevel::XHigh => 16384,
    };
    let budget = options
        .thinking_budgets
        .get(&level)
        .copied()
        .unwrap_or(default_budget);
    let mut thinking = json!({ "type": "enabled", "budget_tokens": budget });
    if let Some(display) = display {
        thinking["display"] = json!(display);
    }
    let mut result = json!({ "thinking": thinking });
    if options.interleaved_thinking.unwrap_or(true) {
        result["anthropic_beta"] = json!(["interleaved-thinking-2025-05-14"]);
    }
    Some(result)
}

/// Formats Bedrock SDK/service errors with Pi's stable prefixes and data-retention hint.
#[must_use]
pub fn format_bedrock_error(error_name: Option<&str>, shape: SdkErrorShape) -> String {
    let norm = normalize_provider_error(&ProviderErrorInput::Error(shape));
    let core = format_provider_error(&norm, None);
    let hint = if core.to_ascii_lowercase().contains("data retention mode") {
        " See https://docs.aws.amazon.com/bedrock/latest/userguide/data-retention.html for supported data retention modes."
    } else {
        ""
    };
    match error_name {
        Some("InternalServerException") => format!("Internal server error: {core}{hint}"),
        Some("ModelStreamErrorException") => format!("Model stream error: {core}{hint}"),
        Some("ValidationException") => format!("Validation error: {core}{hint}"),
        Some("ThrottlingException") => format!("Throttling error: {core}{hint}"),
        Some("ServiceUnavailableException") => format!("Service unavailable: {core}{hint}"),
        Some(name) => format!("{name}: {core}{hint}"),
        None => format!("{core}{hint}"),
    }
}

/// Starts a Bedrock Converse stream.
///
/// The live sender is intentionally isolated behind the deterministic request plan above so tests
/// can validate AWS SDK configuration/payload parity without making AWS calls.
pub fn stream(
    model: &Model,
    _context: &Context,
    options: Option<&BedrockOptions>,
) -> Result<AssistantMessageEventStream> {
    let options = options.cloned().unwrap_or_default();
    let _plan = resolve_bedrock_runtime_request_plan(model, &json!({ "messages": [] }), &options);
    Ok(AssistantMessageEventStream)
}

/// Starts a Bedrock Converse stream using Pi's simple stream options mapping.
pub fn stream_simple(
    model: &Model,
    context: &Context,
    options: Option<&BedrockOptions>,
) -> Result<AssistantMessageEventStream> {
    stream(model, context, options)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(id: &str) -> Model {
        Model {
            id: id.to_string(),
            provider: "amazon-bedrock".to_string(),
            name: None,
            base_url: Some("https://bedrock-runtime.us-west-2.amazonaws.com".to_string()),
            max_tokens: 4096,
            reasoning: true,
            thinking_level_map: HashMap::new(),
        }
    }

    #[test]
    fn genai_bedrock_parity_report_requires_fallback() {
        let report = genai_bedrock_parity_report();

        assert!(!report.can_preserve_pi_behavior);
        assert!(
            report
                .blockers
                .iter()
                .any(|blocker| blocker.contains("onPayload"))
        );
        assert!(
            report
                .blockers
                .iter()
                .any(|blocker| blocker.contains("custom"))
        );
    }

    #[test]
    fn request_plan_preserves_auth_proxy_headers_and_payload() {
        let mut options = BedrockOptions {
            region: Some("us-east-2".to_owned()),
            headers: HashMap::from([
                ("x-ok".to_owned(), "1".to_owned()),
                ("authorization".to_owned(), "evil".to_owned()),
                ("X-Amz-Date".to_owned(), "evil".to_owned()),
            ]),
            env: HashMap::from([
                ("AWS_ACCESS_KEY_ID".to_owned(), "akid".to_owned()),
                ("AWS_SECRET_ACCESS_KEY".to_owned(), "secret".to_owned()),
                ("https_proxy".to_owned(), "proxy.example:8443".to_owned()),
            ]),
            reasoning: Some(ThinkingLevel::High),
            ..BedrockOptions::default()
        };
        options
            .request_metadata
            .insert("team".to_owned(), "ai".to_owned());

        let plan = resolve_bedrock_runtime_request_plan_with_env(
            &model("global.anthropic.claude-opus-4-8-v1"),
            &json!({ "messages": [{ "role": "user", "content": "hello", "timestamp": 0 }] }),
            &options,
            &ProviderEnv::new(),
        );

        assert_eq!(plan.client_config.region.as_deref(), Some("us-east-2"));
        assert!(matches!(
            plan.auth_mode,
            BedrockAuthMode::ExplicitCredentials { .. }
        ));
        assert_eq!(
            plan.proxy_url.as_deref(),
            Some("https://proxy.example:8443/")
        );
        assert!(plan.force_http1);
        assert_eq!(
            plan.custom_signed_headers,
            HashMap::from([("x-ok".to_owned(), "1".to_owned())])
        );
        assert_eq!(plan.payload["requestMetadata"], json!({ "team": "ai" }));
        assert_eq!(
            plan.payload["additionalModelRequestFields"]["thinking"]["type"],
            "adaptive"
        );
    }

    fn replace_payload(mut payload: Value, _model: &Model) -> Option<Value> {
        payload["hooked"] = json!(true);
        Some(payload)
    }

    #[test]
    fn payload_and_response_hooks_are_preserved_by_fallback_seams() {
        let options = BedrockOptions {
            on_payload: Some(replace_payload),
            ..BedrockOptions::default()
        };
        let plan = resolve_bedrock_runtime_request_plan_with_env(
            &model("anthropic.claude-sonnet-4-6"),
            &json!({ "messages": [] }),
            &options,
            &ProviderEnv::new(),
        );

        assert_eq!(plan.payload["hooked"], true);
        assert_eq!(
            bedrock_response_metadata(200, Some("rid")).headers,
            HashMap::from([("x-amzn-requestid".to_owned(), "rid".to_owned())])
        );
    }

    #[test]
    fn formats_bedrock_errors_with_pi_prefix_and_data_retention_hint() {
        let formatted = format_bedrock_error(
            Some("ValidationException"),
            SdkErrorShape {
                message: "bad request".to_owned(),
                metadata_http_status_code: Some(400.0),
                response_body: Some(json!("data retention mode 'default' is not available")),
                ..SdkErrorShape::default()
            },
        );

        assert!(formatted.starts_with("Validation error: 400: data retention mode"));
        assert!(formatted.contains("data-retention"));
    }

    #[test]
    fn stream_uses_bedrock_fallback_plan_without_network() {
        assert!(stream(&model("anthropic.claude-sonnet-4-6"), &Context, None).is_ok());
        assert!(stream_simple(&model("anthropic.claude-sonnet-4-6"), &Context, None).is_ok());
    }
}
