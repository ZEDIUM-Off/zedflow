//! Port of Pi `packages/ai/test/xiaomi-models.test.ts`.

use zedflow_ai::compat;

#[test]
fn keeps_mimo_v2_flash_on_api_billing_provider() {
    let model = compat::get_model("xiaomi", "mimo-v2-flash").expect("xiaomi model is registered");

    assert_eq!(model.id, "mimo-v2-flash");
    assert_eq!(model.api, "openai-completions");
    assert_eq!(model.provider, "xiaomi");
}

#[test]
fn omits_mimo_v2_flash_from_token_plan_providers() {
    let models = compat::get_models().expect("builtin model catalog is registered");

    for provider in [
        "xiaomi-token-plan-ams",
        "xiaomi-token-plan-cn",
        "xiaomi-token-plan-sgp",
    ] {
        assert!(
            !models
                .iter()
                .any(|model| model.provider == provider && model.id == "mimo-v2-flash"),
            "{provider} should not include mimo-v2-flash"
        );
    }
}
