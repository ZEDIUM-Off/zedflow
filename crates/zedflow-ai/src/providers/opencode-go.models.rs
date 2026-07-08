//! OpenCode Zen Go model catalog ported from Pi's `packages/ai/src/providers/opencode-go.models.ts`.

/// Pricing metadata for an OpenCode Zen Go model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpenCodeGoModelCost {
    /// Input token price in Pi's model catalog units.
    pub input: f64,
    /// Output token price in Pi's model catalog units.
    pub output: f64,
    /// Cache-read token price in Pi's model catalog units.
    pub cache_read: f64,
    /// Cache-write token price in Pi's model catalog units.
    pub cache_write: f64,
}

/// Compatibility metadata for an OpenCode Zen Go model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenCodeGoModelCompat {
    /// Whether the model supports the OpenAI store option.
    pub supports_store: Option<bool>,
    /// Whether the model supports the developer role.
    pub supports_developer_role: Option<bool>,
    /// OpenAI-compatible max-token field override.
    pub max_tokens_field: Option<&'static str>,
    /// Whether assistant messages must include provider reasoning content.
    pub requires_reasoning_content_on_assistant_messages: Option<bool>,
    /// Pi thinking payload format identifier.
    pub thinking_format: Option<&'static str>,
    /// Whether the model supports OpenAI reasoning effort.
    pub supports_reasoning_effort: Option<bool>,
    /// Whether the model supports long cache retention.
    pub supports_long_cache_retention: Option<bool>,
}

/// A `thinkingLevelMap` entry from Pi's OpenCode Zen Go model catalog.
pub type OpenCodeGoThinkingLevelMap = &'static [(&'static str, Option<&'static str>)];

/// One OpenCode Zen Go model entry from Pi's generated model catalog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpenCodeGoModel {
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
    pub compat: Option<OpenCodeGoModelCompat>,
    /// Whether the model supports reasoning.
    pub reasoning: bool,
    /// Optional Pi thinking-level mapping.
    pub thinking_level_map: Option<OpenCodeGoThinkingLevelMap>,
    /// Supported input modalities.
    pub input: &'static [&'static str],
    /// Model cost metadata.
    pub cost: OpenCodeGoModelCost,
    /// Context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
}

const TEXT: &[&str] = &["text"];
const TEXT_IMAGE: &[&str] = &["text", "image"];
const OPENCODE_GO_BASE_URL: &str = "https://opencode.ai/zen/go/v1";
const OPENCODE_GO_ANTHROPIC_BASE_URL: &str = "https://opencode.ai/zen/go";
const OPENAI_MAX_TOKENS_COMPAT: OpenCodeGoModelCompat = OpenCodeGoModelCompat {
    supports_store: Some(false),
    supports_developer_role: Some(false),
    max_tokens_field: Some("max_tokens"),
    requires_reasoning_content_on_assistant_messages: None,
    thinking_format: None,
    supports_reasoning_effort: None,
    supports_long_cache_retention: None,
};
const DEEPSEEK_COMPAT: OpenCodeGoModelCompat = OpenCodeGoModelCompat {
    supports_store: Some(false),
    supports_developer_role: Some(false),
    max_tokens_field: Some("max_tokens"),
    requires_reasoning_content_on_assistant_messages: Some(true),
    thinking_format: Some("deepseek"),
    supports_reasoning_effort: None,
    supports_long_cache_retention: None,
};
const KIMI_COMPAT: OpenCodeGoModelCompat = OpenCodeGoModelCompat {
    supports_store: Some(false),
    supports_developer_role: Some(false),
    max_tokens_field: Some("max_tokens"),
    requires_reasoning_content_on_assistant_messages: None,
    thinking_format: Some("deepseek"),
    supports_reasoning_effort: Some(false),
    supports_long_cache_retention: Some(false),
};
const QWEN_COMPAT: OpenCodeGoModelCompat = OpenCodeGoModelCompat {
    supports_store: Some(false),
    supports_developer_role: Some(false),
    max_tokens_field: Some("max_tokens"),
    requires_reasoning_content_on_assistant_messages: None,
    thinking_format: Some("qwen"),
    supports_reasoning_effort: None,
    supports_long_cache_retention: None,
};
const DEEPSEEK_THINKING_LEVEL_MAP: OpenCodeGoThinkingLevelMap = &[
    ("minimal", None),
    ("low", None),
    ("medium", None),
    ("high", Some("high")),
    ("xhigh", Some("max")),
];
const GLM_5_2_THINKING_LEVEL_MAP: OpenCodeGoThinkingLevelMap = &[
    ("off", None),
    ("minimal", None),
    ("low", None),
    ("medium", None),
    ("high", Some("high")),
    ("xhigh", Some("max")),
];
const KIMI_THINKING_LEVEL_MAP: OpenCodeGoThinkingLevelMap =
    &[("minimal", None), ("low", None), ("medium", None)];

/// OpenCode Zen Go models keyed by `id`, preserving Pi's generated catalog order and metadata.
pub const OPENCODE_GO_MODELS: &[OpenCodeGoModel] = &[
    OpenCodeGoModel {
        id: "deepseek-v4-flash",
        name: "DeepSeek V4 Flash",
        api: "openai-completions",
        provider: "opencode-go",
        base_url: OPENCODE_GO_BASE_URL,
        compat: Some(DEEPSEEK_COMPAT),
        reasoning: true,
        thinking_level_map: Some(DEEPSEEK_THINKING_LEVEL_MAP),
        input: TEXT,
        cost: OpenCodeGoModelCost {
            input: 0.14,
            output: 0.28,
            cache_read: 0.0028,
            cache_write: 0.0,
        },
        context_window: 1_000_000,
        max_tokens: 384_000,
    },
    OpenCodeGoModel {
        id: "deepseek-v4-pro",
        name: "DeepSeek V4 Pro",
        api: "openai-completions",
        provider: "opencode-go",
        base_url: OPENCODE_GO_BASE_URL,
        compat: Some(DEEPSEEK_COMPAT),
        reasoning: true,
        thinking_level_map: Some(DEEPSEEK_THINKING_LEVEL_MAP),
        input: TEXT,
        cost: OpenCodeGoModelCost {
            input: 1.74,
            output: 3.48,
            cache_read: 0.0145,
            cache_write: 0.0,
        },
        context_window: 1_000_000,
        max_tokens: 384_000,
    },
    OpenCodeGoModel {
        id: "glm-5.1",
        name: "GLM-5.1",
        api: "openai-completions",
        provider: "opencode-go",
        base_url: OPENCODE_GO_BASE_URL,
        compat: Some(OPENAI_MAX_TOKENS_COMPAT),
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: OpenCodeGoModelCost {
            input: 1.4,
            output: 4.4,
            cache_read: 0.26,
            cache_write: 0.0,
        },
        context_window: 202_752,
        max_tokens: 32_768,
    },
    OpenCodeGoModel {
        id: "glm-5.2",
        name: "GLM-5.2",
        api: "openai-completions",
        provider: "opencode-go",
        base_url: OPENCODE_GO_BASE_URL,
        compat: Some(OPENAI_MAX_TOKENS_COMPAT),
        reasoning: true,
        thinking_level_map: Some(GLM_5_2_THINKING_LEVEL_MAP),
        input: TEXT,
        cost: OpenCodeGoModelCost {
            input: 1.4,
            output: 4.4,
            cache_read: 0.26,
            cache_write: 0.0,
        },
        context_window: 1_000_000,
        max_tokens: 131_072,
    },
    OpenCodeGoModel {
        id: "kimi-k2.6",
        name: "Kimi K2.6",
        api: "openai-completions",
        provider: "opencode-go",
        base_url: OPENCODE_GO_BASE_URL,
        compat: Some(KIMI_COMPAT),
        reasoning: true,
        thinking_level_map: Some(KIMI_THINKING_LEVEL_MAP),
        input: TEXT_IMAGE,
        cost: OpenCodeGoModelCost {
            input: 0.95,
            output: 4.0,
            cache_read: 0.16,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 65_536,
    },
    OpenCodeGoModel {
        id: "kimi-k2.7-code",
        name: "Kimi K2.7 Code",
        api: "openai-completions",
        provider: "opencode-go",
        base_url: OPENCODE_GO_BASE_URL,
        compat: Some(OPENAI_MAX_TOKENS_COMPAT),
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: OpenCodeGoModelCost {
            input: 0.95,
            output: 4.0,
            cache_read: 0.19,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 262_144,
    },
    OpenCodeGoModel {
        id: "mimo-v2.5",
        name: "MiMo V2.5",
        api: "openai-completions",
        provider: "opencode-go",
        base_url: OPENCODE_GO_BASE_URL,
        compat: Some(OPENAI_MAX_TOKENS_COMPAT),
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: OpenCodeGoModelCost {
            input: 0.14,
            output: 0.28,
            cache_read: 0.0028,
            cache_write: 0.0,
        },
        context_window: 1_000_000,
        max_tokens: 128_000,
    },
    OpenCodeGoModel {
        id: "mimo-v2.5-pro",
        name: "MiMo V2.5 Pro",
        api: "openai-completions",
        provider: "opencode-go",
        base_url: OPENCODE_GO_BASE_URL,
        compat: Some(OPENAI_MAX_TOKENS_COMPAT),
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: OpenCodeGoModelCost {
            input: 1.74,
            output: 3.48,
            cache_read: 0.0145,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 128_000,
    },
    OpenCodeGoModel {
        id: "minimax-m2.7",
        name: "MiniMax-M2.7",
        api: "openai-completions",
        provider: "opencode-go",
        base_url: OPENCODE_GO_BASE_URL,
        compat: Some(OPENAI_MAX_TOKENS_COMPAT),
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: OpenCodeGoModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.06,
            cache_write: 0.0,
        },
        context_window: 204_800,
        max_tokens: 131_072,
    },
    OpenCodeGoModel {
        id: "minimax-m3",
        name: "MiniMax-M3",
        api: "anthropic-messages",
        provider: "opencode-go",
        base_url: OPENCODE_GO_ANTHROPIC_BASE_URL,
        compat: None,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: OpenCodeGoModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.06,
            cache_write: 0.0,
        },
        context_window: 1_000_000,
        max_tokens: 131_072,
    },
    OpenCodeGoModel {
        id: "qwen3.6-plus",
        name: "Qwen3.6 Plus",
        api: "openai-completions",
        provider: "opencode-go",
        base_url: OPENCODE_GO_BASE_URL,
        compat: Some(QWEN_COMPAT),
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: OpenCodeGoModelCost {
            input: 0.5,
            output: 3.0,
            cache_read: 0.05,
            cache_write: 0.625,
        },
        context_window: 1_000_000,
        max_tokens: 65_536,
    },
    OpenCodeGoModel {
        id: "qwen3.7-max",
        name: "Qwen3.7 Max",
        api: "anthropic-messages",
        provider: "opencode-go",
        base_url: OPENCODE_GO_ANTHROPIC_BASE_URL,
        compat: None,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: OpenCodeGoModelCost {
            input: 2.5,
            output: 7.5,
            cache_read: 0.5,
            cache_write: 3.125,
        },
        context_window: 1_000_000,
        max_tokens: 65_536,
    },
    OpenCodeGoModel {
        id: "qwen3.7-plus",
        name: "Qwen3.7 Plus",
        api: "anthropic-messages",
        provider: "opencode-go",
        base_url: OPENCODE_GO_ANTHROPIC_BASE_URL,
        compat: None,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: OpenCodeGoModelCost {
            input: 0.4,
            output: 1.6,
            cache_read: 0.04,
            cache_write: 0.5,
        },
        context_window: 1_000_000,
        max_tokens: 65_536,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_opencode_go_catalog_metadata() {
        assert_eq!(OPENCODE_GO_MODELS.len(), 13);
        assert_eq!(OPENCODE_GO_MODELS[0].id, "deepseek-v4-flash");
        assert_eq!(
            OPENCODE_GO_MODELS[3].thinking_level_map,
            Some(GLM_5_2_THINKING_LEVEL_MAP)
        );
        assert_eq!(OPENCODE_GO_MODELS[4].compat, Some(KIMI_COMPAT));
        assert_eq!(OPENCODE_GO_MODELS[9].api, "anthropic-messages");
        assert_eq!(OPENCODE_GO_MODELS[12].cost.cache_write, 0.5);
    }
}
