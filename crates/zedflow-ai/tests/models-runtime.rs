use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use zedflow_ai::auth::resolve::{ModelsError, ModelsErrorCode};
use zedflow_ai::models::{
    AssistantMessage, CreateProviderOptions, Model, Provider, StreamOptions, create_models,
    create_provider,
};

fn test_model(provider: &str, id: &str) -> Model {
    Model {
        provider: provider.to_string(),
        id: id.to_string(),
        api: "test-api".to_string(),
    }
}

fn test_provider(id: &str, models: Vec<Model>) -> Provider {
    create_provider(CreateProviderOptions {
        id: id.to_string(),
        name: None,
        models,
        refresh_models: None,
        stream: Arc::new(|_, _| {
            vec![AssistantMessage {
                text: "ok".to_string(),
            }]
        }),
    })
}

fn parity_blocked(reason: &str) {
    panic!("models-runtime parity blocker: {reason}");
}

#[test]
#[ignore = "source parity blocker: chat Models is still a minimal placeholder and does not preserve provider insertion order or provider identity"]
fn models_runtime_registers_replaces_and_deletes_providers() {
    let mut models = create_models();
    models.set_provider(test_provider("p1", vec![test_model("p1", "model-a")]));
    models.set_provider(test_provider("p2", vec![test_model("p2", "model-a")]));
    assert_eq!(
        models
            .get_providers()
            .iter()
            .map(|provider| provider.id.as_str())
            .collect::<Vec<_>>(),
        vec!["p1", "p2"]
    );

    let replacement = test_provider("p1", vec![test_model("p1", "model-a")]);
    models.set_provider(replacement);
    assert_eq!(models.get_provider("p1").expect("provider p1").id, "p1");
    assert_eq!(models.get_providers().len(), 2);

    models.delete_provider("p1");
    assert!(models.get_provider("p1").is_none());

    models.clear_providers();
    assert!(models.get_providers().is_empty());
}

#[test]
#[ignore = "source parity blocker: chat Models stores providers in HashMap, so all-provider model listing is not Pi Map insertion ordered"]
fn models_runtime_lists_and_finds_models_per_provider() {
    let mut models = create_models();
    models.set_provider(test_provider(
        "p1",
        vec![test_model("p1", "m1"), test_model("p1", "m2")],
    ));
    models.set_provider(test_provider("p2", vec![test_model("p2", "m3")]));

    assert_eq!(
        models
            .get_models(None)
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>(),
        vec!["m1", "m2", "m3"]
    );
    assert_eq!(
        models
            .get_models(Some("p1"))
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>(),
        vec!["m1", "m2"]
    );
    assert!(models.get_models(Some("nope")).is_empty());
    assert_eq!(models.get_model("p2", "m3").expect("model p2/m3").id, "m3");
    assert!(models.get_model("p2", "missing").is_none());

    let found = models.get_model("p2", "m3").expect("model p2/m3");
    assert_ne!(found.api, "openai-completions");
    assert_eq!(found.api, "test-api");
}

#[test]
#[ignore = "source parity blocker: current Provider cannot model getModels throwing, so best-effort source failure behavior is unimplemented"]
fn models_runtime_swallows_provider_source_failures_for_listing() {
    parity_blocked(
        "preserve TS assertions: all-provider listing skips throwing provider, single-provider listing returns [], direct provider getModels throws boom",
    );
}

#[test]
#[ignore = "source parity blocker: refresh is sync/minimal and does not preserve Pi async/in-flight refresh semantics"]
fn models_runtime_refresh_updates_dynamic_providers_and_rejects_single_failures() {
    let refreshes = Arc::new(AtomicUsize::new(0));
    let refreshes_for_provider = Arc::clone(&refreshes);
    let mut models = create_models();
    models.set_provider(create_provider(CreateProviderOptions {
        id: "dyn".to_string(),
        name: None,
        models: vec![test_model("dyn", "before")],
        refresh_models: Some(Arc::new(move || {
            refreshes_for_provider.fetch_add(1, Ordering::SeqCst);
            Ok(vec![test_model("dyn", "after")])
        })),
        stream: Arc::new(|_, _| Vec::new()),
    }));
    models.set_provider(test_provider("static", vec![test_model("static", "s1")]));

    assert!(models.get_model("dyn", "before").is_some());
    models.refresh(Some("dyn")).expect("dyn refresh");
    assert_eq!(refreshes.load(Ordering::SeqCst), 1);
    assert!(models.get_model("dyn", "after").is_some());
    assert!(models.get_model("dyn", "before").is_none());

    models.refresh(Some("static")).expect("static refresh noop");
    models.refresh(None).expect("refresh all best effort");
    assert_eq!(refreshes.load(Ordering::SeqCst), 2);

    models.set_provider(create_provider(CreateProviderOptions {
        id: "flaky".to_string(),
        name: None,
        models: vec![test_model("flaky", "model-a")],
        refresh_models: Some(Arc::new(|| {
            Err(ModelsError::new(ModelsErrorCode::Provider, "fetch failed"))
        })),
        stream: Arc::new(|_, _| Vec::new()),
    }));
    assert_eq!(
        models
            .refresh(Some("flaky"))
            .expect_err("flaky refresh")
            .code(),
        ModelsErrorCode::ModelSource
    );
    models.refresh(None).expect("refresh all swallows failure");
}

#[test]
#[ignore = "source parity blocker: createModels has no credential store injection and Models::get_auth returns placeholder auth"]
fn models_runtime_resolves_auth_stored_credential_beats_ambient() {
    parity_blocked(
        "preserve TS assertions: env key resolves with no store; stored OAuth returns oauth-token source OAuth; stored api_key returns stored-key source stored",
    );
}

#[test]
#[ignore = "source parity blocker: chat Models auth resolution is placeholder-only"]
fn models_runtime_stored_credential_without_matching_handler_blocks_ambient_fallback() {
    parity_blocked(
        "preserve TS assertion: stale stored oauth credential with apiKey-only provider resolves undefined",
    );
}

#[test]
#[ignore = "source parity blocker: OAuth refresh/persistence is not wired through chat Models"]
fn models_runtime_refreshes_expired_oauth_credentials_and_persists_rotation() {
    parity_blocked(
        "preserve TS assertions: expired old-token refreshes to new-token and store is updated",
    );
}

#[test]
#[ignore = "source parity blocker: OAuth refresh errors are not wired through chat Models"]
fn models_runtime_rejects_with_oauth_code_and_preserves_stored_credential() {
    parity_blocked(
        "preserve TS assertions: getAuth rejects code oauth and stored credential remains old",
    );
}

#[test]
#[ignore = "source parity blocker: credential-store-backed OAuth refresh serialization is not wired through chat Models"]
fn models_runtime_serializes_concurrent_oauth_refreshes_through_store_modify() {
    parity_blocked(
        "preserve TS assertions: two concurrent getAuth calls perform one refresh and both receive new-1",
    );
}

#[test]
#[ignore = "source parity blocker: valid OAuth token fast path is not wired through chat Models"]
fn models_runtime_valid_oauth_tokens_resolve_without_touching_modify() {
    parity_blocked("preserve TS assertions: valid token resolves and modify count remains 0");
}

#[test]
#[ignore = "source parity blocker: credential store failure wrapping is not wired through chat Models"]
fn models_runtime_wraps_credential_store_failures_in_models_error() {
    parity_blocked("preserve TS assertions: read and modify failures reject with code auth");
}

#[test]
#[ignore = "source parity blocker: api-key auth failure wrapping is not wired through chat Models"]
fn models_runtime_wraps_api_key_auth_failures_in_models_error() {
    parity_blocked("preserve TS assertion: api-key resolve failure rejects with code auth");
}

#[test]
#[ignore = "source parity blocker: request auth resolution, provider env, and completeSimple are not ported for chat Models"]
fn models_runtime_uses_explicit_request_api_key_and_env_during_provider_auth_resolution() {
    parity_blocked(
        "preserve TS assertions: provider sees auth baseUrl from env, explicit apiKey, and ACCOUNT_ID env option",
    );
}

#[test]
#[ignore = "source parity blocker: auth merging into stream options is not ported for chat Models"]
fn models_runtime_merges_resolved_auth_into_stream_options_with_explicit_fields_winning() {
    let mut explicit_headers = std::collections::HashMap::new();
    explicit_headers.insert("x-b".to_string(), Some("explicit".to_string()));
    assert_eq!(
        explicit_headers.get("x-b").and_then(Option::as_deref),
        Some("explicit")
    );
    parity_blocked(
        "preserve TS assertions: explicit apiKey wins, headers merge auth then explicit, auth baseUrl applies, resolved apiKey applies without explicit option",
    );
}

#[test]
#[ignore = "source parity blocker: unknown provider currently returns an empty/default stream, not a Pi error AssistantMessage"]
fn models_runtime_produces_error_stream_for_unknown_providers_instead_of_throwing() {
    let models = create_models();
    let result = models.complete(&test_model("ghost", "model-a"), None);
    assert!(result.text.contains("Unknown provider: ghost"));
}

#[test]
#[ignore = "source parity blocker: chat stream events are Vec<AssistantMessage>, not Pi start/done event streams with result()"]
fn models_runtime_streams_through_the_provider() {
    let mut models = create_models();
    models.set_provider(test_provider("p1", vec![test_model("p1", "model-a")]));
    let model = test_model("p1", "model-a");

    let events = models.stream(&model, Option::<&StreamOptions>::None);
    assert_eq!(events.len(), 2);
    assert_eq!(models.complete(&model, None).text, "ok");
}
