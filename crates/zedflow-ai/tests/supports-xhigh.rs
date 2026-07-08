//! Port of Pi `packages/ai/test/supports-xhigh.test.ts`.
//!
//! The source assertions depend on `compat::getModel` reading the built-in catalog and on
//! `getSupportedThinkingLevels` from `models.ts`. In the Rust port, compat catalog reads are still
//! documented PORT PLACEHOLDERs and the full `Model` reasoning metadata surface is not available.

use zedflow_ai::api::lazy::Model;
use zedflow_ai::compat::get_model;

const BLOCKER: &str = "PORT PLACEHOLDER: compat getModel catalog reads and models::getSupportedThinkingLevels reasoning metadata are not ported";

fn model(provider: &str, id: &str) -> Option<Model> {
    match get_model(provider, id) {
        Ok(model) => Some(model),
        Err(error) => panic!("{BLOCKER}: {error}"),
    }
}

fn get_supported_thinking_levels(_model: &Model) -> Vec<&'static str> {
    panic!("{BLOCKER}")
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat catalog reads and supported thinking level metadata are not ported"]
fn includes_xhigh_for_anthropic_opus_4_6_on_anthropic_messages_api() {
    let model = model("anthropic", "claude-opus-4-6");
    assert!(model.is_some());
    let model = model.expect("model should be defined");
    assert!(get_supported_thinking_levels(&model).contains(&"xhigh"));
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat catalog reads and supported thinking level metadata are not ported"]
fn includes_xhigh_for_anthropic_opus_4_8_on_anthropic_messages_api() {
    let model = model("anthropic", "claude-opus-4-8");
    assert!(model.is_some());
    let model = model.expect("model should be defined");
    assert!(get_supported_thinking_levels(&model).contains(&"xhigh"));
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat catalog reads and supported thinking level metadata are not ported"]
fn includes_xhigh_for_anthropic_opus_4_8_on_anthropic_messages_api_duplicate() {
    let model = model("anthropic", "claude-opus-4-8");
    assert!(model.is_some());
    let model = model.expect("model should be defined");
    assert!(get_supported_thinking_levels(&model).contains(&"xhigh"));
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat catalog reads and supported thinking level metadata are not ported"]
fn includes_xhigh_but_not_off_for_anthropic_claude_fable_5_on_anthropic_messages_api() {
    let model = model("anthropic", "claude-fable-5");
    assert!(model.is_some());
    let model = model.expect("model should be defined");
    let levels = get_supported_thinking_levels(&model);
    assert!(levels.contains(&"xhigh"));
    assert!(!levels.contains(&"off"));
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat catalog reads and supported thinking level metadata are not ported"]
fn does_not_include_xhigh_for_claude_sonnet_4_5() {
    let model = model("anthropic", "claude-sonnet-4-5");
    assert!(model.is_some());
    let model = model.expect("model should be defined");
    assert!(!get_supported_thinking_levels(&model).contains(&"xhigh"));
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat catalog reads and supported thinking level metadata are not ported"]
fn includes_xhigh_for_gpt_5_4_models() {
    let model = model("openai-codex", "gpt-5.4");
    assert!(model.is_some());
    let model = model.expect("model should be defined");
    assert!(get_supported_thinking_levels(&model).contains(&"xhigh"));
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat catalog reads and supported thinking level metadata are not ported"]
fn includes_xhigh_for_gpt_5_5_models() {
    let model = model("openai-codex", "gpt-5.5");
    assert!(model.is_some());
    let model = model.expect("model should be defined");
    assert!(get_supported_thinking_levels(&model).contains(&"xhigh"));
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat catalog reads and supported thinking level metadata are not ported"]
fn includes_only_medium_high_xhigh_for_openai_gpt_5_5_pro() {
    let model = model("openai", "gpt-5.5-pro");
    assert!(model.is_some());
    let model = model.expect("model should be defined");
    assert_eq!(
        get_supported_thinking_levels(&model),
        vec!["medium", "high", "xhigh"]
    );
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat catalog reads and supported thinking level metadata are not ported"]
fn includes_only_medium_high_xhigh_for_openrouter_gpt_5_5_pro() {
    let model = model("openrouter", "openai/gpt-5.5-pro");
    assert!(model.is_some());
    let model = model.expect("model should be defined");
    assert_eq!(
        get_supported_thinking_levels(&model),
        vec!["medium", "high", "xhigh"]
    );
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat catalog reads and supported thinking level metadata are not ported"]
fn includes_only_high_xhigh_plus_off_for_deepseek_v4_flash_on_the_deepseek_provider() {
    let model = model("deepseek", "deepseek-v4-flash");
    assert!(model.is_some());
    let model = model.expect("model should be defined");
    assert_eq!(
        get_supported_thinking_levels(&model),
        vec!["off", "high", "xhigh"]
    );
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat catalog reads and supported thinking level metadata are not ported"]
fn includes_only_high_xhigh_plus_off_for_deepseek_v4_flash_on_opencode_go() {
    let model = model("opencode-go", "deepseek-v4-flash");
    assert!(model.is_some());
    let model = model.expect("model should be defined");
    assert_eq!(
        get_supported_thinking_levels(&model),
        vec!["off", "high", "xhigh"]
    );
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat catalog reads and supported thinking level metadata are not ported"]
fn includes_only_high_plus_off_for_opencode_go_kimi_k2_6() {
    let model = model("opencode-go", "kimi-k2.6");
    assert!(model.is_some());
    let model = model.expect("model should be defined");
    assert_eq!(get_supported_thinking_levels(&model), vec!["off", "high"]);
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat catalog reads and supported thinking level metadata are not ported"]
fn excludes_thinking_off_for_moonshot_kimi_k2_7_code_models() {
    let cases = [
        model("moonshotai", "kimi-k2.7-code"),
        model("moonshotai-cn", "kimi-k2.7-code"),
    ];

    for model in cases {
        assert!(model.is_some());
        let model = model.expect("model should be defined");
        assert_eq!(
            get_supported_thinking_levels(&model),
            vec!["minimal", "low", "medium", "high"]
        );
    }
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat catalog reads and supported thinking level metadata are not ported"]
fn includes_only_high_for_opencode_grok_build() {
    let model = model("opencode", "grok-build-0.1");
    assert!(model.is_some());
    let model = model.expect("model should be defined");
    assert_eq!(get_supported_thinking_levels(&model), vec!["high"]);
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat catalog reads and supported thinking level metadata are not ported"]
fn includes_only_high_xhigh_plus_off_for_deepseek_v4_flash_on_openrouter() {
    let model = model("openrouter", "deepseek/deepseek-v4-flash");
    assert!(model.is_some());
    let model = model.expect("model should be defined");
    assert_eq!(
        get_supported_thinking_levels(&model),
        vec!["off", "high", "xhigh"]
    );
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat catalog reads and supported thinking level metadata are not ported"]
fn includes_xhigh_for_openrouter_opus_4_6_openai_completions_api() {
    let model = model("openrouter", "anthropic/claude-opus-4.6");
    assert!(model.is_some());
    let model = model.expect("model should be defined");
    assert!(get_supported_thinking_levels(&model).contains(&"xhigh"));
}

#[test]
#[ignore = "PORT PLACEHOLDER: compat catalog reads and supported thinking level metadata are not ported"]
fn includes_xhigh_but_not_off_for_bedrock_claude_fable_5() {
    let model = model("amazon-bedrock", "global.anthropic.claude-fable-5");
    assert!(model.is_some());
    let model = model.expect("model should be defined");
    let levels = get_supported_thinking_levels(&model);
    assert!(levels.contains(&"xhigh"));
    assert!(!levels.contains(&"off"));
}
