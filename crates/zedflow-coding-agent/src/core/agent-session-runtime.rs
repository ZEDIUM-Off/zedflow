//! Runtime ownership for a coding-agent session.

use std::{io, sync::Arc};

use zedflow_agent::harness::types::SessionForkPosition;

use super::agent_session::AgentSession;

/// Persistent storage details required to fork the active session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistentSessionForkContext {
    /// The actual JSONL file backing the active session.
    pub source_path: String,
    /// The repository root into which the replacement session is created.
    pub session_root: String,
}

/// Owns the active harness and the cwd it was created for.
#[derive(Clone)]
pub struct AgentSessionRuntime {
    session: Arc<AgentSession>,
    cwd: String,
    persistent_fork: Option<PersistentSessionForkContext>,
}

impl AgentSessionRuntime {
    #[must_use]
    pub fn new(session: AgentSession, cwd: impl Into<String>) -> Self {
        Self {
            session: Arc::new(session),
            cwd: cwd.into(),
            persistent_fork: None,
        }
    }

    #[must_use]
    pub fn with_persistent_fork_context(mut self, context: PersistentSessionForkContext) -> Self {
        self.persistent_fork = Some(context);
        self
    }

    #[must_use]
    pub fn session(&self) -> Arc<AgentSession> {
        Arc::clone(&self.session)
    }

    #[must_use]
    pub fn cwd(&self) -> &str {
        &self.cwd
    }

    #[must_use]
    pub fn persistent_fork_context(&self) -> Option<&PersistentSessionForkContext> {
        self.persistent_fork.as_ref()
    }

    /// Rebuild a persistent runtime forked at an entry without losing its storage root.
    pub fn fork_at_entry(
        &self,
        entry_id: impl Into<String>,
        position: SessionForkPosition,
    ) -> io::Result<Self> {
        let context = self
            .persistent_fork
            .as_ref()
            .ok_or_else(|| io::Error::other("Active session is not persistent"))?;
        crate::rpc_entry::create_runtime_for_persistent_fork(context, entry_id.into(), position)
    }

    /// Replace the active harness after a session switch.
    pub fn replace(&mut self, session: AgentSession, cwd: impl Into<String>) {
        self.session = Arc::new(session);
        self.cwd = cwd.into();
        self.persistent_fork = None;
    }
}
