use std::sync::{Arc, Mutex};

use zedflow_ai::images_models::{
    AssistantImages, CreateImagesProviderOptions, ImagesContext, ImagesModel, ImagesOptions,
    ImagesProvider, ProviderAuth, create_images_models, create_images_provider,
};
use zedflow_ai::providers::all::builtin_images_models;

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
        input: vec!["a red circle".to_string()],
    }
}

fn test_provider(
    id: &str,
    models: Vec<ImagesModel>,
    calls: Option<Arc<Mutex<Vec<Option<ImagesOptions>>>>>,
) -> ImagesProvider {
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
                        .push(options);
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
}

#[test]
#[ignore = "blocked: ImagesModels uses HashMap, not Pi's insertion-ordered Map, so multi-provider order assertions are not yet deterministic"]
fn registers_multiple_providers_in_insertion_order() {
    panic!(
        "Pi assertion blocked: expected provider ids [p1, p2] and model ids [m1, m2, m3] in registration order"
    );
}

#[test]
#[ignore = "blocked: Rust ImagesProvider lacks Pi provider auth resolver and auth context support"]
fn resolves_auth_through_provider_and_merges_it_into_requests() {
    panic!(
        "Pi assertions blocked: getAuth should resolve env-key, generateImages should apply env-key, and explicit options should win"
    );
}

#[test]
#[ignore = "blocked: Rust ImagesProvider lacks resolved provider env support"]
fn merges_provider_resolved_env_into_image_options() {
    panic!(
        "Pi assertions blocked: provider env should merge with request env, and request env should win shared keys"
    );
}

#[test]
#[ignore = "blocked: Rust create_images_provider does not expose Pi's concurrent in-flight refresh de-duplication semantics"]
fn supports_dynamic_providers_via_refresh_with_in_flight_dedupe() {
    panic!(
        "Pi assertions blocked: concurrent refresh calls should share one fetch, single-provider failures should be ModelsError code model_source, and all-provider refresh should resolve best-effort"
    );
}

#[test]
#[ignore = "blocked: builtin_images_models does not accept Pi auth context or resolve OPENROUTER_API_KEY yet"]
fn builtin_images_models_resolves_openrouter_api_key_from_auth_context() {
    panic!("Pi assertion blocked: getAuth(first openrouter model).auth.apiKey should equal or-key");
}
