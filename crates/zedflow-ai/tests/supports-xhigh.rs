//! Port of Pi `packages/ai/test/supports-xhigh.test.ts`.

use zedflow_ai::compat::{get_model, get_supported_thinking_levels};
use zedflow_ai::types::Model;

fn model(provider: &str, id: &str) -> Model {
    get_model(provider, id).expect("model should exist in built-in catalog")
}

#[test]
fn includes_xhigh_for_anthropic_opus_4_6_on_anthropic_messages_api() {
    assert!(
        get_supported_thinking_levels(&model("anthropic", "claude-opus-4-6")).contains(&"xhigh")
    );
}

#[test]
fn includes_xhigh_for_anthropic_opus_4_8_on_anthropic_messages_api() {
    assert!(
        get_supported_thinking_levels(&model("anthropic", "claude-opus-4-8")).contains(&"xhigh")
    );
}

#[test]
fn includes_xhigh_for_anthropic_opus_4_8_on_anthropic_messages_api_duplicate() {
    assert!(
        get_supported_thinking_levels(&model("anthropic", "claude-opus-4-8")).contains(&"xhigh")
    );
}

#[test]
fn includes_xhigh_but_not_off_for_anthropic_claude_fable_5_on_anthropic_messages_api() {
    let levels = get_supported_thinking_levels(&model("anthropic", "claude-fable-5"));
    assert!(levels.contains(&"xhigh"));
    assert!(!levels.contains(&"off"));
}

#[test]
fn does_not_include_xhigh_for_claude_sonnet_4_5() {
    assert!(
        !get_supported_thinking_levels(&model("anthropic", "claude-sonnet-4-5")).contains(&"xhigh")
    );
}

#[test]
fn includes_xhigh_for_gpt_5_4_models() {
    assert!(get_supported_thinking_levels(&model("openai-codex", "gpt-5.4")).contains(&"xhigh"));
}

#[test]
fn includes_xhigh_for_gpt_5_5_models() {
    assert!(get_supported_thinking_levels(&model("openai-codex", "gpt-5.5")).contains(&"xhigh"));
}

#[test]
fn includes_only_medium_high_xhigh_for_openai_gpt_5_5_pro() {
    assert_eq!(
        get_supported_thinking_levels(&model("openai", "gpt-5.5-pro")),
        vec!["medium", "high", "xhigh"]
    );
}

#[test]
fn includes_only_medium_high_xhigh_for_openrouter_gpt_5_5_pro() {
    assert_eq!(
        get_supported_thinking_levels(&model("openrouter", "openai/gpt-5.5-pro")),
        vec!["medium", "high", "xhigh"]
    );
}

#[test]
fn includes_only_high_xhigh_plus_off_for_deepseek_v4_flash_on_the_deepseek_provider() {
    assert_eq!(
        get_supported_thinking_levels(&model("deepseek", "deepseek-v4-flash")),
        vec!["off", "high", "xhigh"]
    );
}

#[test]
fn includes_only_high_xhigh_plus_off_for_deepseek_v4_flash_on_opencode_go() {
    assert_eq!(
        get_supported_thinking_levels(&model("opencode-go", "deepseek-v4-flash")),
        vec!["off", "high", "xhigh"]
    );
}

#[test]
fn includes_only_high_plus_off_for_opencode_go_kimi_k2_6() {
    assert_eq!(
        get_supported_thinking_levels(&model("opencode-go", "kimi-k2.6")),
        vec!["off", "high"]
    );
}

#[test]
fn excludes_thinking_off_for_moonshot_kimi_k2_7_code_models() {
    for model in [
        model("moonshotai", "kimi-k2.7-code"),
        model("moonshotai-cn", "kimi-k2.7-code"),
    ] {
        assert_eq!(
            get_supported_thinking_levels(&model),
            vec!["minimal", "low", "medium", "high"]
        );
    }
}

#[test]
fn includes_only_high_for_opencode_grok_build() {
    assert_eq!(
        get_supported_thinking_levels(&model("opencode", "grok-build-0.1")),
        vec!["high"]
    );
}

#[test]
fn includes_only_high_xhigh_plus_off_for_deepseek_v4_flash_on_openrouter() {
    assert_eq!(
        get_supported_thinking_levels(&model("openrouter", "deepseek/deepseek-v4-flash")),
        vec!["off", "high", "xhigh"]
    );
}

#[test]
fn includes_xhigh_for_openrouter_opus_4_6_openai_completions_api() {
    assert!(
        get_supported_thinking_levels(&model("openrouter", "anthropic/claude-opus-4.6"))
            .contains(&"xhigh")
    );
}

#[test]
fn includes_xhigh_but_not_off_for_bedrock_claude_fable_5() {
    let levels =
        get_supported_thinking_levels(&model("amazon-bedrock", "global.anthropic.claude-fable-5"));
    assert!(levels.contains(&"xhigh"));
    assert!(!levels.contains(&"off"));
}
