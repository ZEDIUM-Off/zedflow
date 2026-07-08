use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use futures::future::{BoxFuture, FutureExt};
use zedflow_ai::auth::helpers::{
    ApiKeyCredential, AuthContext, AuthEvent, AuthLoginCallbacks, AuthPrompt, AuthPromptKind,
    AuthResult, env_api_key_auth,
};
use zedflow_ai::models::create_models;
use zedflow_ai::providers::faux::{
    FauxResponseStep, RegisterFauxProviderOptions, faux_assistant_message, faux_provider,
};

#[derive(Default)]
struct FakeAuthContext {
    env: BTreeMap<String, String>,
    files: Vec<String>,
}

impl FakeAuthContext {
    fn new<const N: usize>(env: [(&str, &str); N]) -> Self {
        Self {
            env: env
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect(),
            files: Vec::new(),
        }
    }
}

impl AuthContext for FakeAuthContext {
    fn env<'a>(&'a self, name: &'a str) -> BoxFuture<'a, AuthResult<Option<String>>> {
        async move { Ok(self.env.get(name).cloned()) }.boxed()
    }

    fn file_exists<'a>(&'a self, path: &'a str) -> BoxFuture<'a, AuthResult<bool>> {
        async move { Ok(self.files.iter().any(|file| file == path)) }.boxed()
    }
}

#[derive(Default)]
struct CapturingLoginCallbacks {
    prompts: Arc<Mutex<Vec<AuthPrompt>>>,
}

impl AuthLoginCallbacks for CapturingLoginCallbacks {
    fn prompt<'a>(&'a self, prompt: AuthPrompt) -> BoxFuture<'a, AuthResult<String>> {
        async move {
            self.prompts
                .lock()
                .expect("prompt log lock poisoned")
                .push(prompt);
            Ok("entered-key".to_owned())
        }
        .boxed()
    }

    fn notify(&self, _event: AuthEvent) {}
}

#[test]
#[ignore = "parity blocker: providers/all.rs still has placeholder builtin_providers/get_builtin_models, so Pi's >500 builtin model assertions cannot pass"]
fn builtin_models_registers_every_builtin_provider_with_models() {
    parity_blocked(
        "expected builtinModels providers length to match builtinProviders, contain anthropic, resolve claude-haiku-4-5 as anthropic-messages, expose >500 models, and list owned models per provider",
    );
}

#[test]
#[ignore = "parity blocker: anthropic_provider is a PORT PLACEHOLDER and Models::get_auth has no provider auth contract yet"]
fn resolves_anthropic_auth_from_env_with_oauth_token_precedence() {
    parity_blocked(
        "expected ANTHROPIC_OAUTH_TOKEN to win over ANTHROPIC_API_KEY with source ANTHROPIC_OAUTH_TOKEN",
    );
}

#[test]
#[ignore = "parity blocker: Models::get_auth currently returns default auth and does not evaluate ambient Bedrock credential env"]
fn reports_bedrock_as_configured_from_ambient_aws_credentials_without_an_api_key() {
    parity_blocked(
        "expected AWS_PROFILE to configure Bedrock with empty auth and unconfigured env to return None",
    );
}

#[test]
#[ignore = "parity blocker: Cloudflare Workers AI provider documents missing auth/API fields in the current Rust Provider shape"]
fn requires_cloudflare_workers_ai_account_config_and_returns_scoped_env() {
    parity_blocked(
        "expected CLOUDFLARE_API_KEY plus CLOUDFLARE_ACCOUNT_ID to resolve apiKey/baseUrl and scoped env",
    );
}

#[test]
#[ignore = "parity blocker: cloudflare_ai_gateway_provider is a PORT PLACEHOLDER until auth resolver and mixed API wiring exist"]
fn requires_cloudflare_ai_gateway_account_and_gateway_config_and_returns_scoped_env_headers() {
    parity_blocked(
        "expected CLOUDFLARE_API_KEY/CLOUDFLARE_ACCOUNT_ID/CLOUDFLARE_GATEWAY_ID to resolve gateway headers, baseUrl, and scoped env",
    );
}

#[test]
#[ignore = "parity blocker: google_vertex_provider is a PORT PLACEHOLDER until ADC/API-key auth and model wiring exist"]
fn resolves_vertex_via_adc_file_plus_project_and_location() {
    parity_blocked(
        "expected ADC plus project/location to resolve empty auth, partial ADC to be unconfigured, and GOOGLE_CLOUD_API_KEY to win",
    );
}

#[test]
fn env_api_key_auth_prefers_the_stored_credential_key_and_falls_back_through_env_vars_in_order() {
    let auth = env_api_key_auth("Test key", ["FIRST_KEY", "SECOND_KEY"]);

    let stored = block_on(auth.resolve(
        &FakeAuthContext::new([("FIRST_KEY", "env")]),
        Some(&ApiKeyCredential {
            key: Some("stored".to_owned()),
            env: None,
        }),
    ))
    .expect("stored credential resolves")
    .expect("stored credential result");
    assert_eq!(stored.auth.api_key.as_deref(), Some("stored"));
    assert_eq!(stored.source.as_deref(), Some("stored credential"));

    let second = block_on(auth.resolve(&FakeAuthContext::new([("SECOND_KEY", "second")]), None))
        .expect("env credential resolves")
        .expect("env credential result");
    assert_eq!(second.auth.api_key.as_deref(), Some("second"));
    assert_eq!(second.source.as_deref(), Some("SECOND_KEY"));

    assert!(
        block_on(auth.resolve(&FakeAuthContext::default(), None))
            .expect("empty env resolves")
            .is_none()
    );
}

#[test]
fn env_api_key_auth_login_prompts_for_a_secret_and_returns_an_api_key_credential() {
    let auth = env_api_key_auth("Test key", ["TEST_KEY"]);
    let callbacks = CapturingLoginCallbacks::default();

    let credential = block_on(auth.login(&callbacks)).expect("login returns credential");

    assert_eq!(
        credential,
        ApiKeyCredential {
            key: Some("entered-key".to_owned()),
            env: None,
        }
    );
    let prompts = callbacks.prompts.lock().expect("prompt log lock poisoned");
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].kind, AuthPromptKind::Secret);
}

#[test]
#[ignore = "parity blocker: create_provider only accepts one stream callback; Pi's per-model API dispatch map is not represented yet"]
fn create_provider_dispatches_on_model_api_for_mixed_api_providers() {
    parity_blocked("expected api-a/model-a to call a:model-a and api-b/model-b to call b:model-b");
}

#[test]
#[ignore = "parity blocker: Provider auth resolution and request/env merge options are not represented in models.rs StreamOptions yet"]
fn create_provider_merges_provider_resolved_env_into_stream_options() {
    parity_blocked(
        "expected request apiKey to win and provider/request env to merge with request SHARED taking precedence",
    );
}

#[test]
#[ignore = "parity blocker: create_provider has no mixed API map, so missing API implementations cannot synthesize Pi stream errors yet"]
fn create_provider_produces_a_stream_error_for_a_model_whose_api_has_no_implementation() {
    parity_blocked("expected stopReason error with message containing 'no API implementation'");
}

#[test]
#[ignore = "parity blocker: refresh_models is synchronous and does not dedupe concurrent in-flight refreshes like Pi's async provider refresh"]
fn create_provider_supports_dynamic_providers_empty_until_refreshed_in_flight_refreshes_deduped() {
    parity_blocked(
        "expected empty model list before refresh, concurrent refresh fetch count 1, listed model after refresh, and later fetch count 2",
    );
}

#[test]
fn faux_provider_streams_queued_responses_through_a_models_collection() {
    let faux = faux_provider(RegisterFauxProviderOptions::default());
    let provider_id = faux.provider.id.clone();
    let mut models = create_models();
    models.set_provider(faux.provider.clone());
    faux.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
        "hello from faux",
    ))]);

    let model = models
        .get_models(Some(&provider_id))
        .into_iter()
        .next()
        .expect("faux model");
    let result = models.complete(&model, None);

    assert_eq!(result.text, "hello from faux");
    assert_eq!(faux.state.call_count(), 1);
}

#[track_caller]
fn parity_blocked(reason: &str) {
    panic!("parity blocker: {reason}");
}
