//! DeepSeek model catalog ported from Pi's `packages/ai/src/providers/deepseek.models.ts`.

use crate::models::Model;

/// Pricing metadata for a DeepSeek model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeepSeekModelCost {
    /// Input token price in Pi's model catalog units.
    pub input: f64,
    /// Output token price in Pi's model catalog units.
    pub output: f64,
    /// Cache-read token price in Pi's model catalog units.
    pub cache_read: f64,
    /// Cache-write token price in Pi's model catalog units.
    pub cache_write: f64,
}

/// Compatibility metadata for a DeepSeek model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeepSeekModelCompat {
    /// Whether the model supports the OpenAI store option.
    pub supports_store: bool,
    /// Whether the model supports the developer role.
    pub supports_developer_role: bool,
    /// Whether assistant messages must include DeepSeek reasoning content.
    pub requires_reasoning_content_on_assistant_messages: bool,
    /// Pi thinking payload format identifier.
    pub thinking_format: &'static str,
}

/// A `thinkingLevelMap` entry from Pi's DeepSeek model catalog.
pub type DeepSeekThinkingLevelMap = &'static [(&'static str, Option<&'static str>)];

/// One DeepSeek model entry from Pi's generated model catalog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeepSeekModel {
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
    pub compat: DeepSeekModelCompat,
    /// Whether the model supports reasoning.
    pub reasoning: bool,
    /// Pi thinking-level mapping.
    pub thinking_level_map: DeepSeekThinkingLevelMap,
    /// Supported input modalities.
    pub input: &'static [&'static str],
    /// Model cost metadata.
    pub cost: DeepSeekModelCost,
    /// Context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
}

const TEXT: &[&str] = &["text"];
const DEEPSEEK_COMPAT: DeepSeekModelCompat = DeepSeekModelCompat {
    supports_store: false,
    supports_developer_role: false,
    requires_reasoning_content_on_assistant_messages: true,
    thinking_format: "deepseek",
};
const DEEPSEEK_THINKING_LEVEL_MAP: DeepSeekThinkingLevelMap = &[
    ("minimal", None),
    ("low", None),
    ("medium", None),
    ("high", Some("high")),
    ("xhigh", Some("max")),
];

/// DeepSeek models keyed by `id`, preserving Pi's generated catalog order and metadata.
pub const DEEPSEEK_MODELS: &[DeepSeekModel] = &[
    DeepSeekModel {
        id: "deepseek-v4-flash",
        name: "DeepSeek V4 Flash",
        api: "openai-completions",
        provider: "deepseek",
        base_url: "https://api.deepseek.com",
        compat: DEEPSEEK_COMPAT,
        reasoning: true,
        thinking_level_map: DEEPSEEK_THINKING_LEVEL_MAP,
        input: TEXT,
        cost: DeepSeekModelCost {
            input: 0.14,
            output: 0.28,
            cache_read: 0.0028,
            cache_write: 0.0,
        },
        context_window: 1_000_000,
        max_tokens: 384_000,
    },
    DeepSeekModel {
        id: "deepseek-v4-pro",
        name: "DeepSeek V4 Pro",
        api: "openai-completions",
        provider: "deepseek",
        base_url: "https://api.deepseek.com",
        compat: DEEPSEEK_COMPAT,
        reasoning: true,
        thinking_level_map: DEEPSEEK_THINKING_LEVEL_MAP,
        input: TEXT,
        cost: DeepSeekModelCost {
            input: 0.435,
            output: 0.87,
            cache_read: 0.003625,
            cache_write: 0.0,
        },
        context_window: 1_000_000,
        max_tokens: 384_000,
    },
];

/// Returns the DeepSeek catalog as the crate's current minimal runtime model shape.
#[must_use]
pub fn deepseek_models() -> Vec<Model> {
    DEEPSEEK_MODELS
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
    fn preserves_deepseek_catalog_metadata() {
        assert_eq!(DEEPSEEK_MODELS.len(), 2);
        assert_eq!(DEEPSEEK_MODELS[0].id, "deepseek-v4-flash");
        assert_eq!(DEEPSEEK_MODELS[1].cost.input, 0.435);
        assert_eq!(DEEPSEEK_MODELS[1].cost.cache_read, 0.003625);
        assert_eq!(DEEPSEEK_MODELS[1].max_tokens, 384_000);
        assert_eq!(
            DEEPSEEK_MODELS[1].thinking_level_map,
            DEEPSEEK_THINKING_LEVEL_MAP,
        );
        assert_eq!(DEEPSEEK_MODELS[1].compat.thinking_format, "deepseek");
    }

    #[test]
    fn exposes_current_runtime_model_shape() {
        let models = deepseek_models();
        assert_eq!(models.len(), DEEPSEEK_MODELS.len());
        assert!(models.iter().all(|model| model.provider == "deepseek"));
        assert!(models.iter().all(|model| model.api == "openai-completions"));
    }
}
