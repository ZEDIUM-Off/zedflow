use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use futures::channel::oneshot;
use zedflow_ai::auth::types::{
    ApiKeyResolveInput, AuthContext, AuthFuture, AuthResult, ModelAuth, ProviderAuth, ProviderEnv,
    ProviderHeaders, ResolvedAuth,
};
use zedflow_ai::images_models::{
    AssistantImages, CreateImagesProviderOptions, ImagesContext, ImagesModel, ImagesOptions,
    ImagesProvider, ModelsError, create_images_models, create_images_provider,
};
use zedflow_ai::providers::all::{builtin_images_models, builtin_images_models_with_auth_context};

type ImageCalls = Arc<Mutex<Vec<(ImagesModel, Option<ImagesOptions>)>>>;

fn test_image_model(provider: &str, id: &str) -> ImagesModel {
    ImagesModel {
        id: id.to_string(),
        api: "test-images".to_string(),
        provider: provider.to_string(),
        base_url: Some("https://example.test/v1".to_string()),
    }
}

fn ok_result(model: ImagesModel) -> AssistantImages {
    AssistantImages {
        api: model.api,
        provider: model.provider,
        model: model.id,
        output: vec!["aGk=".to_string()],
        stop_reason: "stop".to_string(),
        error_message: None,
    }
}

fn test_context() -> ImagesContext {
    ImagesContext {
        input: vec![zedflow_ai::images_models::ImagesContent::Text {
            text: "a red circle".to_string(),
        }],
    }
}

fn test_provider(id: &str, models: Vec<ImagesModel>, calls: Option<ImageCalls>) -> ImagesProvider {
    create_images_provider(CreateImagesProviderOptions {
        id: id.to_string(),
        name: None,
        auth: ProviderAuth::default(),
        models,
        refresh_models: None,
        generate_images: Arc::new(move |model, _context, options| {
            let calls = calls.clone();
            Box::pin(async move {
                if let Some(calls) = calls {
                    calls
                        .lock()
                        .expect("recorded image calls lock should not be poisoned")
                        .push((model.clone(), options));
                }
                ok_result(model)
            })
        }),
    })
}

#[test]
fn registers_provider_and_reads_models_synchronously() {
    let mut models = create_images_models();
    models.set_provider(test_provider(
        "p1",
        vec![test_image_model("p1", "m1"), test_image_model("p1", "m2")],
        None,
    ));

    assert_eq!(models.get_providers()[0].id, "p1");
    assert_eq!(
        models
            .get_models(None)
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>(),
        vec!["m1", "m2"]
    );
    assert_eq!(
        models
            .get_models(Some("p1"))
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>(),
        vec!["m1", "m2"]
    );
    assert_eq!(
        models
            .get_model("p1", "m2")
            .expect("registered image model should be found")
            .id,
        "m2"
    );
    assert!(models.get_model("p1", "missing").is_none());

    models.delete_provider("p1");
    assert!(models.get_provider("p1").is_none());
}

#[test]
fn explicit_image_options_are_forwarded_to_generation() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut models = create_images_models();
    models.set_provider(test_provider(
        "p1",
        vec![test_image_model("p1", "model-a")],
        Some(Arc::clone(&calls)),
    ));
    let model = models
        .get_model("p1", "model-a")
        .expect("registered image model should be found");

    let result = futures::executor::block_on(models.generate_images(
        model,
        test_context(),
        Some(ImagesOptions {
            api_key: Some("explicit".to_string()),
            ..ImagesOptions::default()
        }),
    ));

    assert_eq!(result.stop_reason, "stop");
    assert_eq!(
        calls
            .lock()
            .expect("recorded image calls lock should not be poisoned")[0]
            .1
            .as_ref()
            .and_then(|options| options.api_key.as_deref()),
        Some("explicit")
    );
}

#[test]
fn returns_error_for_unknown_provider_and_dispatches_without_auth() {
    let mut models = create_images_models();
    let ghost = futures::executor::block_on(models.generate_images(
        test_image_model("ghost", "m"),
        test_context(),
        None,
    ));
    assert_eq!(ghost.stop_reason, "error");
    assert!(
        ghost
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains("Unknown provider: ghost"))
    );

    let calls = Arc::new(Mutex::new(Vec::new()));
    models.set_provider(test_provider(
        "p1",
        vec![test_image_model("p1", "model-a")],
        Some(Arc::clone(&calls)),
    ));
    let model = models
        .get_model("p1", "model-a")
        .expect("registered image model should be found");
    assert!(
        futures::executor::block_on(models.get_auth(&model))
            .expect("auth lookup should not fail")
            .is_none()
    );

    futures::executor::block_on(models.generate_images(model, test_context(), None));

    assert!(
        calls
            .lock()
            .expect("recorded image calls lock should not be poisoned")[0]
            .1
            .as_ref()
            .and_then(|options| options.api_key.as_deref())
            .is_none()
    );
}

#[test]
fn builtin_images_models_registers_openrouter_catalog() {
    let models = builtin_images_models();
    let providers = models.get_providers();
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].id, "openrouter");

    let list = models.get_models(Some("openrouter"));
    assert!(!list.is_empty());
    assert!(list.iter().all(|model| model.api == "openrouter-images"));
    assert!(
        list.iter()
            .all(|model| model.base_url.as_deref() == Some("https://openrouter.ai/api/v1"))
    );
}

#[test]
fn registers_multiple_providers_in_insertion_order() {
    let mut models = create_images_models();
    models.set_provider(test_provider(
        "p1",
        vec![test_image_model("p1", "m1"), test_image_model("p1", "m2")],
        None,
    ));
    models.set_provider(test_provider(
        "p2",
        vec![test_image_model("p2", "m3")],
        None,
    ));

    assert_eq!(
        models
            .get_providers()
            .iter()
            .map(|provider| provider.id.as_str())
            .collect::<Vec<_>>(),
        vec!["p1", "p2"]
    );
    assert_eq!(
        models
            .get_models(None)
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>(),
        vec!["m1", "m2", "m3"]
    );

    models.set_provider(test_provider(
        "p1",
        vec![test_image_model("p1", "m4")],
        None,
    ));
    assert_eq!(
        models
            .get_providers()
            .iter()
            .map(|provider| provider.id.as_str())
            .collect::<Vec<_>>(),
        vec!["p1", "p2"]
    );
    assert_eq!(
        models
            .get_models(None)
            .iter()
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>(),
        vec!["m4", "m3"]
    );
}

#[test]
fn resolves_auth_through_provider_and_merges_it_into_requests() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut models = create_images_models();
    models.set_provider(auth_provider(
        "p1",
        TestAuth {
            api_key: Some("resolved"),
            base_url: Some("https://auth.example/v1"),
            headers: ProviderHeaders::from([("x-auth".to_string(), Some("resolved".to_string()))]),
            env: ProviderEnv::from([("AUTH_ENV".to_string(), "resolved".to_string())]),
        },
        Some(Arc::clone(&calls)),
    ));
    let model = models.get_model("p1", "model-a").expect("model");

    let auth = futures::executor::block_on(models.get_auth(&model))
        .expect("auth lookup should resolve")
        .expect("auth should exist");
    assert_eq!(auth.auth.api_key.as_deref(), Some("resolved"));

    let result = futures::executor::block_on(models.generate_images(
        model,
        test_context(),
        Some(ImagesOptions {
            api_key: Some("explicit".to_string()),
            headers: ProviderHeaders::from([("x-explicit".to_string(), Some("yes".to_string()))]),
            env: ProviderEnv::from([("AUTH_ENV".to_string(), "explicit".to_string())]),
            ..ImagesOptions::default()
        }),
    ));

    assert_eq!(result.stop_reason, "stop");
    let call = &calls.lock().expect("recorded calls")[0];
    assert_eq!(call.0.base_url.as_deref(), Some("https://auth.example/v1"));
    let options = call.1.as_ref().expect("auth creates request options");
    assert_eq!(options.api_key.as_deref(), Some("explicit"));
    assert_eq!(
        options.headers.get("x-auth").and_then(Option::as_deref),
        Some("resolved")
    );
    assert_eq!(
        options.headers.get("x-explicit").and_then(Option::as_deref),
        Some("yes")
    );
    assert_eq!(
        options.env.get("AUTH_ENV").map(String::as_str),
        Some("explicit")
    );
}

#[test]
fn merges_provider_resolved_env_into_image_options() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut models = create_images_models();
    models.set_provider(auth_provider(
        "p1",
        TestAuth {
            api_key: Some("resolved"),
            base_url: None,
            headers: ProviderHeaders::new(),
            env: ProviderEnv::from([
                ("SHARED".to_string(), "resolved".to_string()),
                ("AUTH_ONLY".to_string(), "yes".to_string()),
            ]),
        },
        Some(Arc::clone(&calls)),
    ));
    let model = models.get_model("p1", "model-a").expect("model");

    futures::executor::block_on(models.generate_images(
        model,
        test_context(),
        Some(ImagesOptions {
            env: ProviderEnv::from([
                ("SHARED".to_string(), "explicit".to_string()),
                ("REQUEST_ONLY".to_string(), "yes".to_string()),
            ]),
            ..ImagesOptions::default()
        }),
    ));

    let options = calls.lock().expect("recorded calls")[0]
        .1
        .clone()
        .expect("auth creates request options");
    assert_eq!(
        options.env.get("SHARED").map(String::as_str),
        Some("explicit")
    );
    assert_eq!(
        options.env.get("AUTH_ONLY").map(String::as_str),
        Some("yes")
    );
    assert_eq!(
        options.env.get("REQUEST_ONLY").map(String::as_str),
        Some("yes")
    );
}

#[test]
fn supports_dynamic_providers_via_refresh_with_in_flight_dedupe() {
    let calls = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = oneshot::channel::<Vec<ImagesModel>>();
    let rx = Arc::new(Mutex::new(Some(rx)));
    let provider = create_images_provider(CreateImagesProviderOptions {
        id: "dynamic".to_string(),
        name: None,
        auth: ProviderAuth::default(),
        models: Vec::new(),
        refresh_models: Some(Arc::new({
            let calls = Arc::clone(&calls);
            let rx = Arc::clone(&rx);
            move || {
                calls.fetch_add(1, Ordering::SeqCst);
                let rx = rx
                    .lock()
                    .expect("refresh receiver lock")
                    .take()
                    .expect("single fetch should be shared");
                Box::pin(async move {
                    rx.await
                        .map_err(|error| ModelsError::new("model_source", error.to_string()))
                })
            }
        })),
        generate_images: Arc::new(|model, _, _| Box::pin(async move { ok_result(model) })),
    });
    let mut models = create_images_models();
    models.set_provider(provider.clone());

    let sender = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(10));
        tx.send(vec![test_image_model("dynamic", "fresh")])
            .expect("receiver should be alive");
    });
    futures::executor::block_on(async {
        let (first, second) = futures::join!(provider.refresh_models(), provider.refresh_models());
        first.expect("first refresh should resolve");
        second.expect("second refresh should share in-flight fetch");
    });
    sender.join().expect("refresh sender thread should finish");

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        models
            .get_model("dynamic", "fresh")
            .expect("fresh model")
            .id,
        "fresh"
    );
}

#[test]
fn refresh_wraps_single_provider_failures_and_all_provider_refresh_is_best_effort() {
    let failing = create_images_provider(CreateImagesProviderOptions {
        id: "failing".to_string(),
        name: None,
        auth: ProviderAuth::default(),
        models: vec![test_image_model("failing", "old")],
        refresh_models: Some(Arc::new(|| {
            Box::pin(async { Err(ModelsError::new("boom", "network down")) })
        })),
        generate_images: Arc::new(|model, _, _| Box::pin(async move { ok_result(model) })),
    });
    let mut models = create_images_models();
    models.set_provider(failing);

    let error = futures::executor::block_on(models.refresh(Some("failing")))
        .expect_err("single-provider refresh should reject");
    assert_eq!(error.kind, "model_source");
    assert!(error.message.contains("Model refresh failed for failing"));
    assert_eq!(
        models
            .get_model("failing", "old")
            .expect("old model kept")
            .id,
        "old"
    );

    futures::executor::block_on(models.refresh(None)).expect("all-provider refresh is best-effort");
}

#[test]
fn builtin_images_models_resolves_openrouter_api_key_from_auth_context() {
    let models = builtin_images_models_with_auth_context(TestContext(BTreeMap::from([(
        "OPENROUTER_API_KEY".to_string(),
        "or-key".to_string(),
    )])));
    let model = models
        .get_models(Some("openrouter"))
        .into_iter()
        .next()
        .expect("openrouter model");

    let auth = futures::executor::block_on(models.get_auth(&model))
        .expect("auth lookup should not fail")
        .expect("openrouter auth should resolve");

    assert_eq!(auth.auth.api_key.as_deref(), Some("or-key"));
}

fn auth_provider(id: &str, auth: TestAuth, calls: Option<ImageCalls>) -> ImagesProvider {
    create_images_provider(CreateImagesProviderOptions {
        id: id.to_string(),
        name: None,
        auth: ProviderAuth {
            api_key: Some(Arc::new(auth)),
            oauth: None,
        },
        models: vec![test_image_model(id, "model-a")],
        refresh_models: None,
        generate_images: Arc::new(move |model, _context, options| {
            let calls = calls.clone();
            Box::pin(async move {
                if let Some(calls) = calls {
                    calls
                        .lock()
                        .expect("recorded image calls lock should not be poisoned")
                        .push((model.clone(), options));
                }
                ok_result(model)
            })
        }),
    })
}

#[derive(Debug, Clone)]
struct TestAuth {
    api_key: Option<&'static str>,
    base_url: Option<&'static str>,
    headers: ProviderHeaders,
    env: ProviderEnv,
}

impl zedflow_ai::auth::types::ApiKeyAuth for TestAuth {
    fn name(&self) -> &str {
        "Test API key"
    }

    fn resolve<'a>(
        &'a self,
        _input: ApiKeyResolveInput<'a>,
    ) -> AuthFuture<'a, AuthResult<Option<ResolvedAuth>>> {
        Box::pin(async move {
            Ok(Some(ResolvedAuth {
                auth: ModelAuth {
                    api_key: self.api_key.map(str::to_string),
                    headers: (!self.headers.is_empty()).then(|| self.headers.clone()),
                    base_url: self.base_url.map(str::to_string),
                },
                env: (!self.env.is_empty()).then(|| self.env.clone()),
                source: Some("test".to_string()),
            }))
        })
    }
}

#[derive(Debug, Clone)]
struct TestContext(BTreeMap<String, String>);

impl AuthContext for TestContext {
    fn env<'a>(&'a self, name: &'a str) -> AuthFuture<'a, Option<String>> {
        Box::pin(async move { self.0.get(name).cloned() })
    }

    fn file_exists<'a>(&'a self, _path: &'a str) -> AuthFuture<'a, bool> {
        Box::pin(async { false })
    }
}
