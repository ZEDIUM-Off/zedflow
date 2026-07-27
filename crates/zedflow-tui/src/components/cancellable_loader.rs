use crate::{components::Loader, Component};

/// A loader that stops when Escape is pressed.
pub struct CancellableLoader {
    pub loader: Loader,
    aborted: bool,
}

impl CancellableLoader {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            loader: Loader::new(message),
            aborted: false,
        }
    }

    pub fn abort(&mut self) {
        self.aborted = true;
    }

    pub fn aborted(&self) -> bool {
        self.aborted
    }
}

impl Component for CancellableLoader {
    fn render(&self, width: usize) -> Vec<String> {
        self.loader.render(width)
    }

    fn handle_input(&mut self, data: &str) {
        if data == "\x1b" {
            self.abort();
        }
    }
}
