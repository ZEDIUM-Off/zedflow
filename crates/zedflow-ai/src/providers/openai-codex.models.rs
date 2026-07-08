//! OpenAI Codex model catalog ported from Pi's `packages/ai/src/providers/openai-codex.models.ts`.

/// Pricing metadata for an OpenAI Codex model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpenAICodexModelCost {
    /// Input token price in Pi's model catalog units.
    pub input: f64,
    /// Output token price in Pi's model catalog units.
    pub output: f64,
    /// Cache-read token price in Pi's model catalog units.
    pub cache_read: f64,
    /// Cache-write token price in Pi's model catalog units.
    pub cache_write: f64,
}

/// A `thinkingLevelMap` entry from Pi's OpenAI Codex model catalog.
pub type OpenAICodexThinkingLevelMap = &'static [(&'static str, &'static str)];

/// One OpenAI Codex model entry from Pi's generated model catalog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpenAICodexModel {
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
    /// Pi thinking-level mapping.
    pub thinking_level_map: OpenAICodexThinkingLevelMap,
    /// Supported input modalities.
    pub input: &'static [&'static str],
    /// Model cost metadata.
    pub cost: OpenAICodexModelCost,
    /// Context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
}

const TEXT: &[&str] = &["text"];
const TEXT_IMAGE: &[&str] = &["text", "image"];
const THINKING_XHIGH_MINIMAL: OpenAICodexThinkingLevelMap =
    &[("xhigh", "xhigh"), ("minimal", "low")];

/// OpenAI Codex models keyed by `id`, preserving Pi's generated catalog order and metadata.
pub const OPENAI_CODEX_MODELS: &[OpenAICodexModel] = &[
    OpenAICodexModel {
        id: "gpt-5.3-codex-spark",
        name: "GPT-5.3 Codex Spark",
        api: "openai-codex-responses",
        provider: "openai-codex",
        base_url: "https://chatgpt.com/backend-api",
        reasoning: true,
        thinking_level_map: THINKING_XHIGH_MINIMAL,
        input: TEXT,
        cost: OpenAICodexModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 128_000,
    },
    OpenAICodexModel {
        id: "gpt-5.4",
        name: "GPT-5.4",
        api: "openai-codex-responses",
        provider: "openai-codex",
        base_url: "https://chatgpt.com/backend-api",
        reasoning: true,
        thinking_level_map: THINKING_XHIGH_MINIMAL,
        input: TEXT_IMAGE,
        cost: OpenAICodexModelCost {
            input: 2.5,
            output: 15.0,
            cache_read: 0.25,
            cache_write: 0.0,
        },
        context_window: 272_000,
        max_tokens: 128_000,
    },
    OpenAICodexModel {
        id: "gpt-5.4-mini",
        name: "GPT-5.4 mini",
        api: "openai-codex-responses",
        provider: "openai-codex",
        base_url: "https://chatgpt.com/backend-api",
        reasoning: true,
        thinking_level_map: THINKING_XHIGH_MINIMAL,
        input: TEXT_IMAGE,
        cost: OpenAICodexModelCost {
            input: 0.75,
            output: 4.5,
            cache_read: 0.075,
            cache_write: 0.0,
        },
        context_window: 272_000,
        max_tokens: 128_000,
    },
    OpenAICodexModel {
        id: "gpt-5.5",
        name: "GPT-5.5",
        api: "openai-codex-responses",
        provider: "openai-codex",
        base_url: "https://chatgpt.com/backend-api",
        reasoning: true,
        thinking_level_map: THINKING_XHIGH_MINIMAL,
        input: TEXT_IMAGE,
        cost: OpenAICodexModelCost {
            input: 5.0,
            output: 30.0,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        context_window: 272_000,
        max_tokens: 128_000,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_openai_codex_catalog_metadata() {
        assert_eq!(OPENAI_CODEX_MODELS.len(), 4);
        assert_eq!(OPENAI_CODEX_MODELS[0].id, "gpt-5.3-codex-spark");
        assert_eq!(OPENAI_CODEX_MODELS[0].input, TEXT);
        assert_eq!(OPENAI_CODEX_MODELS[1].input, TEXT_IMAGE);
        assert_eq!(OPENAI_CODEX_MODELS[2].cost.input, 0.75);
        assert_eq!(OPENAI_CODEX_MODELS[3].cost.output, 30.0);
        assert_eq!(OPENAI_CODEX_MODELS[3].context_window, 272_000);
        assert_eq!(OPENAI_CODEX_MODELS[3].max_tokens, 128_000);
        assert_eq!(
            OPENAI_CODEX_MODELS[3].thinking_level_map,
            THINKING_XHIGH_MINIMAL
        );
    }
}
