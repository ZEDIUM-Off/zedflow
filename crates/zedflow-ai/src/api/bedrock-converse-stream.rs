//! Amazon Bedrock Converse Stream API ported from Pi.

use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::future::BoxFuture;

use base64::Engine;
use reqwest::blocking::Client;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use url::Url;
use zedflow_core::error::Result;

use crate::utils::json_parse::parse_streaming_json_value;

use crate::utils::error_body::{
    ProviderErrorInput, ProviderHttpErrorParts, ProviderServiceError, SdkErrorShape,
    format_provider_error, normalize_provider_error,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CacheRetention {
    /// Disable explicit prompt cache points.
    None,
    /// Use Bedrock's default short-lived cache point.
    #[default]
    Short,
    /// Request the one-hour cache TTL when Bedrock supports it.
    Long,
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

/// Pi-compatible assistant message event stream.
pub type AssistantMessageEventStream = crate::types::AssistantMessageEventStream;

/// Callback that can inspect or replace a Bedrock ConverseStream payload before send.
pub type BedrockPayloadHook = Arc<
    dyn Fn(
            Value,
            Model,
        ) -> BoxFuture<
            'static,
            std::result::Result<Option<Value>, crate::types::ProviderHookError>,
        > + Send
        + Sync,
>;

/// Callback invoked after Bedrock returns SDK HTTP metadata.
pub type BedrockResponseHook = Arc<
    dyn Fn(
            BedrockResponseMetadata,
            Model,
        ) -> BoxFuture<'static, std::result::Result<(), crate::types::ProviderHookError>>
        + Send
        + Sync,
>;

/// Options specific to Pi's Bedrock Converse Stream implementation.
#[derive(Clone, Default)]
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
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BedrockGenaiParityReport {
    /// Whether genai can preserve all Pi-observable Bedrock behavior.
    pub(crate) can_preserve_pi_behavior: bool,
    /// Missing behaviors that force the Bedrock fallback path.
    pub(crate) blockers: Vec<&'static str>,
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
#[cfg(test)]
#[must_use]
pub(crate) fn genai_bedrock_parity_report() -> BedrockGenaiParityReport {
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

fn image_block(item: &Value) -> Option<Value> {
    let format = match item.get("mimeType").and_then(Value::as_str)? {
        "image/jpeg" | "image/jpg" => "jpeg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => return None,
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(item.get("data").and_then(Value::as_str)?)
        .ok()?;
    Some(json!({ "image": { "format": format, "source": { "bytes": bytes } } }))
}

fn tool_result_content(message: &Value) -> Vec<Value> {
    let mut content = Vec::new();
    for item in message
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
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
            Some("image") => {
                if let Some(block) = image_block(item) {
                    content.push(block);
                }
            }
            _ => {}
        }
    }
    if content.is_empty() {
        content.push(json!({ "text": "<empty>" }));
    }
    content
}

fn tool_result_block(message: &Value) -> Value {
    json!({
        "toolResult": {
            "toolUseId": message.get("toolCallId").and_then(Value::as_str).unwrap_or_default(),
            "content": tool_result_content(message),
            "status": if message.get("isError").and_then(Value::as_bool).unwrap_or(false) { "error" } else { "success" },
        }
    })
}

fn convert_message(message: &Value, model: &Model) -> Option<Value> {
    match message.get("role")?.as_str()? {
        "user" => {
            let mut content = Vec::new();
            match message.get("content") {
                Some(Value::String(text)) => content.push(text_block(text, true)?),
                Some(Value::Array(items)) => {
                    for item in items {
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
                            Some("image") => {
                                if let Some(block) = image_block(item) {
                                    content.push(block);
                                }
                            }
                            _ => {}
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
                    Some("thinking") => {
                        let Some(thinking) = item
                            .get("thinking")
                            .and_then(Value::as_str)
                            .map(sanitize_surrogates)
                            .filter(|thinking| !thinking.trim().is_empty())
                        else {
                            continue;
                        };
                        let signature = item
                            .get("thinkingSignature")
                            .and_then(Value::as_str)
                            .filter(|signature| !signature.trim().is_empty());
                        if is_anthropic_claude_model(model) {
                            if let Some(signature) = signature {
                                content.push(json!({
                                    "reasoningContent": {
                                        "reasoningText": { "text": thinking, "signature": signature }
                                    }
                                }));
                            } else {
                                content.push(json!({ "text": thinking }));
                            }
                        } else {
                            content.push(json!({
                                "reasoningContent": { "reasoningText": { "text": thinking } }
                            }));
                        }
                    }
                    _ => {}
                }
            }
            (!content.is_empty()).then(|| json!({ "role": "assistant", "content": content }))
        }
        "toolResult" => Some(json!({
            "role": "user",
            "content": [tool_result_block(message)]
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
    let input = context
        .get("messages")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let mut messages = Vec::new();
    let mut index = 0;
    while let Some(message) = input.get(index) {
        if message.get("role").and_then(Value::as_str) == Some("toolResult") {
            let mut content = vec![tool_result_block(message)];
            index += 1;
            while let Some(next) = input.get(index) {
                if next.get("role").and_then(Value::as_str) != Some("toolResult") {
                    break;
                }
                content.push(tool_result_block(next));
                index += 1;
            }
            messages.push(json!({ "role": "user", "content": content }));
            continue;
        }
        if let Some(converted) = convert_message(message, model) {
            messages.push(converted);
        }
        index += 1;
    }
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
    _options: &BedrockOptions,
) -> Value {
    build_bedrock_converse_payload(model, context, _options)
}

async fn apply_bedrock_payload_hook(
    plan: &mut BedrockRuntimeRequestPlan,
    options: &BedrockOptions,
    model: &Model,
) -> std::result::Result<(), crate::types::ProviderHookError> {
    if let Some(hook) = options.on_payload.as_ref()
        && let Some(payload) = hook(plan.payload.clone(), model.clone()).await?
    {
        plan.payload = payload;
    }
    Ok(())
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
pub async fn invoke_bedrock_on_response(
    options: &BedrockOptions,
    metadata: BedrockResponseMetadata,
    model: &Model,
) -> std::result::Result<(), crate::types::ProviderHookError> {
    if let Some(hook) = options.on_response.as_ref() {
        hook(metadata, model.clone()).await?;
    }
    Ok(())
}

fn build_additional_model_request_fields(model: &Model, options: &BedrockOptions) -> Option<Value> {
    let reasoning = options.reasoning?;
    if !model.reasoning || !is_anthropic_claude_model(model) {
        return None;
    }
    let display = (!is_govcloud_target(model, options)).then_some(match options.thinking_display {
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

/// Returns true when this process has enough local configuration for the narrow Bedrock sender.
#[must_use]
pub fn has_bedrock_live_capability() -> bool {
    let ambient = std::env::vars().collect::<ProviderEnv>();
    has_bedrock_live_capability_with_env(&ambient)
}

/// Returns true when an explicit environment map can authorize a Bedrock live request.
#[must_use]
pub fn has_bedrock_live_capability_with_env(ambient: &ProviderEnv) -> bool {
    provider_env_value("AWS_BEARER_TOKEN_BEDROCK", &ProviderEnv::new(), ambient).is_some()
        || provider_env_value("AWS_BEDROCK_SKIP_AUTH", &ProviderEnv::new(), ambient).as_deref()
            == Some("1")
        || (provider_env_value("AWS_ACCESS_KEY_ID", &ProviderEnv::new(), ambient).is_some()
            && provider_env_value("AWS_SECRET_ACCESS_KEY", &ProviderEnv::new(), ambient).is_some())
        || provider_env_value("AWS_PROFILE", &ProviderEnv::new(), ambient)
            .and_then(|profile| load_aws_profile(&profile))
            .and_then(|profile| profile.access_key_id.zip(profile.secret_access_key))
            .is_some()
}

/// Starts a Bedrock Converse stream from an already converted Pi context value.
pub fn stream_with_context_value(
    model: &Model,
    context: &Value,
    options: Option<&BedrockOptions>,
) -> Result<AssistantMessageEventStream> {
    let stream = AssistantMessageEventStream::new();
    let worker_stream = stream.clone();
    let model = model.clone();
    let context = context.clone();
    let options = options.cloned().unwrap_or_default();
    crate::utils::runtime::spawn_worker(async move {
        run_bedrock_live_worker(worker_stream, model, context, options).await;
    });
    Ok(stream)
}

/// Starts a Bedrock Converse stream.
pub fn stream(
    model: &Model,
    _context: &Context,
    options: Option<&BedrockOptions>,
) -> Result<AssistantMessageEventStream> {
    stream_with_context_value(model, &json!({ "messages": [] }), options)
}

/// Starts a Bedrock Converse stream using Pi's simple stream options mapping.
pub fn stream_simple(
    model: &Model,
    context: &Context,
    options: Option<&BedrockOptions>,
) -> Result<AssistantMessageEventStream> {
    stream(model, context, options)
}

async fn run_bedrock_live_worker(
    stream: AssistantMessageEventStream,
    model: Model,
    context: Value,
    options: BedrockOptions,
) {
    let mut output = empty_bedrock_message(&model);
    let mut plan = resolve_bedrock_runtime_request_plan(&model, &context, &options);
    if let Err(error) = apply_bedrock_payload_hook(&mut plan, &options, &model).await {
        output.stop_reason = crate::types::StopReason::Error;
        output.error_message = Some(error.to_string());
        stream.push(crate::types::AssistantMessageEvent::Error {
            reason: crate::types::ErrorStopReason::Error,
            error: output,
        });
        return;
    }
    match execute_bedrock_converse_stream(&model, &plan, &options).await {
        Ok(events) => {
            if let Err(error) = process_bedrock_converse_stream_events(&stream, &model, events) {
                output.stop_reason = crate::types::StopReason::Error;
                output.error_message = Some(error);
                stream.push(crate::types::AssistantMessageEvent::Error {
                    reason: crate::types::ErrorStopReason::Error,
                    error: output,
                });
            }
        }
        Err(error) => {
            output.stop_reason = crate::types::StopReason::Error;
            output.error_message = Some(error.to_string());
            stream.push(crate::types::AssistantMessageEvent::Error {
                reason: crate::types::ErrorStopReason::Error,
                error: output,
            });
        }
    }
}

async fn execute_bedrock_converse_stream(
    model: &Model,
    plan: &BedrockRuntimeRequestPlan,
    options: &BedrockOptions,
) -> std::result::Result<Vec<Value>, ProviderServiceError> {
    let endpoint = bedrock_runtime_endpoint(model, plan);
    let region = bedrock_signing_region(plan, options).unwrap_or_else(|| "us-east-1".to_owned());
    let url = format!(
        "{}/model/{}/converse-stream",
        endpoint.trim_end_matches('/'),
        percent_encode_path_segment(&model.id)
    );
    let mut payload = plan.payload.clone();
    if let Some(object) = payload.as_object_mut() {
        object.remove("modelId");
    }
    let body = serde_json::to_vec(&payload).map_err(|error| {
        ProviderServiceError::with_source(ProviderHttpErrorParts::new(error.to_string()), error)
    })?;
    let headers = bedrock_request_headers(&url, &region, &body, plan, options)
        .map_err(bedrock_service_message)?;
    let response = Client::builder()
        .build()
        .map_err(|error| {
            ProviderServiceError::with_source(ProviderHttpErrorParts::new(error.to_string()), error)
        })?
        .post(url)
        .headers(headers)
        .body(body)
        .send()
        .map_err(|error| {
            ProviderServiceError::with_source(ProviderHttpErrorParts::new(error.to_string()), error)
        })?;

    let status = response.status().as_u16();
    let is_success = response.status().is_success();
    let response_headers = response
        .headers()
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_owned(),
                String::from_utf8_lossy(value.as_bytes()).into_owned(),
            )
        })
        .collect::<ProviderHeaders>();
    let request_id = response_headers.get("x-amzn-requestid").cloned();
    invoke_bedrock_on_response(
        options,
        bedrock_response_metadata(status, request_id.as_deref()),
        model,
    )
    .await
    .map_err(|error| {
        ProviderServiceError::with_source(ProviderHttpErrorParts::new(error.to_string()), error)
    })?;
    if !is_success {
        let bytes = response.bytes().map_err(|error| {
            ProviderServiceError::with_source(
                ProviderHttpErrorParts::new(error.to_string())
                    .with_status(status)
                    .with_headers(response_headers.clone()),
                error,
            )
        })?;
        let body = String::from_utf8_lossy(bytes.as_ref()).into_owned();
        return Err(bedrock_service_error(status, &body, response_headers));
    }
    let bytes = response.bytes().map_err(|error| {
        ProviderServiceError::with_source(
            ProviderHttpErrorParts::new(error.to_string())
                .with_status(status)
                .with_headers(response_headers),
            error,
        )
    })?;
    parse_aws_event_stream(bytes.as_ref()).map_err(bedrock_service_message)
}

fn bedrock_service_message(message: String) -> ProviderServiceError {
    ProviderServiceError::new(ProviderHttpErrorParts::new(message))
}

/// Normalizes a Bedrock HTTP service failure without exposing transport dependency types.
#[must_use]
pub fn bedrock_service_error(
    status: u16,
    body: &str,
    metadata: ProviderHeaders,
) -> ProviderServiceError {
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| format!("{status} status code (response body preserved)"));
    ProviderServiceError::new(
        ProviderHttpErrorParts::new(message)
            .with_status(status)
            .with_body(body)
            .with_headers(metadata),
    )
}

fn bedrock_runtime_endpoint(model: &Model, plan: &BedrockRuntimeRequestPlan) -> String {
    plan.client_config
        .endpoint
        .clone()
        .or_else(|| model.base_url.clone())
        .or_else(|| {
            plan.client_config
                .region
                .as_ref()
                .map(|region| format!("https://bedrock-runtime.{region}.amazonaws.com"))
        })
        .unwrap_or_else(|| "https://bedrock-runtime.us-east-1.amazonaws.com".to_owned())
}

fn bedrock_signing_region(
    plan: &BedrockRuntimeRequestPlan,
    options: &BedrockOptions,
) -> Option<String> {
    plan.client_config
        .region
        .clone()
        .or_else(|| provider_env_value("AWS_REGION", &options.env, &ProviderEnv::new()))
        .or_else(|| provider_env_value("AWS_DEFAULT_REGION", &options.env, &ProviderEnv::new()))
        .or_else(|| {
            plan.client_config
                .profile
                .as_deref()
                .and_then(load_aws_profile)
                .and_then(|profile| profile.region)
        })
}

fn bedrock_request_headers(
    url: &str,
    region: &str,
    body: &[u8],
    plan: &BedrockRuntimeRequestPlan,
    options: &BedrockOptions,
) -> std::result::Result<HeaderMap, String> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        HeaderName::from_static("accept"),
        HeaderValue::from_static("application/vnd.amazon.eventstream"),
    );
    for (name, value) in &plan.custom_signed_headers {
        headers.insert(
            HeaderName::from_bytes(name.as_bytes()).map_err(|error| error.to_string())?,
            HeaderValue::from_str(value).map_err(|error| error.to_string())?,
        );
    }

    match &plan.auth_mode {
        BedrockAuthMode::BearerToken(token) => {
            headers.insert(
                HeaderName::from_static("authorization"),
                HeaderValue::from_str(&format!("Bearer {token}"))
                    .map_err(|error| error.to_string())?,
            );
        }
        BedrockAuthMode::ExplicitCredentials {
            access_key_id,
            secret_access_key,
            session_token,
        } => sign_bedrock_request(
            &mut headers,
            url,
            region,
            body,
            access_key_id,
            secret_access_key,
            session_token.as_deref(),
        )?,
        BedrockAuthMode::SkipAuthDummyCredentials => sign_bedrock_request(
            &mut headers,
            url,
            region,
            body,
            "dummy-access-key",
            "dummy-secret-key",
            None,
        )?,
        BedrockAuthMode::DefaultChain => {
            let profile =
                plan.client_config.profile.clone().or_else(|| {
                    provider_env_value("AWS_PROFILE", &options.env, &ProviderEnv::new())
                });
            let credentials = profile
                .as_deref()
                .and_then(load_aws_profile)
                .and_then(|profile| {
                    Some((
                        profile.access_key_id?,
                        profile.secret_access_key?,
                        profile.session_token,
                    ))
                });
            let Some((access_key_id, secret_access_key, session_token)) = credentials else {
                return Err("missing AWS Bedrock live capability: set AWS_BEARER_TOKEN_BEDROCK, AWS access key credentials, AWS_BEDROCK_SKIP_AUTH=1, or a static AWS_PROFILE".to_owned());
            };
            sign_bedrock_request(
                &mut headers,
                url,
                region,
                body,
                &access_key_id,
                &secret_access_key,
                session_token.as_deref(),
            )?;
        }
    }
    Ok(headers)
}

fn sign_bedrock_request(
    headers: &mut HeaderMap,
    url: &str,
    region: &str,
    body: &[u8],
    access_key_id: &str,
    secret_access_key: &str,
    session_token: Option<&str>,
) -> std::result::Result<(), String> {
    let parsed = Url::parse(url).map_err(|error| error.to_string())?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "missing Bedrock host".to_owned())?;
    let path = if parsed.path().is_empty() {
        "/"
    } else {
        parsed.path()
    };
    let now = aws_timestamp();
    let date = &now[..8];
    headers.insert(
        HeaderName::from_static("host"),
        HeaderValue::from_str(host).map_err(|error| error.to_string())?,
    );
    headers.insert(
        HeaderName::from_static("x-amz-date"),
        HeaderValue::from_str(&now).map_err(|error| error.to_string())?,
    );
    if let Some(session_token) = session_token {
        headers.insert(
            HeaderName::from_static("x-amz-security-token"),
            HeaderValue::from_str(session_token).map_err(|error| error.to_string())?,
        );
    }

    let payload_hash = hex_sha256(body);
    let mut signed = signed_header_values(headers)?;
    signed.sort_by(|left, right| left.0.cmp(&right.0));
    let canonical_headers = signed
        .iter()
        .map(|(name, value)| format!("{name}:{value}\n"))
        .collect::<String>();
    let signed_headers = signed
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>()
        .join(";");
    let canonical_request =
        format!("POST\n{path}\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}");
    let credential_scope = format!("{date}/{region}/bedrock/aws4_request");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{now}\n{credential_scope}\n{}",
        hex_sha256(canonical_request.as_bytes())
    );
    let signing_key = aws_signing_key(secret_access_key, date, region, "bedrock");
    let signature = hex(&hmac_sha256(&signing_key, string_to_sign.as_bytes()));
    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={access_key_id}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}"
    );
    headers.insert(
        HeaderName::from_static("authorization"),
        HeaderValue::from_str(&authorization).map_err(|error| error.to_string())?,
    );
    Ok(())
}

fn signed_header_values(headers: &HeaderMap) -> std::result::Result<Vec<(String, String)>, String> {
    headers
        .iter()
        .map(|(name, value)| {
            Ok((
                name.as_str().to_ascii_lowercase(),
                value
                    .to_str()
                    .map_err(|error| error.to_string())?
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
            ))
        })
        .collect()
}

fn aws_timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let days = seconds / 86_400;
    let day_seconds = seconds % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}Z",
        day_seconds / 3600,
        (day_seconds % 3600) / 60,
        day_seconds % 60
    )
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let days = days_since_epoch + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe as i32 + (era as i32) * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += (month <= 2) as i32;
    (year, month as u32, day as u32)
}

fn aws_signing_key(secret: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
    let date_key = hmac_sha256(format!("AWS4{secret}").as_bytes(), date.as_bytes());
    let region_key = hmac_sha256(&date_key, region.as_bytes());
    let service_key = hmac_sha256(&region_key, service.as_bytes());
    hmac_sha256(&service_key, b"aws4_request")
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    const BLOCK_SIZE: usize = 64;
    let mut key_block = if key.len() > BLOCK_SIZE {
        Sha256::digest(key).to_vec()
    } else {
        key.to_vec()
    };
    key_block.resize(BLOCK_SIZE, 0);
    let mut outer = vec![0x5c; BLOCK_SIZE];
    let mut inner = vec![0x36; BLOCK_SIZE];
    for index in 0..BLOCK_SIZE {
        outer[index] ^= key_block[index];
        inner[index] ^= key_block[index];
    }
    let mut inner_hash = Sha256::new();
    inner_hash.update(&inner);
    inner_hash.update(data);
    let inner_result = inner_hash.finalize();
    let mut outer_hash = Sha256::new();
    outer_hash.update(&outer);
    outer_hash.update(inner_result);
    outer_hash.finalize().to_vec()
}

fn hex_sha256(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn percent_encode_path_segment(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[derive(Default)]
struct AwsProfile {
    access_key_id: Option<String>,
    secret_access_key: Option<String>,
    session_token: Option<String>,
    region: Option<String>,
}

fn load_aws_profile(profile: &str) -> Option<AwsProfile> {
    let mut result = AwsProfile::default();
    merge_profile_file(&mut result, profile, aws_credentials_file()?);
    if let Some(config) = aws_config_file() {
        merge_profile_file(&mut result, &format!("profile {profile}"), config);
        merge_profile_file(&mut result, profile, aws_config_file()?);
    }
    (result.access_key_id.is_some() || result.region.is_some()).then_some(result)
}

fn aws_credentials_file() -> Option<String> {
    std::env::var("AWS_SHARED_CREDENTIALS_FILE")
        .ok()
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|home| format!("{home}/.aws/credentials"))
        })
}

fn aws_config_file() -> Option<String> {
    std::env::var("AWS_CONFIG_FILE").ok().or_else(|| {
        std::env::var("HOME")
            .ok()
            .map(|home| format!("{home}/.aws/config"))
    })
}

fn merge_profile_file(target: &mut AwsProfile, profile: &str, path: String) {
    let Ok(contents) = fs::read_to_string(path) else {
        return;
    };
    let mut in_profile = false;
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_profile = &line[1..line.len() - 1] == profile;
            continue;
        }
        if !in_profile {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().to_owned();
        match key.trim() {
            "aws_access_key_id" if target.access_key_id.is_none() => {
                target.access_key_id = Some(value)
            }
            "aws_secret_access_key" if target.secret_access_key.is_none() => {
                target.secret_access_key = Some(value);
            }
            "aws_session_token" if target.session_token.is_none() => {
                target.session_token = Some(value)
            }
            "region" if target.region.is_none() => target.region = Some(value),
            _ => {}
        }
    }
}

fn parse_aws_event_stream(bytes: &[u8]) -> std::result::Result<Vec<Value>, String> {
    let mut values = Vec::new();
    let mut offset = 0usize;
    while offset < bytes.len() {
        if bytes.len() - offset < 16 {
            return Err("truncated AWS event stream frame".to_owned());
        }
        let total_len = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let headers_len =
            u32::from_be_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        if total_len < 16 || offset + total_len > bytes.len() || headers_len > total_len - 16 {
            return Err("invalid AWS event stream frame".to_owned());
        }
        let headers_start = offset + 12;
        let payload_start = headers_start + headers_len;
        let payload_end = offset + total_len - 4;
        let headers = parse_aws_event_headers(&bytes[headers_start..payload_start])?;
        let payload = &bytes[payload_start..payload_end];
        let message_type = headers.get(":message-type").map(String::as_str);
        if message_type == Some("exception") {
            let message = serde_json::from_slice::<Value>(payload)
                .ok()
                .and_then(|value| {
                    value
                        .get("message")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .unwrap_or_else(|| String::from_utf8_lossy(payload).into_owned());
            let name = headers.get(":exception-type").map(String::as_str);
            return Err(format_bedrock_error(
                name,
                SdkErrorShape {
                    message,
                    ..SdkErrorShape::default()
                },
            ));
        }
        if !payload.is_empty() {
            values.push(
                serde_json::from_slice(payload)
                    .map_err(|error| format!("Bedrock event JSON error: {error}"))?,
            );
        }
        offset += total_len;
    }
    Ok(values)
}

fn parse_aws_event_headers(bytes: &[u8]) -> std::result::Result<HashMap<String, String>, String> {
    let mut headers = HashMap::new();
    let mut offset = 0usize;
    while offset < bytes.len() {
        let name_len = *bytes
            .get(offset)
            .ok_or_else(|| "truncated AWS event header".to_owned())?
            as usize;
        offset += 1;
        let name_end = offset + name_len;
        let name = std::str::from_utf8(
            bytes
                .get(offset..name_end)
                .ok_or_else(|| "truncated AWS event header name".to_owned())?,
        )
        .map_err(|error| error.to_string())?
        .to_owned();
        offset = name_end;
        let value_type = *bytes
            .get(offset)
            .ok_or_else(|| "truncated AWS event header type".to_owned())?;
        offset += 1;
        match value_type {
            7 => {
                let len = u16::from_be_bytes(
                    bytes
                        .get(offset..offset + 2)
                        .ok_or_else(|| "truncated AWS event header string length".to_owned())?
                        .try_into()
                        .unwrap(),
                ) as usize;
                offset += 2;
                let value_end = offset + len;
                let value = std::str::from_utf8(
                    bytes
                        .get(offset..value_end)
                        .ok_or_else(|| "truncated AWS event header string".to_owned())?,
                )
                .map_err(|error| error.to_string())?
                .to_owned();
                offset = value_end;
                headers.insert(name, value);
            }
            _ => return Err(format!("unsupported AWS event header type {value_type}")),
        }
    }
    Ok(headers)
}

#[derive(Clone)]
enum BedrockBlock {
    Text {
        text: String,
        provider_index: usize,
    },
    Thinking {
        thinking: String,
        signature: String,
        provider_index: usize,
    },
    ToolCall {
        id: String,
        name: String,
        partial_json: String,
        provider_index: usize,
    },
}

/// Maps Bedrock ConverseStream events into Pi canonical assistant-message events.
pub fn process_bedrock_converse_stream_events(
    stream: &AssistantMessageEventStream,
    model: &Model,
    events: impl IntoIterator<Item = Value>,
) -> std::result::Result<crate::types::AssistantMessage, String> {
    let mut output = empty_bedrock_message(model);
    let mut blocks: Vec<BedrockBlock> = Vec::new();
    for event in events {
        if event.get("messageStart").is_some() {
            let role = event["messageStart"].get("role").and_then(Value::as_str);
            if role != Some("assistant") {
                return Err(
                    "Unexpected assistant message start but got user message start instead"
                        .to_owned(),
                );
            }
            stream.push(crate::types::AssistantMessageEvent::Start {
                partial: output.clone(),
            });
        } else if let Some(start) = event.get("contentBlockStart") {
            handle_bedrock_content_block_start(start, &mut blocks, &mut output, stream);
        } else if let Some(delta) = event.get("contentBlockDelta") {
            handle_bedrock_content_block_delta(delta, &mut blocks, &mut output, stream);
        } else if let Some(stop) = event.get("contentBlockStop") {
            handle_bedrock_content_block_stop(stop, &mut blocks, &mut output, stream);
        } else if let Some(stop) = event.get("messageStop") {
            output.stop_reason =
                map_bedrock_stop_reason(stop.get("stopReason").and_then(Value::as_str));
        } else if let Some(metadata) = event.get("metadata") {
            handle_bedrock_metadata(metadata, &mut output);
        } else if let Some(error) = bedrock_event_error(&event) {
            return Err(error);
        }
    }
    if output.stop_reason == crate::types::StopReason::Error
        || output.stop_reason == crate::types::StopReason::Aborted
    {
        return Err("An unknown error occurred".to_owned());
    }
    stream.push(crate::types::AssistantMessageEvent::Done {
        reason: canonical_done_reason(output.stop_reason),
        message: output.clone(),
    });
    Ok(output)
}

fn handle_bedrock_content_block_start(
    event: &Value,
    blocks: &mut Vec<BedrockBlock>,
    output: &mut crate::types::AssistantMessage,
    stream: &AssistantMessageEventStream,
) {
    let provider_index = event
        .get("contentBlockIndex")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let Some(tool_use) = event.get("start").and_then(|start| start.get("toolUse")) else {
        return;
    };
    let index = output.content.len();
    let id = tool_use
        .get("toolUseId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let name = tool_use
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    output
        .content
        .push(crate::types::AssistantContentBlock::ToolCall(
            crate::types::ToolCall {
                content_type: crate::types::ToolCallType::ToolCall,
                id: id.clone(),
                name: name.clone(),
                arguments: HashMap::new(),
                thought_signature: None,
            },
        ));
    blocks.push(BedrockBlock::ToolCall {
        id,
        name,
        partial_json: String::new(),
        provider_index,
    });
    stream.push(crate::types::AssistantMessageEvent::ToolcallStart {
        content_index: index,
        partial: output.clone(),
    });
}

fn handle_bedrock_content_block_delta(
    event: &Value,
    blocks: &mut Vec<BedrockBlock>,
    output: &mut crate::types::AssistantMessage,
    stream: &AssistantMessageEventStream,
) {
    let provider_index = event
        .get("contentBlockIndex")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let Some(delta) = event.get("delta") else {
        return;
    };
    if let Some(text) = delta.get("text").and_then(Value::as_str) {
        let index = find_or_create_text_block(provider_index, blocks, output, stream);
        if let Some(crate::types::AssistantContentBlock::Text(block)) =
            output.content.get_mut(index)
        {
            block.text.push_str(text);
        }
        if let Some(BedrockBlock::Text { text: scratch, .. }) = blocks
            .iter_mut()
            .find(|block| block.provider_index() == provider_index)
        {
            scratch.push_str(text);
        }
        stream.push(crate::types::AssistantMessageEvent::TextDelta {
            content_index: index,
            delta: text.to_owned(),
            partial: output.clone(),
        });
    } else if let Some(tool_use) = delta.get("toolUse") {
        let delta = tool_use
            .get("input")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if let Some((index, block)) = find_block_mut(provider_index, blocks)
            && let BedrockBlock::ToolCall { partial_json, .. } = block
        {
            partial_json.push_str(delta);
            let arguments = value_object_to_hashmap(parse_streaming_json_value(Some(partial_json)));
            if let Some(crate::types::AssistantContentBlock::ToolCall(tool_call)) =
                output.content.get_mut(index)
            {
                tool_call.arguments = arguments;
            }
            stream.push(crate::types::AssistantMessageEvent::ToolcallDelta {
                content_index: index,
                delta: delta.to_owned(),
                partial: output.clone(),
            });
        }
    } else if let Some(reasoning) = delta.get("reasoningContent") {
        let index = find_or_create_thinking_block(provider_index, blocks, output, stream);
        if let Some(text) = reasoning.get("text").and_then(Value::as_str) {
            if let Some(crate::types::AssistantContentBlock::Thinking(block)) =
                output.content.get_mut(index)
            {
                block.thinking.push_str(text);
            }
            if let Some(BedrockBlock::Thinking { thinking, .. }) = blocks
                .iter_mut()
                .find(|block| block.provider_index() == provider_index)
            {
                thinking.push_str(text);
            }
            stream.push(crate::types::AssistantMessageEvent::ThinkingDelta {
                content_index: index,
                delta: text.to_owned(),
                partial: output.clone(),
            });
        }
        if let Some(signature) = reasoning.get("signature").and_then(Value::as_str) {
            if let Some(crate::types::AssistantContentBlock::Thinking(block)) =
                output.content.get_mut(index)
            {
                block
                    .thinking_signature
                    .get_or_insert_with(String::new)
                    .push_str(signature);
            }
            if let Some(BedrockBlock::Thinking {
                signature: scratch, ..
            }) = blocks
                .iter_mut()
                .find(|block| block.provider_index() == provider_index)
            {
                scratch.push_str(signature);
            }
        }
    }
}

fn handle_bedrock_content_block_stop(
    event: &Value,
    blocks: &mut [BedrockBlock],
    output: &mut crate::types::AssistantMessage,
    stream: &AssistantMessageEventStream,
) {
    let provider_index = event
        .get("contentBlockIndex")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let Some((index, block)) = find_block_mut(provider_index, blocks) else {
        return;
    };
    match block {
        BedrockBlock::Text { text, .. } => {
            stream.push(crate::types::AssistantMessageEvent::TextEnd {
                content_index: index,
                content: text.clone(),
                partial: output.clone(),
            })
        }
        BedrockBlock::Thinking { thinking, .. } => {
            stream.push(crate::types::AssistantMessageEvent::ThinkingEnd {
                content_index: index,
                content: thinking.clone(),
                partial: output.clone(),
            })
        }
        BedrockBlock::ToolCall {
            id,
            name,
            partial_json,
            ..
        } => {
            let tool_call = crate::types::ToolCall {
                content_type: crate::types::ToolCallType::ToolCall,
                id: id.clone(),
                name: name.clone(),
                arguments: value_object_to_hashmap(parse_streaming_json_value(Some(partial_json))),
                thought_signature: None,
            };
            if let Some(crate::types::AssistantContentBlock::ToolCall(block)) =
                output.content.get_mut(index)
            {
                *block = tool_call.clone();
            }
            stream.push(crate::types::AssistantMessageEvent::ToolcallEnd {
                content_index: index,
                tool_call,
                partial: output.clone(),
            });
        }
    }
}

fn handle_bedrock_metadata(metadata: &Value, output: &mut crate::types::AssistantMessage) {
    let Some(usage) = metadata.get("usage") else {
        return;
    };
    output.usage.input = usage
        .get("inputTokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    output.usage.output = usage
        .get("outputTokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    output.usage.cache_read = usage
        .get("cacheReadInputTokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    output.usage.cache_write = usage
        .get("cacheWriteInputTokens")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    output.usage.total_tokens = usage
        .get("totalTokens")
        .and_then(Value::as_u64)
        .unwrap_or(output.usage.input + output.usage.output);
}

fn find_or_create_text_block(
    provider_index: usize,
    blocks: &mut Vec<BedrockBlock>,
    output: &mut crate::types::AssistantMessage,
    stream: &AssistantMessageEventStream,
) -> usize {
    if let Some((index, _)) = find_block_mut(provider_index, blocks) {
        return index;
    }
    let index = output.content.len();
    output
        .content
        .push(crate::types::AssistantContentBlock::Text(
            crate::types::TextContent {
                content_type: crate::types::TextContentType::Text,
                text: String::new(),
                text_signature: None,
            },
        ));
    blocks.push(BedrockBlock::Text {
        text: String::new(),
        provider_index,
    });
    stream.push(crate::types::AssistantMessageEvent::TextStart {
        content_index: index,
        partial: output.clone(),
    });
    index
}

fn find_or_create_thinking_block(
    provider_index: usize,
    blocks: &mut Vec<BedrockBlock>,
    output: &mut crate::types::AssistantMessage,
    stream: &AssistantMessageEventStream,
) -> usize {
    if let Some((index, _)) = find_block_mut(provider_index, blocks) {
        return index;
    }
    let index = output.content.len();
    output
        .content
        .push(crate::types::AssistantContentBlock::Thinking(
            crate::types::ThinkingContent {
                content_type: crate::types::ThinkingContentType::Thinking,
                thinking: String::new(),
                thinking_signature: Some(String::new()),
                redacted: None,
            },
        ));
    blocks.push(BedrockBlock::Thinking {
        thinking: String::new(),
        signature: String::new(),
        provider_index,
    });
    stream.push(crate::types::AssistantMessageEvent::ThinkingStart {
        content_index: index,
        partial: output.clone(),
    });
    index
}

fn find_block_mut(
    provider_index: usize,
    blocks: &mut [BedrockBlock],
) -> Option<(usize, &mut BedrockBlock)> {
    blocks
        .iter_mut()
        .enumerate()
        .find(|(_, block)| block.provider_index() == provider_index)
}

impl BedrockBlock {
    fn provider_index(&self) -> usize {
        match self {
            Self::Text { provider_index, .. }
            | Self::Thinking { provider_index, .. }
            | Self::ToolCall { provider_index, .. } => *provider_index,
        }
    }
}

fn value_object_to_hashmap(value: Value) -> HashMap<String, Value> {
    match value {
        Value::Object(map) => map.into_iter().collect(),
        _ => HashMap::new(),
    }
}

fn map_bedrock_stop_reason(reason: Option<&str>) -> crate::types::StopReason {
    match reason {
        Some("end_turn" | "stop_sequence") => crate::types::StopReason::Stop,
        Some("max_tokens" | "model_context_window_exceeded") => crate::types::StopReason::Length,
        Some("tool_use") => crate::types::StopReason::ToolUse,
        _ => crate::types::StopReason::Error,
    }
}

fn canonical_done_reason(reason: crate::types::StopReason) -> crate::types::DoneStopReason {
    match reason {
        crate::types::StopReason::Length => crate::types::DoneStopReason::Length,
        crate::types::StopReason::ToolUse => crate::types::DoneStopReason::ToolUse,
        _ => crate::types::DoneStopReason::Stop,
    }
}

fn bedrock_event_error(event: &Value) -> Option<String> {
    for (key, name) in [
        ("internalServerException", "InternalServerException"),
        ("modelStreamErrorException", "ModelStreamErrorException"),
        ("validationException", "ValidationException"),
        ("throttlingException", "ThrottlingException"),
        ("serviceUnavailableException", "ServiceUnavailableException"),
    ] {
        if let Some(shape) = event.get(key) {
            return Some(format_bedrock_error(
                Some(name),
                SdkErrorShape {
                    message: shape
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("UnknownError")
                        .to_owned(),
                    response_body: Some(shape.clone()),
                    ..SdkErrorShape::default()
                },
            ));
        }
    }
    None
}

fn empty_bedrock_message(model: &Model) -> crate::types::AssistantMessage {
    crate::types::AssistantMessage {
        role: crate::types::AssistantMessageRole::Assistant,
        content: Vec::new(),
        api: "bedrock-converse-stream".to_owned(),
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

fn unix_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
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
                .any(|blocker| blocker.contains("caller headers"))
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

    fn replace_payload(
        mut payload: Value,
        _model: Model,
    ) -> BoxFuture<'static, std::result::Result<Option<Value>, crate::types::ProviderHookError>>
    {
        payload["hooked"] = json!(true);
        Box::pin(async move { Ok(Some(payload)) })
    }

    #[test]
    fn payload_and_response_hooks_are_preserved_by_fallback_seams() {
        let options = BedrockOptions {
            on_payload: Some(Arc::new(replace_payload)),
            ..BedrockOptions::default()
        };
        let plan = resolve_bedrock_runtime_request_plan_with_env(
            &model("anthropic.claude-sonnet-4-6"),
            &json!({ "messages": [] }),
            &options,
            &ProviderEnv::new(),
        );

        let mut plan = plan;
        futures::executor::block_on(apply_bedrock_payload_hook(
            &mut plan,
            &options,
            &model("anthropic.claude-sonnet-4-6"),
        ))
        .expect("hook should succeed");
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
    fn stream_returns_canonical_event_stream_without_blocking_for_network() {
        assert!(stream(&model("anthropic.claude-sonnet-4-6"), &Context, None).is_ok());
        assert!(stream_simple(&model("anthropic.claude-sonnet-4-6"), &Context, None).is_ok());
    }

    #[test]
    fn maps_bedrock_converse_events_to_canonical_assistant_stream() {
        let stream = AssistantMessageEventStream::new();
        let output = process_bedrock_converse_stream_events(
            &stream,
            &model("anthropic.claude-sonnet-4-6"),
            [
                json!({ "messageStart": { "role": "assistant" } }),
                json!({ "contentBlockDelta": { "contentBlockIndex": 0, "delta": { "text": "hel" } } }),
                json!({ "contentBlockDelta": { "contentBlockIndex": 0, "delta": { "text": "lo" } } }),
                json!({ "contentBlockStop": { "contentBlockIndex": 0 } }),
                json!({ "metadata": { "usage": { "inputTokens": 2, "outputTokens": 3, "totalTokens": 5 } } }),
                json!({ "messageStop": { "stopReason": "end_turn" } }),
            ],
        )
        .expect("events should map");

        assert_eq!(output.usage.input, 2);
        assert_eq!(output.usage.output, 3);
        assert_eq!(output.usage.total_tokens, 5);
        assert!(matches!(
            output.content.first(),
            Some(crate::types::AssistantContentBlock::Text(text)) if text.text == "hello"
        ));
        assert_eq!(
            futures::executor::block_on(stream.result()).content,
            output.content
        );
    }

    #[test]
    fn bedrock_live_capability_requires_explicit_credentials_or_static_profile() {
        assert!(has_bedrock_live_capability_with_env(&ProviderEnv::from([
            ("AWS_BEARER_TOKEN_BEDROCK".to_owned(), "token".to_owned(),)
        ])));
        assert!(!has_bedrock_live_capability_with_env(&ProviderEnv::new()));
    }
}
