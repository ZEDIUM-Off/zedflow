//! In-memory session repository.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::harness::types::{
    Session as SessionTrait, SessionCreateOptions, SessionError, SessionErrorCode,
    SessionForkOptions, SessionMetadata, SessionRepo,
};

use super::memory_storage::{InMemorySessionStorage, InMemorySessionStorageOptions};
use super::repo_utils::{
    create_session_id, create_timestamp, get_entries_to_fork, to_shared_session,
};

/// Repository that keeps all sessions in process memory.
#[derive(Debug, Default)]
pub struct InMemorySessionRepo {
    sessions: Mutex<HashMap<String, Arc<InMemorySessionStorage>>>,
}

impl InMemorySessionRepo {
    /// Create an empty repository.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl SessionRepo for InMemorySessionRepo {
    fn create<'a>(
        &'a self,
        options: SessionCreateOptions,
    ) -> crate::harness::types::HarnessFuture<'a, Result<Arc<dyn SessionTrait>, SessionError>> {
        Box::pin(async move {
            let metadata = SessionMetadata {
                id: options.id.unwrap_or_else(create_session_id),
                created_at: create_timestamp(),
            };
            let storage = Arc::new(InMemorySessionStorage::new(Some(
                InMemorySessionStorageOptions {
                    entries: None,
                    metadata: Some(metadata.clone()),
                },
            ))?);
            self.sessions
                .lock()
                .expect("session repo lock")
                .insert(metadata.id, storage.clone());
            Ok(Arc::new(to_shared_session(storage)) as Arc<dyn SessionTrait>)
        })
    }

    fn open<'a>(
        &'a self,
        metadata: SessionMetadata,
    ) -> crate::harness::types::HarnessFuture<'a, Result<Arc<dyn SessionTrait>, SessionError>> {
        Box::pin(async move {
            let storage = self
                .sessions
                .lock()
                .expect("session repo lock")
                .get(&metadata.id)
                .cloned()
                .ok_or_else(|| {
                    SessionError::new(
                        SessionErrorCode::NotFound,
                        format!("Session not found: {}", metadata.id),
                        None,
                    )
                })?;
            Ok(Arc::new(to_shared_session(storage)) as Arc<dyn SessionTrait>)
        })
    }

    fn list<'a>(
        &'a self,
    ) -> crate::harness::types::HarnessFuture<'a, Result<Vec<SessionMetadata>, SessionError>> {
        Box::pin(async move {
            let storages = self
                .sessions
                .lock()
                .expect("session repo lock")
                .values()
                .cloned()
                .collect::<Vec<_>>();
            let mut sessions = Vec::with_capacity(storages.len());
            for storage in storages {
                sessions.push(crate::harness::types::SessionStorage::get_metadata(&*storage).await);
            }
            Ok(sessions)
        })
    }

    fn delete<'a>(
        &'a self,
        metadata: SessionMetadata,
    ) -> crate::harness::types::HarnessFuture<'a, Result<(), SessionError>> {
        Box::pin(async move {
            self.sessions
                .lock()
                .expect("session repo lock")
                .remove(&metadata.id);
            Ok(())
        })
    }

    fn fork<'a>(
        &'a self,
        source: SessionMetadata,
        options: SessionForkOptions,
    ) -> crate::harness::types::HarnessFuture<'a, Result<Arc<dyn SessionTrait>, SessionError>> {
        Box::pin(async move {
            let source_storage = self
                .sessions
                .lock()
                .expect("session repo lock")
                .get(&source.id)
                .cloned()
                .ok_or_else(|| {
                    SessionError::new(
                        SessionErrorCode::NotFound,
                        format!("Session not found: {}", source.id),
                        None,
                    )
                })?;
            let forked_entries = get_entries_to_fork(source_storage.as_ref(), &options).await?;
            let metadata = SessionMetadata {
                id: options.id.unwrap_or_else(create_session_id),
                created_at: create_timestamp(),
            };
            let storage = Arc::new(InMemorySessionStorage::new(Some(
                InMemorySessionStorageOptions {
                    entries: Some(forked_entries),
                    metadata: Some(metadata.clone()),
                },
            ))?);
            self.sessions
                .lock()
                .expect("session repo lock")
                .insert(metadata.id, storage.clone());
            Ok(Arc::new(to_shared_session(storage)) as Arc<dyn SessionTrait>)
        })
    }
}
