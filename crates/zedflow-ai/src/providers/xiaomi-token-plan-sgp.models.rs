//! Xiaomi Token Plan SGP model catalog ported from Pi's `packages/ai/src/providers/xiaomi-token-plan-sgp.models.ts`.

use crate::models::Model;

/// Pricing metadata for a Xiaomi Token Plan SGP model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XiaomiTokenPlanSgpModelCost {
    /// Input token price in Pi's model catalog units.
    pub input: f64,
    /// Output token price in Pi's model catalog units.
    pub output: f64,
    /// Cache-read token price in Pi's model catalog units.
    pub cache_read: f64,
    /// Cache-write token price in Pi's model catalog units.
    pub cache_write: f64,
}

/// OpenAI-compatible behavior overrides for a Xiaomi Token Plan SGP model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XiaomiTokenPlanSgpModelCompat {
    /// Whether replayed assistant messages require reasoning content.
    pub requires_reasoning_content_on_assistant_messages: bool,
    /// Pi thinking payload format identifier.
    pub thinking_format: &'static str,
}

/// One Xiaomi Token Plan SGP model entry from Pi's generated model catalog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XiaomiTokenPlanSgpModel {
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
    pub compat: XiaomiTokenPlanSgpModelCompat,
    /// Whether the model supports reasoning.
    pub reasoning: bool,
    /// Supported input modalities.
    pub input: &'static [&'static str],
    /// Model cost metadata.
    pub cost: XiaomiTokenPlanSgpModelCost,
    /// Context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
}

const TEXT: &[&str] = &["text"];
const TEXT_IMAGE: &[&str] = &["text", "image"];
const COMPAT: XiaomiTokenPlanSgpModelCompat = XiaomiTokenPlanSgpModelCompat {
    requires_reasoning_content_on_assistant_messages: true,
    thinking_format: "deepseek",
};

/// Xiaomi Token Plan SGP models keyed by `id`, preserving Pi's generated catalog order and metadata.
pub const XIAOMI_TOKEN_PLAN_SGP_MODELS: &[XiaomiTokenPlanSgpModel] = &[
    XiaomiTokenPlanSgpModel {
        id: "mimo-v2-omni",
        name: "MiMo-V2-Omni",
        api: "openai-completions",
        provider: "xiaomi-token-plan-sgp",
        base_url: "https://token-plan-sgp.xiaomimimo.com/v1",
        compat: COMPAT,
        reasoning: true,
        input: TEXT_IMAGE,
        cost: XiaomiTokenPlanSgpModelCost {
            input: 0.14,
            output: 0.28,
            cache_read: 0.0028,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 131_072,
    },
    XiaomiTokenPlanSgpModel {
        id: "mimo-v2-pro",
        name: "MiMo-V2-Pro",
        api: "openai-completions",
        provider: "xiaomi-token-plan-sgp",
        base_url: "https://token-plan-sgp.xiaomimimo.com/v1",
        compat: COMPAT,
        reasoning: true,
        input: TEXT,
        cost: XiaomiTokenPlanSgpModelCost {
            input: 0.435,
            output: 0.87,
            cache_read: 0.0036,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 131_072,
    },
    XiaomiTokenPlanSgpModel {
        id: "mimo-v2.5",
        name: "MiMo-V2.5",
        api: "openai-completions",
        provider: "xiaomi-token-plan-sgp",
        base_url: "https://token-plan-sgp.xiaomimimo.com/v1",
        compat: COMPAT,
        reasoning: true,
        input: TEXT_IMAGE,
        cost: XiaomiTokenPlanSgpModelCost {
            input: 0.14,
            output: 0.28,
            cache_read: 0.0028,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 131_072,
    },
    XiaomiTokenPlanSgpModel {
        id: "mimo-v2.5-pro",
        name: "MiMo-V2.5-Pro",
        api: "openai-completions",
        provider: "xiaomi-token-plan-sgp",
        base_url: "https://token-plan-sgp.xiaomimimo.com/v1",
        compat: COMPAT,
        reasoning: true,
        input: TEXT,
        cost: XiaomiTokenPlanSgpModelCost {
            input: 0.435,
            output: 0.87,
            cache_read: 0.0036,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 131_072,
    },
    XiaomiTokenPlanSgpModel {
        id: "mimo-v2.5-pro-ultraspeed",
        name: "MiMo-V2.5-Pro-UltraSpeed",
        api: "openai-completions",
        provider: "xiaomi-token-plan-sgp",
        base_url: "https://token-plan-sgp.xiaomimimo.com/v1",
        compat: COMPAT,
        reasoning: true,
        input: TEXT,
        cost: XiaomiTokenPlanSgpModelCost {
            input: 1.305,
            output: 2.61,
            cache_read: 0.0108,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 131_072,
    },
];

/// Returns the Xiaomi Token Plan SGP catalog as the crate's current minimal runtime model shape.
#[must_use]
pub fn xiaomi_token_plan_sgp_models() -> Vec<Model> {
    XIAOMI_TOKEN_PLAN_SGP_MODELS
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
    fn preserves_xiaomi_token_plan_sgp_catalog_metadata() {
        assert_eq!(XIAOMI_TOKEN_PLAN_SGP_MODELS.len(), 5);
        assert_eq!(XIAOMI_TOKEN_PLAN_SGP_MODELS[0].id, "mimo-v2-omni");
        assert_eq!(XIAOMI_TOKEN_PLAN_SGP_MODELS[0].input, TEXT_IMAGE);
        assert_eq!(XIAOMI_TOKEN_PLAN_SGP_MODELS[0].context_window, 262_144);
        assert_eq!(XIAOMI_TOKEN_PLAN_SGP_MODELS[1].cost.input, 0.435);
        assert_eq!(XIAOMI_TOKEN_PLAN_SGP_MODELS[4].cost.output, 2.61);
        assert_eq!(XIAOMI_TOKEN_PLAN_SGP_MODELS[4].max_tokens, 131_072);
        assert!(
            XIAOMI_TOKEN_PLAN_SGP_MODELS[4]
                .compat
                .requires_reasoning_content_on_assistant_messages
        );
        assert_eq!(
            XIAOMI_TOKEN_PLAN_SGP_MODELS[4].compat.thinking_format,
            "deepseek"
        );
    }

    #[test]
    fn exposes_current_runtime_model_shape() {
        let models = xiaomi_token_plan_sgp_models();
        assert_eq!(models.len(), XIAOMI_TOKEN_PLAN_SGP_MODELS.len());
        assert!(
            models
                .iter()
                .all(|model| model.provider == "xiaomi-token-plan-sgp")
        );
        assert!(models.iter().all(|model| model.api == "openai-completions"));
    }
}
