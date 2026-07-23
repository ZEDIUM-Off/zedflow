//! Runtime ownership for a coding-agent session.

use std::sync::Arc;

use super::agent_session::AgentSession;

/// Owns the active harness and the cwd it was created for.
#[derive(Clone)]
pub struct AgentSessionRuntime {
    session: Arc<AgentSession>,
    cwd: String,
}

impl AgentSessionRuntime {
    #[must_use]
    pub fn new(session: AgentSession, cwd: impl Into<String>) -> Self {
        Self {
            session: Arc::new(session),
            cwd: cwd.into(),
        }
    }

    #[must_use]
    pub fn session(&self) -> Arc<AgentSession> {
        Arc::clone(&self.session)
    }

    #[must_use]
    pub fn cwd(&self) -> &str {
        &self.cwd
    }

    /// Replace the active harness after a session switch.
    pub fn replace(&mut self, session: AgentSession, cwd: impl Into<String>) {
        self.session = Arc::new(session);
        self.cwd = cwd.into();
    }
}
