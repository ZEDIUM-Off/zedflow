//! Z.AI model catalog ported from Pi's `packages/ai/src/providers/zai.models.ts`.

use crate::models::Model;

/// Pricing metadata for a Z.AI model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZaiModelCost {
    /// Input token price in Pi's model catalog units.
    pub input: f64,
    /// Output token price in Pi's model catalog units.
    pub output: f64,
    /// Cache-read token price in Pi's model catalog units.
    pub cache_read: f64,
    /// Cache-write token price in Pi's model catalog units.
    pub cache_write: f64,
}

/// Compatibility metadata for a Z.AI model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZaiModelCompat {
    /// Whether the model supports the OpenAI store option.
    pub supports_store: bool,
    /// Whether the model supports the developer role.
    pub supports_developer_role: bool,
    /// Whether the model supports reasoning effort.
    pub supports_reasoning_effort: bool,
    /// Pi thinking payload format identifier.
    pub thinking_format: &'static str,
    /// Whether Pi enables Z.AI tool stream handling for this model.
    pub zai_tool_stream: Option<bool>,
}

/// A `thinkingLevelMap` entry from Pi's Z.AI model catalog.
pub type ZaiThinkingLevelMap = &'static [(&'static str, Option<&'static str>)];

/// One Z.AI model entry from Pi's generated model catalog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZaiModel {
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
    pub compat: ZaiModelCompat,
    /// Whether the model supports reasoning.
    pub reasoning: bool,
    /// Optional Pi thinking-level mapping.
    pub thinking_level_map: Option<ZaiThinkingLevelMap>,
    /// Supported input modalities.
    pub input: &'static [&'static str],
    /// Model cost metadata.
    pub cost: ZaiModelCost,
    /// Context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
}

const BASE_URL: &str = "https://api.z.ai/api/coding/paas/v4";
const TEXT: &[&str] = &["text"];
const TEXT_IMAGE: &[&str] = &["text", "image"];
const COMPAT_NO_TOOL_STREAM: ZaiModelCompat = ZaiModelCompat {
    supports_store: false,
    supports_developer_role: false,
    supports_reasoning_effort: false,
    thinking_format: "zai",
    zai_tool_stream: None,
};
const COMPAT_TOOL_STREAM: ZaiModelCompat = ZaiModelCompat {
    supports_store: false,
    supports_developer_role: false,
    supports_reasoning_effort: false,
    thinking_format: "zai",
    zai_tool_stream: Some(true),
};
const COMPAT_REASONING_EFFORT: ZaiModelCompat = ZaiModelCompat {
    supports_store: false,
    supports_developer_role: false,
    supports_reasoning_effort: true,
    thinking_format: "zai",
    zai_tool_stream: Some(true),
};
const GLM_5_2_THINKING_LEVEL_MAP: ZaiThinkingLevelMap = &[
    ("minimal", None),
    ("low", Some("high")),
    ("medium", Some("high")),
    ("high", Some("high")),
    ("xhigh", Some("max")),
];

/// Z.AI models keyed by `id`, preserving Pi's generated catalog order and metadata.
pub const ZAI_MODELS: &[ZaiModel] = &[
    ZaiModel {
        id: "glm-4.5-air",
        name: "GLM-4.5-Air",
        api: "openai-completions",
        provider: "zai",
        base_url: BASE_URL,
        compat: COMPAT_NO_TOOL_STREAM,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: ZaiModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072,
        max_tokens: 98_304,
    },
    ZaiModel {
        id: "glm-4.7",
        name: "GLM-4.7",
        api: "openai-completions",
        provider: "zai",
        base_url: BASE_URL,
        compat: COMPAT_TOOL_STREAM,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: ZaiModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 204_800,
        max_tokens: 131_072,
    },
    ZaiModel {
        id: "glm-5-turbo",
        name: "GLM-5-Turbo",
        api: "openai-completions",
        provider: "zai",
        base_url: BASE_URL,
        compat: COMPAT_TOOL_STREAM,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: ZaiModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 200_000,
        max_tokens: 131_072,
    },
    ZaiModel {
        id: "glm-5.1",
        name: "GLM-5.1",
        api: "openai-completions",
        provider: "zai",
        base_url: BASE_URL,
        compat: COMPAT_TOOL_STREAM,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: ZaiModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 200_000,
        max_tokens: 131_072,
    },
    ZaiModel {
        id: "glm-5.2",
        name: "GLM-5.2",
        api: "openai-completions",
        provider: "zai",
        base_url: BASE_URL,
        compat: COMPAT_REASONING_EFFORT,
        reasoning: true,
        thinking_level_map: Some(GLM_5_2_THINKING_LEVEL_MAP),
        input: TEXT,
        cost: ZaiModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_000_000,
        max_tokens: 131_072,
    },
    ZaiModel {
        id: "glm-5v-turbo",
        name: "GLM-5V-Turbo",
        api: "openai-completions",
        provider: "zai",
        base_url: BASE_URL,
        compat: COMPAT_TOOL_STREAM,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: ZaiModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 200_000,
        max_tokens: 131_072,
    },
];

/// Returns the Z.AI catalog as the crate's current minimal runtime model shape.
#[must_use]
pub fn zai_models() -> Vec<Model> {
    ZAI_MODELS
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
    fn preserves_zai_catalog_metadata() {
        assert_eq!(ZAI_MODELS.len(), 6);
        assert_eq!(ZAI_MODELS[0].id, "glm-4.5-air");
        assert_eq!(ZAI_MODELS[0].base_url, BASE_URL);
        assert_eq!(ZAI_MODELS[0].compat.zai_tool_stream, None);
        assert_eq!(ZAI_MODELS[1].compat.zai_tool_stream, Some(true));
        assert_eq!(ZAI_MODELS[4].context_window, 1_000_000);
        assert_eq!(
            ZAI_MODELS[4].thinking_level_map,
            Some(GLM_5_2_THINKING_LEVEL_MAP),
        );
        assert_eq!(ZAI_MODELS[5].input, TEXT_IMAGE);
    }

    #[test]
    fn exposes_current_runtime_model_shape() {
        let models = zai_models();
        assert_eq!(models.len(), ZAI_MODELS.len());
        assert!(models.iter().all(|model| model.provider == "zai"));
        assert!(models.iter().all(|model| model.api == "openai-completions"));
    }
}
