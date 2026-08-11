//! State transitions for Pi's first-time theme and analytics setup dialog.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalTheme {
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirstTimeSetupResult {
    pub theme: TerminalTheme,
    pub share_analytics: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstTimeSetupAction {
    Preview(TerminalTheme),
    Continue,
    Submit(FirstTimeSetupResult),
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Theme,
    Analytics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirstTimeSetup {
    step: Step,
    theme: TerminalTheme,
    share_analytics: bool,
}

impl FirstTimeSetup {
    #[must_use]
    pub const fn new(detected_theme: TerminalTheme) -> Self {
        Self {
            step: Step::Theme,
            theme: detected_theme,
            share_analytics: true,
        }
    }

    #[must_use]
    pub const fn theme(&self) -> TerminalTheme {
        self.theme
    }

    /// Pi clamps selection at both ends rather than wrapping it.
    pub fn move_selection(&mut self, delta: i8) -> Option<FirstTimeSetupAction> {
        match self.step {
            Step::Theme if delta != 0 => {
                let previous = self.theme;
                self.theme = if delta < 0 {
                    TerminalTheme::Dark
                } else {
                    TerminalTheme::Light
                };
                (self.theme != previous).then_some(FirstTimeSetupAction::Preview(self.theme))
            }
            Step::Analytics if delta != 0 => {
                self.share_analytics = delta < 0;
                None
            }
            _ => None,
        }
    }

    #[must_use]
    pub const fn cancel(&self) -> FirstTimeSetupAction {
        FirstTimeSetupAction::Cancel
    }

    /// Advances from theme selection, then returns the finished settings.
    pub fn confirm(&mut self) -> Option<FirstTimeSetupResult> {
        match self.action_confirm() {
            FirstTimeSetupAction::Submit(result) => Some(result),
            _ => None,
        }
    }

    pub fn action_confirm(&mut self) -> FirstTimeSetupAction {
        if self.step == Step::Theme {
            self.step = Step::Analytics;
            FirstTimeSetupAction::Continue
        } else {
            FirstTimeSetupAction::Submit(FirstTimeSetupResult {
                theme: self.theme,
                share_analytics: self.share_analytics,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn setup_clamps_then_returns_selected_values() {
        let mut setup = FirstTimeSetup::new(TerminalTheme::Light);
        setup.move_selection(-1);
        assert_eq!(setup.theme(), TerminalTheme::Dark);
        assert_eq!(setup.confirm(), None);
        setup.move_selection(1);
        assert_eq!(setup.confirm().unwrap().share_analytics, false);
    }
}
