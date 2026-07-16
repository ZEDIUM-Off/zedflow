//! Port of Pi `packages/ai/test/together-models.test.ts`.

use zedflow_ai::env_api_keys::{ProviderEnv, find_env_keys, get_env_api_key};
use zedflow_ai::providers::together_models::{
    TOGETHER_MODELS, TogetherModel, TogetherModelCompat, TogetherModelCost,
};

fn together_model(id: &str) -> &'static TogetherModel {
    TOGETHER_MODELS
        .iter()
        .find(|model| model.id == id)
        .unwrap_or_else(|| panic!("missing Together model: {id}"))
}

#[test]
fn registers_the_default_kimi_k2_6_model_via_openai_compatible_chat_completions_api() {
    let model = together_model("moonshotai/Kimi-K2.6");

    assert_eq!(model.api, "openai-completions");
    assert_eq!(model.provider, "together");
    assert_eq!(model.base_url, "https://api.together.ai/v1");
    assert!(model.reasoning);
    assert_eq!(
        model.thinking_level_map,
        Some(&[("minimal", None), ("low", None), ("medium", None)][..])
    );
    assert_eq!(model.input, ["text", "image"]);
    assert_eq!(model.context_window, 262_144);
    assert_eq!(model.max_tokens, 131_000);
    assert_eq!(
        model.cost,
        TogetherModelCost {
            input: 1.2,
            output: 4.5,
            cache_read: 0.2,
            cache_write: 0.0,
        }
    );
    assert_eq!(
        model.compat,
        TogetherModelCompat {
            supports_store: false,
            supports_developer_role: false,
            supports_reasoning_effort: false,
            max_tokens_field: "max_tokens",
            thinking_format: Some("together"),
            supports_strict_mode: false,
            supports_long_cache_retention: false,
        }
    );
}

#[test]
fn models_together_reasoning_controls_from_the_together_api_surface() {
    let gpt_oss = together_model("openai/gpt-oss-120b");
    assert_eq!(
        gpt_oss.thinking_level_map,
        Some(&[("off", None), ("minimal", None)][..])
    );
    assert!(gpt_oss.compat.supports_reasoning_effort);
    assert_eq!(gpt_oss.compat.thinking_format, Some("openai"));

    let deep_seek_v4 = together_model("deepseek-ai/DeepSeek-V4-Pro");
    assert_eq!(
        deep_seek_v4.thinking_level_map,
        Some(
            &[
                ("minimal", None),
                ("low", None),
                ("medium", None),
                ("high", Some("high")),
                ("xhigh", None),
            ][..]
        )
    );
    assert!(deep_seek_v4.compat.supports_reasoning_effort);
    assert_eq!(deep_seek_v4.compat.thinking_format, Some("together"));

    let minimax = together_model("MiniMaxAI/MiniMax-M2.7");
    assert_eq!(
        minimax.thinking_level_map,
        Some(
            &[
                ("off", None),
                ("minimal", None),
                ("low", None),
                ("medium", None)
            ][..]
        )
    );
    assert_eq!(minimax.compat.thinking_format, None);
    assert!(!minimax.compat.supports_reasoning_effort);
}

#[test]
fn resolves_together_api_key_from_the_environment() {
    let env = ProviderEnv::from([(
        "TOGETHER_API_KEY".to_owned(),
        "test-together-key".to_owned(),
    )]);

    assert_eq!(
        find_env_keys("together", Some(&env)),
        Some(vec!["TOGETHER_API_KEY"])
    );
    assert_eq!(
        get_env_api_key("together", Some(&env)),
        Some("test-together-key".to_owned())
    );
}
