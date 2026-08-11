//! Thinking-level selector choices and descriptions.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinkingLevel {
    Off,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
}

impl ThinkingLevel {
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Off => "No reasoning",
            Self::Minimal => "Very brief reasoning (~1k tokens)",
            Self::Low => "Light reasoning (~2k tokens)",
            Self::Medium => "Moderate reasoning (~8k tokens)",
            Self::High => "Deep reasoning (~16k tokens)",
            Self::XHigh => "Maximum reasoning (~32k tokens)",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThinkingSelector {
    pub levels: Vec<ThinkingLevel>,
    pub selected: usize,
}
impl ThinkingSelector {
    #[must_use]
    pub fn new(current: ThinkingLevel, levels: Vec<ThinkingLevel>) -> Self {
        let selected = levels
            .iter()
            .position(|level| *level == current)
            .unwrap_or(0);
        Self { levels, selected }
    }
    pub fn move_selection(&mut self, delta: isize) {
        self.selected = self
            .selected
            .saturating_add_signed(delta)
            .min(self.levels.len().saturating_sub(1));
    }
    #[must_use]
    pub fn selected(&self) -> Option<ThinkingLevel> {
        self.levels.get(self.selected).copied()
    }
}
