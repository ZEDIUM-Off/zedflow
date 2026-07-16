//! Port of Pi `packages/ai/test/xhigh.test.ts`.
//!
//! These are live OpenAI transport/error smoke tests. P5 validates local catalog metadata;
//! P7/P8 own capability-gated live provider execution.

use zedflow_ai::compat;

fn assert_catalog_model(provider: &str, id: &str) {
    compat::get_model(provider, id).expect("catalog model should be present");
}

#[test]
#[ignore = "live OpenAI Codex/Responses transport test; requires capability-gated OpenAI Codex credentials"]
fn codex_max_supports_xhigh_on_openai_responses() {
    assert_catalog_model("openai", "gpt-5.1-codex-max");
}

#[test]
#[ignore = "live OpenAI Responses xhigh error test; requires OPENAI_API_KEY and network"]
fn gpt_5_mini_errors_with_xhigh_on_openai_responses() {
    assert_catalog_model("openai", "gpt-5-mini");
}

#[test]
#[ignore = "live OpenAI Completions xhigh error test; requires OPENAI_API_KEY and network"]
fn gpt_5_mini_errors_with_xhigh_on_openai_completions() {
    assert_catalog_model("openai", "gpt-5-mini");
}
