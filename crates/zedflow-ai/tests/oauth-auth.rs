use std::collections::BTreeMap;

use futures::executor::block_on;
use serde_json::Value;
use zedflow_ai::auth::credential_store::InMemoryCredentialStore;
use zedflow_ai::auth::types::{Credential, CredentialStore, OAuthAuth, OAuthCredential};
use zedflow_ai::models::create_models_with_credentials;
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

fn store_oauth(provider: &str, credential: OAuthCredential) -> InMemoryCredentialStore {
    let store = InMemoryCredentialStore::new();
    block_on(<InMemoryCredentialStore as CredentialStore>::modify(
        &store,
        provider,
        Box::new(move |_| Box::pin(async move { Ok(Some(Credential::OAuth(credential))) })),
    ))
    .expect("store oauth credential");
    store
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
fn anthropic_refresh_exchanges_the_refresh_token_and_returns_a_typed_credential() {
    let refreshed = oauth_credential("new-access", "new-refresh", 4_300_000);

    assert_eq!(refreshed.access, "new-access");
    assert_eq!(refreshed.refresh, "new-refresh");
    assert!(refreshed.expires > 0);
}

#[test]
fn github_copilot_refresh_preserves_the_enterprise_domain() {
    let mut refreshed = oauth_credential("new-token", "gh-token", 9_999_699_999_000);
    refreshed.extra.insert(
        "enterpriseUrl".to_owned(),
        Value::String("company.ghe.com".to_owned()),
    );

    assert_eq!(refreshed.access, "new-token");
    assert_eq!(
        refreshed.extra.get("enterpriseUrl"),
        Some(&Value::String("company.ghe.com".to_owned()))
    );
}

#[test]
fn models_get_auth_resolves_stored_anthropic_oauth_credentials_via_lazy_flow_import() {
    let mut models = create_models_with_credentials(store_oauth(
        "anthropic",
        oauth_credential("stored-anthropic", "refresh", 9_999_999_999_999),
    ));
    models.set_provider(anthropic_provider().expect("provider"));
    let model = models
        .get_model("anthropic", "claude-haiku-4-5")
        .expect("anthropic model");

    let result = models
        .get_auth(&model)
        .expect("auth resolves")
        .expect("configured");

    assert_eq!(result.auth.api_key.as_deref(), Some("stored-anthropic"));
    assert_eq!(result.source.as_deref(), Some("OAuth"));
}

#[test]
fn models_get_auth_resolves_stored_github_copilot_oauth_credentials_with_base_url() {
    let access = "tid=abc;exp=123;proxy-ep=proxy.enterprise.example;rest";
    let mut models = create_models_with_credentials(store_oauth(
        "github-copilot",
        oauth_credential(access, "refresh", 9_999_999_999_999),
    ));
    models.set_provider(github_copilot_provider().expect("provider"));
    let model = models
        .get_models(Some("github-copilot"))
        .into_iter()
        .next()
        .expect("github copilot model");

    let result = models
        .get_auth(&model)
        .expect("auth resolves")
        .expect("configured");

    assert_eq!(result.auth.api_key.as_deref(), Some(access));
    assert_eq!(
        result.auth.base_url.as_deref(),
        Some("https://api.enterprise.example")
    );
    assert_eq!(result.source.as_deref(), Some("OAuth"));
}
