use std::{collections::BTreeMap, fs, sync::Arc};
use zedflow_ai::{
    auth::types::{AuthFuture, AuthLoginCallbacks, AuthResult, OAuthCredential},
    compat::{self, CompatError},
    types::{Context, Model},
    utils::oauth::index::{OAuthProviderInterface, get_oauth_provider},
};
use zedflow_coding_agent::{
    auth_storage::{AuthCredential, AuthStorage},
    model_registry::{ModelRegistry, ProviderConfigInput},
};

fn temp_path(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "zedflow-model-registry-{name}-{}",
        std::process::id()
    ))
}

#[tokio::test]
async fn loads_custom_models_overrides_and_resolves_request_auth() {
    let path = temp_path("models.json");
    fs::write(
        &path,
        r#"{
      // JSON comments are accepted by Pi.
      "providers": {
        "anthropic": { "baseUrl": "https://proxy.test", "modelOverrides": {
          "claude-sonnet-4-5": { "maxTokens": 1234 }
        }},
        "local": { "baseUrl": "http://localhost:11434/v1", "api": "openai-completions",
          "apiKey": "secret", "authHeader": true, "headers": { "X-Test": "yes" },
          "models": [{ "id": "tiny" }]
        }
      }
    }"#,
    )
    .unwrap();

    let mut registry = ModelRegistry::create(AuthStorage::in_memory(Default::default()), &path);
    assert!(registry.get_error().is_none());
    assert_eq!(
        registry.find("local", "tiny").unwrap().context_window,
        128_000
    );
    assert!(
        registry
            .get_all()
            .iter()
            .filter(|m| m.provider == "anthropic")
            .all(|m| m.base_url == "https://proxy.test")
    );

    let model = registry.find("local", "tiny").unwrap().clone();
    let auth = registry.get_api_key_and_headers(&model).await.unwrap();
    assert_eq!(auth.api_key.as_deref(), Some("secret"));
    assert_eq!(
        auth.headers
            .as_ref()
            .unwrap()
            .get("Authorization")
            .map(String::as_str),
        Some("Bearer secret")
    );
    assert_eq!(
        auth.headers
            .as_ref()
            .unwrap()
            .get("X-Test")
            .map(String::as_str),
        Some("yes")
    );

    fs::remove_file(path).ok();
}

#[test]
fn registers_custom_stream_and_oauth_provider() {
    struct OAuth;
    impl OAuthProviderInterface for OAuth {
        fn id(&self) -> &str {
            "ignored"
        }
        fn name(&self) -> &str {
            "Custom OAuth"
        }
        fn login<'a>(
            &'a self,
            _: &'a dyn AuthLoginCallbacks,
        ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
            Box::pin(async { Err("unused".into()) })
        }
        fn refresh_token<'a>(
            &'a self,
            _: &'a OAuthCredential,
        ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
            Box::pin(async { Err("unused".into()) })
        }
        fn get_api_key(&self, credentials: &OAuthCredential) -> String {
            credentials.access.clone()
        }
    }

    let mut registry = ModelRegistry::in_memory(AuthStorage::in_memory(Default::default()));
    let api = "custom-test-api".to_string();
    registry
        .register_provider(
            "dynamic",
            ProviderConfigInput {
                api: Some(api.clone()),
                oauth: Some(Arc::new(OAuth)),
                stream_simple: Some(Arc::new(|_, _, _| {
                    Err(CompatError::Porting("called".into()))
                })),
                ..Default::default()
            },
        )
        .unwrap();

    let model = Model {
        api,
        provider: "dynamic".into(),
        ..Default::default()
    };
    assert!(
        matches!(compat::stream_simple(&model, &Context::default(), None), Err(CompatError::Porting(message)) if message == "called")
    );
    assert_eq!(
        get_oauth_provider("dynamic").unwrap().name(),
        "Custom OAuth"
    );

    registry.unregister_provider("dynamic");
    assert!(get_oauth_provider("dynamic").is_none());
}

#[test]
fn oauth_provider_modifies_registered_models_when_credentials_exist() {
    struct OAuth;
    impl OAuthProviderInterface for OAuth {
        fn id(&self) -> &str {
            "dynamic-modify"
        }
        fn name(&self) -> &str {
            "Dynamic OAuth"
        }
        fn login<'a>(
            &'a self,
            _: &'a dyn AuthLoginCallbacks,
        ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
            Box::pin(async { Err("unused".into()) })
        }
        fn refresh_token<'a>(
            &'a self,
            _: &'a OAuthCredential,
        ) -> AuthFuture<'a, AuthResult<OAuthCredential>> {
            Box::pin(async { Err("unused".into()) })
        }
        fn get_api_key(&self, credentials: &OAuthCredential) -> String {
            credentials.access.clone()
        }
        fn modify_models(&self, models: &[Model], credentials: &OAuthCredential) -> Vec<Model> {
            models
                .iter()
                .cloned()
                .map(|mut model| {
                    if model.provider == "dynamic-modify" {
                        model.base_url = format!("https://{}.test", credentials.access);
                    }
                    model
                })
                .collect()
        }
    }

    let mut auth = AuthStorage::in_memory(Default::default());
    auth.set(
        "dynamic-modify",
        AuthCredential::OAuth {
            refresh: "refresh".into(),
            access: "credential".into(),
            expires: i64::MAX,
            extra: BTreeMap::new(),
        },
    )
    .unwrap();
    let mut registry = ModelRegistry::in_memory(auth);
    registry
        .register_provider(
            "dynamic-modify",
            ProviderConfigInput {
                api: Some("custom-test-api".into()),
                base_url: Some("https://before.test".into()),
                oauth: Some(Arc::new(OAuth)),
                models: Some(vec![Model {
                    id: "model".into(),
                    api: "custom-test-api".into(),
                    ..Default::default()
                }]),
                ..Default::default()
            },
        )
        .unwrap();

    assert_eq!(
        registry.find("dynamic-modify", "model").unwrap().base_url,
        "https://credential.test"
    );
}

#[test]
fn invalid_custom_provider_keeps_builtin_models() {
    let path = temp_path("invalid.json");
    fs::write(
        &path,
        r#"{"providers":{"local":{"api":"openai-completions","models":[{"id":"tiny"}]}}}"#,
    )
    .unwrap();
    let registry = ModelRegistry::create(AuthStorage::in_memory(Default::default()), &path);
    assert!(registry.get_error().unwrap().contains("baseUrl"));
    assert!(registry.get_all().iter().any(|m| m.provider == "anthropic"));
    fs::remove_file(path).ok();
}
