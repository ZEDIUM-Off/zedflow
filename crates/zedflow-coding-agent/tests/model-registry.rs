use std::fs;
use zedflow_coding_agent::{auth_storage::AuthStorage, model_registry::ModelRegistry};

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
