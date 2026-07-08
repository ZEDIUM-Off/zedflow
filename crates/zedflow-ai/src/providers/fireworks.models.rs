//! Fireworks model catalog ported from Pi's `packages/ai/src/providers/fireworks.models.ts`.

/// Pricing metadata for a Fireworks model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FireworksModelCost {
    /// Input token price in Pi's model catalog units.
    pub input: f64,
    /// Output token price in Pi's model catalog units.
    pub output: f64,
    /// Cache-read token price in Pi's model catalog units.
    pub cache_read: f64,
    /// Cache-write token price in Pi's model catalog units.
    pub cache_write: f64,
}

/// Compatibility metadata for a Fireworks model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FireworksModelCompat {
    /// Whether the model supports the OpenAI store option.
    pub supports_store: Option<bool>,
    /// Whether the model supports the developer role.
    pub supports_developer_role: Option<bool>,
    /// Whether Pi sends session affinity headers for the model.
    pub send_session_affinity_headers: Option<bool>,
    /// Whether the model supports eager tool input streaming.
    pub supports_eager_tool_input_streaming: Option<bool>,
    /// Whether cache control metadata is supported on tool calls.
    pub supports_cache_control_on_tools: Option<bool>,
    /// Whether the model supports long cache retention.
    pub supports_long_cache_retention: Option<bool>,
}

/// A `thinkingLevelMap` entry from Pi's Fireworks model catalog.
pub type FireworksThinkingLevelMap = &'static [(&'static str, Option<&'static str>)];

/// One Fireworks model entry from Pi's generated model catalog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FireworksModel {
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
    /// Compatibility flags.
    pub compat: FireworksModelCompat,
    /// Whether the model supports reasoning.
    pub reasoning: bool,
    /// Optional Pi thinking-level mapping.
    pub thinking_level_map: Option<FireworksThinkingLevelMap>,
    /// Supported input modalities.
    pub input: &'static [&'static str],
    /// Model cost metadata.
    pub cost: FireworksModelCost,
    /// Context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
}

const TEXT: &[&str] = &["text"];
const TEXT_IMAGE: &[&str] = &["text", "image"];
const ANTHROPIC_MESSAGES_COMPAT: FireworksModelCompat = FireworksModelCompat {
    supports_store: None,
    supports_developer_role: None,
    send_session_affinity_headers: Some(true),
    supports_eager_tool_input_streaming: Some(false),
    supports_cache_control_on_tools: Some(false),
    supports_long_cache_retention: Some(false),
};
const OPENAI_COMPLETIONS_COMPAT: FireworksModelCompat = FireworksModelCompat {
    supports_store: Some(false),
    supports_developer_role: Some(false),
    send_session_affinity_headers: None,
    supports_eager_tool_input_streaming: None,
    supports_cache_control_on_tools: None,
    supports_long_cache_retention: None,
};
const THINKING_GLM_5P2: FireworksThinkingLevelMap = &[
    ("off", Some("none")),
    ("minimal", None),
    ("low", Some("high")),
    ("medium", Some("high")),
    ("xhigh", Some("max")),
];

/// Fireworks models keyed by `id`, preserving Pi's generated catalog order and metadata.
pub const FIREWORKS_MODELS: &[FireworksModel] = &[
    FireworksModel {
        id: "accounts/fireworks/models/deepseek-v4-flash",
        name: "DeepSeek V4 Flash",
        api: "anthropic-messages",
        provider: "fireworks",
        base_url: "https://api.fireworks.ai/inference",
        compat: ANTHROPIC_MESSAGES_COMPAT,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: FireworksModelCost {
            input: 0.14,
            output: 0.28,
            cache_read: 0.028,
            cache_write: 0.0,
        },
        context_window: 1_000_000,
        max_tokens: 384_000,
    },
    FireworksModel {
        id: "accounts/fireworks/models/deepseek-v4-pro",
        name: "DeepSeek V4 Pro",
        api: "anthropic-messages",
        provider: "fireworks",
        base_url: "https://api.fireworks.ai/inference",
        compat: ANTHROPIC_MESSAGES_COMPAT,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: FireworksModelCost {
            input: 1.74,
            output: 3.48,
            cache_read: 0.145,
            cache_write: 0.0,
        },
        context_window: 1_000_000,
        max_tokens: 384_000,
    },
    FireworksModel {
        id: "accounts/fireworks/models/glm-5p1",
        name: "GLM 5.1",
        api: "anthropic-messages",
        provider: "fireworks",
        base_url: "https://api.fireworks.ai/inference",
        compat: ANTHROPIC_MESSAGES_COMPAT,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: FireworksModelCost {
            input: 1.4,
            output: 4.4,
            cache_read: 0.26,
            cache_write: 0.0,
        },
        context_window: 202_800,
        max_tokens: 131_072,
    },
    FireworksModel {
        id: "accounts/fireworks/models/glm-5p2",
        name: "GLM 5.2",
        api: "openai-completions",
        provider: "fireworks",
        base_url: "https://api.fireworks.ai/inference/v1",
        compat: OPENAI_COMPLETIONS_COMPAT,
        reasoning: true,
        thinking_level_map: Some(THINKING_GLM_5P2),
        input: TEXT,
        cost: FireworksModelCost {
            input: 1.4,
            output: 4.4,
            cache_read: 0.26,
            cache_write: 0.0,
        },
        context_window: 1_048_575,
        max_tokens: 131_072,
    },
    FireworksModel {
        id: "accounts/fireworks/models/gpt-oss-120b",
        name: "GPT OSS 120B",
        api: "anthropic-messages",
        provider: "fireworks",
        base_url: "https://api.fireworks.ai/inference",
        compat: ANTHROPIC_MESSAGES_COMPAT,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: FireworksModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.015,
            cache_write: 0.0,
        },
        context_window: 131_072,
        max_tokens: 32_768,
    },
    FireworksModel {
        id: "accounts/fireworks/models/gpt-oss-20b",
        name: "GPT OSS 20B",
        api: "anthropic-messages",
        provider: "fireworks",
        base_url: "https://api.fireworks.ai/inference",
        compat: ANTHROPIC_MESSAGES_COMPAT,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: FireworksModelCost {
            input: 0.07,
            output: 0.3,
            cache_read: 0.035,
            cache_write: 0.0,
        },
        context_window: 131_072,
        max_tokens: 32_768,
    },
    FireworksModel {
        id: "accounts/fireworks/models/kimi-k2p6",
        name: "Kimi K2.6",
        api: "anthropic-messages",
        provider: "fireworks",
        base_url: "https://api.fireworks.ai/inference",
        compat: ANTHROPIC_MESSAGES_COMPAT,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: FireworksModelCost {
            input: 0.95,
            output: 4.0,
            cache_read: 0.16,
            cache_write: 0.0,
        },
        context_window: 262_000,
        max_tokens: 262_000,
    },
    FireworksModel {
        id: "accounts/fireworks/models/kimi-k2p7-code",
        name: "Kimi K2.7 Code",
        api: "anthropic-messages",
        provider: "fireworks",
        base_url: "https://api.fireworks.ai/inference",
        compat: ANTHROPIC_MESSAGES_COMPAT,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: FireworksModelCost {
            input: 0.95,
            output: 4.0,
            cache_read: 0.19,
            cache_write: 0.0,
        },
        context_window: 262_000,
        max_tokens: 262_000,
    },
    FireworksModel {
        id: "accounts/fireworks/models/minimax-m2p7",
        name: "MiniMax-M2.7",
        api: "anthropic-messages",
        provider: "fireworks",
        base_url: "https://api.fireworks.ai/inference",
        compat: ANTHROPIC_MESSAGES_COMPAT,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: FireworksModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.06,
            cache_write: 0.0,
        },
        context_window: 196_608,
        max_tokens: 196_608,
    },
    FireworksModel {
        id: "accounts/fireworks/models/minimax-m3",
        name: "MiniMax-M3",
        api: "anthropic-messages",
        provider: "fireworks",
        base_url: "https://api.fireworks.ai/inference",
        compat: ANTHROPIC_MESSAGES_COMPAT,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: FireworksModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.06,
            cache_write: 0.0,
        },
        context_window: 512_000,
        max_tokens: 512_000,
    },
    FireworksModel {
        id: "accounts/fireworks/models/qwen3p7-plus",
        name: "Qwen 3.7 Plus",
        api: "anthropic-messages",
        provider: "fireworks",
        base_url: "https://api.fireworks.ai/inference",
        compat: ANTHROPIC_MESSAGES_COMPAT,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: FireworksModelCost {
            input: 0.4,
            output: 1.6,
            cache_read: 0.08,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 65_536,
    },
    FireworksModel {
        id: "accounts/fireworks/routers/glm-5p1-fast",
        name: "GLM 5.1 Fast",
        api: "anthropic-messages",
        provider: "fireworks",
        base_url: "https://api.fireworks.ai/inference",
        compat: ANTHROPIC_MESSAGES_COMPAT,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: FireworksModelCost {
            input: 2.8,
            output: 8.8,
            cache_read: 0.52,
            cache_write: 0.0,
        },
        context_window: 202_800,
        max_tokens: 131_072,
    },
    FireworksModel {
        id: "accounts/fireworks/routers/glm-5p2-fast",
        name: "GLM 5.2 Fast",
        api: "openai-completions",
        provider: "fireworks",
        base_url: "https://api.fireworks.ai/inference/v1",
        compat: OPENAI_COMPLETIONS_COMPAT,
        reasoning: true,
        thinking_level_map: Some(THINKING_GLM_5P2),
        input: TEXT,
        cost: FireworksModelCost {
            input: 2.1,
            output: 6.6,
            cache_read: 0.21,
            cache_write: 0.0,
        },
        context_window: 1_048_575,
        max_tokens: 131_072,
    },
    FireworksModel {
        id: "accounts/fireworks/routers/kimi-k2p6-fast",
        name: "Kimi K2.6 Fast",
        api: "anthropic-messages",
        provider: "fireworks",
        base_url: "https://api.fireworks.ai/inference",
        compat: ANTHROPIC_MESSAGES_COMPAT,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: FireworksModelCost {
            input: 2.0,
            output: 8.0,
            cache_read: 0.3,
            cache_write: 0.0,
        },
        context_window: 262_000,
        max_tokens: 262_000,
    },
    FireworksModel {
        id: "accounts/fireworks/routers/kimi-k2p6-turbo",
        name: "Kimi K2.6 Turbo",
        api: "anthropic-messages",
        provider: "fireworks",
        base_url: "https://api.fireworks.ai/inference",
        compat: ANTHROPIC_MESSAGES_COMPAT,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: FireworksModelCost {
            input: 2.0,
            output: 8.0,
            cache_read: 0.3,
            cache_write: 0.0,
        },
        context_window: 262_000,
        max_tokens: 262_000,
    },
    FireworksModel {
        id: "accounts/fireworks/routers/kimi-k2p7-code-fast",
        name: "Kimi K2.7 Code Fast",
        api: "anthropic-messages",
        provider: "fireworks",
        base_url: "https://api.fireworks.ai/inference",
        compat: ANTHROPIC_MESSAGES_COMPAT,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: FireworksModelCost {
            input: 1.9,
            output: 8.0,
            cache_read: 0.38,
            cache_write: 0.0,
        },
        context_window: 262_000,
        max_tokens: 262_000,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn model(id: &str) -> &'static FireworksModel {
        FIREWORKS_MODELS
            .iter()
            .find(|model| model.id == id)
            .expect("Fireworks model fixture exists")
    }

    #[test]
    fn registers_default_kimi_k2p6_model_via_anthropic_messages_api() {
        let model = model("accounts/fireworks/models/kimi-k2p6");

        assert_eq!(model.api, "anthropic-messages");
        assert_eq!(model.provider, "fireworks");
        assert_eq!(model.base_url, "https://api.fireworks.ai/inference");
        assert!(model.reasoning);
        assert_eq!(model.input, ["text", "image"]);
        assert_eq!(model.context_window, 262_000);
        assert_eq!(model.max_tokens, 262_000);
        assert_eq!(
            model.cost,
            FireworksModelCost {
                input: 0.95,
                output: 4.0,
                cache_read: 0.16,
                cache_write: 0.0,
            }
        );
    }

    #[test]
    fn registers_fire_pass_turbo_router_model() {
        let model = FIREWORKS_MODELS
            .iter()
            .find(|candidate| {
                candidate.id.starts_with("accounts/fireworks/routers/")
                    && candidate.id.ends_with("-turbo")
            })
            .expect("Fire Pass turbo router model exists");

        assert_eq!(model.api, "anthropic-messages");
        assert_eq!(model.base_url, "https://api.fireworks.ai/inference");
        assert_eq!(model.input, ["text", "image"]);
    }

    #[test]
    fn aligns_glm_5p2_fast_with_glm_5p2_openai_compatible_config() {
        let base = model("accounts/fireworks/models/glm-5p2");
        let fast = model("accounts/fireworks/routers/glm-5p2-fast");

        assert_eq!(fast.api, base.api);
        assert_eq!(fast.base_url, base.base_url);
        assert_eq!(fast.compat, base.compat);
        assert_eq!(fast.thinking_level_map, base.thinking_level_map);
    }

    #[test]
    fn sets_fireworks_specific_compat_for_session_affinity_and_unsupported_tool_fields() {
        let model = model("accounts/fireworks/models/kimi-k2p6");

        assert_eq!(model.compat.send_session_affinity_headers, Some(true));
        assert_eq!(
            model.compat.supports_eager_tool_input_streaming,
            Some(false)
        );
        assert_eq!(model.compat.supports_cache_control_on_tools, Some(false));
        assert_eq!(model.compat.supports_long_cache_retention, Some(false));
    }
}
