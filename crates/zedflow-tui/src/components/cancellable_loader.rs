use crate::{Component, components::Loader};
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
        self.cancelled = true
    }
}
impl Component for CancellableLoader {
    fn render(&self, w: usize) -> Vec<String> {
        self.loader.render(w)
    }
    fn handle_input(&mut self, d: &str) {
        if d == "\x1b" {
            self.cancel()
        }
    }
}
