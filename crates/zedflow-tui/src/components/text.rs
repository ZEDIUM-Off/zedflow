use std::sync::Arc;

use crate::utils::{apply_background_to_line, visible_width};
use crate::{Component, Focusable};

type Background = Arc<dyn Fn(&str) -> String + Send + Sync>;

pub struct Text {
    pub text: String,
    pub padding_x: usize,
    pub padding_y: usize,
    background: Option<Background>,
    focused: bool,
}

impl Text {
    pub fn new(text: impl Into<String>, padding_x: usize, padding_y: usize) -> Self {
        Self {
            text: text.into(),
            padding_x,
            padding_y,
            background: None,
            focused: false,
        }
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    pub fn get_text(&self) -> &str {
        &self.text
    }

    pub fn set_custom_bg_fn(&mut self, background: Option<Background>) {
        self.background = background;
    }

    fn apply_background(&self, line: &str, width: usize) -> String {
        if let Some(background) = &self.background {
            apply_background_to_line(line, width, |text| background(text))
        } else {
            format!(
                "{line}{}",
                " ".repeat(width.saturating_sub(visible_width(line)))
            )
        }
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
            return Vec::new();
        }
        let content_width = width.saturating_sub(self.padding_x * 2).max(1);
        let left = " ".repeat(self.padding_x);
        let right = " ".repeat(self.padding_x);
        let empty = self.apply_background(&" ".repeat(width), width);
        let mut lines = Vec::new();
        lines.extend(std::iter::repeat_n(empty.clone(), self.padding_y));
        lines.extend(
            crate::utils::wrap_text_with_ansi(&self.text.replace('\t', "   "), content_width)
                .into_iter()
                .map(|line| self.apply_background(&format!("{left}{line}{right}"), width)),
        );
        lines.extend(std::iter::repeat_n(empty, self.padding_y));
        lines
    }
}

impl Focusable for Text {
    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    fn is_focused(&self) -> bool {
        self.focused
    }
}
