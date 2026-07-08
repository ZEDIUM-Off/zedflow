//! Port of Pi `packages/ai/test/xiaomi-models.test.ts`.
//!
//! The source checks generated Xiaomi provider catalogs. The Rust builtin catalog remains a
//! documented `PORT PLACEHOLDER`, so these parity assertions stay ignored.

use zedflow_ai::compat;
use zedflow_core::error::Error as CoreError;

fn assert_catalog_blocked() {
    let error = compat::get_models().expect_err("builtin model catalog is still a placeholder");
    assert!(matches!(error, CoreError::PortPlaceholder(_)));
}

#[test]
#[ignore = "PORT PLACEHOLDER: generated builtin Xiaomi model catalog is not ported"]
fn keeps_mimo_v2_flash_on_api_billing_provider() {
    let error = compat::get_model("xiaomi", "mimo-v2-flash")
        .expect_err("builtin model catalog is still a placeholder");
    assert!(matches!(error, CoreError::PortPlaceholder(_)));
}

#[test]
#[ignore = "PORT PLACEHOLDER: generated builtin Xiaomi token-plan catalogs are not ported"]
fn omits_mimo_v2_flash_from_token_plan_providers() {
    assert_catalog_blocked();
}
