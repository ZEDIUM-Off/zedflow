//! Moonshot AI CN model catalog ported from Pi's `packages/ai/src/providers/moonshotai-cn.models.ts`.

use crate::models::Model;

/// Pricing metadata for a Moonshot AI CN model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoonshotAiCnModelCost {
    /// Input token price in Pi's model catalog units.
    pub input: f64,
    /// Output token price in Pi's model catalog units.
    pub output: f64,
    /// Cache-read token price in Pi's model catalog units.
    pub cache_read: f64,
    /// Cache-write token price in Pi's model catalog units.
    pub cache_write: f64,
}

/// Compatibility metadata for a Moonshot AI CN model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoonshotAiCnModelCompat {
    /// Whether the model supports the OpenAI store option.
    pub supports_store: bool,
    /// Whether the model supports the developer role.
    pub supports_developer_role: bool,
    /// Whether the model supports OpenAI reasoning effort.
    pub supports_reasoning_effort: bool,
    /// Token limit field name used by the OpenAI-compatible API.
    pub max_tokens_field: &'static str,
    /// Whether the model supports strict structured-output mode.
    pub supports_strict_mode: bool,
    /// Pi thinking payload format identifier.
    pub thinking_format: &'static str,
}

/// A `thinkingLevelMap` entry from Pi's Moonshot AI CN model catalog.
pub type MoonshotAiCnThinkingLevelMap = &'static [(&'static str, Option<&'static str>)];

/// One Moonshot AI CN model entry from Pi's generated model catalog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoonshotAiCnModel {
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
    pub compat: MoonshotAiCnModelCompat,
    /// Whether the model supports reasoning.
    pub reasoning: bool,
    /// Optional Pi thinking-level mapping.
    pub thinking_level_map: Option<MoonshotAiCnThinkingLevelMap>,
    /// Supported input modalities.
    pub input: &'static [&'static str],
    /// Model cost metadata.
    pub cost: MoonshotAiCnModelCost,
    /// Context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
}

const TEXT: &[&str] = &["text"];
const TEXT_IMAGE: &[&str] = &["text", "image"];
const MOONSHOTAI_CN_COMPAT: MoonshotAiCnModelCompat = MoonshotAiCnModelCompat {
    supports_store: false,
    supports_developer_role: false,
    supports_reasoning_effort: false,
    max_tokens_field: "max_tokens",
    supports_strict_mode: false,
    thinking_format: "deepseek",
};
const THINKING_OFF: MoonshotAiCnThinkingLevelMap = &[("off", None)];

/// Moonshot AI CN models keyed by `id`, preserving Pi's generated catalog order and metadata.
pub const MOONSHOTAI_CN_MODELS: &[MoonshotAiCnModel] = &[
    MoonshotAiCnModel {
        id: "kimi-k2-0711-preview",
        name: "Kimi K2 0711",
        api: "openai-completions",
        provider: "moonshotai-cn",
        base_url: "https://api.moonshot.cn/v1",
        compat: MOONSHOTAI_CN_COMPAT,
        reasoning: false,
        thinking_level_map: None,
        input: TEXT,
        cost: MoonshotAiCnModelCost {
            input: 0.6,
            output: 2.5,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 131_072,
        max_tokens: 16_384,
    },
    MoonshotAiCnModel {
        id: "kimi-k2-0905-preview",
        name: "Kimi K2 0905",
        api: "openai-completions",
        provider: "moonshotai-cn",
        base_url: "https://api.moonshot.cn/v1",
        compat: MOONSHOTAI_CN_COMPAT,
        reasoning: false,
        thinking_level_map: None,
        input: TEXT,
        cost: MoonshotAiCnModelCost {
            input: 0.6,
            output: 2.5,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 262_144,
    },
    MoonshotAiCnModel {
        id: "kimi-k2-thinking",
        name: "Kimi K2 Thinking",
        api: "openai-completions",
        provider: "moonshotai-cn",
        base_url: "https://api.moonshot.cn/v1",
        compat: MOONSHOTAI_CN_COMPAT,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: MoonshotAiCnModelCost {
            input: 0.6,
            output: 2.5,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 262_144,
    },
    MoonshotAiCnModel {
        id: "kimi-k2-thinking-turbo",
        name: "Kimi K2 Thinking Turbo",
        api: "openai-completions",
        provider: "moonshotai-cn",
        base_url: "https://api.moonshot.cn/v1",
        compat: MOONSHOTAI_CN_COMPAT,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: MoonshotAiCnModelCost {
            input: 1.15,
            output: 8.0,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 262_144,
    },
    MoonshotAiCnModel {
        id: "kimi-k2-turbo-preview",
        name: "Kimi K2 Turbo",
        api: "openai-completions",
        provider: "moonshotai-cn",
        base_url: "https://api.moonshot.cn/v1",
        compat: MOONSHOTAI_CN_COMPAT,
        reasoning: false,
        thinking_level_map: None,
        input: TEXT,
        cost: MoonshotAiCnModelCost {
            input: 2.4,
            output: 10.0,
            cache_read: 0.6,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 262_144,
    },
    MoonshotAiCnModel {
        id: "kimi-k2.5",
        name: "Kimi K2.5",
        api: "openai-completions",
        provider: "moonshotai-cn",
        base_url: "https://api.moonshot.cn/v1",
        compat: MOONSHOTAI_CN_COMPAT,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: MoonshotAiCnModelCost {
            input: 0.6,
            output: 3.0,
            cache_read: 0.1,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 262_144,
    },
    MoonshotAiCnModel {
        id: "kimi-k2.6",
        name: "Kimi K2.6",
        api: "openai-completions",
        provider: "moonshotai-cn",
        base_url: "https://api.moonshot.cn/v1",
        compat: MOONSHOTAI_CN_COMPAT,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: MoonshotAiCnModelCost {
            input: 0.95,
            output: 4.0,
            cache_read: 0.16,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 262_144,
    },
    MoonshotAiCnModel {
        id: "kimi-k2.7-code",
        name: "Kimi K2.7 Code",
        api: "openai-completions",
        provider: "moonshotai-cn",
        base_url: "https://api.moonshot.cn/v1",
        compat: MOONSHOTAI_CN_COMPAT,
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF),
        input: TEXT_IMAGE,
        cost: MoonshotAiCnModelCost {
            input: 0.95,
            output: 4.0,
            cache_read: 0.19,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 262_144,
    },
    MoonshotAiCnModel {
        id: "kimi-k2.7-code-highspeed",
        name: "Kimi K2.7 Code HighSpeed",
        api: "openai-completions",
        provider: "moonshotai-cn",
        base_url: "https://api.moonshot.cn/v1",
        compat: MOONSHOTAI_CN_COMPAT,
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF),
        input: TEXT_IMAGE,
        cost: MoonshotAiCnModelCost {
            input: 1.9,
            output: 8.0,
            cache_read: 0.38,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 262_144,
    },
];

/// Returns the Moonshot AI CN catalog as the crate's current minimal runtime model shape.
#[must_use]
pub fn moonshotai_cn_models() -> Vec<Model> {
    MOONSHOTAI_CN_MODELS
        .iter()
        .map(|model| Model {
            provider: model.provider.to_owned(),
            id: model.id.to_owned(),
            api: model.api.to_owned(),
            ..Model::default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_moonshotai_cn_catalog_metadata() {
        assert_eq!(MOONSHOTAI_CN_MODELS.len(), 9);
        assert_eq!(MOONSHOTAI_CN_MODELS[0].id, "kimi-k2-0711-preview");
        assert_eq!(MOONSHOTAI_CN_MODELS[0].context_window, 131_072);
        assert_eq!(MOONSHOTAI_CN_MODELS[4].cost.output, 10.0);
        assert_eq!(MOONSHOTAI_CN_MODELS[5].input, TEXT_IMAGE);
        assert_eq!(
            MOONSHOTAI_CN_MODELS[7].thinking_level_map,
            Some(THINKING_OFF)
        );
        assert_eq!(MOONSHOTAI_CN_MODELS[8].cost.cache_read, 0.38);
        assert_eq!(MOONSHOTAI_CN_MODELS[8].max_tokens, 262_144);
    }

    #[test]
    fn exposes_current_runtime_model_shape() {
        let models = moonshotai_cn_models();
        assert_eq!(models.len(), MOONSHOTAI_CN_MODELS.len());
        assert!(models.iter().all(|model| model.provider == "moonshotai-cn"));
        assert!(models.iter().all(|model| model.api == "openai-completions"));
    }
}
