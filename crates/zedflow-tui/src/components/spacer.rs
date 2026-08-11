use crate::Component;
pub struct Spacer {
    pub lines: usize,
}
impl Spacer {
    pub fn new(lines: usize) -> Self {
        Self { lines }
    }
    pub fn set_lines(&mut self, lines: usize) {
        self.lines = lines;
    }
}
impl Component for Spacer {
    fn render(&self, _: usize) -> Vec<String> {
        vec![String::new(); self.lines]
    }
}
