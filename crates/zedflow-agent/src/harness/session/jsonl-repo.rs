//! JSONL session repository.

use std::sync::{Arc, Mutex};

use crate::harness::session::session::Session;
use crate::harness::types::{
    CreateDirOptions, FileKind, FileSystem, JsonlSessionCreateOptions, JsonlSessionListOptions,
    JsonlSessionMetadata, RemoveOptions, SessionError, SessionErrorCode, SessionForkOptions,
};

use super::jsonl_storage::{
    JsonlSessionStorage, JsonlSessionStorageCreateOptions, JsonlSessionStorageFileSystem,
    load_jsonl_session_metadata,
};
use super::repo_utils::{
    create_session_id, create_timestamp, get_entries_to_fork, get_file_system_result_or_throw,
    to_shared_session,
};

/// Repository that stores sessions as Pi-compatible JSONL files.
pub struct JsonlSessionRepo {
    fs: JsonlSessionStorageFileSystem,
    sessions_root_input: String,
    sessions_root: Mutex<Option<String>>,
}

impl JsonlSessionRepo {
    /// Create a JSONL session repository rooted at `sessions_root`.
    #[must_use]
    pub fn new(fs: Arc<dyn FileSystem>, sessions_root: impl Into<String>) -> Self {
        Self {
            fs,
            sessions_root_input: sessions_root.into(),
            sessions_root: Mutex::new(None),
        }
    }

    /// Create a session.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] when directories or session files cannot be created.
    pub async fn create(
        &self,
        options: JsonlSessionCreateOptions,
    ) -> Result<Session<Arc<JsonlSessionStorage>>, SessionError> {
        let id = options.id.unwrap_or_else(create_session_id);
        let created_at = create_timestamp();
        let session_dir = self.get_session_dir(&options.cwd).await?;
        get_file_system_result_or_throw(
            self.fs
                .create_dir(
                    &session_dir,
                    CreateDirOptions {
                        recursive: true,
                        abort_signal: None,
                    },
                )
                .await,
            &format!("Failed to create session directory {session_dir}"),
        )?;
        let file_path = self
            .create_session_file_path(&options.cwd, &id, &created_at)
            .await?;
        let storage = Arc::new(
            JsonlSessionStorage::create(
                self.fs.clone(),
                file_path,
                JsonlSessionStorageCreateOptions {
                    cwd: options.cwd,
                    session_id: id,
                    parent_session_path: options.parent_session_path,
                },
            )
            .await?,
        );
        Ok(to_shared_session(storage))
    }

    /// Open a session from metadata.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] when the session file is missing or invalid.
    pub async fn open(
        &self,
        metadata: JsonlSessionMetadata,
    ) -> Result<Session<Arc<JsonlSessionStorage>>, SessionError> {
        let exists = get_file_system_result_or_throw(
            self.fs.exists(&metadata.path, None).await,
            &format!("Failed to check session {}", metadata.path),
        )?;
        if !exists {
            return Err(SessionError::new(
                SessionErrorCode::NotFound,
                format!("Session not found: {}", metadata.path),
                None,
            ));
        }
        let storage = Arc::new(JsonlSessionStorage::open(self.fs.clone(), metadata.path).await?);
        Ok(to_shared_session(storage))
    }

    /// List sessions, newest first.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] when repository directories cannot be listed.
    pub async fn list(
        &self,
        options: JsonlSessionListOptions,
    ) -> Result<Vec<JsonlSessionMetadata>, SessionError> {
        let dirs = if let Some(cwd) = options.cwd {
            vec![self.get_session_dir(&cwd).await?]
        } else {
            self.list_session_dirs().await?
        };
        let mut sessions = Vec::new();
        for dir in dirs {
            let exists = get_file_system_result_or_throw(
                self.fs.exists(&dir, None).await,
                &format!("Failed to check session directory {dir}"),
            )?;
            if !exists {
                continue;
            }
            let files = get_file_system_result_or_throw(
                self.fs.list_dir(&dir, None).await,
                &format!("Failed to list sessions in {dir}"),
            )?;
            for file in files
                .into_iter()
                .filter(|file| file.kind != FileKind::Directory && file.name.ends_with(".jsonl"))
            {
                match load_jsonl_session_metadata(self.fs.as_ref(), &file.path).await {
                    Ok(metadata) => sessions.push(metadata),
                    Err(error) if error.code == SessionErrorCode::InvalidSession => {}
                    Err(error) => return Err(error),
                }
            }
        }
        sessions.sort_by(|left, right| right.base.created_at.cmp(&left.base.created_at));
        Ok(sessions)
    }

    /// Delete a session file.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] when removal fails.
    pub async fn delete(&self, metadata: JsonlSessionMetadata) -> Result<(), SessionError> {
        get_file_system_result_or_throw(
            self.fs
                .remove(
                    &metadata.path,
                    RemoveOptions {
                        force: true,
                        recursive: false,
                        abort_signal: None,
                    },
                )
                .await,
            &format!("Failed to delete session {}", metadata.path),
        )
    }

    /// Fork a session into a new JSONL file.
    ///
    /// # Errors
    ///
    /// Returns [`SessionError`] when the source is missing or the fork cannot be written.
    pub async fn fork(
        &self,
        source_metadata: JsonlSessionMetadata,
        create_options: JsonlSessionCreateOptions,
        fork_options: SessionForkOptions,
    ) -> Result<Session<Arc<JsonlSessionStorage>>, SessionError> {
        let source_storage = Arc::new(
            JsonlSessionStorage::open(self.fs.clone(), source_metadata.path.clone()).await?,
        );
        let forked_entries = get_entries_to_fork(source_storage.as_ref(), &fork_options).await?;
        let id = create_options
            .id
            .or(fork_options.id)
            .unwrap_or_else(create_session_id);
        let created_at = create_timestamp();
        let session_dir = self.get_session_dir(&create_options.cwd).await?;
        get_file_system_result_or_throw(
            self.fs
                .create_dir(
                    &session_dir,
                    CreateDirOptions {
                        recursive: true,
                        abort_signal: None,
                    },
                )
                .await,
            &format!("Failed to create session directory {session_dir}"),
        )?;
        let storage = Arc::new(
            JsonlSessionStorage::create(
                self.fs.clone(),
                self.create_session_file_path(&create_options.cwd, &id, &created_at)
                    .await?,
                JsonlSessionStorageCreateOptions {
                    cwd: create_options.cwd,
                    session_id: id,
                    parent_session_path: create_options
                        .parent_session_path
                        .or(Some(source_metadata.path)),
                },
            )
            .await?,
        );
        for entry in forked_entries {
            crate::harness::types::SessionStorage::append_entry(storage.as_ref(), entry).await?;
        }
        Ok(to_shared_session(storage))
    }

    async fn get_sessions_root(&self) -> Result<String, SessionError> {
        if let Some(root) = self
            .sessions_root
            .lock()
            .expect("jsonl repo root lock")
            .clone()
        {
            return Ok(root);
        }
        let root = get_file_system_result_or_throw(
            self.fs.absolute_path(&self.sessions_root_input, None).await,
            &format!(
                "Failed to resolve sessions root {}",
                self.sessions_root_input
            ),
        )?;
        *self.sessions_root.lock().expect("jsonl repo root lock") = Some(root.clone());
        Ok(root)
    }

    async fn get_session_dir(&self, cwd: &str) -> Result<String, SessionError> {
        get_file_system_result_or_throw(
            self.fs
                .join_path(&[self.get_sessions_root().await?, encode_cwd(cwd)], None)
                .await,
            &format!("Failed to resolve session directory for {cwd}"),
        )
    }

    async fn create_session_file_path(
        &self,
        cwd: &str,
        session_id: &str,
        timestamp: &str,
    ) -> Result<String, SessionError> {
        get_file_system_result_or_throw(
            self.fs
                .join_path(
                    &[
                        self.get_session_dir(cwd).await?,
                        format!(
                            "{}_{}.jsonl",
                            timestamp.replace([':', '.'], "-"),
                            session_id
                        ),
                    ],
                    None,
                )
                .await,
            &format!("Failed to resolve session file path for {session_id}"),
        )
    }

    async fn list_session_dirs(&self) -> Result<Vec<String>, SessionError> {
        let sessions_root = self.get_sessions_root().await?;
        let exists = get_file_system_result_or_throw(
            self.fs.exists(&sessions_root, None).await,
            &format!("Failed to check sessions root {sessions_root}"),
        )?;
        if !exists {
            return Ok(Vec::new());
        }
        let entries = get_file_system_result_or_throw(
            self.fs.list_dir(&sessions_root, None).await,
            &format!("Failed to list sessions root {sessions_root}"),
        )?;
        Ok(entries
            .into_iter()
            .filter(|entry| entry.kind == FileKind::Directory)
            .map(|entry| entry.path)
            .collect())
    }
}

fn encode_cwd(cwd: &str) -> String {
    format!(
        "--{}--",
        cwd.trim_start_matches(['/', '\\'])
            .replace(['/', '\\', ':'], "-")
    )
}
