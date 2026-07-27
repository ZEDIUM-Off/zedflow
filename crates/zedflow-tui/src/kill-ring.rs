#[derive(Default, Clone, Debug)]
pub struct KillRing {
    items: Vec<String>,
}
impl KillRing {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, s: impl Into<String>) {
        self.items.push(s.into())
    }
    pub fn yank(&self) -> Option<&str> {
        self.items.last().map(String::as_str)
    }
}
