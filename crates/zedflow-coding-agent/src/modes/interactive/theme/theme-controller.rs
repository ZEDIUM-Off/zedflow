//! State machine for interactive theme selection and terminal light/dark changes.

use crate::modes_interactive_theme_theme as super_theme;
use super_theme::{ColorMode, TerminalTheme, Theme, resolve_theme_setting};

#[derive(Debug, Clone)]
pub struct ThemeResult {
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InteractiveThemeController {
    terminal_theme: TerminalTheme,
    theme_setting: Option<String>,
    active_theme_name: Option<String>,
    auto_sync_enabled: bool,
    theme: Theme,
}

impl InteractiveThemeController {
    pub fn new(
        theme_setting: Option<String>,
        terminal_theme: TerminalTheme,
        mode: ColorMode,
    ) -> Self {
        let active = resolve_theme_setting(theme_setting.as_deref(), terminal_theme)
            .unwrap_or_else(|| "dark".into());
        let (theme, active) = Theme::builtin(&active, mode)
            .map(|theme| (theme, active))
            .unwrap_or_else(|_| {
                (
                    Theme::builtin("dark", mode).expect("embedded dark theme is valid"),
                    "dark".into(),
                )
            });
        Self {
            terminal_theme,
            theme_setting,
            active_theme_name: Some(active),
            auto_sync_enabled: false,
            theme,
        }
    }

    #[must_use]
    pub fn theme(&self) -> &Theme {
        &self.theme
    }
    #[must_use]
    pub fn active_theme_name(&self) -> Option<&str> {
        self.active_theme_name.as_deref()
    }
    #[must_use]
    pub fn terminal_theme(&self) -> TerminalTheme {
        self.terminal_theme
    }
    #[must_use]
    pub fn auto_sync_enabled(&self) -> bool {
        self.auto_sync_enabled
    }

    pub fn apply_from_settings(&mut self) -> ThemeResult {
        self.auto_sync_enabled =
            super_theme::parse_auto_theme_setting(self.theme_setting.as_deref()).is_some();
        let name = resolve_theme_setting(self.theme_setting.as_deref(), self.terminal_theme)
            .unwrap_or_else(|| match self.terminal_theme {
                TerminalTheme::Dark => "dark".into(),
                TerminalTheme::Light => "light".into(),
            });
        self.apply_theme_name(&name)
    }

    pub fn set_theme_name(&mut self, name: &str) -> ThemeResult {
        self.auto_sync_enabled = false;
        self.theme_setting = Some(name.into());
        self.apply_theme_name(name)
    }

    pub fn set_theme_instance(&mut self, theme: Theme) -> ThemeResult {
        self.auto_sync_enabled = false;
        self.theme = theme;
        self.active_theme_name = Some("<in-memory>".into());
        ThemeResult {
            success: true,
            error: None,
        }
    }

    pub fn preview(&mut self, setting_or_name: &str) -> ThemeResult {
        let name = resolve_theme_setting(Some(setting_or_name), self.terminal_theme)
            .or_else(|| self.active_theme_name.clone());
        match name {
            Some(name) => self.apply_theme_name(&name),
            None => ThemeResult {
                success: false,
                error: Some("No theme selected".into()),
            },
        }
    }

    pub fn disable_auto_sync(&mut self) {
        self.auto_sync_enabled = false;
    }

    pub fn apply_terminal_theme(&mut self, terminal_theme: TerminalTheme) -> ThemeResult {
        if !self.auto_sync_enabled {
            return ThemeResult {
                success: true,
                error: None,
            };
        }
        self.terminal_theme = terminal_theme;
        let Some(name) = resolve_theme_setting(self.theme_setting.as_deref(), terminal_theme)
        else {
            self.auto_sync_enabled = false;
            return ThemeResult {
                success: false,
                error: Some("Invalid automatic theme setting".into()),
            };
        };
        self.apply_theme_name(&name)
    }

    fn apply_theme_name(&mut self, name: &str) -> ThemeResult {
        match Theme::builtin(name, self.theme.color_mode()) {
            Ok(theme) => {
                self.theme = theme;
                self.active_theme_name = Some(name.into());
                ThemeResult {
                    success: true,
                    error: None,
                }
            }
            Err(error) => {
                self.theme = Theme::builtin("dark", self.theme.color_mode())
                    .expect("embedded dark theme is valid");
                self.active_theme_name = Some("dark".into());
                ThemeResult {
                    success: false,
                    error: Some(error),
                }
            }
        }
    }
}
