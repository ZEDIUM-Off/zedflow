use crate::{Component, utils::truncate_to_width};
pub struct TruncatedText {
    pub text: String,
    pub padding_x: usize,
    pub padding_y: usize,
}
impl TruncatedText {
    pub fn new(text: impl Into<String>, x: usize, y: usize) -> Self {
        Self {
            text: text.into(),
            padding_x: x,
            padding_y: y,
        }
    }
    pub fn set_text(&mut self, t: impl Into<String>) {
        self.text = t.into()
    }
}
impl Component for TruncatedText {
    fn render(&self, width: usize) -> Vec<String> {
        let w = width.saturating_sub(self.padding_x * 2);
        let p = " ".repeat(self.padding_x);
        let line = format!("{}{}", p, truncate_to_width(&self.text, w));
        let line = format!(
            "{}{}",
            line,
            " ".repeat(width.saturating_sub(crate::utils::visible_width(&line)))
        );
        let mut v = vec![" ".repeat(width); self.padding_y]
            .into_iter()
            .collect::<Vec<_>>();
        v.push(line);
        v.extend(std::iter::repeat_n(" ".repeat(width), self.padding_y));
        v
    }
}
