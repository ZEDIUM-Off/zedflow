use std::collections::BTreeMap;
use std::fmt::Display;

use futures::executor::block_on;
use serde_json::Value;
use zedflow_ai::auth::types::{OAuthAuth, OAuthCredential};
use zedflow_ai::providers::anthropic::anthropic_provider;
use zedflow_ai::providers::github_copilot::github_copilot_provider;
use zedflow_ai::utils::oauth::anthropic::ANTHROPIC_OAUTH;
use zedflow_ai::utils::oauth::github_copilot::GITHUB_COPILOT_OAUTH;
use zedflow_ai::utils::oauth::openai_codex::OPENAI_CODEX_OAUTH;

fn oauth_credential(access: &str, refresh: &str, expires: i64) -> OAuthCredential {
    OAuthCredential {
        refresh: refresh.to_owned(),
        access: access.to_owned(),
        expires,
        extra: BTreeMap::new(),
    }
}

fn error_text<T>(result: Result<T, impl Display>) -> String {
    match result {
        Ok(_) => panic!("expected port placeholder"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn anthropic_to_auth_derives_the_api_key_from_the_access_token() {
    let auth = block_on(ANTHROPIC_OAUTH.to_auth(&oauth_credential("token", "r", 0)))
        .expect("anthropic to_auth");

    assert_eq!(auth.api_key.as_deref(), Some("token"));
    assert_eq!(auth.headers, None);
    assert_eq!(auth.base_url, None);
}

#[test]
fn openai_codex_to_auth_derives_the_api_key_from_the_access_token() {
    let auth = block_on(OPENAI_CODEX_OAUTH.to_auth(&oauth_credential("token", "r", 0)))
        .expect("openai codex to_auth");

    assert_eq!(auth.api_key.as_deref(), Some("token"));
    assert_eq!(auth.headers, None);
    assert_eq!(auth.base_url, None);
}

#[test]
fn github_copilot_to_auth_derives_base_url_from_the_token_proxy_endpoint() {
    let access = "tid=abc;exp=123;proxy-ep=proxy.enterprise.example;rest";
    let auth = block_on(GITHUB_COPILOT_OAUTH.to_auth(&oauth_credential(access, "r", 0)))
        .expect("github copilot to_auth");

    assert_eq!(auth.api_key.as_deref(), Some(access));
    assert_eq!(
        auth.base_url.as_deref(),
        Some("https://api.enterprise.example")
    );
}

#[test]
fn github_copilot_to_auth_falls_back_to_enterprise_domain_then_individual_endpoint() {
    let mut enterprise = oauth_credential("no-proxy-ep", "r", 0);
    enterprise.extra.insert(
        "enterpriseUrl".to_owned(),
        Value::String("https://company.ghe.com".to_owned()),
    );
    let enterprise_auth = block_on(GITHUB_COPILOT_OAUTH.to_auth(&enterprise))
        .expect("github copilot enterprise to_auth");
    assert_eq!(
        enterprise_auth.base_url.as_deref(),
        Some("https://copilot-api.company.ghe.com")
    );

    let individual_auth =
        block_on(GITHUB_COPILOT_OAUTH.to_auth(&oauth_credential("no-proxy-ep", "r", 0)))
            .expect("github copilot individual to_auth");
    assert_eq!(
        individual_auth.base_url.as_deref(),
        Some("https://api.individual.githubcopilot.com")
    );
}

#[test]
#[ignore = "blocked: Anthropic refresh is a PORT PLACEHOLDER until a Rust HTTP client/token exchange is selected"]
fn anthropic_refresh_exchanges_the_refresh_token_and_returns_a_typed_credential() {
    let error = block_on(ANTHROPIC_OAUTH.refresh(&oauth_credential("old", "old-r", 0)))
        .expect_err("anthropic refresh remains blocked by placeholder");

    assert!(error.to_string().contains("port placeholder"));
    assert!(error.to_string().contains("refresh tokens"));
}

#[test]
#[ignore = "blocked: GitHub Copilot refresh is a PORT PLACEHOLDER until HTTP/model fetch wiring is selected"]
fn github_copilot_refresh_preserves_the_enterprise_domain() {
    let mut credential = oauth_credential("old", "gh-token", 0);
    credential.extra.insert(
        "enterpriseUrl".to_owned(),
        Value::String("company.ghe.com".to_owned()),
    );

    let error = block_on(GITHUB_COPILOT_OAUTH.refresh(&credential))
        .expect_err("github copilot refresh remains blocked by placeholder");

    assert!(error.to_string().contains("port placeholder"));
    assert!(error.to_string().contains("Copilot token"));
}

#[test]
#[ignore = "blocked: Models::get_auth has no credential-store/OAuth lazy-load wiring and Anthropic provider factory is a PORT PLACEHOLDER"]
fn models_get_auth_resolves_stored_anthropic_oauth_credentials_via_lazy_flow_import() {
    let error = error_text(anthropic_provider());

    assert!(error.contains("port placeholder"));
    assert!(error.contains("loadAnthropicOAuth"));
}

#[test]
#[ignore = "blocked: Models::get_auth has no credential-store/OAuth lazy-load wiring and GitHub Copilot provider factory is a PORT PLACEHOLDER"]
fn models_get_auth_resolves_stored_github_copilot_oauth_credentials_with_base_url() {
    let error = error_text(github_copilot_provider());

    assert!(error.contains("port placeholder"));
    assert!(error.contains("loadGitHubCopilotOAuth"));
}
