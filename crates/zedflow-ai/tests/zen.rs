//! Port of Pi `packages/ai/test/zen.test.ts`.
//!
//! The source is a live OpenCode model smoke suite over generated `MODELS`. Rust generated
//! catalogs and live completion dispatch are still documented `PORT PLACEHOLDER`s, so parity is
//! represented as an ignored test.

use zedflow_ai::compat;
use zedflow_core::error::Error as CoreError;

#[test]
#[ignore = "PORT PLACEHOLDER: generated OpenCode catalogs and live complete dispatch are not ported"]
fn opencode_models_smoke_suite_is_blocked_by_generated_catalog_placeholder() {
    let error = compat::get_models().expect_err("builtin model catalog is still a placeholder");
    assert!(matches!(error, CoreError::PortPlaceholder(_)));
}
