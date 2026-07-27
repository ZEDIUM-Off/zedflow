use crate::Component;
pub struct Markdown {
    pub text: String,
}
impl Markdown {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
    pub fn set_text(&mut self, t: impl Into<String>) {
        self.text = t.into()
    }
}
impl Component for Markdown {
    fn render(&self, w: usize) -> Vec<String> {
        crate::utils::wrap_text_with_ansi(&self.text, w)
    }
}
