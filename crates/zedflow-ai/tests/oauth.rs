//! Port of Pi `packages/ai/test/oauth.ts`.
//!
//! The source file is a test helper for `~/.pi/agent/auth.json`; these tests cover its deterministic
//! storage and OAuth-token resolution behavior without live provider calls.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use futures::executor::block_on;
use zedflow_ai::auth::types::{Credential, OAuthCredential};
use zedflow_ai::utils::oauth::index::get_oauth_api_key;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

type AuthStorage = BTreeMap<String, Credential>;

fn load_auth_storage(auth_path: &Path) -> AuthStorage {
    let Ok(content) = fs::read_to_string(auth_path) else {
        return AuthStorage::new();
    };

    serde_json::from_str(&content).unwrap_or_default()
}

fn save_auth_storage(auth_path: &Path, storage: &AuthStorage) -> io::Result<()> {
    if let Some(config_dir) = auth_path.parent() {
        fs::create_dir_all(config_dir)?;
        #[cfg(unix)]
        fs::set_permissions(config_dir, fs::Permissions::from_mode(0o700))?;
    }

    fs::write(auth_path, serde_json::to_string_pretty(storage)?)?;
    #[cfg(unix)]
    fs::set_permissions(auth_path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

async fn resolve_api_key(auth_path: &Path, provider: &str) -> Option<String> {
    let mut storage = load_auth_storage(auth_path);
    let entry = storage.get(provider)?.clone();

    match entry {
        Credential::ApiKey(credential) => credential.key,
        Credential::OAuth(_) => {
            let oauth_credentials = storage
                .iter()
                .filter_map(|(provider_id, credential)| match credential {
                    Credential::OAuth(credential) => {
                        Some((provider_id.clone(), credential.clone()))
                    }
                    Credential::ApiKey(_) => None,
                })
                .collect::<BTreeMap<String, OAuthCredential>>();

            let result = get_oauth_api_key(provider, &oauth_credentials)
                .await
                .ok()??;
            storage.insert(
                provider.to_owned(),
                Credential::OAuth(result.new_credentials),
            );
            save_auth_storage(auth_path, &storage).ok()?;
            Some(result.api_key)
        }
    }
}

#[test]
fn returns_none_when_auth_file_is_missing() {
    let sandbox = Sandbox::new("missing");

    assert_eq!(
        block_on(resolve_api_key(&sandbox.auth_path, "anthropic")),
        None
    );
}

#[test]
fn returns_api_key_credentials_directly() {
    let sandbox = Sandbox::new("api-key");
    fs::create_dir_all(sandbox.auth_path.parent().expect("auth path has parent"))
        .expect("create auth directory");
    fs::write(
        &sandbox.auth_path,
        r#"{"anthropic":{"type":"api_key","key":"sk-test"}}"#,
    )
    .expect("write auth fixture");

    assert_eq!(
        block_on(resolve_api_key(&sandbox.auth_path, "anthropic")).as_deref(),
        Some("sk-test")
    );
}

#[test]
fn malformed_auth_file_behaves_like_empty_storage() {
    let sandbox = Sandbox::new("malformed");
    fs::create_dir_all(sandbox.auth_path.parent().expect("auth path has parent"))
        .expect("create auth directory");
    fs::write(&sandbox.auth_path, "not json").expect("write malformed fixture");

    assert_eq!(
        block_on(resolve_api_key(&sandbox.auth_path, "anthropic")),
        None
    );
}

#[test]
fn returns_oauth_access_token_and_saves_storage_shape() {
    let sandbox = Sandbox::new("oauth");
    let mut storage = AuthStorage::new();
    storage.insert(
        "anthropic".to_owned(),
        Credential::OAuth(OAuthCredential {
            refresh: "refresh-token".to_owned(),
            access: "access-token".to_owned(),
            expires: i64::MAX,
            extra: BTreeMap::new(),
        }),
    );
    save_auth_storage(&sandbox.auth_path, &storage).expect("write oauth fixture");

    assert_eq!(
        block_on(resolve_api_key(&sandbox.auth_path, "anthropic")).as_deref(),
        Some("access-token")
    );

    let saved = fs::read_to_string(&sandbox.auth_path).expect("read saved auth storage");
    assert!(saved.contains(r#""type": "oauth""#));
    assert!(saved.contains(r#""access": "access-token""#));
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(&sandbox.auth_path)
            .expect("read saved auth file metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn oauth_resolution_errors_return_none() {
    let sandbox = Sandbox::new("unknown-oauth");
    let mut storage = AuthStorage::new();
    storage.insert(
        "unknown".to_owned(),
        Credential::OAuth(OAuthCredential {
            refresh: "refresh-token".to_owned(),
            access: "access-token".to_owned(),
            expires: i64::MAX,
            extra: BTreeMap::new(),
        }),
    );
    save_auth_storage(&sandbox.auth_path, &storage).expect("write unknown oauth fixture");

    assert_eq!(
        block_on(resolve_api_key(&sandbox.auth_path, "unknown")),
        None
    );
}

struct Sandbox {
    root: PathBuf,
    auth_path: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "zedflow-ai-oauth-{name}-{}-{unique}",
            std::process::id()
        ));
        let auth_path = root.join(".pi").join("agent").join("auth.json");
        Self { root, auth_path }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
