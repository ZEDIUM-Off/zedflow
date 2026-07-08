//! Cloudflare AI Gateway model catalog ported from Pi's `packages/ai/src/providers/cloudflare-ai-gateway.models.ts`.

use crate::models::Model;

/// Pricing metadata for a Cloudflare AI Gateway model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudflareAiGatewayModelCost {
    /// Input token price in Pi's model catalog units.
    pub input: f64,
    /// Output token price in Pi's model catalog units.
    pub output: f64,
    /// Cache-read token price in Pi's model catalog units.
    pub cache_read: f64,
    /// Cache-write token price in Pi's model catalog units.
    pub cache_write: f64,
}

/// Compatibility metadata for a Cloudflare AI Gateway model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CloudflareAiGatewayModelCompat {
    /// Whether the model supports the OpenAI store option.
    pub supports_store: Option<bool>,
    /// Whether the model supports the developer role.
    pub supports_developer_role: Option<bool>,
    /// Whether the model supports OpenAI reasoning effort.
    pub supports_reasoning_effort: Option<bool>,
    /// OpenAI-compatible max-token field override.
    pub max_tokens_field: Option<&'static str>,
    /// Whether strict tool schema mode is supported.
    pub supports_strict_mode: Option<bool>,
    /// Whether the model supports long cache retention.
    pub supports_long_cache_retention: Option<bool>,
    /// Whether Pi sends session affinity headers for the model.
    pub send_session_affinity_headers: Option<bool>,
    /// Whether Pi forces adaptive thinking for the model.
    pub force_adaptive_thinking: Option<bool>,
    /// Whether the model supports the temperature parameter.
    pub supports_temperature: Option<bool>,
}

/// A `thinkingLevelMap` entry from Pi's Cloudflare AI Gateway model catalog.
pub type CloudflareAiGatewayThinkingLevelMap = &'static [(&'static str, Option<&'static str>)];

/// One Cloudflare AI Gateway model entry from Pi's generated model catalog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudflareAiGatewayModel {
    /// Model id and catalog key.
    pub id: &'static str,
    /// Human-readable model name.
    pub name: &'static str,
    /// API implementation id.
    pub api: &'static str,
    /// Provider id.
    pub provider: &'static str,
    /// Base API URL.
    pub base_url: &'static str,
    /// Optional compatibility flags.
    pub compat: Option<CloudflareAiGatewayModelCompat>,
    /// Whether the model supports reasoning.
    pub reasoning: bool,
    /// Optional Pi thinking-level mapping.
    pub thinking_level_map: Option<CloudflareAiGatewayThinkingLevelMap>,
    /// Supported input modalities.
    pub input: &'static [&'static str],
    /// Model cost metadata.
    pub cost: CloudflareAiGatewayModelCost,
    /// Context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
}

const TEXT: &[&str] = &["text"];
const TEXT_IMAGE: &[&str] = &["text", "image"];
const CLOUDFLARE_AI_GATEWAY_ANTHROPIC_COMPAT: CloudflareAiGatewayModelCompat =
    CloudflareAiGatewayModelCompat {
        supports_store: None,
        supports_developer_role: None,
        supports_reasoning_effort: None,
        max_tokens_field: None,
        supports_strict_mode: None,
        supports_long_cache_retention: None,
        send_session_affinity_headers: Some(true),
        force_adaptive_thinking: None,
        supports_temperature: None,
    };
const CLOUDFLARE_AI_GATEWAY_ANTHROPIC_ADAPTIVE_COMPAT: CloudflareAiGatewayModelCompat =
    CloudflareAiGatewayModelCompat {
        supports_store: None,
        supports_developer_role: None,
        supports_reasoning_effort: None,
        max_tokens_field: None,
        supports_strict_mode: None,
        supports_long_cache_retention: None,
        send_session_affinity_headers: Some(true),
        force_adaptive_thinking: Some(true),
        supports_temperature: None,
    };
const CLOUDFLARE_AI_GATEWAY_ANTHROPIC_ADAPTIVE_NO_TEMPERATURE_COMPAT:
    CloudflareAiGatewayModelCompat = CloudflareAiGatewayModelCompat {
    supports_store: None,
    supports_developer_role: None,
    supports_reasoning_effort: None,
    max_tokens_field: None,
    supports_strict_mode: None,
    supports_long_cache_retention: None,
    send_session_affinity_headers: Some(true),
    force_adaptive_thinking: Some(true),
    supports_temperature: Some(false),
};
const CLOUDFLARE_AI_GATEWAY_WORKERS_COMPAT: CloudflareAiGatewayModelCompat =
    CloudflareAiGatewayModelCompat {
        supports_store: Some(false),
        supports_developer_role: Some(false),
        supports_reasoning_effort: Some(false),
        max_tokens_field: Some("max_tokens"),
        supports_strict_mode: Some(false),
        supports_long_cache_retention: Some(false),
        send_session_affinity_headers: Some(true),
        force_adaptive_thinking: None,
        supports_temperature: None,
    };
const THINKING_OFF_UNSUPPORTED_XHIGH: CloudflareAiGatewayThinkingLevelMap =
    &[("off", None), ("xhigh", Some("xhigh"))];
const THINKING_XHIGH_MAX: CloudflareAiGatewayThinkingLevelMap = &[("xhigh", Some("max"))];
const THINKING_XHIGH: CloudflareAiGatewayThinkingLevelMap = &[("xhigh", Some("xhigh"))];
const THINKING_OFF_UNSUPPORTED: CloudflareAiGatewayThinkingLevelMap = &[("off", None)];

/// Cloudflare AI Gateway models keyed by `id`, preserving Pi's generated catalog order and metadata.
pub const CLOUDFLARE_AI_GATEWAY_MODELS: &[CloudflareAiGatewayModel] = &[
    CloudflareAiGatewayModel {
        id: "claude-3-5-haiku",
        name: "Claude Haiku 3.5 (latest)",
        api: "anthropic-messages",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic",
        compat: Some(CLOUDFLARE_AI_GATEWAY_ANTHROPIC_COMPAT),
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 0.8,
            output: 4.0,
            cache_read: 0.08,
            cache_write: 1.0,
        },
        context_window: 200_000,
        max_tokens: 8_192,
    },
    CloudflareAiGatewayModel {
        id: "claude-3-haiku",
        name: "Claude Haiku 3",
        api: "anthropic-messages",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic",
        compat: Some(CLOUDFLARE_AI_GATEWAY_ANTHROPIC_COMPAT),
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 0.25,
            output: 1.25,
            cache_read: 0.03,
            cache_write: 0.3,
        },
        context_window: 200_000,
        max_tokens: 4_096,
    },
    CloudflareAiGatewayModel {
        id: "claude-3-opus",
        name: "Claude Opus 3",
        api: "anthropic-messages",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic",
        compat: Some(CLOUDFLARE_AI_GATEWAY_ANTHROPIC_COMPAT),
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 15.0,
            output: 75.0,
            cache_read: 1.5,
            cache_write: 18.75,
        },
        context_window: 200_000,
        max_tokens: 4_096,
    },
    CloudflareAiGatewayModel {
        id: "claude-3-sonnet",
        name: "Claude Sonnet 3",
        api: "anthropic-messages",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic",
        compat: Some(CLOUDFLARE_AI_GATEWAY_ANTHROPIC_COMPAT),
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 0.3,
        },
        context_window: 200_000,
        max_tokens: 4_096,
    },
    CloudflareAiGatewayModel {
        id: "claude-3.5-haiku",
        name: "Claude Haiku 3.5 (latest)",
        api: "anthropic-messages",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic",
        compat: Some(CLOUDFLARE_AI_GATEWAY_ANTHROPIC_COMPAT),
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 0.8,
            output: 4.0,
            cache_read: 0.08,
            cache_write: 1.0,
        },
        context_window: 200_000,
        max_tokens: 8_192,
    },
    CloudflareAiGatewayModel {
        id: "claude-3.5-sonnet",
        name: "Claude Sonnet 3.5 v2",
        api: "anthropic-messages",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic",
        compat: Some(CLOUDFLARE_AI_GATEWAY_ANTHROPIC_COMPAT),
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 200_000,
        max_tokens: 8_192,
    },
    CloudflareAiGatewayModel {
        id: "claude-fable-5",
        name: "Claude Fable 5",
        api: "anthropic-messages",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic",
        compat: Some(CLOUDFLARE_AI_GATEWAY_ANTHROPIC_ADAPTIVE_COMPAT),
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED_XHIGH),
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 10.0,
            output: 50.0,
            cache_read: 1.0,
            cache_write: 12.5,
        },
        context_window: 1_000_000,
        max_tokens: 128_000,
    },
    CloudflareAiGatewayModel {
        id: "claude-haiku-4-5",
        name: "Claude Haiku 4.5 (latest)",
        api: "anthropic-messages",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic",
        compat: Some(CLOUDFLARE_AI_GATEWAY_ANTHROPIC_COMPAT),
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 1.0,
            output: 5.0,
            cache_read: 0.1,
            cache_write: 1.25,
        },
        context_window: 200_000,
        max_tokens: 64_000,
    },
    CloudflareAiGatewayModel {
        id: "claude-opus-4",
        name: "Claude Opus 4 (latest)",
        api: "anthropic-messages",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic",
        compat: Some(CLOUDFLARE_AI_GATEWAY_ANTHROPIC_COMPAT),
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 15.0,
            output: 75.0,
            cache_read: 1.5,
            cache_write: 18.75,
        },
        context_window: 200_000,
        max_tokens: 32_000,
    },
    CloudflareAiGatewayModel {
        id: "claude-opus-4-1",
        name: "Claude Opus 4.1 (latest)",
        api: "anthropic-messages",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic",
        compat: Some(CLOUDFLARE_AI_GATEWAY_ANTHROPIC_COMPAT),
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 15.0,
            output: 75.0,
            cache_read: 1.5,
            cache_write: 18.75,
        },
        context_window: 200_000,
        max_tokens: 32_000,
    },
    CloudflareAiGatewayModel {
        id: "claude-opus-4-5",
        name: "Claude Opus 4.5 (latest)",
        api: "anthropic-messages",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic",
        compat: Some(CLOUDFLARE_AI_GATEWAY_ANTHROPIC_COMPAT),
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 200_000,
        max_tokens: 64_000,
    },
    CloudflareAiGatewayModel {
        id: "claude-opus-4-6",
        name: "Claude Opus 4.6 (latest)",
        api: "anthropic-messages",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic",
        compat: Some(CLOUDFLARE_AI_GATEWAY_ANTHROPIC_ADAPTIVE_COMPAT),
        reasoning: true,
        thinking_level_map: Some(THINKING_XHIGH_MAX),
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 1_000_000,
        max_tokens: 128_000,
    },
    CloudflareAiGatewayModel {
        id: "claude-opus-4-7",
        name: "Claude Opus 4.7",
        api: "anthropic-messages",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic",
        compat: Some(CLOUDFLARE_AI_GATEWAY_ANTHROPIC_ADAPTIVE_NO_TEMPERATURE_COMPAT),
        reasoning: true,
        thinking_level_map: Some(THINKING_XHIGH),
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 1_000_000,
        max_tokens: 128_000,
    },
    CloudflareAiGatewayModel {
        id: "claude-opus-4-8",
        name: "Claude Opus 4.8",
        api: "anthropic-messages",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic",
        compat: Some(CLOUDFLARE_AI_GATEWAY_ANTHROPIC_ADAPTIVE_NO_TEMPERATURE_COMPAT),
        reasoning: true,
        thinking_level_map: Some(THINKING_XHIGH),
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 1_000_000,
        max_tokens: 128_000,
    },
    CloudflareAiGatewayModel {
        id: "claude-sonnet-4",
        name: "Claude Sonnet 4 (latest)",
        api: "anthropic-messages",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic",
        compat: Some(CLOUDFLARE_AI_GATEWAY_ANTHROPIC_COMPAT),
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 200_000,
        max_tokens: 64_000,
    },
    CloudflareAiGatewayModel {
        id: "claude-sonnet-4-5",
        name: "Claude Sonnet 4.5 (latest)",
        api: "anthropic-messages",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic",
        compat: Some(CLOUDFLARE_AI_GATEWAY_ANTHROPIC_COMPAT),
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 200_000,
        max_tokens: 64_000,
    },
    CloudflareAiGatewayModel {
        id: "claude-sonnet-4-6",
        name: "Claude Sonnet 4.6",
        api: "anthropic-messages",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic",
        compat: Some(CLOUDFLARE_AI_GATEWAY_ANTHROPIC_ADAPTIVE_COMPAT),
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 1_000_000,
        max_tokens: 64_000,
    },
    CloudflareAiGatewayModel {
        id: "claude-sonnet-5",
        name: "Claude Sonnet 5",
        api: "anthropic-messages",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic",
        compat: Some(CLOUDFLARE_AI_GATEWAY_ANTHROPIC_ADAPTIVE_COMPAT),
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 2.0,
            output: 10.0,
            cache_read: 0.2,
            cache_write: 2.5,
        },
        context_window: 1_000_000,
        max_tokens: 128_000,
    },
    CloudflareAiGatewayModel {
        id: "gpt-4",
        name: "GPT-4",
        api: "openai-responses",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai",
        compat: None,
        reasoning: false,
        thinking_level_map: None,
        input: TEXT,
        cost: CloudflareAiGatewayModelCost {
            input: 30.0,
            output: 60.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 8_192,
        max_tokens: 8_192,
    },
    CloudflareAiGatewayModel {
        id: "gpt-4-turbo",
        name: "GPT-4 Turbo",
        api: "openai-responses",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai",
        compat: None,
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 10.0,
            output: 30.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 4_096,
    },
    CloudflareAiGatewayModel {
        id: "gpt-4o",
        name: "GPT-4o",
        api: "openai-responses",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai",
        compat: None,
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 2.5,
            output: 10.0,
            cache_read: 1.25,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 16_384,
    },
    CloudflareAiGatewayModel {
        id: "gpt-4o-mini",
        name: "GPT-4o mini",
        api: "openai-responses",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai",
        compat: None,
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.08,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 16_384,
    },
    CloudflareAiGatewayModel {
        id: "gpt-5.1",
        name: "GPT-5.1",
        api: "openai-responses",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai",
        compat: None,
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED),
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.13,
            cache_write: 0.0,
        },
        context_window: 400_000,
        max_tokens: 128_000,
    },
    CloudflareAiGatewayModel {
        id: "gpt-5.1-codex",
        name: "GPT-5.1 Codex",
        api: "openai-responses",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai",
        compat: None,
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED),
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 400_000,
        max_tokens: 128_000,
    },
    CloudflareAiGatewayModel {
        id: "gpt-5.2",
        name: "GPT-5.2",
        api: "openai-responses",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai",
        compat: None,
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED_XHIGH),
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 400_000,
        max_tokens: 128_000,
    },
    CloudflareAiGatewayModel {
        id: "gpt-5.2-codex",
        name: "GPT-5.2 Codex",
        api: "openai-responses",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai",
        compat: None,
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED_XHIGH),
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 400_000,
        max_tokens: 128_000,
    },
    CloudflareAiGatewayModel {
        id: "gpt-5.3-codex",
        name: "GPT-5.3 Codex",
        api: "openai-responses",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai",
        compat: None,
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED_XHIGH),
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 400_000,
        max_tokens: 128_000,
    },
    CloudflareAiGatewayModel {
        id: "gpt-5.4",
        name: "GPT-5.4",
        api: "openai-responses",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai",
        compat: None,
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED_XHIGH),
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 2.5,
            output: 15.0,
            cache_read: 0.25,
            cache_write: 0.0,
        },
        context_window: 1_050_000,
        max_tokens: 128_000,
    },
    CloudflareAiGatewayModel {
        id: "gpt-5.5",
        name: "GPT-5.5",
        api: "openai-responses",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai",
        compat: None,
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED_XHIGH),
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 5.0,
            output: 30.0,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        context_window: 1_050_000,
        max_tokens: 128_000,
    },
    CloudflareAiGatewayModel {
        id: "o1",
        name: "o1",
        api: "openai-responses",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai",
        compat: None,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 15.0,
            output: 60.0,
            cache_read: 7.5,
            cache_write: 0.0,
        },
        context_window: 200_000,
        max_tokens: 100_000,
    },
    CloudflareAiGatewayModel {
        id: "o3",
        name: "o3",
        api: "openai-responses",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai",
        compat: None,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 2.0,
            output: 8.0,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        context_window: 200_000,
        max_tokens: 100_000,
    },
    CloudflareAiGatewayModel {
        id: "o3-mini",
        name: "o3-mini",
        api: "openai-responses",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai",
        compat: None,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: CloudflareAiGatewayModelCost {
            input: 1.1,
            output: 4.4,
            cache_read: 0.55,
            cache_write: 0.0,
        },
        context_window: 200_000,
        max_tokens: 100_000,
    },
    CloudflareAiGatewayModel {
        id: "o3-pro",
        name: "o3-pro",
        api: "openai-responses",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai",
        compat: None,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 20.0,
            output: 80.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 200_000,
        max_tokens: 100_000,
    },
    CloudflareAiGatewayModel {
        id: "o4-mini",
        name: "o4-mini",
        api: "openai-responses",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai",
        compat: None,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 1.1,
            output: 4.4,
            cache_read: 0.28,
            cache_write: 0.0,
        },
        context_window: 200_000,
        max_tokens: 100_000,
    },
    CloudflareAiGatewayModel {
        id: "workers-ai/@cf/moonshotai/kimi-k2.5",
        name: "Kimi K2.5",
        api: "openai-completions",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/compat",
        compat: Some(CLOUDFLARE_AI_GATEWAY_WORKERS_COMPAT),
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 0.6,
            output: 3.0,
            cache_read: 0.1,
            cache_write: 0.0,
        },
        context_window: 256_000,
        max_tokens: 256_000,
    },
    CloudflareAiGatewayModel {
        id: "workers-ai/@cf/moonshotai/kimi-k2.6",
        name: "Kimi K2.6",
        api: "openai-completions",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/compat",
        compat: Some(CLOUDFLARE_AI_GATEWAY_WORKERS_COMPAT),
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: CloudflareAiGatewayModelCost {
            input: 0.95,
            output: 4.0,
            cache_read: 0.16,
            cache_write: 0.0,
        },
        context_window: 256_000,
        max_tokens: 256_000,
    },
    CloudflareAiGatewayModel {
        id: "workers-ai/@cf/nvidia/nemotron-3-120b-a12b",
        name: "Nemotron 3 Super 120B",
        api: "openai-completions",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/compat",
        compat: Some(CLOUDFLARE_AI_GATEWAY_WORKERS_COMPAT),
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: CloudflareAiGatewayModelCost {
            input: 0.5,
            output: 1.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000,
        max_tokens: 256_000,
    },
    CloudflareAiGatewayModel {
        id: "workers-ai/@cf/zai-org/glm-4.7-flash",
        name: "GLM-4.7-Flash",
        api: "openai-completions",
        provider: "cloudflare-ai-gateway",
        base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/compat",
        compat: Some(CLOUDFLARE_AI_GATEWAY_WORKERS_COMPAT),
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: CloudflareAiGatewayModelCost {
            input: 0.06,
            output: 0.4,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072,
        max_tokens: 131_072,
    },
];

/// Returns the Cloudflare AI Gateway catalog as the crate's current minimal runtime model shape.
#[must_use]
pub fn cloudflare_ai_gateway_models() -> Vec<Model> {
    CLOUDFLARE_AI_GATEWAY_MODELS
        .iter()
        .map(|model| Model {
            provider: model.provider.to_owned(),
            id: model.id.to_owned(),
            api: model.api.to_owned(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_cloudflare_ai_gateway_catalog_metadata() {
        assert_eq!(CLOUDFLARE_AI_GATEWAY_MODELS.len(), 38);
        assert_eq!(CLOUDFLARE_AI_GATEWAY_MODELS[0].id, "claude-3-5-haiku");

        let opus = CLOUDFLARE_AI_GATEWAY_MODELS
            .iter()
            .find(|model| model.id == "claude-opus-4-7")
            .expect("claude-opus-4-7 is present");
        assert_eq!(opus.name, "Claude Opus 4.7");
        assert_eq!(opus.context_window, 1_000_000);
        assert_eq!(opus.max_tokens, 128_000);
        assert_eq!(
            opus.compat.expect("compat").supports_temperature,
            Some(false)
        );
        assert_eq!(
            opus.thinking_level_map.expect("thinking map"),
            THINKING_XHIGH
        );

        let workers = CLOUDFLARE_AI_GATEWAY_MODELS
            .iter()
            .find(|model| model.id == "workers-ai/@cf/moonshotai/kimi-k2.5")
            .expect("workers gateway model is present");
        assert_eq!(workers.api, "openai-completions");
        assert_eq!(
            workers.compat.expect("compat").max_tokens_field,
            Some("max_tokens")
        );
    }

    #[test]
    fn exposes_current_runtime_model_shape() {
        let models = cloudflare_ai_gateway_models();
        assert_eq!(models.len(), CLOUDFLARE_AI_GATEWAY_MODELS.len());
        assert!(
            models
                .iter()
                .all(|model| model.provider == "cloudflare-ai-gateway")
        );
        assert!(models.iter().any(|model| model.api == "anthropic-messages"));
        assert!(models.iter().any(|model| model.api == "openai-responses"));
        assert!(models.iter().any(|model| model.api == "openai-completions"));
    }
}
