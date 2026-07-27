use crate::utils::{truncate_to_width, visible_width};
use crate::{Component, Focusable};
pub struct Text {
    pub text: String,
    pub padding_x: usize,
    pub padding_y: usize,
    pub custom_bg_fn: Option<fn(&str) -> String>,
    focused: bool,
}
impl Text {
    pub fn new(text: impl Into<String>, padding_x: usize, padding_y: usize) -> Self {
        Self {
            text: text.into(),
            padding_x,
            padding_y,
            custom_bg_fn: None,
            focused: false,
        }
    }
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into()
    }
    pub fn get_text(&self) -> &str {
        &self.text
    }
    pub fn set_custom_bg_fn(&mut self, f: Option<fn(&str) -> String>) {
        self.custom_bg_fn = f
    }
}
impl Default for Text {
    fn default() -> Self {
        Self::new("", 1, 1)
    }
}
impl Component for Text {
    fn render(&self, width: usize) -> Vec<String> {
        if self.text.trim().is_empty() {
            return vec![];
        }
        let w = width.saturating_sub(self.padding_x * 2).max(1);
        let mut out = Vec::new();
        let pad = " ".repeat(self.padding_x);
        for _ in 0..self.padding_y {
            out.push(" ".repeat(width));
        }
        for l in crate::utils::wrap_text_with_ansi(&self.text.replace('\t', "   "), w) {
            let s = format!("{}{}", pad, truncate_to_width(&l, w, "", false));
            let s = format!(
                "{}{}",
                s,
                " ".repeat(width.saturating_sub(visible_width(&s)))
            );
            out.push(if let Some(f) = self.custom_bg_fn {
                f(&s)
            } else {
                s
            });
        }
        for _ in 0..self.padding_y {
            out.push(" ".repeat(width));
        }
        out
    }
    fn invalidate(&mut self) {}
}
impl Focusable for Text {
    fn set_focused(&mut self, v: bool) {
        self.focused = v
    }
    fn is_focused(&self) -> bool {
        self.focused
    }
}
