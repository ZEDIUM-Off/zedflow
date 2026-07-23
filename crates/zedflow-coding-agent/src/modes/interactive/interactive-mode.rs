//! Interactive-mode lifecycle contract.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractiveState {
    Created,
    Running,
    Stopped,
}

#[derive(Debug, Default)]
pub struct InteractiveMode {
    state: InteractiveState,
}

impl Default for InteractiveState {
    fn default() -> Self {
        Self::Created
    }
}
impl InteractiveMode {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    pub fn run(&mut self) {
        self.state = InteractiveState::Running;
    }
    pub fn stop(&mut self) {
        self.state = InteractiveState::Stopped;
    }
    #[must_use]
    pub fn state(&self) -> InteractiveState {
        self.state
    }
}
