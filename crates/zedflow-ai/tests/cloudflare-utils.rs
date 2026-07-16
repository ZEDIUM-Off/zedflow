//! Port of Pi `packages/ai/test/cloudflare-utils.ts`.
//!
//! Test-only Cloudflare credential helpers. These only inspect environment values;
//! they do not make live provider calls.

#![allow(
    dead_code,
    reason = "standalone port of helpers imported by Pi test suites"
)]

fn has_env_value(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

fn has_value(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.is_empty())
}

pub(crate) fn has_cloudflare_workers_ai_credentials() -> bool {
    has_env_value("CLOUDFLARE_API_KEY") && has_env_value("CLOUDFLARE_ACCOUNT_ID")
}

pub(crate) fn has_cloudflare_ai_gateway_credentials() -> bool {
    has_cloudflare_workers_ai_credentials() && has_env_value("CLOUDFLARE_GATEWAY_ID")
}

fn has_cloudflare_workers_ai_credentials_from(
    api_key: Option<&str>,
    account_id: Option<&str>,
) -> bool {
    has_value(api_key) && has_value(account_id)
}

fn has_cloudflare_ai_gateway_credentials_from(
    api_key: Option<&str>,
    account_id: Option<&str>,
    gateway_id: Option<&str>,
) -> bool {
    has_cloudflare_workers_ai_credentials_from(api_key, account_id) && has_value(gateway_id)
}

#[test]
fn checks_workers_ai_credentials_without_live_calls() {
    assert!(has_cloudflare_workers_ai_credentials_from(
        Some("key"),
        Some("account"),
    ));
    assert!(!has_cloudflare_workers_ai_credentials_from(
        None,
        Some("account"),
    ));
    assert!(!has_cloudflare_workers_ai_credentials_from(
        Some("key"),
        None
    ));
    assert!(!has_cloudflare_workers_ai_credentials_from(
        Some(""),
        Some("account"),
    ));
}

#[test]
fn checks_ai_gateway_credentials_without_live_calls() {
    assert!(has_cloudflare_ai_gateway_credentials_from(
        Some("key"),
        Some("account"),
        Some("gateway"),
    ));
    assert!(!has_cloudflare_ai_gateway_credentials_from(
        Some("key"),
        Some("account"),
        None,
    ));
    assert!(!has_cloudflare_ai_gateway_credentials_from(
        Some("key"),
        None,
        Some("gateway"),
    ));
    assert!(!has_cloudflare_ai_gateway_credentials_from(
        Some("key"),
        Some("account"),
        Some(""),
    ));
}
