use std::{
    collections::BTreeMap,
    fs,
    time::{SystemTime, UNIX_EPOCH},
};
use zedflow_coding_agent::auth_storage::{AuthCredential, AuthStorage};

fn path() -> std::path::PathBuf {
    std::env::temp_dir()
        .join(format!(
            "zedflow-auth-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
        .join("auth.json")
}

#[tokio::test]
async fn runtime_override_and_locked_updates_preserve_other_providers() {
    let path = path();
    let mut first = AuthStorage::create(&path);
    first
        .set(
            "anthropic",
            AuthCredential::ApiKey {
                key: "stored".into(),
                env: None,
            },
        )
        .unwrap();

    let mut second = AuthStorage::create(&path);
    second
        .set(
            "openai",
            AuthCredential::ApiKey {
                key: "other".into(),
                env: None,
            },
        )
        .unwrap();
    first.set_runtime_api_key("anthropic", "runtime");

    assert_eq!(
        first.get_api_key("anthropic", true).await.as_deref(),
        Some("runtime")
    );
    first.reload();
    assert!(first.has("anthropic") && first.has("openai"));
    assert_eq!(fs::metadata(&path).unwrap().permissions().readonly(), false);
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[tokio::test]
async fn command_keys_resolve_and_fallback_can_be_disabled() {
    let mut data = BTreeMap::new();
    data.insert(
        "test".into(),
        AuthCredential::ApiKey {
            key: "!printf ' key '".into(),
            env: None,
        },
    );
    let mut storage = AuthStorage::in_memory(data);
    assert_eq!(
        storage.get_api_key("test", true).await.as_deref(),
        Some("key")
    );
    assert_eq!(storage.get_api_key("missing", false).await, None);
}
