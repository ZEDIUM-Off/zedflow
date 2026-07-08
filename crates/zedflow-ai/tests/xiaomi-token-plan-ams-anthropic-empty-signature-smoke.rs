//! Port of Pi `packages/ai/test/xiaomi-token-plan-ams-anthropic-empty-signature-smoke.test.ts`.
//!
//! The source is a live Xiaomi Token Plan Anthropic smoke test. Rust `completeSimple`,
//! `streamSimple`, env auth, and Anthropic replay payload capture are still provider transport
//! placeholders, so the live parity test is ignored per P1.T2/RF3.

use zedflow_ai::compat;
use zedflow_core::error::Error as CoreError;

#[test]
#[ignore = "PORT PLACEHOLDER: live Xiaomi Token Plan Anthropic transport and replay payload capture are not ported"]
fn preserves_empty_thinking_signature_for_replay() {
    let error = compat::get_model("xiaomi-token-plan-ams", "mimo-v2.5-pro")
        .expect_err("builtin model catalog is still a placeholder");
    assert!(matches!(error, CoreError::PortPlaceholder(_)));
}
