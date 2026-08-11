//! Yes/no selector for inline terminal images.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShowImagesSelector {
    pub selected: usize,
}
impl ShowImagesSelector {
    #[must_use]
    pub const fn new(current: bool) -> Self {
        Self {
            selected: if current { 0 } else { 1 },
        }
    }
    pub fn move_selection(&mut self, delta: isize) {
        self.selected = self.selected.saturating_add_signed(delta).min(1);
    }
    #[must_use]
    pub const fn selected_value(self) -> bool {
        self.selected == 0
    }
    #[must_use]
    pub const fn description(self) -> &'static str {
        if self.selected_value() {
            "Show images inline in terminal"
        } else {
            "Show text placeholder instead"
        }
    }
}
