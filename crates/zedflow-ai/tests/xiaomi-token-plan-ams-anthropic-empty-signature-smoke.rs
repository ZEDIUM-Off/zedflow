//! Port of Pi `packages/ai/test/xiaomi-token-plan-ams-anthropic-empty-signature-smoke.test.ts`.
//!
//! The source is a live Xiaomi Token Plan Anthropic smoke test. The live replay remains
//! capability-gated; this deterministic catalog regression verifies its selected model exists.

use zedflow_ai::compat;

#[test]
fn registers_xiaomi_token_plan_model_for_anthropic_replay() {
    let model = compat::get_model("xiaomi-token-plan-ams", "mimo-v2.5-pro")
        .expect("Xiaomi Token Plan model is in the builtin catalog");
    assert_eq!(model.api, "openai-completions");
}
