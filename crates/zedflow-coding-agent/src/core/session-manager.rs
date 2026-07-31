//! Pi-compatible session contracts.
//!
//! Storage is implemented once in `zedflow-agent`; this module is the
//! coding-agent-facing namespace and keeps callers from depending on the
//! lower-level harness path.

use std::time::SystemTime;

pub use zedflow_agent::harness::session::{
    InMemorySessionRepo, InMemorySessionStorage, InMemorySessionStorageOptions, JsonlSessionRepo,
    JsonlSessionStorage, JsonlSessionStorageCreateOptions, JsonlSessionStorageFileSystem,
    build_session_context, create_session_id, create_timestamp, get_entries_to_fork,
    load_jsonl_session_metadata, uuidv7,
};
pub use zedflow_agent::harness::types::{
    ActiveToolsChangeEntry, BranchSummaryEntry, CompactionEntry, CustomMessageEntry,
    JsonlSessionCreateOptions, JsonlSessionListOptions, JsonlSessionMetadata, LabelEntry,
    MessageEntry, ModelChangeEntry, SessionContext, SessionError, SessionErrorCode,
    SessionForkOptions, SessionForkPosition, SessionMetadata, SessionRepo, SessionStorage,
    SessionTreeEntry, ThinkingLevelChangeEntry,
};

/// Current session state used by the coding-agent layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub cwd: String,
    pub session_file: Option<String>,
    pub session_id: String,
}

impl SessionInfo {
    #[must_use]
    pub fn in_memory(cwd: impl Into<String>, session_id: impl Into<String>) -> Self {
        Self {
            cwd: cwd.into(),
            session_file: None,
            session_id: session_id.into(),
        }
    }

    #[must_use]
    pub fn persisted(
        cwd: impl Into<String>,
        file: impl Into<String>,
        session_id: impl Into<String>,
    ) -> Self {
        Self {
            cwd: cwd.into(),
            session_file: Some(file.into()),
            session_id: session_id.into(),
        }
    }

    #[must_use]
    pub fn is_persisted(&self) -> bool {
        self.session_file.is_some()
    }
}

/// Pi lists sessions by the timestamp of their last user or assistant message,
/// falling back to the file modification time for header-only sessions.
#[must_use]
pub fn session_modified_timestamp(
    file_modified: SystemTime,
    message_timestamps: impl IntoIterator<Item = Option<SystemTime>>,
) -> SystemTime {
    message_timestamps
        .into_iter()
        .flatten()
        .last()
        .unwrap_or(file_modified)
}
