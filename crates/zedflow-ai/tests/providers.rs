use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::Poll;

use futures::executor::block_on;
use futures::future::{BoxFuture, FutureExt, join};
use zedflow_ai::auth::helpers::{
    ApiKeyCredential, AuthContext, AuthEvent, AuthLoginCallbacks, AuthPrompt, AuthPromptKind,
    AuthResult, env_api_key_auth,
};
use zedflow_ai::auth::types::{
    ApiKeyAuth as CanonicalApiKeyAuth, ApiKeyResolveInput, AuthFuture, ModelAuth, ResolvedAuth,
};
use zedflow_ai::models::{
    Context, CreateProviderOptions, Model, ProviderApi, ProviderAuth, create_models,
    create_models_with_auth_context, create_provider,
};
use zedflow_ai::providers::faux::{
    FauxResponseStep, RegisterFauxProviderOptions, faux_assistant_message, faux_provider,
};
use zedflow_ai::types::{
    AssistantContentBlock, AssistantMessageEvent, AssistantMessageRole, DoneStopReason,
    ProviderStreams, SimpleStreamOptions, StopReason, TextContent, TextContentType, Usage,
    UsageCost,
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

impl zedflow_ai::auth::types::AuthContext for FakeAuthContext {
    fn env<'a>(&'a self, name: &'a str) -> zedflow_ai::auth::types::AuthFuture<'a, Option<String>> {
        Box::pin(async move { self.env.get(name).cloned() })
    }

    fn file_exists<'a>(&'a self, path: &'a str) -> zedflow_ai::auth::types::AuthFuture<'a, bool> {
        Box::pin(async move { self.files.iter().any(|file| file == path) })
    }
}

#[derive(Debug)]
struct RequestScopedAuth;

impl CanonicalApiKeyAuth for RequestScopedAuth {
    fn name(&self) -> &str {
        "Request scoped test auth"
    }

    fn resolve<'a>(
        &'a self,
        _input: ApiKeyResolveInput<'a>,
    ) -> AuthFuture<'a, zedflow_ai::auth::types::AuthResult<Option<ResolvedAuth>>> {
        Box::pin(async {
            Ok(Some(ResolvedAuth {
                auth: ModelAuth {
                    api_key: Some("provider-key".to_owned()),
                    headers: None,
                    base_url: Some("https://provider.example".to_owned()),
                },
                env: Some(BTreeMap::from([
                    ("PROVIDER".to_owned(), "provider".to_owned()),
                    ("SHARED".to_owned(), "provider".to_owned()),
                ])),
                source: Some("test".to_owned()),
            }))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SeenRequestOptions {
    base_url: String,
    api_key: Option<String>,
    provider: Option<String>,
    shared: Option<String>,
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

fn test_model(api: &str, id: &str) -> Model {
    Model {
        provider: "mixed".to_owned(),
        id: id.to_owned(),
        api: api.to_owned(),
        ..Model::default()
    }
}

fn text_stream(model: &Model, text: String) -> zedflow_ai::models::AssistantMessageEventStream {
    let stream = zedflow_ai::models::AssistantMessageEventStream::new();
    stream.push(AssistantMessageEvent::Done {
        reason: DoneStopReason::Stop,
        message: zedflow_ai::models::AssistantMessage {
            role: AssistantMessageRole::Assistant,
            content: vec![AssistantContentBlock::Text(TextContent {
                content_type: TextContentType::Text,
                text,
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
        },
    });
    stream
}

fn provider_streams(prefix: &'static str) -> ProviderStreams {
    ProviderStreams {
        stream: Arc::new(move |model, _, _| text_stream(model, format!("{prefix}:{}", model.id))),
        stream_simple: Arc::new(move |model, _, _: Option<&SimpleStreamOptions>| {
            text_stream(model, format!("{prefix}:{}", model.id))
        }),
    }
}

fn message_text(message: &zedflow_ai::models::AssistantMessage) -> String {
    message
        .content
        .iter()
        .filter_map(|block| match block {
            AssistantContentBlock::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn builtin_models_registers_every_builtin_provider_with_models() {
    let models = zedflow_ai::providers::all::builtin_models();
    let provider_ids = zedflow_ai::providers::all::get_builtin_providers();
    let providers = models.get_providers();

    assert_eq!(providers.len(), provider_ids.len());
    assert!(providers.iter().any(|provider| provider.id == "anthropic"));
    assert_eq!(
        models
            .get_model("anthropic", "claude-haiku-4-5")
            .map(|model| model.api),
        Some("anthropic-messages".to_owned())
    );
    assert!(models.get_models(None).len() > 500);
    assert!(
        providers
            .iter()
            .all(|provider| !provider.get_models().is_empty())
    );
}

#[test]
fn builtin_models_route_to_their_api_and_return_terminal_transport_errors() {
    let models = zedflow_ai::providers::all::builtin_models();
    let model = models.get_model("openai", "gpt-4").expect("builtin model");

    let result = models.complete(&model, &Context::default(), None);
    assert_eq!(result.stop_reason, StopReason::Error);
    assert!(
        result
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains(&model.provider)),
        "expected a terminal error from provider {}, got {:?}",
        model.provider,
        result.error_message
    );
}

#[test]
fn resolves_anthropic_auth_from_env_with_oauth_token_precedence() {
    let mut models = create_models_with_auth_context(FakeAuthContext::new([
        ("ANTHROPIC_API_KEY", "key"),
        ("ANTHROPIC_OAUTH_TOKEN", "oauth-token"),
    ]));
    models.set_provider(zedflow_ai::providers::anthropic::anthropic_provider().expect("provider"));
    let model = models
        .get_model("anthropic", "claude-haiku-4-5")
        .expect("model");

    let result = models
        .get_auth(&model)
        .expect("auth resolves")
        .expect("configured");
    assert_eq!(result.auth.api_key.as_deref(), Some("oauth-token"));
    assert_eq!(result.source.as_deref(), Some("ANTHROPIC_OAUTH_TOKEN"));
}

#[test]
fn reports_bedrock_as_configured_from_ambient_aws_credentials_without_an_api_key() {
    let mut models =
        create_models_with_auth_context(FakeAuthContext::new([("AWS_PROFILE", "dev")]));
    models.set_provider(zedflow_ai::providers::amazon_bedrock::amazon_bedrock_provider());
    let model = models.get_models(Some("amazon-bedrock")).remove(0);

    let result = models
        .get_auth(&model)
        .expect("auth resolves")
        .expect("configured");
    assert_eq!(result.auth, zedflow_ai::auth::types::ModelAuth::default());
    assert_eq!(result.source.as_deref(), Some("AWS_PROFILE"));

    let mut unconfigured = create_models_with_auth_context(FakeAuthContext::default());
    unconfigured.set_provider(zedflow_ai::providers::amazon_bedrock::amazon_bedrock_provider());
    assert!(
        unconfigured
            .get_auth(&model)
            .expect("auth resolves")
            .is_none()
    );
}

#[test]
fn requires_cloudflare_workers_ai_account_config_and_returns_scoped_env() {
    let mut missing =
        create_models_with_auth_context(FakeAuthContext::new([("CLOUDFLARE_API_KEY", "cf-key")]));
    missing.set_provider(
        zedflow_ai::providers::cloudflare_workers_ai::cloudflare_workers_ai_provider(),
    );
    let model = missing.get_models(Some("cloudflare-workers-ai")).remove(0);
    assert!(missing.get_auth(&model).expect("auth resolves").is_none());

    let mut configured = create_models_with_auth_context(FakeAuthContext::new([
        ("CLOUDFLARE_API_KEY", "cf-key"),
        ("CLOUDFLARE_ACCOUNT_ID", "account-id"),
    ]));
    configured.set_provider(
        zedflow_ai::providers::cloudflare_workers_ai::cloudflare_workers_ai_provider(),
    );
    let result = configured
        .get_auth(&model)
        .expect("auth resolves")
        .expect("configured");
    assert_eq!(result.auth.api_key.as_deref(), Some("cf-key"));
    assert_eq!(
        result.auth.base_url.as_deref(),
        Some("https://api.cloudflare.com/client/v4/accounts/account-id/ai/v1")
    );
    assert_eq!(
        result
            .env
            .as_ref()
            .and_then(|env| env.get("CLOUDFLARE_ACCOUNT_ID"))
            .map(String::as_str),
        Some("account-id")
    );
}

#[test]
fn requires_cloudflare_ai_gateway_account_and_gateway_config_and_returns_scoped_env_headers() {
    let mut missing = create_models_with_auth_context(FakeAuthContext::new([
        ("CLOUDFLARE_API_KEY", "cf-key"),
        ("CLOUDFLARE_ACCOUNT_ID", "account-id"),
    ]));
    missing.set_provider(
        zedflow_ai::providers::cloudflare_ai_gateway::cloudflare_ai_gateway_provider()
            .expect("provider"),
    );
    let model = missing.get_models(Some("cloudflare-ai-gateway")).remove(0);
    assert!(missing.get_auth(&model).expect("auth resolves").is_none());

    let mut configured = create_models_with_auth_context(FakeAuthContext::new([
        ("CLOUDFLARE_API_KEY", "cf-key"),
        ("CLOUDFLARE_ACCOUNT_ID", "account-id"),
        ("CLOUDFLARE_GATEWAY_ID", "gateway-id"),
    ]));
    configured.set_provider(
        zedflow_ai::providers::cloudflare_ai_gateway::cloudflare_ai_gateway_provider()
            .expect("provider"),
    );
    let result = configured
        .get_auth(&model)
        .expect("auth resolves")
        .expect("configured");
    let headers = result.auth.headers.expect("headers");
    assert_eq!(
        headers
            .get("cf-aig-authorization")
            .and_then(Option::as_deref),
        Some("Bearer cf-key")
    );
    assert_eq!(headers.get("Authorization"), Some(&None));
    assert_eq!(headers.get("x-api-key"), Some(&None));
    assert_eq!(
        result.auth.base_url.as_deref(),
        Some("https://gateway.ai.cloudflare.com/v1/account-id/gateway-id/anthropic")
    );
}

#[test]
fn resolves_vertex_via_adc_file_plus_project_and_location() {
    let adc = "~/.config/gcloud/application_default_credentials.json";
    let mut context = FakeAuthContext::new([
        ("GOOGLE_CLOUD_PROJECT", "proj"),
        ("GOOGLE_CLOUD_LOCATION", "us-central1"),
    ]);
    context.files.push(adc.to_owned());
    let mut configured = create_models_with_auth_context(context);
    configured.set_provider(
        zedflow_ai::providers::google_vertex::google_vertex_provider().expect("provider"),
    );
    let model = configured.get_models(Some("google-vertex")).remove(0);
    let result = configured
        .get_auth(&model)
        .expect("auth resolves")
        .expect("configured");
    assert_eq!(result.auth, zedflow_ai::auth::types::ModelAuth::default());
    assert!(
        result
            .source
            .as_deref()
            .unwrap_or_default()
            .contains("application default")
    );

    let mut keyed = create_models_with_auth_context(FakeAuthContext::new([(
        "GOOGLE_CLOUD_API_KEY",
        "vertex-key",
    )]));
    keyed.set_provider(
        zedflow_ai::providers::google_vertex::google_vertex_provider().expect("provider"),
    );
    assert_eq!(
        keyed
            .get_auth(&model)
            .expect("auth resolves")
            .expect("configured")
            .auth
            .api_key
            .as_deref(),
        Some("vertex-key")
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
fn create_provider_dispatches_on_model_api_for_mixed_api_providers() {
    let mut api = BTreeMap::new();
    api.insert("api-a".to_owned(), provider_streams("a"));
    api.insert("api-b".to_owned(), provider_streams("b"));
    let provider = create_provider(CreateProviderOptions {
        id: "mixed".to_owned(),
        name: None,
        base_url: Some("https://provider.example".to_owned()),
        headers: None,
        auth: ProviderAuth::default(),
        models: vec![
            test_model("api-a", "model-a"),
            test_model("api-b", "model-b"),
        ],
        refresh_models: None,
        api: ProviderApi::ByApi(api.into_iter().collect()),
    });

    assert_eq!(
        provider.base_url.as_deref(),
        Some("https://provider.example")
    );
    assert_eq!(
        message_text(&block_on(
            provider
                .stream(&test_model("api-a", "model-a"), &Context::default(), None)
                .result()
        )),
        "a:model-a"
    );
    assert_eq!(
        message_text(&block_on(
            provider
                .stream(&test_model("api-b", "model-b"), &Context::default(), None)
                .result()
        )),
        "b:model-b"
    );
}

#[test]
fn create_provider_merges_provider_resolved_env_into_stream_options() {
    let seen = Arc::new(Mutex::new(None));
    let seen_for_stream = Arc::clone(&seen);
    let provider = create_provider(CreateProviderOptions {
        id: "scoped".to_owned(),
        name: None,
        base_url: None,
        headers: None,
        auth: ProviderAuth {
            api_key: Some(Arc::new(RequestScopedAuth)),
            oauth: None,
        },
        models: vec![Model {
            provider: "scoped".to_owned(),
            id: "model-a".to_owned(),
            api: "test-api".to_owned(),
            ..Model::default()
        }],
        refresh_models: None,
        api: ProviderApi::Single(ProviderStreams {
            stream: Arc::new(move |model, _, options| {
                let options = options.expect("resolved request options");
                *seen_for_stream.lock().expect("seen lock") = Some(SeenRequestOptions {
                    base_url: model.base_url.clone(),
                    api_key: options.api_key.clone(),
                    provider: options
                        .env
                        .as_ref()
                        .and_then(|env| env.get("PROVIDER"))
                        .cloned(),
                    shared: options
                        .env
                        .as_ref()
                        .and_then(|env| env.get("SHARED"))
                        .cloned(),
                });
                text_stream(model, "ok".to_owned())
            }),
            stream_simple: Arc::new(|model, _, _| text_stream(model, "ok".to_owned())),
        }),
    });
    let mut models = create_models();
    models.set_provider(provider);
    let model = models
        .get_model("scoped", "model-a")
        .expect("catalog model");
    let options = zedflow_ai::models::StreamOptions {
        api_key: Some("request-key".to_owned()),
        env: Some(std::collections::HashMap::from([(
            "SHARED".to_owned(),
            "request".to_owned(),
        )])),
        ..zedflow_ai::models::StreamOptions::default()
    };

    let result = models.complete(&model, &Context::default(), Some(&options));
    assert_eq!(message_text(&result), "ok");
    assert!(model.base_url.is_empty(), "catalog model remains immutable");
    assert_eq!(
        seen.lock().expect("seen lock").clone(),
        Some(SeenRequestOptions {
            base_url: "https://provider.example".to_owned(),
            api_key: Some("request-key".to_owned()),
            provider: Some("provider".to_owned()),
            shared: Some("request".to_owned()),
        })
    );
}

#[test]
fn create_provider_produces_a_stream_error_for_a_model_whose_api_has_no_implementation() {
    let mut api = BTreeMap::new();
    api.insert("api-a".to_owned(), provider_streams("a"));
    let provider = create_provider(CreateProviderOptions {
        id: "mixed".to_owned(),
        name: None,
        base_url: None,
        headers: None,
        auth: ProviderAuth::default(),
        models: vec![test_model("api-missing", "model-missing")],
        refresh_models: None,
        api: ProviderApi::ByApi(api.into_iter().collect()),
    });

    let result = block_on(
        provider
            .stream(
                &test_model("api-missing", "model-missing"),
                &Context::default(),
                None,
            )
            .result(),
    );

    assert_eq!(result.stop_reason, StopReason::Error);
    assert!(
        result
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("no API implementation for \"api-missing\"")
    );
}

#[test]
fn create_provider_supports_dynamic_providers_empty_until_refreshed_in_flight_refreshes_deduped() {
    let fetches = Arc::new(AtomicUsize::new(0));
    let fetches_for_provider = Arc::clone(&fetches);
    let provider = create_provider(CreateProviderOptions {
        id: "dyn".to_owned(),
        name: None,
        base_url: None,
        headers: None,
        auth: ProviderAuth::default(),
        models: Vec::new(),
        refresh_models: Some(Arc::new(move || {
            fetches_for_provider.fetch_add(1, Ordering::SeqCst);
            let mut yielded = false;
            futures::future::poll_fn(move |cx| {
                if yielded {
                    Poll::Ready(Ok(vec![Model {
                        provider: "dyn".to_owned(),
                        id: "model-a".to_owned(),
                        api: "test-api".to_owned(),
                        ..Model::default()
                    }]))
                } else {
                    yielded = true;
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            })
            .boxed()
        })),
        api: ProviderApi::Single(provider_streams("dyn")),
    });

    assert!(provider.get_models().is_empty());
    let (first, second) = block_on(join(provider.refresh_models(), provider.refresh_models()));
    first.expect("first refresh");
    second.expect("second refresh");
    assert_eq!(fetches.load(Ordering::SeqCst), 1);
    assert_eq!(provider.get_models()[0].id, "model-a");

    block_on(provider.refresh_models()).expect("later refresh");
    assert_eq!(fetches.load(Ordering::SeqCst), 2);
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
    let result = models.complete(&model, &Context::default(), None);
    let text = result
        .content
        .iter()
        .filter_map(|block| match block {
            AssistantContentBlock::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect::<String>();

    assert_eq!(text, "hello from faux");
    assert_eq!(faux.state.call_count(), 1);
}
