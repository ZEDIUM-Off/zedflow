use crate::{utils::{truncate_to_width, visible_width}, Component};

/// Text component that displays only the first line and fits the viewport.
pub struct TruncatedText {
    pub text: String,
    pub padding_x: usize,
    pub padding_y: usize,
}

impl TruncatedText {
    pub fn new(text: impl Into<String>, padding_x: usize, padding_y: usize) -> Self {
        Self { text: text.into(), padding_x, padding_y }
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }
}

impl Component for TruncatedText {
    fn render(&self, width: usize) -> Vec<String> {
        let empty = " ".repeat(width);
        let text = self.text.split('\n').next().unwrap_or_default();
        let available = (width.saturating_sub(self.padding_x * 2)).max(1);
        let content = truncate_to_width(text, available, "…", false);
        let line = format!(
            "{}{}{}",
            " ".repeat(self.padding_x),
            content,
            " ".repeat(width.saturating_sub(self.padding_x + visible_width(&content)))
        );
        let mut lines = vec![empty.clone(); self.padding_y];
        lines.push(line);
        lines.extend(std::iter::repeat_n(empty, self.padding_y));
        lines
    }
}
