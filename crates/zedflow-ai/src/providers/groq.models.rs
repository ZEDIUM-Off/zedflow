//! Groq model catalog ported from Pi's `packages/ai/src/providers/groq.models.ts`.

/// Pricing metadata for a Groq model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroqModelCost {
    /// Input token price in Pi's model catalog units.
    pub input: f64,
    /// Output token price in Pi's model catalog units.
    pub output: f64,
    /// Cache-read token price in Pi's model catalog units.
    pub cache_read: f64,
    /// Cache-write token price in Pi's model catalog units.
    pub cache_write: f64,
}

/// A `thinkingLevelMap` entry from Pi's Groq model catalog.
pub type GroqThinkingLevelMap = &'static [(&'static str, Option<&'static str>)];

/// One Groq model entry from Pi's generated model catalog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroqModel {
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
    /// Whether the model supports reasoning.
    pub reasoning: bool,
    /// Optional Pi thinking-level mapping.
    pub thinking_level_map: Option<GroqThinkingLevelMap>,
    /// Supported input modalities.
    pub input: &'static [&'static str],
    /// Model cost metadata.
    pub cost: GroqModelCost,
    /// Context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
}

const TEXT: &[&str] = &["text"];
const TEXT_IMAGE: &[&str] = &["text", "image"];
const QWEN3_32B_THINKING_LEVEL_MAP: GroqThinkingLevelMap = &[
    ("minimal", None),
    ("low", None),
    ("medium", None),
    ("high", Some("default")),
];

/// Groq models keyed by `id`, preserving Pi's generated catalog order and metadata.
pub const GROQ_MODELS: &[GroqModel] = &[
    GroqModel {
        id: "llama-3.1-8b-instant",
        name: "Llama 3.1 8B",
        api: "openai-completions",
        provider: "groq",
        base_url: "https://api.groq.com/openai/v1",
        reasoning: false,
        thinking_level_map: None,
        input: TEXT,
        cost: GroqModelCost {
            input: 0.05,
            output: 0.08,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072,
        max_tokens: 131_072,
    },
    GroqModel {
        id: "llama-3.3-70b-versatile",
        name: "Llama 3.3 70B",
        api: "openai-completions",
        provider: "groq",
        base_url: "https://api.groq.com/openai/v1",
        reasoning: false,
        thinking_level_map: None,
        input: TEXT,
        cost: GroqModelCost {
            input: 0.59,
            output: 0.79,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072,
        max_tokens: 32_768,
    },
    GroqModel {
        id: "meta-llama/llama-4-scout-17b-16e-instruct",
        name: "Llama 4 Scout 17B 16E",
        api: "openai-completions",
        provider: "groq",
        base_url: "https://api.groq.com/openai/v1",
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: GroqModelCost {
            input: 0.11,
            output: 0.34,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072,
        max_tokens: 8_192,
    },
    GroqModel {
        id: "openai/gpt-oss-120b",
        name: "GPT OSS 120B",
        api: "openai-completions",
        provider: "groq",
        base_url: "https://api.groq.com/openai/v1",
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: GroqModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.075,
            cache_write: 0.0,
        },
        context_window: 131_072,
        max_tokens: 65_536,
    },
    GroqModel {
        id: "openai/gpt-oss-20b",
        name: "GPT OSS 20B",
        api: "openai-completions",
        provider: "groq",
        base_url: "https://api.groq.com/openai/v1",
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: GroqModelCost {
            input: 0.075,
            output: 0.3,
            cache_read: 0.0375,
            cache_write: 0.0,
        },
        context_window: 131_072,
        max_tokens: 65_536,
    },
    GroqModel {
        id: "openai/gpt-oss-safeguard-20b",
        name: "Safety GPT OSS 20B",
        api: "openai-completions",
        provider: "groq",
        base_url: "https://api.groq.com/openai/v1",
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: GroqModelCost {
            input: 0.075,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072,
        max_tokens: 65_536,
    },
    GroqModel {
        id: "qwen/qwen3-32b",
        name: "Qwen3-32B",
        api: "openai-completions",
        provider: "groq",
        base_url: "https://api.groq.com/openai/v1",
        reasoning: true,
        thinking_level_map: Some(QWEN3_32B_THINKING_LEVEL_MAP),
        input: TEXT,
        cost: GroqModelCost {
            input: 0.29,
            output: 0.59,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072,
        max_tokens: 40_960,
    },
];
