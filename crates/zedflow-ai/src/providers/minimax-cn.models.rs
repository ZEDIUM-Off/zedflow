//! MiniMax CN model catalog ported from Pi's `packages/ai/src/providers/minimax-cn.models.ts`.

use crate::models::Model;

/// Pricing metadata for a MiniMax CN model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinimaxCnModelCost {
    /// Input token price in Pi's model catalog units.
    pub input: f64,
    /// Output token price in Pi's model catalog units.
    pub output: f64,
    /// Cache-read token price in Pi's model catalog units.
    pub cache_read: f64,
    /// Cache-write token price in Pi's model catalog units.
    pub cache_write: f64,
}

/// One MiniMax CN model entry from Pi's generated model catalog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinimaxCnModel {
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
    /// Supported input modalities.
    pub input: &'static [&'static str],
    /// Model cost metadata.
    pub cost: MinimaxCnModelCost,
    /// Context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
}

const TEXT: &[&str] = &["text"];
const TEXT_IMAGE: &[&str] = &["text", "image"];

/// MiniMax CN models keyed by `id`, preserving Pi's generated catalog order and metadata.
pub const MINIMAX_CN_MODELS: &[MinimaxCnModel] = &[
    MinimaxCnModel {
        id: "MiniMax-M2.7",
        name: "MiniMax-M2.7",
        api: "anthropic-messages",
        provider: "minimax-cn",
        base_url: "https://api.minimaxi.com/anthropic",
        reasoning: true,
        input: TEXT,
        cost: MinimaxCnModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.06,
            cache_write: 0.375,
        },
        context_window: 204_800,
        max_tokens: 131_072,
    },
    MinimaxCnModel {
        id: "MiniMax-M2.7-highspeed",
        name: "MiniMax-M2.7-highspeed",
        api: "anthropic-messages",
        provider: "minimax-cn",
        base_url: "https://api.minimaxi.com/anthropic",
        reasoning: true,
        input: TEXT,
        cost: MinimaxCnModelCost {
            input: 0.6,
            output: 2.4,
            cache_read: 0.06,
            cache_write: 0.375,
        },
        context_window: 204_800,
        max_tokens: 131_072,
    },
    MinimaxCnModel {
        id: "MiniMax-M3",
        name: "MiniMax-M3",
        api: "anthropic-messages",
        provider: "minimax-cn",
        base_url: "https://api.minimaxi.com/anthropic",
        reasoning: true,
        input: TEXT_IMAGE,
        cost: MinimaxCnModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.06,
            cache_write: 0.0,
        },
        context_window: 1_000_000,
        max_tokens: 128_000,
    },
];

/// Returns the MiniMax CN catalog as the crate's current minimal runtime model shape.
#[must_use]
pub fn minimax_cn_models() -> Vec<Model> {
    MINIMAX_CN_MODELS
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
    fn preserves_minimax_cn_catalog_metadata() {
        assert_eq!(MINIMAX_CN_MODELS.len(), 3);
        assert_eq!(MINIMAX_CN_MODELS[0].id, "MiniMax-M2.7");
        assert_eq!(MINIMAX_CN_MODELS[0].input, TEXT);
        assert_eq!(MINIMAX_CN_MODELS[1].cost.input, 0.6);
        assert_eq!(MINIMAX_CN_MODELS[2].input, TEXT_IMAGE);
        assert_eq!(MINIMAX_CN_MODELS[2].context_window, 1_000_000);
        assert_eq!(MINIMAX_CN_MODELS[2].max_tokens, 128_000);
    }

    #[test]
    fn exposes_current_runtime_model_shape() {
        let models = minimax_cn_models();
        assert_eq!(models.len(), MINIMAX_CN_MODELS.len());
        assert!(models.iter().all(|model| model.provider == "minimax-cn"));
        assert!(models.iter().all(|model| model.api == "anthropic-messages"));
    }
}
