use crate::{utils::truncate_to_width, Component};

pub struct TruncatedText {
    pub text: String,
    pub padding_x: usize,
    pub padding_y: usize,
}

impl TruncatedText {
    pub fn new(text: impl Into<String>, padding_x: usize, padding_y: usize) -> Self {
        Self {
            text: text.into(),
            padding_x,
            padding_y,
        }
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }
}

impl Component for TruncatedText {
    fn render(&self, width: usize) -> Vec<String> {
        let content_width = width.saturating_sub(self.padding_x * 2);
        let left_padding = " ".repeat(self.padding_x);
        let content = truncate_to_width(&self.text, content_width);
        let line = format!("{left_padding}{content}");
        let line = format!(
            "{line}{}",
            " ".repeat(width.saturating_sub(crate::utils::visible_width(&line)))
        );

        let mut lines = vec![" ".repeat(width); self.padding_y];
        lines.push(line);
        lines.extend(std::iter::repeat_n(" ".repeat(width), self.padding_y));
        lines
    }
}
