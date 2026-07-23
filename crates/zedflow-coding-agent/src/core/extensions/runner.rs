use super::types::{
    Extension, ExtensionError, ExtensionErrorListener, ExtensionRuntime, ProjectTrustEvent,
    ProjectTrustEventDecision, ProjectTrustEventResult,
};

pub type NewSessionHandler = Box<dyn Fn() + Send + Sync>;
pub type ForkHandler = Box<dyn Fn() + Send + Sync>;
pub type NavigateTreeHandler = Box<dyn Fn() + Send + Sync>;
pub type SwitchSessionHandler = Box<dyn Fn() + Send + Sync>;
pub type ReloadHandler = Box<dyn Fn() + Send + Sync>;
pub type ShutdownHandler = Box<dyn Fn() + Send + Sync>;

#[must_use]
pub fn emit_project_trust_event(
    event: ProjectTrustEvent,
    extensions: &[Extension],
) -> ProjectTrustEventResult {
    let _ = (event, extensions);
    ProjectTrustEventResult {
        decision: ProjectTrustEventDecision::Undecided,
    }
}

#[derive(Default)]
pub struct ExtensionRunner {
    pub extensions: Vec<Extension>,
    pub runtime: ExtensionRuntime,
    error_listener: Option<ExtensionErrorListener>,
}

impl ExtensionRunner {
    #[must_use]
    pub fn new(extensions: Vec<Extension>) -> Self {
        Self {
            extensions,
            runtime: ExtensionRuntime::default(),
            error_listener: None,
        }
    }
    pub fn set_error_listener(&mut self, listener: ExtensionErrorListener) {
        self.error_listener = Some(listener);
    }
    pub fn report_error(&self, error: ExtensionError) {
        if let Some(listener) = &self.error_listener {
            listener(error);
        }
    }
}
