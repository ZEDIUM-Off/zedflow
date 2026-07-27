use crate::{Component, components::Text};
pub struct Loader {
    pub text: Text,
    pub frames: Vec<String>,
    pub frame: usize,
}
impl Loader {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            text: Text::new(message, 1, 0),
            frames: vec!["⠋".into(), "⠙".into(), "⠹".into(), "⠸".into()],
            frame: 0,
        }
    }
    pub fn set_message(&mut self, m: impl Into<String>) {
        self.text.set_text(m)
    }
}
impl Component for Loader {
    fn render(&self, w: usize) -> Vec<String> {
        self.text.render(w)
    }
}
