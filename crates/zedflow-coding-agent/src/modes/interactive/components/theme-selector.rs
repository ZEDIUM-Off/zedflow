//! Theme selector with deterministic preview actions.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeAction {
    Preview(String),
    Select(String),
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeSelector {
    themes: Vec<String>,
    current: String,
    pub selected: usize,
}
impl ThemeSelector {
    #[must_use]
    pub fn new(current: impl Into<String>, themes: Vec<String>) -> Self {
        let current = current.into();
        let selected = themes
            .iter()
            .position(|theme| theme == &current)
            .unwrap_or(0);
        Self {
            themes,
            current,
            selected,
        }
    }
    pub fn move_selection(&mut self, delta: isize) -> Option<ThemeAction> {
        if self.themes.is_empty() {
            return None;
        }
        self.selected = self
            .selected
            .saturating_add_signed(delta)
            .min(self.themes.len() - 1);
        Some(ThemeAction::Preview(self.themes[self.selected].clone()))
    }
    #[must_use]
    pub fn confirm(&self) -> Option<ThemeAction> {
        self.themes
            .get(self.selected)
            .cloned()
            .map(ThemeAction::Select)
    }
    #[must_use]
    pub const fn cancel(&self) -> ThemeAction {
        ThemeAction::Cancel
    }
    #[must_use]
    pub fn is_current(&self, index: usize) -> bool {
        self.themes.get(index) == Some(&self.current)
    }
}
