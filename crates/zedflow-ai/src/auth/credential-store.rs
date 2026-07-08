//! In-memory credential storage ported from Pi.

use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard};

use futures::lock::Mutex as AsyncMutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Provider-scoped environment/config values stored with credentials.
pub type ProviderEnv = HashMap<String, String>;

/// Stored API-key credential.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ApiKeyCredential {
    /// API key value, when configured directly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Provider-scoped environment/config values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<ProviderEnv>,
}

/// Stored OAuth credential.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OAuthCredential {
    /// Refresh token.
    pub refresh: String,
    /// Access token.
    pub access: String,
    /// Expiration timestamp in milliseconds, matching Pi's `Date.now()` values.
    pub expires: i64,
    /// Provider-specific OAuth fields preserved from Pi's open credential shape.
    #[serde(default, flatten, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, Value>,
}

/// One type-tagged credential per provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Credential {
    /// API-key credential variant.
    #[serde(rename = "api_key")]
    ApiKey(ApiKeyCredential),
    /// OAuth credential variant.
    #[serde(rename = "oauth")]
    OAuth(OAuthCredential),
}

/// App-owned credential storage keyed by provider id.
pub trait CredentialStore {
    /// Reads the stored credential, if any.
    fn read<'a>(&'a self, provider_id: &'a str) -> impl Future<Output = Option<Credential>> + 'a;

    /// Runs a serialized read-modify-write for one provider id.
    ///
    /// Returning `None` leaves the current credential unchanged and resolves to
    /// the current value, matching Pi's `next ?? current` behavior.
    ///
    /// # Errors
    ///
    /// Returns any error produced by `f`.
    fn modify<'a, F, Fut, E>(
        &'a self,
        provider_id: &'a str,
        f: F,
    ) -> impl Future<Output = Result<Option<Credential>, E>> + 'a
    where
        F: FnOnce(Option<Credential>) -> Fut + 'a,
        Fut: Future<Output = Result<Option<Credential>, E>> + 'a,
        E: 'a;

    /// Removes a credential, serialized against [`CredentialStore::modify`].
    fn delete<'a>(&'a self, provider_id: &'a str) -> impl Future<Output = ()> + 'a;
}

/// Default in-memory credential store.
///
/// Apps inject persistent stores; this implementation keeps one credential per
/// provider and serializes writes per provider id.
#[derive(Debug, Clone, Default)]
pub struct InMemoryCredentialStore {
    inner: Arc<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    credentials: Mutex<HashMap<String, Credential>>,
    locks: Mutex<HashMap<String, Arc<AsyncMutex<()>>>>,
}

impl InMemoryCredentialStore {
    /// Creates an empty in-memory credential store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads the stored credential, if any.
    pub async fn read(&self, provider_id: &str) -> Option<Credential> {
        lock_unpoisoned(&self.inner.credentials)
            .get(provider_id)
            .cloned()
    }

    /// Runs a serialized read-modify-write for one provider id.
    ///
    /// Returning `None` leaves the current credential unchanged and resolves to
    /// the current value, matching Pi's `next ?? current` behavior.
    ///
    /// # Errors
    ///
    /// Returns any error produced by `f`.
    pub async fn modify<F, Fut, E>(&self, provider_id: &str, f: F) -> Result<Option<Credential>, E>
    where
        F: FnOnce(Option<Credential>) -> Fut,
        Fut: Future<Output = Result<Option<Credential>, E>>,
    {
        let provider_lock = self.provider_lock(provider_id);
        let _write_guard = provider_lock.lock().await;

        let current = self.read(provider_id).await;
        let next = f(current.clone()).await?;

        if let Some(next_credential) = next.clone() {
            lock_unpoisoned(&self.inner.credentials)
                .insert(provider_id.to_string(), next_credential);
        }

        Ok(next.or(current))
    }

    /// Removes a credential, serialized against [`InMemoryCredentialStore::modify`].
    pub async fn delete(&self, provider_id: &str) {
        let provider_lock = self.provider_lock(provider_id);
        let _write_guard = provider_lock.lock().await;
        lock_unpoisoned(&self.inner.credentials).remove(provider_id);
    }

    fn provider_lock(&self, provider_id: &str) -> Arc<AsyncMutex<()>> {
        lock_unpoisoned(&self.inner.locks)
            .entry(provider_id.to_string())
            .or_default()
            .clone()
    }
}

impl CredentialStore for InMemoryCredentialStore {
    async fn read<'a>(&'a self, provider_id: &'a str) -> Option<Credential> {
        Self::read(self, provider_id).await
    }

    async fn modify<'a, F, Fut, E>(
        &'a self,
        provider_id: &'a str,
        f: F,
    ) -> Result<Option<Credential>, E>
    where
        F: FnOnce(Option<Credential>) -> Fut + 'a,
        Fut: Future<Output = Result<Option<Credential>, E>> + 'a,
        E: 'a,
    {
        Self::modify(self, provider_id, f).await
    }

    async fn delete<'a>(&'a self, provider_id: &'a str) {
        Self::delete(self, provider_id).await;
    }
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use futures::channel::oneshot;
    use futures::executor::block_on;
    use futures::future;

    use super::*;

    fn api_key(key: &str) -> Credential {
        Credential::ApiKey(ApiKeyCredential {
            key: Some(key.to_string()),
            env: None,
        })
    }

    #[test]
    fn modify_stores_and_preserves_current_on_none() {
        block_on(async {
            let store = InMemoryCredentialStore::new();

            let stored = store
                .modify("anthropic", |_| async {
                    Ok::<_, Infallible>(Some(api_key("first")))
                })
                .await
                .unwrap();
            assert_eq!(stored, Some(api_key("first")));

            let unchanged = store
                .modify("anthropic", |_| async { Ok::<_, Infallible>(None) })
                .await
                .unwrap();
            assert_eq!(unchanged, Some(api_key("first")));
            assert_eq!(store.read("anthropic").await, Some(api_key("first")));
        });
    }

    #[test]
    fn modify_is_serialized_per_provider() {
        block_on(async {
            let store = InMemoryCredentialStore::new();
            let (started_tx, started_rx) = oneshot::channel();
            let (release_tx, release_rx) = oneshot::channel();

            let first = store.modify("openai", |_| async move {
                let _ = started_tx.send(());
                let _ = release_rx.await;
                Ok::<_, Infallible>(Some(api_key("first")))
            });

            let second = store.modify("openai", |current| async move {
                assert_eq!(current, Some(api_key("first")));
                Ok::<_, Infallible>(Some(api_key("second")))
            });

            let release = async move {
                let _ = started_rx.await;
                let _ = release_tx.send(());
            };

            let (_, first_result, second_result) = future::join3(release, first, second).await;

            assert_eq!(first_result.unwrap(), Some(api_key("first")));
            assert_eq!(second_result.unwrap(), Some(api_key("second")));
            assert_eq!(store.read("openai").await, Some(api_key("second")));
        });
    }

    #[test]
    fn delete_removes_credential() {
        block_on(async {
            let store = InMemoryCredentialStore::new();
            store
                .modify("google", |_| async {
                    Ok::<_, Infallible>(Some(api_key("key")))
                })
                .await
                .unwrap();

            store.delete("google").await;

            assert_eq!(store.read("google").await, None);
        });
    }
}
