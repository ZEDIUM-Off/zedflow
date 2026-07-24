//! Amazon Bedrock Converse Stream API ported from Pi.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::future::BoxFuture;

use base64::Engine;
use reqwest::header::{HeaderName, HeaderValue};
use serde_json::{Map, Value, json};
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
    /// Request cancellation signal.
    pub signal: Option<crate::types::AbortSignal>,
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

/// Bedrock authentication mode selected for the SDK client.
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

/// Deterministic Bedrock Runtime SDK request plan.
#[derive(Debug, Clone, PartialEq)]
pub struct BedrockRuntimeRequestPlan {
    /// SDK client region/profile/endpoint resolution.
    pub client_config: BedrockClientConfig,
    /// Authentication mode for the SDK client.
    pub auth_mode: BedrockAuthMode,
    /// Pi proxy URL selected for the target endpoint, if any.
    pub proxy_url: Option<String>,
    /// Whether Pi requests HTTP/1-only handling for custom endpoints/proxies.
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

/// Resolves the Bedrock Runtime SDK request without making a network call.
#[must_use]
pub fn resolve_bedrock_runtime_request_plan(
    model: &Model,
    context: &Value,
    options: &BedrockOptions,
) -> BedrockRuntimeRequestPlan {
    let ambient = std::env::vars().collect::<ProviderEnv>();
    resolve_bedrock_runtime_request_plan_with_env(model, context, options, &ambient)
}

/// Resolves the Bedrock Runtime SDK request using an explicit ambient environment map.
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

fn build_tool_config(context: &Value, choice: Option<&BedrockToolChoice>) -> Option<Value> {
    let tools = context.get("tools")?.as_array()?;
    if tools.is_empty() || matches!(choice, Some(BedrockToolChoice::None)) {
        return None;
    }
    let tools = tools
        .iter()
        .filter_map(|tool| {
            Some(json!({
                "toolSpec": {
                    "name": tool.get("name")?.as_str()?,
                    "description": tool.get("description").and_then(Value::as_str).unwrap_or_default(),
                    "inputSchema": { "json": tool.get("parameters").cloned().unwrap_or_else(|| json!({ "type": "object" })) },
                }
            }))
        })
        .collect::<Vec<_>>();
    if tools.is_empty() {
        return None;
    }
    let tool_choice = match choice {
        Some(BedrockToolChoice::Any) => json!({ "any": {} }),
        Some(BedrockToolChoice::Tool { name }) => json!({ "tool": { "name": name } }),
        _ => json!({ "auto": {} }),
    };
    Some(json!({ "tools": tools, "toolChoice": tool_choice }))
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

    if let Some(tool_config) = build_tool_config(context, options.tool_choice.as_ref()) {
        payload["toolConfig"] = tool_config;
    }
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
        || provider_env_value("AWS_PROFILE", &ProviderEnv::new(), ambient).is_some()
}

/// Returns the canonical Bedrock Converse Stream implementation.
#[must_use]
pub fn provider_streams() -> crate::types::ProviderStreams {
    crate::types::ProviderStreams {
        stream: Arc::new(stream_registered),
        stream_simple: Arc::new(stream_simple_registered),
    }
}

#[must_use]
fn stream_registered(
    model: &crate::types::Model,
    context: &crate::types::Context,
    options: Option<&crate::types::StreamOptions>,
) -> crate::types::AssistantMessageEventStream {
    let transformed = crate::api::transform_messages::transform_context(
        context,
        model,
        Some(&|id, _, _| normalize_tool_call_id(id)),
    );
    let local_model = registered_model(model);
    let local_options = registered_options(model, options);
    let context = serde_json::to_value(transformed).unwrap_or_else(|_| json!({ "messages": [] }));
    stream_with_context_value_and_cost(
        &local_model,
        &context,
        Some(&local_options),
        model.cost.clone(),
    )
    .unwrap_or_else(|error| crate::models::terminal_stream_error(model, error.to_string()))
}

#[must_use]
fn stream_simple_registered(
    model: &crate::types::Model,
    context: &crate::types::Context,
    options: Option<&crate::types::SimpleStreamOptions>,
) -> crate::types::AssistantMessageEventStream {
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

fn registered_model(model: &crate::types::Model) -> Model {
    Model {
        id: model.id.clone(),
        provider: model.provider.clone(),
        name: Some(model.name.clone()),
        base_url: (!model.base_url.is_empty()).then(|| model.base_url.clone()),
        max_tokens: u32::try_from(model.max_tokens).unwrap_or(u32::MAX),
        reasoning: model.reasoning,
        thinking_level_map: HashMap::new(),
    }
}

fn registered_options(
    model: &crate::types::Model,
    options: Option<&crate::types::StreamOptions>,
) -> BedrockOptions {
    let options = options.cloned().unwrap_or_default();
    let canonical_model = model.clone();
    let payload_model = canonical_model.clone();
    BedrockOptions {
        temperature: options.temperature,
        max_tokens: options.max_tokens,
        signal: options.signal,
        cache_retention: options.cache_retention.map(|value| match value {
            crate::types::CacheRetention::None => CacheRetention::None,
            crate::types::CacheRetention::Short => CacheRetention::Short,
            crate::types::CacheRetention::Long => CacheRetention::Long,
        }),
        headers: options
            .headers
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(key, value)| value.map(|value| (key, value)))
            .collect(),
        env: options.env.unwrap_or_default(),
        region: options
            .extra
            .get("region")
            .and_then(Value::as_str)
            .map(str::to_owned),
        profile: options
            .extra
            .get("profile")
            .and_then(Value::as_str)
            .map(str::to_owned),
        tool_choice: bedrock_tool_choice(options.extra.get("toolChoice")),
        reasoning: options
            .extra
            .get("reasoning")
            .and_then(Value::as_str)
            .and_then(parse_thinking_level),
        thinking_budgets: HashMap::new(),
        interleaved_thinking: options
            .extra
            .get("interleavedThinking")
            .and_then(Value::as_bool),
        thinking_display: options
            .extra
            .get("thinkingDisplay")
            .and_then(Value::as_str)
            .and_then(|value| match value {
                "summarized" => Some(BedrockThinkingDisplay::Summarized),
                "omitted" => Some(BedrockThinkingDisplay::Omitted),
                _ => None,
            }),
        request_metadata: options
            .extra
            .get("requestMetadata")
            .and_then(Value::as_object)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|(key, value)| {
                        value.as_str().map(|value| (key.clone(), value.to_owned()))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        bearer_token: options
            .extra
            .get("bearerToken")
            .and_then(Value::as_str)
            .map(str::to_owned),
        on_payload: options.on_payload.map(|hook| {
            Arc::new(move |payload, _| hook(payload, payload_model.clone())) as BedrockPayloadHook
        }),
        on_response: options.on_response.map(|hook| {
            Arc::new(move |response: BedrockResponseMetadata, _: Model| {
                hook(
                    crate::types::ProviderResponse {
                        status: response.status,
                        headers: response.headers,
                    },
                    canonical_model.clone(),
                )
            }) as BedrockResponseHook
        }),
    }
}

fn parse_thinking_level(value: &str) -> Option<ThinkingLevel> {
    match value {
        "minimal" => Some(ThinkingLevel::Minimal),
        "low" => Some(ThinkingLevel::Low),
        "medium" => Some(ThinkingLevel::Medium),
        "high" => Some(ThinkingLevel::High),
        "xhigh" => Some(ThinkingLevel::XHigh),
        _ => None,
    }
}

fn bedrock_tool_choice(value: Option<&Value>) -> Option<BedrockToolChoice> {
    match value {
        Some(Value::String(value)) if value == "auto" => Some(BedrockToolChoice::Auto),
        Some(Value::String(value)) if value == "any" => Some(BedrockToolChoice::Any),
        Some(Value::String(value)) if value == "none" => Some(BedrockToolChoice::None),
        Some(Value::Object(value)) => {
            value
                .get("name")
                .and_then(Value::as_str)
                .map(|name| BedrockToolChoice::Tool {
                    name: name.to_owned(),
                })
        }
        _ => None,
    }
}

fn normalize_tool_call_id(id: &str) -> String {
    id.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .take(64)
        .collect()
}

/// Starts a Bedrock Converse stream from an already converted Pi context value.
pub fn stream_with_context_value(
    model: &Model,
    context: &Value,
    options: Option<&BedrockOptions>,
) -> Result<AssistantMessageEventStream> {
    stream_with_context_value_and_cost(model, context, options, crate::types::ModelCost::default())
}

fn stream_with_context_value_and_cost(
    model: &Model,
    context: &Value,
    options: Option<&BedrockOptions>,
    cost: crate::types::ModelCost,
) -> Result<AssistantMessageEventStream> {
    let stream = AssistantMessageEventStream::new();
    let worker_stream = stream.clone();
    let model = model.clone();
    let context = context.clone();
    let options = options.cloned().unwrap_or_default();
    let identity = crate::utils::runtime::StreamIdentity::new(
        "bedrock-converse-stream",
        model.provider.clone(),
        model.id.clone(),
    );
    crate::utils::runtime::spawn_stream_worker(stream.clone(), identity, async move {
        run_bedrock_live_worker(worker_stream, model, context, options, cost).await;
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

fn json_document(value: &Value) -> aws_smithy_types::Document {
    use aws_smithy_types::{Document, Number};
    match value {
        Value::Null => Document::Null,
        Value::Bool(value) => Document::Bool(*value),
        Value::String(value) => Document::String(value.clone()),
        Value::Number(value) => {
            if let Some(value) = value.as_u64() {
                Document::Number(Number::PosInt(value))
            } else if let Some(value) = value.as_i64() {
                Document::Number(Number::NegInt(value))
            } else {
                Document::Number(Number::Float(value.as_f64().unwrap_or_default()))
            }
        }
        Value::Array(values) => Document::Array(values.iter().map(json_document).collect()),
        Value::Object(values) => Document::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), json_document(value)))
                .collect(),
        ),
    }
}

fn sdk_image(
    value: &Value,
) -> std::result::Result<aws_sdk_bedrockruntime::types::ImageBlock, String> {
    use aws_sdk_bedrockruntime::types::{ImageBlock, ImageFormat, ImageSource};
    let format = value
        .get("format")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing image format".to_owned())?;
    let format = match format {
        "jpeg" => ImageFormat::Jpeg,
        "png" => ImageFormat::Png,
        "gif" => ImageFormat::Gif,
        "webp" => ImageFormat::Webp,
        other => return Err(format!("unsupported image format {other}")),
    };
    let bytes = value
        .pointer("/source/bytes")
        .and_then(Value::as_array)
        .ok_or_else(|| "missing image bytes".to_owned())?
        .iter()
        .filter_map(Value::as_u64)
        .map(|value| value as u8)
        .collect::<Vec<_>>();
    ImageBlock::builder()
        .format(format)
        .source(ImageSource::Bytes(aws_smithy_types::Blob::new(bytes)))
        .build()
        .map_err(|error| error.to_string())
}

fn sdk_cache_point(
    value: &Value,
) -> std::result::Result<aws_sdk_bedrockruntime::types::CachePointBlock, String> {
    use aws_sdk_bedrockruntime::types::{CachePointBlock, CachePointType, CacheTtl};
    let mut builder = CachePointBlock::builder().r#type(CachePointType::Default);
    if value.get("ttl").and_then(Value::as_str) == Some("ONE_HOUR") {
        builder = builder.ttl(CacheTtl::OneHour);
    }
    builder.build().map_err(|error| error.to_string())
}

fn sdk_content(
    value: &Value,
) -> std::result::Result<aws_sdk_bedrockruntime::types::ContentBlock, String> {
    use aws_sdk_bedrockruntime::types::{
        ContentBlock, ReasoningContentBlock, ReasoningTextBlock, ToolResultBlock,
        ToolResultContentBlock, ToolResultStatus, ToolUseBlock,
    };
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        return Ok(ContentBlock::Text(text.to_owned()));
    }
    if let Some(image) = value.get("image") {
        return Ok(ContentBlock::Image(sdk_image(image)?));
    }
    if let Some(cache) = value.get("cachePoint") {
        return Ok(ContentBlock::CachePoint(sdk_cache_point(cache)?));
    }
    if let Some(tool) = value.get("toolUse") {
        return Ok(ContentBlock::ToolUse(
            ToolUseBlock::builder()
                .tool_use_id(
                    tool.get("toolUseId")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                )
                .name(tool.get("name").and_then(Value::as_str).unwrap_or_default())
                .input(json_document(tool.get("input").unwrap_or(&Value::Null)))
                .build()
                .map_err(|error| error.to_string())?,
        ));
    }
    if let Some(result) = value.get("toolResult") {
        let mut content = Vec::new();
        for item in result
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(text) = item.get("text").and_then(Value::as_str) {
                content.push(ToolResultContentBlock::Text(text.to_owned()));
            } else if let Some(image) = item.get("image") {
                content.push(ToolResultContentBlock::Image(sdk_image(image)?));
            }
        }
        let status = if result.get("status").and_then(Value::as_str) == Some("error") {
            ToolResultStatus::Error
        } else {
            ToolResultStatus::Success
        };
        return Ok(ContentBlock::ToolResult(
            ToolResultBlock::builder()
                .tool_use_id(
                    result
                        .get("toolUseId")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                )
                .set_content(Some(content))
                .status(status)
                .build()
                .map_err(|error| error.to_string())?,
        ));
    }
    if let Some(reasoning) = value.pointer("/reasoningContent/reasoningText") {
        let mut builder = ReasoningTextBlock::builder().text(
            reasoning
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        );
        if let Some(signature) = reasoning.get("signature").and_then(Value::as_str) {
            builder = builder.signature(signature);
        }
        return Ok(ContentBlock::ReasoningContent(
            ReasoningContentBlock::ReasoningText(
                builder.build().map_err(|error| error.to_string())?,
            ),
        ));
    }
    Err("unsupported Bedrock content block".to_owned())
}

fn sdk_messages(
    payload: &Value,
) -> std::result::Result<Vec<aws_sdk_bedrockruntime::types::Message>, String> {
    use aws_sdk_bedrockruntime::types::{ConversationRole, Message};
    payload
        .get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|message| {
            let role = if message.get("role").and_then(Value::as_str) == Some("assistant") {
                ConversationRole::Assistant
            } else {
                ConversationRole::User
            };
            let content = message
                .get("content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(sdk_content)
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Message::builder()
                .role(role)
                .set_content(Some(content))
                .build()
                .map_err(|error| error.to_string())
        })
        .collect()
}

fn sdk_system(
    payload: &Value,
) -> std::result::Result<Vec<aws_sdk_bedrockruntime::types::SystemContentBlock>, String> {
    use aws_sdk_bedrockruntime::types::SystemContentBlock;
    payload
        .get("system")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|value| {
            if let Some(text) = value.get("text").and_then(Value::as_str) {
                Ok(SystemContentBlock::Text(text.to_owned()))
            } else if let Some(cache) = value.get("cachePoint") {
                Ok(SystemContentBlock::CachePoint(sdk_cache_point(cache)?))
            } else {
                Err("unsupported Bedrock system block".to_owned())
            }
        })
        .collect()
}

fn sdk_tool_config(
    payload: &Value,
) -> std::result::Result<Option<aws_sdk_bedrockruntime::types::ToolConfiguration>, String> {
    use aws_sdk_bedrockruntime::types::{
        AnyToolChoice, AutoToolChoice, SpecificToolChoice, Tool, ToolChoice, ToolConfiguration,
        ToolInputSchema, ToolSpecification,
    };
    let Some(config) = payload.get("toolConfig") else {
        return Ok(None);
    };
    let tools = config
        .get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|tool| tool.get("toolSpec"))
        .map(|spec| {
            let mut builder = ToolSpecification::builder()
                .name(spec.get("name").and_then(Value::as_str).unwrap_or_default());
            if let Some(description) = spec.get("description").and_then(Value::as_str) {
                builder = builder.description(description);
            }
            if let Some(schema) = spec.pointer("/inputSchema/json") {
                builder = builder.input_schema(ToolInputSchema::Json(json_document(schema)));
            }
            builder
                .build()
                .map(Tool::ToolSpec)
                .map_err(|error| error.to_string())
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let choice = if config.pointer("/toolChoice/any").is_some() {
        Some(ToolChoice::Any(AnyToolChoice::builder().build()))
    } else if let Some(name) = config
        .pointer("/toolChoice/tool/name")
        .and_then(Value::as_str)
    {
        Some(ToolChoice::Tool(
            SpecificToolChoice::builder()
                .name(name)
                .build()
                .map_err(|error| error.to_string())?,
        ))
    } else {
        Some(ToolChoice::Auto(AutoToolChoice::builder().build()))
    };
    ToolConfiguration::builder()
        .set_tools(Some(tools))
        .set_tool_choice(choice)
        .build()
        .map(Some)
        .map_err(|error| error.to_string())
}

fn sdk_event_value(
    event: aws_sdk_bedrockruntime::types::ConverseStreamOutput,
) -> std::result::Result<Value, String> {
    use aws_sdk_bedrockruntime::types::{
        ContentBlockDelta, ContentBlockStart, ConverseStreamOutput, ReasoningContentBlockDelta,
    };
    match event {
        ConverseStreamOutput::MessageStart(event) => {
            Ok(json!({ "messageStart": { "role": event.role().as_str() } }))
        }
        ConverseStreamOutput::ContentBlockStart(event) => {
            let start = match event.start() {
                Some(ContentBlockStart::ToolUse(tool)) => json!({ "toolUse": {
                    "toolUseId": tool.tool_use_id(), "name": tool.name()
                }}),
                Some(ContentBlockStart::Image(_)) | Some(ContentBlockStart::ToolResult(_)) => {
                    return Err("unsupported Bedrock output content block start".to_owned());
                }
                Some(_) | None => Value::Null,
            };
            Ok(json!({ "contentBlockStart": {
                "contentBlockIndex": event.content_block_index(), "start": start
            }}))
        }
        ConverseStreamOutput::ContentBlockDelta(event) => {
            let delta = match event.delta() {
                Some(ContentBlockDelta::Text(text)) => json!({ "text": text }),
                Some(ContentBlockDelta::ToolUse(tool)) => {
                    json!({ "toolUse": { "input": tool.input() } })
                }
                Some(ContentBlockDelta::ReasoningContent(reasoning)) => match reasoning {
                    ReasoningContentBlockDelta::Text(text) => {
                        json!({ "reasoningContent": { "text": text } })
                    }
                    ReasoningContentBlockDelta::Signature(signature) => {
                        json!({ "reasoningContent": { "signature": signature } })
                    }
                    ReasoningContentBlockDelta::RedactedContent(bytes) => {
                        json!({ "reasoningContent": { "redactedContent": base64::engine::general_purpose::STANDARD.encode(bytes.as_ref()) } })
                    }
                    _ => return Err("unknown Bedrock reasoning delta".to_owned()),
                },
                Some(_) => return Err("unsupported Bedrock output content block delta".to_owned()),
                None => Value::Null,
            };
            Ok(json!({ "contentBlockDelta": {
                "contentBlockIndex": event.content_block_index(), "delta": delta
            }}))
        }
        ConverseStreamOutput::ContentBlockStop(event) => Ok(json!({ "contentBlockStop": {
            "contentBlockIndex": event.content_block_index()
        }})),
        ConverseStreamOutput::MessageStop(event) => Ok(json!({ "messageStop": {
            "stopReason": event.stop_reason().as_str()
        }})),
        ConverseStreamOutput::Metadata(event) => {
            let usage = event
                .usage()
                .map(|usage| {
                    json!({
                        "inputTokens": usage.input_tokens(),
                        "outputTokens": usage.output_tokens(),
                        "totalTokens": usage.total_tokens(),
                        "cacheReadInputTokens": usage.cache_read_input_tokens(),
                        "cacheWriteInputTokens": usage.cache_write_input_tokens(),
                    })
                })
                .unwrap_or_else(|| json!({}));
            Ok(json!({ "metadata": { "usage": usage } }))
        }
        _ => Err("unknown Bedrock ConverseStream event".to_owned()),
    }
}

enum BedrockExecutionError {
    Aborted(Box<crate::types::AssistantMessage>),
    Failed(String),
}

async fn run_bedrock_live_worker(
    stream: AssistantMessageEventStream,
    model: Model,
    context: Value,
    options: BedrockOptions,
    cost: crate::types::ModelCost,
) {
    let mut plan = resolve_bedrock_runtime_request_plan(&model, &context, &options);
    if let Err(error) = apply_bedrock_payload_hook(&mut plan, &options, &model).await {
        stream.fail(
            "bedrock-converse-stream",
            &model.provider,
            &model.id,
            error.to_string(),
        );
        return;
    }
    match execute_bedrock_converse_stream(&stream, &model, &plan, &options, cost).await {
        Ok(()) => {}
        Err(BedrockExecutionError::Aborted(mut output)) => {
            output.stop_reason = crate::types::StopReason::Aborted;
            output.error_message = Some("Request aborted".to_owned());
            stream.push(crate::types::AssistantMessageEvent::Error {
                reason: crate::types::ErrorStopReason::Aborted,
                error: *output,
            });
        }
        Err(BedrockExecutionError::Failed(error)) => {
            stream.fail("bedrock-converse-stream", &model.provider, &model.id, error);
        }
    }
}

async fn bedrock_sdk_client(
    plan: &BedrockRuntimeRequestPlan,
) -> std::result::Result<aws_sdk_bedrockruntime::Client, String> {
    use aws_config::BehaviorVersion;
    use aws_sdk_bedrockruntime::config::{Credentials, Region, Token};

    let mut loader = aws_config::defaults(BehaviorVersion::latest());
    if let Some(region) = &plan.client_config.region {
        loader = loader.region(Region::new(region.clone()));
    }
    if let Some(profile) = &plan.client_config.profile {
        loader = loader.profile_name(profile);
    }
    match &plan.auth_mode {
        BedrockAuthMode::ExplicitCredentials {
            access_key_id,
            secret_access_key,
            session_token,
        } => {
            loader = loader.credentials_provider(Credentials::new(
                access_key_id,
                secret_access_key,
                session_token.clone(),
                None,
                "zedflow-explicit",
            ));
        }
        BedrockAuthMode::SkipAuthDummyCredentials => {
            loader = loader.credentials_provider(Credentials::new(
                "dummy-access-key",
                "dummy-secret-key",
                None,
                None,
                "zedflow-skip-auth",
            ));
        }
        BedrockAuthMode::DefaultChain | BedrockAuthMode::BearerToken(_) => {}
    }

    let shared = loader.load().await;
    let mut config = aws_sdk_bedrockruntime::config::Builder::from(&shared);
    if let Some(endpoint) = &plan.client_config.endpoint {
        config = config.endpoint_url(endpoint);
    }
    if let BedrockAuthMode::BearerToken(token) = &plan.auth_mode {
        config = config
            .token_provider(Token::new(token.clone(), None))
            .auth_scheme_preference([aws_smithy_runtime_api::client::auth::AuthSchemeId::from(
                "httpBearerAuth",
            )]);
    }
    if plan.force_http1 {
        use aws_smithy_http_client::{Builder, Connector, proxy::ProxyConfig, tls};
        let proxy = plan
            .proxy_url
            .as_deref()
            .map(ProxyConfig::http)
            .transpose()
            .map_err(|error| error.to_string())?;
        // The public Smithy connector is HTTP/1-compatible, but currently enables HTTP/2
        // ALPN too; aws-smithy-http-client exposes no public HTTP/1-only switch.
        let http_client = Builder::new().build_with_connector_fn(move |_, _| {
            let mut connector = Connector::builder();
            if let Some(proxy) = proxy.clone() {
                connector = connector.proxy_config(proxy);
            }
            connector
                .tls_provider(tls::Provider::Rustls(
                    tls::rustls_provider::CryptoMode::AwsLc,
                ))
                .build()
        });
        config = config.http_client(http_client);
    }
    Ok(aws_sdk_bedrockruntime::Client::from_conf(config.build()))
}

fn sdk_operation(
    client: &aws_sdk_bedrockruntime::Client,
    model: &Model,
    plan: &BedrockRuntimeRequestPlan,
) -> std::result::Result<
    aws_sdk_bedrockruntime::operation::converse_stream::builders::ConverseStreamFluentBuilder,
    String,
> {
    use aws_sdk_bedrockruntime::types::InferenceConfiguration;

    let payload = &plan.payload;
    let mut inference = InferenceConfiguration::builder();
    if let Some(max_tokens) = payload
        .pointer("/inferenceConfig/maxTokens")
        .and_then(Value::as_i64)
    {
        inference = inference
            .max_tokens(i32::try_from(max_tokens).map_err(|_| "maxTokens exceeds i32".to_owned())?);
    }
    if let Some(temperature) = payload
        .pointer("/inferenceConfig/temperature")
        .and_then(Value::as_f64)
    {
        inference = inference.temperature(temperature as f32);
    }
    let inference = inference.build();

    let model_id = payload
        .get("modelId")
        .and_then(Value::as_str)
        .unwrap_or(&model.id);
    let mut operation = client
        .converse_stream()
        .model_id(model_id)
        .set_messages(Some(sdk_messages(payload)?))
        .set_system(Some(sdk_system(payload)?))
        .set_inference_config(Some(inference))
        .set_tool_config(sdk_tool_config(payload)?);
    if let Some(additional) = payload.get("additionalModelRequestFields") {
        operation = operation.additional_model_request_fields(json_document(additional));
    }
    if let Some(metadata) = payload.get("requestMetadata").and_then(Value::as_object) {
        operation = operation.set_request_metadata(Some(
            metadata
                .iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|value| (key.clone(), value.to_owned()))
                })
                .collect(),
        ));
    }
    Ok(operation)
}

fn format_bedrock_send_error(
    error: &aws_smithy_runtime_api::client::result::SdkError<
        aws_sdk_bedrockruntime::operation::converse_stream::ConverseStreamError,
        aws_smithy_runtime_api::client::orchestrator::HttpResponse,
    >,
) -> String {
    let response = error.raw_response();
    let service_error = error.as_service_error();
    format_bedrock_error(
        service_error.and_then(aws_smithy_types::error::metadata::ProvideErrorMetadata::code),
        SdkErrorShape {
            message: service_error
                .and_then(aws_smithy_types::error::metadata::ProvideErrorMetadata::message)
                .map(str::to_owned)
                .unwrap_or_else(|| error.to_string()),
            metadata_http_status_code: response.map(|response| response.status().as_u16() as f64),
            response_body: response
                .and_then(|response| response.body().bytes())
                .map(|body| Value::String(String::from_utf8_lossy(body).into_owned())),
            ..SdkErrorShape::default()
        },
    )
}

fn format_bedrock_sdk_error<E, R>(
    error: &aws_smithy_runtime_api::client::result::SdkError<E, R>,
) -> String
where
    E: aws_smithy_types::error::metadata::ProvideErrorMetadata + std::fmt::Display,
{
    let Some(service_error) = error.as_service_error() else {
        return error.to_string();
    };
    format_bedrock_error(
        service_error.code(),
        SdkErrorShape {
            message: service_error
                .message()
                .map(str::to_owned)
                .unwrap_or_else(|| service_error.to_string()),
            ..SdkErrorShape::default()
        },
    )
}

async fn execute_bedrock_converse_stream(
    stream: &AssistantMessageEventStream,
    model: &Model,
    plan: &BedrockRuntimeRequestPlan,
    options: &BedrockOptions,
    cost: crate::types::ModelCost,
) -> std::result::Result<(), BedrockExecutionError> {
    use aws_sdk_bedrockruntime::operation::RequestId;
    use futures::future::{Either, select};

    let client = bedrock_sdk_client(plan)
        .await
        .map_err(BedrockExecutionError::Failed)?;
    let headers = plan.custom_signed_headers.clone();
    for (name, value) in &headers {
        HeaderName::from_bytes(name.as_bytes())
            .map_err(|error| BedrockExecutionError::Failed(error.to_string()))?;
        HeaderValue::from_str(value)
            .map_err(|error| BedrockExecutionError::Failed(error.to_string()))?;
    }
    let operation = sdk_operation(&client, model, plan)
        .map_err(BedrockExecutionError::Failed)?
        .customize()
        .mutate_request(move |request| {
            for (name, value) in &headers {
                let _ = request.headers_mut().insert(name.clone(), value.clone());
            }
        });
    let mut response = if let Some(signal) = options.signal.as_ref() {
        if signal.aborted() {
            return Err(BedrockExecutionError::Aborted(Box::new(
                empty_bedrock_message(model),
            )));
        }
        let send = Box::pin(operation.send());
        let cancelled = Box::pin(signal.cancelled());
        match select(send, cancelled).await {
            Either::Left((response, _)) => response,
            Either::Right(((), _)) => {
                return Err(BedrockExecutionError::Aborted(Box::new(
                    empty_bedrock_message(model),
                )));
            }
        }
    } else {
        operation.send().await
    }
    .map_err(|error| BedrockExecutionError::Failed(format_bedrock_send_error(&error)))?;

    invoke_bedrock_on_response(
        options,
        bedrock_response_metadata(200, response.request_id()),
        model,
    )
    .await
    .map_err(|error| BedrockExecutionError::Failed(error.to_string()))?;

    let mut processor = BedrockStreamProcessor::new(model, cost);
    loop {
        let event = if let Some(signal) = options.signal.as_ref() {
            let recv = Box::pin(response.stream.recv());
            let cancelled = Box::pin(signal.cancelled());
            match select(recv, cancelled).await {
                Either::Left((event, _)) => event,
                Either::Right(((), _)) => {
                    return Err(BedrockExecutionError::Aborted(Box::new(processor.output)));
                }
            }
        } else {
            response.stream.recv().await
        };
        match event {
            Ok(Some(event)) => processor
                .push(
                    stream,
                    sdk_event_value(event).map_err(BedrockExecutionError::Failed)?,
                )
                .map_err(BedrockExecutionError::Failed)?,
            Ok(None) => break,
            Err(error) => {
                return Err(BedrockExecutionError::Failed(format_bedrock_sdk_error(
                    &error,
                )));
            }
        }
    }
    processor
        .finish(stream)
        .map(|_| ())
        .map_err(BedrockExecutionError::Failed)
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

struct BedrockStreamProcessor {
    output: crate::types::AssistantMessage,
    blocks: Vec<BedrockBlock>,
    saw_start: bool,
    saw_stop: bool,
    cost: crate::types::ModelCost,
}

impl BedrockStreamProcessor {
    fn new(model: &Model, cost: crate::types::ModelCost) -> Self {
        Self {
            output: empty_bedrock_message(model),
            blocks: Vec::new(),
            saw_start: false,
            saw_stop: false,
            cost,
        }
    }

    fn push(
        &mut self,
        stream: &AssistantMessageEventStream,
        event: Value,
    ) -> std::result::Result<(), String> {
        if event.get("messageStart").is_some() {
            let role = event["messageStart"].get("role").and_then(Value::as_str);
            if role != Some("assistant") {
                return Err(
                    "Unexpected assistant message start but got user message start instead"
                        .to_owned(),
                );
            }
            self.saw_start = true;
            stream.push(crate::types::AssistantMessageEvent::Start {
                partial: self.output.clone().into(),
            });
        } else if let Some(start) = event.get("contentBlockStart") {
            handle_bedrock_content_block_start(start, &mut self.blocks, &mut self.output, stream);
        } else if let Some(delta) = event.get("contentBlockDelta") {
            handle_bedrock_content_block_delta(delta, &mut self.blocks, &mut self.output, stream);
        } else if let Some(stop) = event.get("contentBlockStop") {
            handle_bedrock_content_block_stop(stop, &mut self.blocks, &mut self.output, stream);
        } else if let Some(stop) = event.get("messageStop") {
            self.output.stop_reason =
                map_bedrock_stop_reason(stop.get("stopReason").and_then(Value::as_str));
            self.saw_stop = true;
        } else if let Some(metadata) = event.get("metadata") {
            handle_bedrock_metadata(metadata, &mut self.output, &self.cost);
        } else if let Some(error) = bedrock_event_error(&event) {
            return Err(error);
        }
        Ok(())
    }

    fn finish(
        self,
        stream: &AssistantMessageEventStream,
    ) -> std::result::Result<crate::types::AssistantMessage, String> {
        if !self.saw_start
            || !self.saw_stop
            || matches!(
                self.output.stop_reason,
                crate::types::StopReason::Error | crate::types::StopReason::Aborted
            )
        {
            return Err("Bedrock stream ended without a valid terminal message".to_owned());
        }
        stream.push(crate::types::AssistantMessageEvent::Done {
            reason: canonical_done_reason(self.output.stop_reason),
            message: self.output.clone(),
        });
        Ok(self.output)
    }
}

/// Maps Bedrock ConverseStream events into Pi canonical assistant-message events.
pub fn process_bedrock_converse_stream_events(
    stream: &AssistantMessageEventStream,
    model: &Model,
    events: impl IntoIterator<Item = Value>,
) -> std::result::Result<crate::types::AssistantMessage, String> {
    let mut processor = BedrockStreamProcessor::new(model, crate::types::ModelCost::default());
    for event in events {
        processor.push(stream, event)?;
    }
    processor.finish(stream)
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
        partial: output.clone().into(),
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
            partial: output.clone().into(),
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
                partial: output.clone().into(),
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
                partial: output.clone().into(),
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
                partial: output.clone().into(),
            })
        }
        BedrockBlock::Thinking { thinking, .. } => {
            stream.push(crate::types::AssistantMessageEvent::ThinkingEnd {
                content_index: index,
                content: thinking.clone(),
                partial: output.clone().into(),
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
                partial: output.clone().into(),
            });
        }
    }
}

fn handle_bedrock_metadata(
    metadata: &Value,
    output: &mut crate::types::AssistantMessage,
    cost: &crate::types::ModelCost,
) {
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
    output.usage.cost.input = cost.input * output.usage.input as f64 / 1_000_000.0;
    output.usage.cost.output = cost.output * output.usage.output as f64 / 1_000_000.0;
    output.usage.cost.cache_read = cost.cache_read * output.usage.cache_read as f64 / 1_000_000.0;
    output.usage.cost.cache_write =
        cost.cache_write * output.usage.cache_write as f64 / 1_000_000.0;
    output.usage.cost.total = output.usage.cost.input
        + output.usage.cost.output
        + output.usage.cost.cache_read
        + output.usage.cost.cache_write;
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
        partial: output.clone().into(),
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
        partial: output.clone().into(),
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
    fn payload_and_response_hooks_are_preserved_by_sdk_seams() {
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
    fn canonical_provider_streams_are_registered_without_network() {
        let _ = provider_streams();
    }

    #[test]
    fn payload_includes_tools_and_specific_tool_choice() {
        let payload = build_bedrock_converse_payload(
            &model("anthropic.claude-sonnet-4-6"),
            &json!({
                "messages": [],
                "tools": [{
                    "name": "lookup",
                    "description": "Look something up",
                    "parameters": { "type": "object", "properties": { "query": { "type": "string" } } }
                }]
            }),
            &BedrockOptions {
                tool_choice: Some(BedrockToolChoice::Tool {
                    name: "lookup".to_owned(),
                }),
                ..BedrockOptions::default()
            },
        );

        assert_eq!(
            payload["toolConfig"]["toolChoice"],
            json!({ "tool": { "name": "lookup" } })
        );
        assert_eq!(
            payload["toolConfig"]["tools"][0]["toolSpec"]["name"],
            "lookup"
        );
        assert!(
            sdk_tool_config(&payload)
                .expect("valid SDK tool config")
                .is_some()
        );
    }

    #[test]
    fn incremental_processor_applies_model_cost() {
        let stream = AssistantMessageEventStream::new();
        let mut processor = BedrockStreamProcessor::new(
            &model("anthropic.claude-sonnet-4-6"),
            crate::types::ModelCost {
                input: 1.0,
                output: 2.0,
                cache_read: 0.5,
                cache_write: 1.5,
            },
        );
        for event in [
            json!({ "messageStart": { "role": "assistant" } }),
            json!({ "metadata": { "usage": { "inputTokens": 1_000_000, "outputTokens": 500_000, "cacheReadInputTokens": 200_000, "cacheWriteInputTokens": 100_000 } } }),
            json!({ "messageStop": { "stopReason": "end_turn" } }),
        ] {
            processor
                .push(&stream, event)
                .expect("valid incremental event");
        }
        let output = processor.finish(&stream).expect("terminal message");

        assert_eq!(output.usage.cost.input, 1.0);
        assert_eq!(output.usage.cost.output, 1.0);
        assert_eq!(output.usage.cost.cache_read, 0.1);
        assert_eq!(output.usage.cost.cache_write, 0.15);
        assert_eq!(output.usage.cost.total, 2.25);
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
