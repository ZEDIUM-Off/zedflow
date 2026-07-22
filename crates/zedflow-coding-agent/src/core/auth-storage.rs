//! Credential storage ported from Pi's `core/auth-storage.ts`.

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use zedflow_ai::{
    auth::types::{AuthLoginCallbacks, OAuthCredential},
    env_api_keys::{find_env_keys, get_env_api_key},
    utils::oauth::index::{OAuthProviderInterface, get_oauth_provider, get_oauth_providers},
};

pub type AuthStorageData = BTreeMap<String, AuthCredential>;
pub type AuthError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AuthCredential {
    #[serde(rename = "api_key")]
    ApiKey {
        key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        env: Option<HashMap<String, String>>,
    },
    #[serde(rename = "oauth")]
    OAuth {
        refresh: String,
        access: String,
        expires: i64,
        #[serde(default, flatten, skip_serializing_if = "BTreeMap::is_empty")]
        extra: BTreeMap<String, serde_json::Value>,
    },
}

impl AuthCredential {
    fn oauth(&self) -> Option<OAuthCredential> {
        match self {
            Self::OAuth {
                refresh,
                access,
                expires,
                extra,
            } => Some(OAuthCredential {
                refresh: refresh.clone(),
                access: access.clone(),
                expires: *expires,
                extra: extra.clone(),
            }),
            _ => None,
        }
    }
}

impl From<OAuthCredential> for AuthCredential {
    fn from(value: OAuthCredential) -> Self {
        Self::OAuth {
            refresh: value.refresh,
            access: value.access,
            expires: value.expires,
            extra: value.extra,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthSource {
    Stored,
    Runtime,
    Environment,
    Fallback,
    ModelsJsonKey,
    ModelsJsonCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthStatus {
    pub configured: bool,
    pub source: Option<AuthSource>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct GetApiKeyOptions {
    pub include_fallback: bool,
}
impl Default for GetApiKeyOptions {
    fn default() -> Self {
        Self {
            include_fallback: true,
        }
    }
}

#[derive(Clone)]
enum Backend {
    File(PathBuf),
    Memory(Arc<Mutex<Option<String>>>),
}

pub struct AuthStorage {
    backend: Backend,
    data: AuthStorageData,
    runtime_overrides: HashMap<String, String>,
    load_error: Option<String>,
    errors: Vec<String>,
}

impl AuthStorage {
    pub fn create(path: impl Into<PathBuf>) -> Self {
        Self::new(Backend::File(path.into()))
    }
    pub fn in_memory(data: AuthStorageData) -> Self {
        let value = serde_json::to_string_pretty(&data).ok();
        Self::new(Backend::Memory(Arc::new(Mutex::new(value))))
    }
    fn new(backend: Backend) -> Self {
        let mut value = Self {
            backend,
            data: BTreeMap::new(),
            runtime_overrides: HashMap::new(),
            load_error: None,
            errors: Vec::new(),
        };
        value.reload();
        value
    }

    fn locked<T>(
        &self,
        update: bool,
        f: impl FnOnce(Option<&str>) -> Result<(T, Option<String>), AuthError>,
    ) -> Result<T, AuthError> {
        match &self.backend {
            Backend::Memory(value) => {
                let mut guard = value
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let (result, next) = f(guard.as_deref())?;
                if update {
                    if let Some(next) = next {
                        *guard = Some(next);
                    }
                }
                Ok(result)
            }
            Backend::File(path) => {
                ensure_file(path)?;
                let mut file = OpenOptions::new().read(true).write(true).open(path)?;
                file.lock_exclusive()?;
                let result = (|| {
                    let mut current = String::new();
                    file.read_to_string(&mut current)?;
                    let (result, next) = f(Some(&current))?;
                    if update {
                        if let Some(next) = next {
                            file.seek(SeekFrom::Start(0))?;
                            file.set_len(0)?;
                            file.write_all(next.as_bytes())?;
                            file.sync_all()?;
                            set_private_file_mode(path)?;
                        }
                    }
                    Ok(result)
                })();
                let _ = file.unlock();
                result
            }
        }
    }

    pub fn reload(&mut self) {
        match self.locked(false, |current| Ok((parse(current)?, None))) {
            Ok(data) => {
                self.data = data;
                self.load_error = None;
            }
            Err(error) => {
                self.load_error = Some(error.to_string());
                self.errors.push(error.to_string());
            }
        }
    }
    pub fn set_runtime_api_key(&mut self, provider: impl Into<String>, key: impl Into<String>) {
        self.runtime_overrides.insert(provider.into(), key.into());
    }
    pub fn remove_runtime_api_key(&mut self, provider: &str) {
        self.runtime_overrides.remove(provider);
    }
    pub fn get(&self, provider: &str) -> Option<&AuthCredential> {
        self.data.get(provider)
    }
    pub fn get_provider_env(&self, provider: &str) -> Option<HashMap<String, String>> {
        match self.data.get(provider) {
            Some(AuthCredential::ApiKey { env, .. }) => env.clone(),
            _ => None,
        }
    }
    pub fn list(&self) -> Vec<String> {
        self.data.keys().cloned().collect()
    }
    pub fn has(&self, provider: &str) -> bool {
        self.data.contains_key(provider)
    }
    pub fn get_all(&self) -> AuthStorageData {
        self.data.clone()
    }
    pub fn drain_errors(&mut self) -> Vec<String> {
        std::mem::take(&mut self.errors)
    }

    fn change(
        &mut self,
        provider: &str,
        credential: Option<AuthCredential>,
    ) -> Result<(), AuthError> {
        if self.load_error.is_some() {
            self.reload();
        }
        if let Some(error) = &self.load_error {
            return Err(format!(
                "Cannot update auth storage because it could not be loaded: {error}"
            )
            .into());
        }
        let provider = provider.to_owned();
        let data = self.locked(true, |current| {
            let mut data = parse(current)?;
            if let Some(value) = credential {
                data.insert(provider, value);
            } else {
                data.remove(&provider);
            }
            let json = serde_json::to_string_pretty(&data)?;
            Ok((data, Some(json)))
        })?;
        self.data = data;
        Ok(())
    }
    pub fn set(&mut self, provider: &str, credential: AuthCredential) -> Result<(), AuthError> {
        self.change(provider, Some(credential))
    }
    pub fn remove(&mut self, provider: &str) -> Result<(), AuthError> {
        self.change(provider, None)
    }
    pub fn logout(&mut self, provider: &str) -> Result<(), AuthError> {
        self.remove(provider)
    }

    pub fn has_auth(&self, provider: &str) -> bool {
        self.runtime_overrides.contains_key(provider)
            || self.has(provider)
            || get_env_api_key(provider, None).is_some()
    }
    pub fn get_auth_status(&self, provider: &str) -> AuthStatus {
        if self.has(provider) {
            return AuthStatus {
                configured: true,
                source: Some(AuthSource::Stored),
                label: None,
            };
        }
        if self.runtime_overrides.contains_key(provider) {
            return AuthStatus {
                configured: false,
                source: Some(AuthSource::Runtime),
                label: Some("--api-key".into()),
            };
        }
        if let Some(key) = find_env_keys(provider, None).and_then(|v| v.first().copied()) {
            return AuthStatus {
                configured: false,
                source: Some(AuthSource::Environment),
                label: Some(key.into()),
            };
        }
        AuthStatus {
            configured: false,
            source: None,
            label: None,
        }
    }

    pub async fn login(
        &mut self,
        provider_id: &str,
        callbacks: &dyn AuthLoginCallbacks,
    ) -> Result<(), AuthError> {
        let provider = get_oauth_provider(provider_id)
            .ok_or_else(|| format!("Unknown OAuth provider: {provider_id}"))?;
        self.set(provider_id, provider.login(callbacks).await?.into())
    }

    pub async fn get_api_key(
        &mut self,
        provider_id: &str,
        include_fallback: bool,
    ) -> Option<String> {
        if let Some(key) = self.runtime_overrides.get(provider_id) {
            return Some(key.clone());
        }
        match self.data.get(provider_id).cloned() {
            Some(AuthCredential::ApiKey { key, env }) => {
                return crate::resolve_config_value::resolve_config_value(&key, env.as_ref());
            }
            Some(credential @ AuthCredential::OAuth { expires, .. }) => {
                let provider = get_oauth_provider(provider_id)?;
                if now_ms() < expires {
                    return credential.oauth().map(|c| provider.get_api_key(&c));
                }
                match self.refresh_locked(provider_id, provider).await {
                    Ok(value) => return value,
                    Err(error) => {
                        self.errors.push(error.to_string());
                        self.reload();
                        return self
                            .data
                            .get(provider_id)
                            .and_then(AuthCredential::oauth)
                            .filter(|c| now_ms() < c.expires)
                            .map(|c| get_oauth_provider(provider_id).unwrap().get_api_key(&c));
                    }
                }
            }
            None => {}
        }
        include_fallback
            .then(|| get_env_api_key(provider_id, None))
            .flatten()
    }

    async fn refresh_locked(
        &mut self,
        provider_id: &str,
        provider: Arc<dyn OAuthProviderInterface>,
    ) -> Result<Option<String>, AuthError> {
        // Hold the backend lock across refresh, matching Pi and preventing duplicate refreshes.
        match &self.backend {
            Backend::Memory(value) => {
                let current = value
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone();
                self.refresh_and_store(provider_id, provider, current, None)
                    .await
            }
            Backend::File(path) => {
                ensure_file(path)?;
                let mut file = OpenOptions::new().read(true).write(true).open(path)?;
                file.lock_exclusive()?;
                let mut current = String::new();
                file.read_to_string(&mut current)?;
                let result = self
                    .refresh_and_store(provider_id, provider, Some(current), Some(&mut file))
                    .await;
                let _ = file.unlock();
                result
            }
        }
    }

    async fn refresh_and_store(
        &mut self,
        id: &str,
        provider: Arc<dyn OAuthProviderInterface>,
        current: Option<String>,
        mut file: Option<&mut File>,
    ) -> Result<Option<String>, AuthError> {
        let mut data = parse(current.as_deref())?;
        self.data = data.clone();
        self.load_error = None;
        let Some(credential) = data.get(id).and_then(AuthCredential::oauth) else {
            return Ok(None);
        };
        if now_ms() < credential.expires {
            return Ok(Some(provider.get_api_key(&credential)));
        }
        let refreshed = provider.refresh_token(&credential).await?;
        let key = provider.get_api_key(&refreshed);
        data.insert(id.to_owned(), refreshed.into());
        let json = serde_json::to_string_pretty(&data)?;
        if let Some(file) = file.as_mut() {
            file.seek(SeekFrom::Start(0))?;
            file.set_len(0)?;
            file.write_all(json.as_bytes())?;
            file.sync_all()?;
        } else if let Backend::Memory(value) = &self.backend {
            *value
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(json);
        }
        self.data = data;
        Ok(Some(key))
    }

    pub fn oauth_providers(&self) -> Vec<Arc<dyn OAuthProviderInterface>> {
        get_oauth_providers()
    }
}

fn parse(content: Option<&str>) -> Result<AuthStorageData, AuthError> {
    Ok(match content.filter(|s| !s.is_empty()) {
        Some(s) => serde_json::from_str(s)?,
        None => BTreeMap::new(),
    })
}
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}
fn ensure_file(path: &Path) -> Result<(), AuthError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        set_private_dir_mode(parent)?;
    }
    if !path.exists() {
        fs::write(path, "{}")?;
        set_private_file_mode(path)?;
    }
    Ok(())
}
#[cfg(unix)]
fn set_private_file_mode(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}
#[cfg(not(unix))]
fn set_private_file_mode(_: &Path) -> std::io::Result<()> {
    Ok(())
}
#[cfg(unix)]
fn set_private_dir_mode(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}
#[cfg(not(unix))]
fn set_private_dir_mode(_: &Path) -> std::io::Result<()> {
    Ok(())
}
