//! Kimi Coding model catalog ported from Pi's `packages/ai/src/providers/kimi-coding.models.ts`.

use crate::models::Model;

/// Pricing metadata for a Kimi Coding model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KimiCodingModelCost {
    /// Input token price in Pi's model catalog units.
    pub input: f64,
    /// Output token price in Pi's model catalog units.
    pub output: f64,
    /// Cache-read token price in Pi's model catalog units.
    pub cache_read: f64,
    /// Cache-write token price in Pi's model catalog units.
    pub cache_write: f64,
}

/// Request headers from Pi's Kimi Coding model catalog.
pub type KimiCodingHeaders = &'static [(&'static str, &'static str)];

/// One Kimi Coding model entry from Pi's generated model catalog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KimiCodingModel {
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
    /// Request headers.
    pub headers: KimiCodingHeaders,
    /// Whether the model supports reasoning.
    pub reasoning: bool,
    /// Supported input modalities.
    pub input: &'static [&'static str],
    /// Model cost metadata.
    pub cost: KimiCodingModelCost,
    /// Context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
}

const TEXT: &[&str] = &["text"];
const TEXT_IMAGE: &[&str] = &["text", "image"];
const KIMI_CODING_HEADERS: KimiCodingHeaders = &[("User-Agent", "KimiCLI/1.5")];

/// Kimi Coding models keyed by `id`, preserving Pi's generated catalog order and metadata.
pub const KIMI_CODING_MODELS: &[KimiCodingModel] = &[
    KimiCodingModel {
        id: "k2p7",
        name: "Kimi K2.7 Code",
        api: "anthropic-messages",
        provider: "kimi-coding",
        base_url: "https://api.kimi.com/coding",
        headers: KIMI_CODING_HEADERS,
        reasoning: true,
        input: TEXT_IMAGE,
        cost: KimiCodingModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 32_768,
    },
    KimiCodingModel {
        id: "kimi-for-coding",
        name: "Kimi For Coding",
        api: "anthropic-messages",
        provider: "kimi-coding",
        base_url: "https://api.kimi.com/coding",
        headers: KIMI_CODING_HEADERS,
        reasoning: true,
        input: TEXT_IMAGE,
        cost: KimiCodingModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 32_768,
    },
    KimiCodingModel {
        id: "kimi-k2-thinking",
        name: "Kimi K2 Thinking",
        api: "anthropic-messages",
        provider: "kimi-coding",
        base_url: "https://api.kimi.com/coding",
        headers: KIMI_CODING_HEADERS,
        reasoning: true,
        input: TEXT,
        cost: KimiCodingModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 32_768,
    },
];

/// Returns the Kimi Coding catalog as the crate's current minimal runtime model shape.
#[must_use]
pub fn kimi_coding_models() -> Vec<Model> {
    KIMI_CODING_MODELS
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
    fn preserves_kimi_coding_catalog_metadata() {
        assert_eq!(KIMI_CODING_MODELS.len(), 3);
        assert_eq!(KIMI_CODING_MODELS[0].id, "k2p7");
        assert_eq!(KIMI_CODING_MODELS[0].headers, KIMI_CODING_HEADERS);
        assert_eq!(KIMI_CODING_MODELS[1].input, TEXT_IMAGE);
        assert_eq!(KIMI_CODING_MODELS[2].input, TEXT);
        assert_eq!(KIMI_CODING_MODELS[2].context_window, 262_144);
        assert_eq!(KIMI_CODING_MODELS[2].max_tokens, 32_768);
    }

    #[test]
    fn exposes_current_runtime_model_shape() {
        let models = kimi_coding_models();
        assert_eq!(models.len(), KIMI_CODING_MODELS.len());
        assert!(models.iter().all(|model| model.provider == "kimi-coding"));
        assert!(models.iter().all(|model| model.api == "anthropic-messages"));
    }
}
