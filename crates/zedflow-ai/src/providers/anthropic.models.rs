//! Anthropic model catalog ported from Pi's `packages/ai/src/providers/anthropic.models.ts`.

use crate::models::Model;

/// Pricing metadata for an Anthropic model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnthropicModelCost {
    /// Input token price in Pi's model catalog units.
    pub input: f64,
    /// Output token price in Pi's model catalog units.
    pub output: f64,
    /// Cache-read token price in Pi's model catalog units.
    pub cache_read: f64,
    /// Cache-write token price in Pi's model catalog units.
    pub cache_write: f64,
}

/// Compatibility metadata for an Anthropic model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnthropicModelCompat {
    /// Whether Pi forces adaptive thinking for the model.
    pub force_adaptive_thinking: Option<bool>,
    /// Whether the model supports the temperature parameter.
    pub supports_temperature: Option<bool>,
}

/// A `thinkingLevelMap` entry from Pi's Anthropic model catalog.
pub type AnthropicThinkingLevelMap = &'static [(&'static str, Option<&'static str>)];

/// One Anthropic model entry from Pi's generated model catalog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnthropicModel {
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
    pub compat: Option<AnthropicModelCompat>,
    /// Whether the model supports reasoning.
    pub reasoning: bool,
    /// Optional Pi thinking-level mapping.
    pub thinking_level_map: Option<AnthropicThinkingLevelMap>,
    /// Supported input modalities.
    pub input: &'static [&'static str],
    /// Model cost metadata.
    pub cost: AnthropicModelCost,
    /// Context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
}

const TEXT_IMAGE: &[&str] = &["text", "image"];
const THINKING_FABLE_5: AnthropicThinkingLevelMap = &[("off", None), ("xhigh", Some("xhigh"))];
const THINKING_OPUS_4_6: AnthropicThinkingLevelMap = &[("xhigh", Some("max"))];
const THINKING_XHIGH: AnthropicThinkingLevelMap = &[("xhigh", Some("xhigh"))];

/// Anthropic models keyed by `id`, preserving Pi's generated catalog order and metadata.
pub const ANTHROPIC_MODELS: &[AnthropicModel] = &[
    AnthropicModel {
        id: "claude-3-5-sonnet-20240620",
        name: "Claude Sonnet 3.5",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: None,
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 200_000,
        max_tokens: 8_192,
    },
    AnthropicModel {
        id: "claude-3-5-sonnet-20241022",
        name: "Claude Sonnet 3.5 v2",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: None,
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 200_000,
        max_tokens: 8_192,
    },
    AnthropicModel {
        id: "claude-3-7-sonnet-20250219",
        name: "Claude Sonnet 3.7",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: None,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 200_000,
        max_tokens: 64_000,
    },
    AnthropicModel {
        id: "claude-3-haiku-20240307",
        name: "Claude Haiku 3",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: None,
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 0.25,
            output: 1.25,
            cache_read: 0.03,
            cache_write: 0.3,
        },
        context_window: 200_000,
        max_tokens: 4_096,
    },
    AnthropicModel {
        id: "claude-3-opus-20240229",
        name: "Claude Opus 3",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: None,
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 15.0,
            output: 75.0,
            cache_read: 1.5,
            cache_write: 18.75,
        },
        context_window: 200_000,
        max_tokens: 4_096,
    },
    AnthropicModel {
        id: "claude-3-sonnet-20240229",
        name: "Claude Sonnet 3",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: None,
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 0.3,
        },
        context_window: 200_000,
        max_tokens: 4_096,
    },
    AnthropicModel {
        id: "claude-fable-5",
        name: "Claude Fable 5",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: Some(AnthropicModelCompat {
            force_adaptive_thinking: Some(true),
            supports_temperature: None,
        }),
        reasoning: true,
        thinking_level_map: Some(THINKING_FABLE_5),
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 10.0,
            output: 50.0,
            cache_read: 1.0,
            cache_write: 12.5,
        },
        context_window: 1_000_000,
        max_tokens: 128_000,
    },
    AnthropicModel {
        id: "claude-haiku-4-5",
        name: "Claude Haiku 4.5 (latest)",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: None,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 1.0,
            output: 5.0,
            cache_read: 0.1,
            cache_write: 1.25,
        },
        context_window: 200_000,
        max_tokens: 64_000,
    },
    AnthropicModel {
        id: "claude-haiku-4-5-20251001",
        name: "Claude Haiku 4.5",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: None,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 1.0,
            output: 5.0,
            cache_read: 0.1,
            cache_write: 1.25,
        },
        context_window: 200_000,
        max_tokens: 64_000,
    },
    AnthropicModel {
        id: "claude-opus-4-0",
        name: "Claude Opus 4 (latest)",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: None,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 15.0,
            output: 75.0,
            cache_read: 1.5,
            cache_write: 18.75,
        },
        context_window: 200_000,
        max_tokens: 32_000,
    },
    AnthropicModel {
        id: "claude-opus-4-1",
        name: "Claude Opus 4.1 (latest)",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: None,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 15.0,
            output: 75.0,
            cache_read: 1.5,
            cache_write: 18.75,
        },
        context_window: 200_000,
        max_tokens: 32_000,
    },
    AnthropicModel {
        id: "claude-opus-4-1-20250805",
        name: "Claude Opus 4.1",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: None,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 15.0,
            output: 75.0,
            cache_read: 1.5,
            cache_write: 18.75,
        },
        context_window: 200_000,
        max_tokens: 32_000,
    },
    AnthropicModel {
        id: "claude-opus-4-20250514",
        name: "Claude Opus 4",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: None,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 15.0,
            output: 75.0,
            cache_read: 1.5,
            cache_write: 18.75,
        },
        context_window: 200_000,
        max_tokens: 32_000,
    },
    AnthropicModel {
        id: "claude-opus-4-5",
        name: "Claude Opus 4.5 (latest)",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: None,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 200_000,
        max_tokens: 64_000,
    },
    AnthropicModel {
        id: "claude-opus-4-5-20251101",
        name: "Claude Opus 4.5",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: None,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 200_000,
        max_tokens: 64_000,
    },
    AnthropicModel {
        id: "claude-opus-4-6",
        name: "Claude Opus 4.6",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: Some(AnthropicModelCompat {
            force_adaptive_thinking: Some(true),
            supports_temperature: None,
        }),
        reasoning: true,
        thinking_level_map: Some(THINKING_OPUS_4_6),
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 1_000_000,
        max_tokens: 128_000,
    },
    AnthropicModel {
        id: "claude-opus-4-7",
        name: "Claude Opus 4.7",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: Some(AnthropicModelCompat {
            force_adaptive_thinking: Some(true),
            supports_temperature: Some(false),
        }),
        reasoning: true,
        thinking_level_map: Some(THINKING_XHIGH),
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 1_000_000,
        max_tokens: 128_000,
    },
    AnthropicModel {
        id: "claude-opus-4-8",
        name: "Claude Opus 4.8",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: Some(AnthropicModelCompat {
            force_adaptive_thinking: Some(true),
            supports_temperature: Some(false),
        }),
        reasoning: true,
        thinking_level_map: Some(THINKING_XHIGH),
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 1_000_000,
        max_tokens: 128_000,
    },
    AnthropicModel {
        id: "claude-sonnet-4-0",
        name: "Claude Sonnet 4 (latest)",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: None,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 200_000,
        max_tokens: 64_000,
    },
    AnthropicModel {
        id: "claude-sonnet-4-20250514",
        name: "Claude Sonnet 4",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: None,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 200_000,
        max_tokens: 64_000,
    },
    AnthropicModel {
        id: "claude-sonnet-4-5",
        name: "Claude Sonnet 4.5 (latest)",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: None,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 200_000,
        max_tokens: 64_000,
    },
    AnthropicModel {
        id: "claude-sonnet-4-5-20250929",
        name: "Claude Sonnet 4.5",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: None,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 200_000,
        max_tokens: 64_000,
    },
    AnthropicModel {
        id: "claude-sonnet-4-6",
        name: "Claude Sonnet 4.6",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: Some(AnthropicModelCompat {
            force_adaptive_thinking: Some(true),
            supports_temperature: None,
        }),
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 1_000_000,
        max_tokens: 64_000,
    },
    AnthropicModel {
        id: "claude-sonnet-5",
        name: "Claude Sonnet 5",
        api: "anthropic-messages",
        provider: "anthropic",
        base_url: "https://api.anthropic.com",
        compat: Some(AnthropicModelCompat {
            force_adaptive_thinking: Some(true),
            supports_temperature: None,
        }),
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AnthropicModelCost {
            input: 2.0,
            output: 10.0,
            cache_read: 0.2,
            cache_write: 2.5,
        },
        context_window: 1_000_000,
        max_tokens: 128_000,
    },
];

/// Returns the Anthropic catalog as the crate's current minimal runtime model shape.
#[must_use]
pub fn anthropic_models() -> Vec<Model> {
    ANTHROPIC_MODELS
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
    fn preserves_anthropic_catalog_metadata() {
        assert_eq!(ANTHROPIC_MODELS.len(), 24);
        assert_eq!(ANTHROPIC_MODELS[0].id, "claude-3-5-sonnet-20240620");

        let opus = ANTHROPIC_MODELS
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
    }

    #[test]
    fn exposes_current_runtime_model_shape() {
        let models = anthropic_models();
        assert_eq!(models.len(), ANTHROPIC_MODELS.len());
        assert!(models.iter().all(|model| model.provider == "anthropic"));
        assert!(models.iter().all(|model| model.api == "anthropic-messages"));
    }
}
