use crate::{Component, Focusable};
pub struct Input {
    pub value: String,
    pub cursor: usize,
    pub focused: bool,
}
impl Input {
    pub fn new(value: impl Into<String>) -> Self {
        let v = value.into();
        Self {
            cursor: v.len(),
            value: v,
            focused: false,
        }
    }
    pub fn value(&self) -> &str {
        &self.value
    }
    pub fn set_value(&mut self, v: impl Into<String>) {
        self.value = v.into();
        self.cursor = self.value.len()
    }
    pub fn get_value(&self) -> &str {
        &self.value
    }
}
impl Component for Input {
    fn render(&self, w: usize) -> Vec<String> {
        vec![crate::utils::truncate_to_width(&self.value, w)]
    }
    fn handle_input(&mut self, d: &str) {
        if d == "\x7f" {
            self.value.pop();
            self.cursor = self.value.len()
        } else if d.chars().count() == 1 {
            self.value.insert_str(self.cursor, d);
            self.cursor += d.len()
        }
    }
}
impl Focusable for Input {
    fn set_focused(&mut self, v: bool) {
        self.focused = v
    }
    fn is_focused(&self) -> bool {
        self.focused
    }
}
