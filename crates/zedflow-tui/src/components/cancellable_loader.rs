use crate::{components::Loader, Component};

pub struct CancellableLoader {
    pub loader: Loader,
    pub cancelled: bool,
}

impl CancellableLoader {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            loader: Loader::new(message),
            cancelled: false,
        }
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
    }
}

impl Component for CancellableLoader {
    fn render(&self, width: usize) -> Vec<String> {
        self.loader.render(width)
    }

    fn handle_input(&mut self, data: &str) {
        if data == "\x1b" {
            self.cancel();
        }
    }
}
