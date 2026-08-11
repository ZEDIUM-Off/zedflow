//! Port of Pi `packages/ai/test/zen.test.ts`.
//!
//! The source is a live OpenCode model smoke suite over generated `MODELS`. The generated catalog
//! is now deterministic locally; live completion dispatch stays ignored because it needs provider
//! network access.

use zedflow_ai::compat;

#[test]
#[ignore = "live OpenCode smoke requires provider network credentials; deterministic catalog coverage is local"]
fn opencode_models_smoke_suite_requires_live_completion_dispatch() {
    let models = compat::get_models();
    assert!(models.iter().any(|model| model.provider == "opencode"));
}
