//! Port of Pi `packages/ai/test/xhigh.test.ts`.
//!
//! The source is a live OpenAI reasoning smoke test. Rust compat catalog lookup and live
//! OpenAI stream transports are still `PORT PLACEHOLDER`s, so parity is represented as
//! ignored tests until those seams are ported.

use zedflow_ai::compat;
use zedflow_core::error::Error as CoreError;

fn assert_catalog_blocked(provider: &str, id: &str) {
    let error = compat::get_model(provider, id).expect_err("compat catalog is still a placeholder");
    assert!(matches!(error, CoreError::PortPlaceholder(_)));
}

#[test]
#[ignore = "PORT PLACEHOLDER: live OpenAI Responses xhigh stream parity needs compat catalog and provider transport"]
fn codex_max_supports_xhigh_on_openai_responses() {
    assert_catalog_blocked("openai", "gpt-5.1-codex-max");
}

#[test]
#[ignore = "PORT PLACEHOLDER: live OpenAI Responses xhigh error parity needs compat catalog and provider transport"]
fn gpt_5_mini_errors_with_xhigh_on_openai_responses() {
    assert_catalog_blocked("openai", "gpt-5-mini");
}

#[test]
#[ignore = "PORT PLACEHOLDER: live OpenAI Completions xhigh error parity needs compat catalog and provider transport"]
fn gpt_5_mini_errors_with_xhigh_on_openai_completions() {
    assert_catalog_blocked("openai", "gpt-5-mini");
}
