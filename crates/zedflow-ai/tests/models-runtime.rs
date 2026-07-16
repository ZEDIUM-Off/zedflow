use std::collections::BTreeMap;
use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use std::task::Poll;

use futures::executor::block_on;
use futures::future::{FutureExt, join};
use zedflow_ai::auth::credential_store::InMemoryCredentialStore;
use zedflow_ai::auth::resolve::{ModelsError, ModelsErrorCode};
use zedflow_ai::auth::types::{
    ApiKeyAuth, ApiKeyCredential, ApiKeyResolveInput, AuthContext, AuthFuture, AuthLoginCallbacks,
    BoxError, Credential, CredentialModify, CredentialStore, ModelAuth, OAuthAuth, OAuthCredential,
    ResolvedAuth,
};
use zedflow_ai::models::{
    AssistantMessage, AssistantMessageEventStream, Context, CreateProviderOptions, Model, Provider,
    ProviderApi, ProviderAuth, StreamOptions, create_models,
    create_models_with_auth_context_and_credentials, create_provider,
};
use zedflow_ai::types::{
    AssistantContentBlock, AssistantMessageEvent, AssistantMessageRole, DoneStopReason,
    ProviderStreams, SimpleStreamOptions, StopReason, TextContent, TextContentType, Usage,
    UsageCost,
};

fn test_model(provider: &str, id: &str) -> Model {
    Model {
        provider: provider.to_string(),
        id: id.to_string(),
        api: "test-api".to_string(),
        ..Model::default()
    }
}

fn assistant_text_message(model: &Model, text: &str) -> AssistantMessage {
    AssistantMessage {
        role: AssistantMessageRole::Assistant,
        content: vec![AssistantContentBlock::Text(TextContent {
            content_type: TextContentType::Text,
            text: text.to_owned(),
            text_signature: None,
        })],
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage {
            cost: UsageCost::default(),
            ..Usage::default()
        },
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp: 0,
    }
}

fn done_stream(message: AssistantMessage) -> AssistantMessageEventStream {
    let stream = AssistantMessageEventStream::new();
    stream.push(AssistantMessageEvent::Done {
        reason: DoneStopReason::Stop,
        message,
    });
    stream
}

fn assistant_text(message: &AssistantMessage) -> String {
    message
        .content
        .iter()
        .filter_map(|block| match block {
            AssistantContentBlock::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn test_streams(text: &'static str) -> ProviderStreams {
    ProviderStreams {
        stream: Arc::new(move |model, _, _| done_stream(assistant_text_message(model, text))),
        stream_simple: Arc::new(move |model, _, _: Option<&SimpleStreamOptions>| {
            done_stream(assistant_text_message(model, text))
        }),
    }
}

fn test_provider(id: &str, models: Vec<Model>) -> Provider {
    create_provider(CreateProviderOptions {
        id: id.to_string(),
        name: None,
        base_url: None,
        headers: None,
        auth: ProviderAuth::default(),
        models,
        refresh_models: None,
        api: ProviderApi::Single(test_streams("ok")),
    })
}

#[derive(Default)]
struct FakeAuthContext {
    env: BTreeMap<String, String>,
}

impl FakeAuthContext {
    fn new<const N: usize>(env: [(&str, &str); N]) -> Self {
        Self {
            env: env
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect(),
        }
    }
}

impl AuthContext for FakeAuthContext {
    fn env<'a>(&'a self, name: &'a str) -> AuthFuture<'a, Option<String>> {
        Box::pin(async move { self.env.get(name).cloned() })
    }

    fn file_exists<'a>(&'a self, _path: &'a str) -> AuthFuture<'a, bool> {
        Box::pin(async { false })
    }
}

fn oauth_credential(access: &str) -> Credential {
    Credential::OAuth(OAuthCredential {
        refresh: "refresh".to_owned(),
        access: access.to_owned(),
        expires: 9_999_999_999_999,
        extra: BTreeMap::new(),
    })
}

fn expired_oauth_credential(access: &str) -> Credential {
    Credential::OAuth(OAuthCredential {
        refresh: "refresh".to_owned(),
        access: access.to_owned(),
        expires: 0,
        extra: BTreeMap::new(),
    })
}

fn api_key_credential(key: &str) -> Credential {
    Credential::ApiKey(ApiKeyCredential {
        key: Some(key.to_owned()),
        env: None,
    })
}

fn credential_store(provider: &str, credential: Credential) -> InMemoryCredentialStore {
    let store = InMemoryCredentialStore::new();
    block_on(<InMemoryCredentialStore as CredentialStore>::modify(
        &store,
        provider,
        Box::new(move |_| Box::pin(async move { Ok(Some(credential)) })),
    ))
    .expect("store credential");
    store
}

#[derive(Debug)]
struct ScopedAuth;

impl zedflow_ai::auth::types::ApiKeyAuth for ScopedAuth {
    fn name(&self) -> &str {
        "Scoped test auth"
    }

    fn resolve<'a>(
        &'a self,
        input: ApiKeyResolveInput<'a>,
    ) -> AuthFuture<'a, zedflow_ai::auth::types::AuthResult<Option<ResolvedAuth>>> {
        Box::pin(async move {
            let account = input.ctx.env("ACCOUNT_ID").await.unwrap_or_default();
            let key = input
                .credential
                .and_then(|credential| credential.key.clone())
                .unwrap_or_else(|| "resolved-key".to_owned());
            Ok(Some(ResolvedAuth {
                auth: zedflow_ai::auth::types::ModelAuth {
                    api_key: Some(key),
                    headers: Some(BTreeMap::from([
                        ("x-a".to_owned(), Some("auth".to_owned())),
                        ("x-b".to_owned(), Some("auth".to_owned())),
                    ])),
                    base_url: Some(format!("https://{account}.example")),
                },
                env: Some(BTreeMap::from([
                    ("ACCOUNT_ID".to_owned(), account),
                    ("SHARED".to_owned(), "auth".to_owned()),
                ])),
                source: Some("scoped".to_owned()),
            }))
        })
    }
}

#[derive(Debug)]
struct FailingApiKeyAuth;

impl ApiKeyAuth for FailingApiKeyAuth {
    fn name(&self) -> &str {
        "Failing test auth"
    }

    fn resolve<'a>(
        &'a self,
        _input: ApiKeyResolveInput<'a>,
    ) -> AuthFuture<'a, zedflow_ai::auth::types::AuthResult<Option<ResolvedAuth>>> {
        Box::pin(async {
            Err(Box::new(std::io::Error::other("api-key resolution failed")) as BoxError)
        })
    }
}

#[derive(Debug)]
struct FailingCredentialStore;

impl CredentialStore for FailingCredentialStore {
    fn read<'a>(
        &'a self,
        _provider_id: &'a str,
    ) -> AuthFuture<'a, zedflow_ai::auth::types::AuthResult<Option<Credential>>> {
        Box::pin(async {
            Err(Box::new(std::io::Error::other("credential read failed")) as BoxError)
        })
    }

    fn modify<'a>(
        &'a self,
        _provider_id: &'a str,
        _update: CredentialModify<'a>,
    ) -> AuthFuture<'a, zedflow_ai::auth::types::AuthResult<Option<Credential>>> {
        Box::pin(async {
            Err(Box::new(std::io::Error::other("credential modify failed")) as BoxError)
        })
    }

    fn delete<'a>(
        &'a self,
        _provider_id: &'a str,
    ) -> AuthFuture<'a, zedflow_ai::auth::types::AuthResult<()>> {
        Box::pin(async {
            Err(Box::new(std::io::Error::other("credential delete failed")) as BoxError)
        })
    }
}

#[derive(Debug)]
struct FakeOAuth {
    refreshes: Arc<AtomicUsize>,
    fail_refresh: bool,
}

impl OAuthAuth for FakeOAuth {
    fn name(&self) -> &str {
        "Fake OAuth"
    }

    fn login<'a>(
        &'a self,
        _callbacks: &'a dyn AuthLoginCallbacks,
    ) -> AuthFuture<'a, zedflow_ai::auth::types::AuthResult<OAuthCredential>> {
        Box::pin(async { unreachable!("login is not used in resolver tests") })
    }

    fn refresh<'a>(
        &'a self,
        credential: &'a OAuthCredential,
    ) -> AuthFuture<'a, zedflow_ai::auth::types::AuthResult<OAuthCredential>> {
        Box::pin(async move {
            let count = self.refreshes.fetch_add(1, Ordering::SeqCst) + 1;
            if self.fail_refresh {
                return Err(Box::new(std::io::Error::other("refresh failed"))
                    as zedflow_ai::auth::types::BoxError);
            }
            Ok(OAuthCredential {
                refresh: credential.refresh.clone(),
                access: format!("new-{count}"),
                expires: 9_999_999_999_999,
                extra: BTreeMap::new(),
            })
        })
    }

    fn to_auth<'a>(
        &'a self,
        credential: &'a OAuthCredential,
    ) -> AuthFuture<'a, zedflow_ai::auth::types::AuthResult<ModelAuth>> {
        Box::pin(async move {
            Ok(ModelAuth {
                api_key: Some(credential.access.clone()),
                ..ModelAuth::default()
            })
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SeenRequest {
    base_url: String,
    api_key: Option<String>,
    header_a: Option<String>,
    header_b: Option<String>,
    account_id: Option<String>,
    shared: Option<String>,
}

fn oauth_provider(refreshes: Arc<AtomicUsize>, fail_refresh: bool) -> Provider {
    create_provider(CreateProviderOptions {
        id: "oauth".to_owned(),
        name: None,
        base_url: None,
        headers: None,
        auth: ProviderAuth {
            api_key: None,
            oauth: Some(Arc::new(FakeOAuth {
                refreshes,
                fail_refresh,
            })),
        },
        models: vec![test_model("oauth", "model-a")],
        refresh_models: None,
        api: ProviderApi::Single(test_streams("ok")),
    })
}

fn failing_api_key_provider() -> Provider {
    create_provider(CreateProviderOptions {
        id: "failing-api-key".to_owned(),
        name: None,
        base_url: None,
        headers: None,
        auth: ProviderAuth {
            api_key: Some(Arc::new(FailingApiKeyAuth)),
            oauth: None,
        },
        models: vec![test_model("failing-api-key", "model-a")],
        refresh_models: None,
        api: ProviderApi::Single(test_streams("ok")),
    })
}

fn observing_provider(seen: Arc<Mutex<Option<SeenRequest>>>) -> Provider {
    let seen_for_stream = Arc::clone(&seen);
    create_provider(CreateProviderOptions {
        id: "scoped".to_owned(),
        name: None,
        base_url: None,
        headers: None,
        auth: ProviderAuth {
            api_key: Some(Arc::new(ScopedAuth)),
            oauth: None,
        },
        models: vec![test_model("scoped", "model-a")],
        refresh_models: None,
        api: ProviderApi::Single(ProviderStreams {
            stream: Arc::new(move |model, _, options: Option<&StreamOptions>| {
                let options = options.expect("stream options");
                *seen_for_stream.lock().expect("seen lock") = Some(SeenRequest {
                    base_url: model.base_url.clone(),
                    api_key: options.api_key.clone(),
                    header_a: options
                        .headers
                        .as_ref()
                        .and_then(|headers| headers.get("x-a"))
                        .and_then(Option::clone),
                    header_b: options
                        .headers
                        .as_ref()
                        .and_then(|headers| headers.get("x-b"))
                        .and_then(Option::clone),
                    account_id: options
                        .env
                        .as_ref()
                        .and_then(|env| env.get("ACCOUNT_ID"))
                        .cloned(),
                    shared: options
                        .env
                        .as_ref()
                        .and_then(|env| env.get("SHARED"))
                        .cloned(),
                });
                done_stream(assistant_text_message(model, "ok"))
            }),
            stream_simple: Arc::new(move |model, _, options: Option<&SimpleStreamOptions>| {
                let options = options.expect("simple options");
                *seen.lock().expect("seen lock") = Some(SeenRequest {
                    base_url: model.base_url.clone(),
                    api_key: options.stream.api_key.clone(),
                    header_a: options
                        .stream
                        .headers
                        .as_ref()
                        .and_then(|headers| headers.get("x-a"))
                        .and_then(Option::clone),
                    header_b: options
                        .stream
                        .headers
                        .as_ref()
                        .and_then(|headers| headers.get("x-b"))
                        .and_then(Option::clone),
                    account_id: options
                        .stream
                        .env
                        .as_ref()
                        .and_then(|env| env.get("ACCOUNT_ID"))
                        .cloned(),
                    shared: options
                        .stream
                        .env
                        .as_ref()
                        .and_then(|env| env.get("SHARED"))
                        .cloned(),
                });
                done_stream(assistant_text_message(model, "ok"))
            }),
        }),
    })
}

#[test]
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
fn models_runtime_swallows_provider_source_failures_for_listing() {
    let mut models = create_models();
    models.set_provider(test_provider("ok", vec![test_model("ok", "model-a")]));
    let failing = test_provider("bad", vec![test_model("bad", "hidden")]);
    *failing.model_source.lock().expect("model source lock") = Err(ModelsError::with_source(
        ModelsErrorCode::ModelSource,
        "boom",
        Box::new(std::io::Error::other("catalog source failed")),
    ));
    models.set_provider(failing.clone());

    assert_eq!(
        models
            .get_models(None)
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>(),
        vec!["model-a"]
    );
    assert!(models.get_models(Some("bad")).is_empty());
    let error = failing
        .get_models_result()
        .expect_err("direct source result");
    assert_eq!(error.message(), "boom");
    assert_eq!(
        error.source().map(ToString::to_string).as_deref(),
        Some("catalog source failed")
    );
}

#[test]
fn models_runtime_refresh_updates_dynamic_providers_and_rejects_single_failures() {
    let refreshes = Arc::new(AtomicUsize::new(0));
    let refreshes_for_provider = Arc::clone(&refreshes);
    let mut models = create_models();
    models.set_provider(create_provider(CreateProviderOptions {
        id: "dyn".to_string(),
        name: None,
        base_url: None,
        headers: None,
        auth: ProviderAuth::default(),
        models: vec![test_model("dyn", "before")],
        refresh_models: Some(Arc::new(move || {
            refreshes_for_provider.fetch_add(1, Ordering::SeqCst);
            async { Ok(vec![test_model("dyn", "after")]) }.boxed()
        })),
        api: ProviderApi::Single(test_streams("")),
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
        base_url: None,
        headers: None,
        auth: ProviderAuth::default(),
        models: vec![test_model("flaky", "model-a")],
        refresh_models: Some(Arc::new(|| {
            async { Err(ModelsError::new(ModelsErrorCode::Provider, "fetch failed")) }.boxed()
        })),
        api: ProviderApi::Single(test_streams("")),
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
fn models_runtime_dedupes_concurrent_provider_refreshes() {
    let refreshes = Arc::new(AtomicUsize::new(0));
    let refreshes_for_provider = Arc::clone(&refreshes);
    let mut models = create_models();
    models.set_provider(create_provider(CreateProviderOptions {
        id: "dyn".to_string(),
        name: None,
        base_url: None,
        headers: None,
        auth: ProviderAuth::default(),
        models: Vec::new(),
        refresh_models: Some(Arc::new(move || {
            refreshes_for_provider.fetch_add(1, Ordering::SeqCst);
            let mut yielded = false;
            futures::future::poll_fn(move |cx| {
                if yielded {
                    Poll::Ready(Ok(vec![test_model("dyn", "after")]))
                } else {
                    yielded = true;
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            })
            .boxed()
        })),
        api: ProviderApi::Single(test_streams("")),
    }));

    let (first, second) = block_on(join(
        models.refresh_async(Some("dyn")),
        models.refresh_async(Some("dyn")),
    ));
    first.expect("first refresh");
    second.expect("second refresh");
    assert_eq!(refreshes.load(Ordering::SeqCst), 1);
    assert!(models.get_model("dyn", "after").is_some());

    block_on(models.refresh_async(Some("dyn"))).expect("later refresh retries");
    assert_eq!(refreshes.load(Ordering::SeqCst), 2);
}

#[test]
fn models_runtime_resolves_auth_stored_credential_beats_ambient() {
    let mut ambient = create_models_with_auth_context_and_credentials(
        FakeAuthContext::new([("ANTHROPIC_API_KEY", "env-key")]),
        InMemoryCredentialStore::new(),
    );
    ambient.set_provider(zedflow_ai::providers::anthropic::anthropic_provider().expect("provider"));
    let model = ambient
        .get_model("anthropic", "claude-haiku-4-5")
        .expect("model");

    assert_eq!(
        ambient
            .get_auth(&model)
            .expect("ambient auth")
            .expect("configured")
            .auth
            .api_key
            .as_deref(),
        Some("env-key")
    );

    let mut oauth = create_models_with_auth_context_and_credentials(
        FakeAuthContext::new([("ANTHROPIC_API_KEY", "env-key")]),
        credential_store("anthropic", oauth_credential("oauth-token")),
    );
    oauth.set_provider(zedflow_ai::providers::anthropic::anthropic_provider().expect("provider"));
    let oauth_resolution = oauth
        .get_auth(&model)
        .expect("oauth auth")
        .expect("configured");
    assert_eq!(
        oauth_resolution.auth.api_key.as_deref(),
        Some("oauth-token")
    );
    assert_eq!(oauth_resolution.source.as_deref(), Some("OAuth"));

    let mut stored_key = create_models_with_auth_context_and_credentials(
        FakeAuthContext::new([("ANTHROPIC_API_KEY", "env-key")]),
        credential_store("anthropic", api_key_credential("stored-key")),
    );
    stored_key
        .set_provider(zedflow_ai::providers::anthropic::anthropic_provider().expect("provider"));
    let api_key_resolution = stored_key
        .get_auth(&model)
        .expect("api key auth")
        .expect("configured");
    assert_eq!(
        api_key_resolution.auth.api_key.as_deref(),
        Some("stored-key")
    );
    assert_eq!(
        api_key_resolution.source.as_deref(),
        Some("stored credential")
    );
}

#[test]
fn models_runtime_stored_credential_without_matching_handler_blocks_ambient_fallback() {
    let mut models = create_models_with_auth_context_and_credentials(
        FakeAuthContext::new([("AWS_PROFILE", "dev")]),
        credential_store("amazon-bedrock", oauth_credential("stale-token")),
    );
    models.set_provider(zedflow_ai::providers::amazon_bedrock::amazon_bedrock_provider());
    let model = models
        .get_models(Some("amazon-bedrock"))
        .into_iter()
        .next()
        .expect("bedrock model");

    assert!(models.get_auth(&model).expect("auth resolves").is_none());
}

#[test]
fn models_runtime_refreshes_expired_oauth_credentials_and_persists_rotation() {
    let refreshes = Arc::new(AtomicUsize::new(0));
    let store = credential_store("oauth", expired_oauth_credential("old-token"));
    let mut models =
        create_models_with_auth_context_and_credentials(FakeAuthContext::default(), store.clone());
    models.set_provider(oauth_provider(Arc::clone(&refreshes), false));
    let model = test_model("oauth", "model-a");

    let resolved = models
        .get_auth(&model)
        .expect("oauth refresh resolves")
        .expect("configured");
    assert_eq!(resolved.auth.api_key.as_deref(), Some("new-1"));
    assert_eq!(refreshes.load(Ordering::SeqCst), 1);

    let stored = block_on(<InMemoryCredentialStore as CredentialStore>::read(
        &store, "oauth",
    ))
    .expect("store read");
    assert!(matches!(stored, Some(Credential::OAuth(credential)) if credential.access == "new-1"));
}

#[test]
fn models_runtime_rejects_with_oauth_code_and_preserves_stored_credential() {
    let refreshes = Arc::new(AtomicUsize::new(0));
    let store = credential_store("oauth", expired_oauth_credential("old-token"));
    let mut models =
        create_models_with_auth_context_and_credentials(FakeAuthContext::default(), store.clone());
    models.set_provider(oauth_provider(Arc::clone(&refreshes), true));
    let model = test_model("oauth", "model-a");

    let error = models.get_auth(&model).expect_err("oauth fails");
    assert_eq!(error.code(), ModelsErrorCode::OAuth);
    assert_eq!(
        error.source().map(ToString::to_string).as_deref(),
        Some("refresh failed")
    );
    assert_eq!(refreshes.load(Ordering::SeqCst), 1);
    let stored = block_on(<InMemoryCredentialStore as CredentialStore>::read(
        &store, "oauth",
    ))
    .expect("store read");
    assert!(
        matches!(stored, Some(Credential::OAuth(credential)) if credential.access == "old-token")
    );
}

#[test]
fn models_runtime_serializes_concurrent_oauth_refreshes_through_store_modify() {
    let refreshes = Arc::new(AtomicUsize::new(0));
    let store = credential_store("oauth", expired_oauth_credential("old-token"));
    let mut models =
        create_models_with_auth_context_and_credentials(FakeAuthContext::default(), store);
    models.set_provider(oauth_provider(Arc::clone(&refreshes), false));
    let model = test_model("oauth", "model-a");

    let (first, second) = block_on(join(
        models.get_auth_async(&model),
        models.get_auth_async(&model),
    ));
    assert_eq!(
        first
            .expect("first auth")
            .expect("first configured")
            .auth
            .api_key
            .as_deref(),
        Some("new-1")
    );
    assert_eq!(
        second
            .expect("second auth")
            .expect("second configured")
            .auth
            .api_key
            .as_deref(),
        Some("new-1")
    );
    assert_eq!(refreshes.load(Ordering::SeqCst), 1);
}

#[test]
fn models_runtime_valid_oauth_tokens_resolve_without_touching_modify() {
    let refreshes = Arc::new(AtomicUsize::new(0));
    let mut models = create_models_with_auth_context_and_credentials(
        FakeAuthContext::default(),
        credential_store("oauth", oauth_credential("valid-token")),
    );
    models.set_provider(oauth_provider(Arc::clone(&refreshes), false));
    let model = test_model("oauth", "model-a");

    assert_eq!(
        models
            .get_auth(&model)
            .expect("valid oauth resolves")
            .expect("configured")
            .auth
            .api_key
            .as_deref(),
        Some("valid-token")
    );
    assert_eq!(refreshes.load(Ordering::SeqCst), 0);
}

#[test]
fn models_runtime_wraps_credential_store_failures_in_models_error() {
    let mut models = create_models_with_auth_context_and_credentials(
        FakeAuthContext::default(),
        FailingCredentialStore,
    );
    models.set_provider(test_provider(
        "store-failure",
        vec![test_model("store-failure", "model-a")],
    ));

    let error = models
        .get_auth(&test_model("store-failure", "model-a"))
        .expect_err("credential-store read fails");
    assert_eq!(error.code(), ModelsErrorCode::Auth);
    assert_eq!(
        error.source().map(ToString::to_string).as_deref(),
        Some("credential read failed")
    );
}

#[test]
fn models_runtime_wraps_api_key_auth_failures_in_models_error() {
    let mut models = create_models();
    models.set_provider(failing_api_key_provider());

    let error = models
        .get_auth(&test_model("failing-api-key", "model-a"))
        .expect_err("api-key resolver fails");
    assert_eq!(error.code(), ModelsErrorCode::Auth);
    assert_eq!(
        error.source().map(ToString::to_string).as_deref(),
        Some("api-key resolution failed")
    );
}

#[test]
fn models_runtime_uses_explicit_request_api_key_and_env_during_provider_auth_resolution() {
    let seen = Arc::new(Mutex::new(None));
    let mut models = create_models_with_auth_context_and_credentials(
        FakeAuthContext::default(),
        InMemoryCredentialStore::new(),
    );
    models.set_provider(observing_provider(Arc::clone(&seen)));
    let mut options = SimpleStreamOptions::default();
    options.stream.api_key = Some("explicit-key".to_owned());
    options.stream.env = Some(std::collections::HashMap::from([(
        "ACCOUNT_ID".to_owned(),
        "request-account".to_owned(),
    )]));

    let model = test_model("scoped", "model-a");
    assert_eq!(
        assistant_text(&models.complete_simple(&model, &Context::default(), Some(&options))),
        "ok"
    );

    assert_eq!(
        seen.lock().expect("seen lock").clone().expect("seen"),
        SeenRequest {
            base_url: "https://request-account.example".to_owned(),
            api_key: Some("explicit-key".to_owned()),
            header_a: Some("auth".to_owned()),
            header_b: Some("auth".to_owned()),
            account_id: Some("request-account".to_owned()),
            shared: Some("auth".to_owned()),
        }
    );
}

#[test]
fn models_runtime_merges_resolved_auth_into_stream_options_with_explicit_fields_winning() {
    let seen = Arc::new(Mutex::new(None));
    let mut models = create_models_with_auth_context_and_credentials(
        FakeAuthContext::new([("ACCOUNT_ID", "ambient-account")]),
        InMemoryCredentialStore::new(),
    );
    models.set_provider(observing_provider(Arc::clone(&seen)));
    let options = StreamOptions {
        api_key: Some("explicit-key".to_owned()),
        headers: Some(std::collections::HashMap::from([(
            "x-b".to_owned(),
            Some("explicit".to_owned()),
        )])),
        env: Some(std::collections::HashMap::from([(
            "SHARED".to_owned(),
            "explicit".to_owned(),
        )])),
        ..StreamOptions::default()
    };

    let model = test_model("scoped", "model-a");
    assert_eq!(
        assistant_text(&models.complete(&model, &Context::default(), Some(&options))),
        "ok"
    );
    assert!(
        model.base_url.is_empty(),
        "request auth must not mutate the catalog model"
    );

    assert_eq!(
        seen.lock().expect("seen lock").clone().expect("seen"),
        SeenRequest {
            base_url: "https://ambient-account.example".to_owned(),
            api_key: Some("explicit-key".to_owned()),
            header_a: Some("auth".to_owned()),
            header_b: Some("explicit".to_owned()),
            account_id: Some("ambient-account".to_owned()),
            shared: Some("explicit".to_owned()),
        }
    );
}

#[test]
fn models_runtime_produces_error_stream_for_unknown_providers_instead_of_throwing() {
    let models = create_models();
    let result = models.complete(&test_model("ghost", "model-a"), &Context::default(), None);
    assert!(assistant_text(&result).contains("Unknown provider: ghost"));
}

#[test]
fn models_runtime_streams_through_the_provider() {
    let mut models = create_models();
    models.set_provider(test_provider("p1", vec![test_model("p1", "model-a")]));
    let model = test_model("p1", "model-a");

    let result = block_on(
        models
            .stream(&model, &Context::default(), Option::<&StreamOptions>::None)
            .result(),
    );
    assert_eq!(assistant_text(&result), "ok");
    assert_eq!(
        assistant_text(&models.complete(&model, &Context::default(), None)),
        "ok"
    );
}
