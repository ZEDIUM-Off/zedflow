//! Xiaomi Token Plan AMS model catalog ported from Pi's `packages/ai/src/providers/xiaomi-token-plan-ams.models.ts`.

use crate::models::Model;

/// Pricing metadata for a Xiaomi Token Plan AMS model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XiaomiTokenPlanAmsModelCost {
    /// Input token price in Pi's model catalog units.
    pub input: f64,
    /// Output token price in Pi's model catalog units.
    pub output: f64,
    /// Cache-read token price in Pi's model catalog units.
    pub cache_read: f64,
    /// Cache-write token price in Pi's model catalog units.
    pub cache_write: f64,
}

/// OpenAI-compatible behavior overrides for a Xiaomi Token Plan AMS model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XiaomiTokenPlanAmsModelCompat {
    /// Whether replayed assistant messages require reasoning content.
    pub requires_reasoning_content_on_assistant_messages: bool,
    /// Pi thinking payload format identifier.
    pub thinking_format: &'static str,
}

/// One Xiaomi Token Plan AMS model entry from Pi's generated model catalog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XiaomiTokenPlanAmsModel {
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
    pub compat: XiaomiTokenPlanAmsModelCompat,
    /// Whether the model supports reasoning.
    pub reasoning: bool,
    /// Supported input modalities.
    pub input: &'static [&'static str],
    /// Model cost metadata.
    pub cost: XiaomiTokenPlanAmsModelCost,
    /// Context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
}

const TEXT: &[&str] = &["text"];
const TEXT_IMAGE: &[&str] = &["text", "image"];
const COMPAT: XiaomiTokenPlanAmsModelCompat = XiaomiTokenPlanAmsModelCompat {
    requires_reasoning_content_on_assistant_messages: true,
    thinking_format: "deepseek",
};

/// Xiaomi Token Plan AMS models keyed by `id`, preserving Pi's generated catalog order and metadata.
pub const XIAOMI_TOKEN_PLAN_AMS_MODELS: &[XiaomiTokenPlanAmsModel] = &[
    XiaomiTokenPlanAmsModel {
        id: "mimo-v2-omni",
        name: "MiMo-V2-Omni",
        api: "openai-completions",
        provider: "xiaomi-token-plan-ams",
        base_url: "https://token-plan-ams.xiaomimimo.com/v1",
        compat: COMPAT,
        reasoning: true,
        input: TEXT_IMAGE,
        cost: XiaomiTokenPlanAmsModelCost {
            input: 0.14,
            output: 0.28,
            cache_read: 0.0028,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 131_072,
    },
    XiaomiTokenPlanAmsModel {
        id: "mimo-v2-pro",
        name: "MiMo-V2-Pro",
        api: "openai-completions",
        provider: "xiaomi-token-plan-ams",
        base_url: "https://token-plan-ams.xiaomimimo.com/v1",
        compat: COMPAT,
        reasoning: true,
        input: TEXT,
        cost: XiaomiTokenPlanAmsModelCost {
            input: 0.435,
            output: 0.87,
            cache_read: 0.0036,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 131_072,
    },
    XiaomiTokenPlanAmsModel {
        id: "mimo-v2.5",
        name: "MiMo-V2.5",
        api: "openai-completions",
        provider: "xiaomi-token-plan-ams",
        base_url: "https://token-plan-ams.xiaomimimo.com/v1",
        compat: COMPAT,
        reasoning: true,
        input: TEXT_IMAGE,
        cost: XiaomiTokenPlanAmsModelCost {
            input: 0.14,
            output: 0.28,
            cache_read: 0.0028,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 131_072,
    },
    XiaomiTokenPlanAmsModel {
        id: "mimo-v2.5-pro",
        name: "MiMo-V2.5-Pro",
        api: "openai-completions",
        provider: "xiaomi-token-plan-ams",
        base_url: "https://token-plan-ams.xiaomimimo.com/v1",
        compat: COMPAT,
        reasoning: true,
        input: TEXT,
        cost: XiaomiTokenPlanAmsModelCost {
            input: 0.435,
            output: 0.87,
            cache_read: 0.0036,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 131_072,
    },
    XiaomiTokenPlanAmsModel {
        id: "mimo-v2.5-pro-ultraspeed",
        name: "MiMo-V2.5-Pro-UltraSpeed",
        api: "openai-completions",
        provider: "xiaomi-token-plan-ams",
        base_url: "https://token-plan-ams.xiaomimimo.com/v1",
        compat: COMPAT,
        reasoning: true,
        input: TEXT,
        cost: XiaomiTokenPlanAmsModelCost {
            input: 1.305,
            output: 2.61,
            cache_read: 0.0108,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 131_072,
    },
];

/// Returns the Xiaomi Token Plan AMS catalog as the crate's current minimal runtime model shape.
#[must_use]
pub fn xiaomi_token_plan_ams_models() -> Vec<Model> {
    XIAOMI_TOKEN_PLAN_AMS_MODELS
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
    fn preserves_xiaomi_token_plan_ams_catalog_metadata() {
        assert_eq!(XIAOMI_TOKEN_PLAN_AMS_MODELS.len(), 5);
        assert_eq!(XIAOMI_TOKEN_PLAN_AMS_MODELS[0].id, "mimo-v2-omni");
        assert_eq!(XIAOMI_TOKEN_PLAN_AMS_MODELS[0].input, TEXT_IMAGE);
        assert_eq!(XIAOMI_TOKEN_PLAN_AMS_MODELS[0].context_window, 262_144);
        assert_eq!(XIAOMI_TOKEN_PLAN_AMS_MODELS[1].cost.input, 0.435);
        assert_eq!(XIAOMI_TOKEN_PLAN_AMS_MODELS[4].cost.output, 2.61);
        assert_eq!(XIAOMI_TOKEN_PLAN_AMS_MODELS[4].max_tokens, 131_072);
        assert!(
            XIAOMI_TOKEN_PLAN_AMS_MODELS[4]
                .compat
                .requires_reasoning_content_on_assistant_messages
        );
        assert_eq!(
            XIAOMI_TOKEN_PLAN_AMS_MODELS[4].compat.thinking_format,
            "deepseek"
        );
    }

    #[test]
    fn exposes_current_runtime_model_shape() {
        let models = xiaomi_token_plan_ams_models();
        assert_eq!(models.len(), XIAOMI_TOKEN_PLAN_AMS_MODELS.len());
        assert!(
            models
                .iter()
                .all(|model| model.provider == "xiaomi-token-plan-ams")
        );
        assert!(models.iter().all(|model| model.api == "openai-completions"));
    }
}
