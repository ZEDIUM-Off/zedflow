//! Cerebras model catalog ported from Pi's `packages/ai/src/providers/cerebras.models.ts`.

/// Pricing metadata for a Cerebras model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CerebrasModelCost {
    /// Input token price in Pi's model catalog units.
    pub input: f64,
    /// Output token price in Pi's model catalog units.
    pub output: f64,
    /// Cache-read token price in Pi's model catalog units.
    pub cache_read: f64,
    /// Cache-write token price in Pi's model catalog units.
    pub cache_write: f64,
}

/// Compatibility metadata for a Cerebras model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CerebrasModelCompat {
    /// Whether the model supports the OpenAI store option.
    pub supports_store: bool,
    /// Whether the model supports the developer role.
    pub supports_developer_role: bool,
}

/// One Cerebras model entry from Pi's generated model catalog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CerebrasModel {
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
    pub compat: CerebrasModelCompat,
    /// Whether the model supports reasoning.
    pub reasoning: bool,
    /// Supported input modalities.
    pub input: &'static [&'static str],
    /// Model cost metadata.
    pub cost: CerebrasModelCost,
    /// Context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
}

const TEXT: &[&str] = &["text"];
const TEXT_IMAGE: &[&str] = &["text", "image"];
const CEREBRAS_COMPAT: CerebrasModelCompat = CerebrasModelCompat {
    supports_store: false,
    supports_developer_role: false,
};

/// Cerebras models keyed by `id`, preserving Pi's generated catalog order and metadata.
pub const CEREBRAS_MODELS: &[CerebrasModel] = &[
    CerebrasModel {
        id: "gemma-4-31b",
        name: "Gemma 4 31B IT",
        api: "openai-completions",
        provider: "cerebras",
        base_url: "https://api.cerebras.ai/v1",
        compat: CEREBRAS_COMPAT,
        reasoning: true,
        input: TEXT_IMAGE,
        cost: CerebrasModelCost {
            input: 0.99,
            output: 1.49,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072,
        max_tokens: 40_960,
    },
    CerebrasModel {
        id: "gpt-oss-120b",
        name: "GPT OSS 120B",
        api: "openai-completions",
        provider: "cerebras",
        base_url: "https://api.cerebras.ai/v1",
        compat: CEREBRAS_COMPAT,
        reasoning: true,
        input: TEXT,
        cost: CerebrasModelCost {
            input: 0.35,
            output: 0.75,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072,
        max_tokens: 40_960,
    },
    CerebrasModel {
        id: "zai-glm-4.7",
        name: "Z.AI GLM-4.7",
        api: "openai-completions",
        provider: "cerebras",
        base_url: "https://api.cerebras.ai/v1",
        compat: CEREBRAS_COMPAT,
        reasoning: true,
        input: TEXT,
        cost: CerebrasModelCost {
            input: 2.25,
            output: 2.75,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072,
        max_tokens: 40_960,
    },
];
