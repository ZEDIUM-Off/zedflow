use regex::Regex;
use zedflow_ai::providers::anthropic_models::ANTHROPIC_MODELS;
use zedflow_ai::providers::cloudflare_ai_gateway_models::CLOUDFLARE_AI_GATEWAY_MODELS;
use zedflow_ai::providers::opencode_models::OPENCODE_MODELS;
use zedflow_ai::providers::vercel_ai_gateway_models::VERCEL_AI_GATEWAY_MODELS;

const EXPECTED_CURRENT_ADAPTIVE_THINKING_MODELS: &[&str] = &[
    "anthropic/claude-fable-5",
    "anthropic/claude-opus-4-8",
    "anthropic/claude-sonnet-5",
    "cloudflare-ai-gateway/claude-fable-5",
    "opencode/claude-opus-4-8",
    "vercel-ai-gateway/anthropic/claude-opus-4.8",
    "vercel-ai-gateway/anthropic/claude-sonnet-5",
];

#[test]
fn marks_built_in_anthropic_messages_models_that_use_adaptive_thinking() {
    let flagged_models = flagged_adaptive_thinking_models();

    for expected in sorted_expected_models() {
        assert!(
            flagged_models.contains(&expected),
            "missing expected adaptive thinking model: {expected}"
        );
    }

    let current_adaptive_model_pattern =
        Regex::new(r"(opus[-.]4[-.][678]|sonnet[-.]4[-.]6|sonnet[-.]5|fable[-.]5)")
            .expect("adaptive thinking model pattern is valid");
    let filtered_models = flagged_models
        .iter()
        .filter(|model_id| current_adaptive_model_pattern.is_match(model_id))
        .cloned()
        .collect::<Vec<_>>();

    assert_eq!(flagged_models, filtered_models);
}

fn sorted_expected_models() -> Vec<String> {
    let mut models = EXPECTED_CURRENT_ADAPTIVE_THINKING_MODELS
        .iter()
        .map(|model| (*model).to_owned())
        .collect::<Vec<_>>();
    models.sort();
    models
}

fn flagged_adaptive_thinking_models() -> Vec<String> {
    let mut models = Vec::new();

    models.extend(
        ANTHROPIC_MODELS
            .iter()
            .filter(|&model| {
                model.api == "anthropic-messages"
                    && model
                        .compat
                        .is_some_and(|compat| compat.force_adaptive_thinking == Some(true))
            })
            .map(|model| format!("{}/{}", model.provider, model.id)),
    );
    models.extend(
        CLOUDFLARE_AI_GATEWAY_MODELS
            .iter()
            .filter(|&model| {
                model.api == "anthropic-messages"
                    && model
                        .compat
                        .is_some_and(|compat| compat.force_adaptive_thinking == Some(true))
            })
            .map(|model| format!("{}/{}", model.provider, model.id)),
    );
    models.extend(
        OPENCODE_MODELS
            .iter()
            .filter(|&model| {
                model.api == "anthropic-messages"
                    && model
                        .compat
                        .is_some_and(|compat| compat.force_adaptive_thinking == Some(true))
            })
            .map(|model| format!("{}/{}", model.provider, model.id)),
    );
    models.extend(
        VERCEL_AI_GATEWAY_MODELS
            .iter()
            .filter(|&model| {
                model.api == "anthropic-messages"
                    && model
                        .compat
                        .is_some_and(|compat| compat.force_adaptive_thinking == Some(true))
            })
            .map(|model| format!("{}/{}", model.provider, model.id)),
    );

    models.sort();
    models
}
