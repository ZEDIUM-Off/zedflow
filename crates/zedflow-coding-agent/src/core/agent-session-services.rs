//! Cwd-bound services used to construct an agent session.

use std::path::PathBuf;

use super::settings_manager::SettingsManager;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSessionRuntimeDiagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct AgentSessionServices {
    pub cwd: PathBuf,
    pub agent_dir: PathBuf,
    pub settings: SettingsManager,
    pub diagnostics: Vec<AgentSessionRuntimeDiagnostic>,
}

impl AgentSessionServices {
    #[must_use]
    pub fn new(cwd: impl Into<PathBuf>, agent_dir: impl Into<PathBuf>) -> Self {
        let cwd = cwd.into();
        let agent_dir = agent_dir.into();
        Self {
            settings: SettingsManager::create(&cwd, &agent_dir),
            cwd,
            agent_dir,
            diagnostics: Vec::new(),
        }
    }
}
