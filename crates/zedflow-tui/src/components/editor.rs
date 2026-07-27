use super::Input;
use crate::{Component, Focusable};
pub struct Editor {
    pub input: Input,
}
impl Editor {
    pub fn new() -> Self {
        Self {
            input: Input::new(""),
        }
    }
    pub fn get_text(&self) -> &str {
        self.input.value()
    }
    pub fn set_text(&mut self, t: impl Into<String>) {
        self.input.set_value(t)
    }
}
impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}
impl Component for Editor {
    fn render(&self, w: usize) -> Vec<String> {
        self.input.render(w)
    }
    fn handle_input(&mut self, d: &str) {
        self.input.handle_input(d)
    }
}
impl Focusable for Editor {
    fn set_focused(&mut self, v: bool) {
        Focusable::set_focused(&mut self.input, v)
    }
    fn is_focused(&self) -> bool {
        Focusable::is_focused(&self.input)
    }
}
