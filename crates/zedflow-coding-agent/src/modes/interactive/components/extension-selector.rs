//! String selection dialog exposed to extensions.

use std::{cell::Cell, rc::Rc};

use crate::modes_interactive_components_index::countdown_timer::CountdownTimer;
use zedflow_tui::{Component, get_keybindings};

#[derive(Default)]
pub struct ExtensionSelectorOptions {
    pub timeout_ms: Option<u64>,
    pub on_toggle_tools_expanded: Option<Box<dyn FnMut()>>,
}

pub struct ExtensionSelectorComponent {
    base_title: String,
    options: Vec<String>,
    selected_index: usize,
    on_select: Box<dyn FnMut(String)>,
    on_cancel: Rc<std::cell::RefCell<Box<dyn FnMut()>>>,
    on_toggle_tools_expanded: Option<Box<dyn FnMut()>>,
    countdown: Option<CountdownTimer>,
    remaining_seconds: Rc<Cell<Option<u64>>>,
}

impl ExtensionSelectorComponent {
    pub fn new(
        title: impl Into<String>,
        options: Vec<String>,
        on_select: impl FnMut(String) + 'static,
        on_cancel: impl FnMut() + 'static,
        mut settings: ExtensionSelectorOptions,
    ) -> Self {
        let remaining_seconds = Rc::new(Cell::new(None));
        let on_cancel: Rc<std::cell::RefCell<Box<dyn FnMut()>>> =
            Rc::new(std::cell::RefCell::new(Box::new(on_cancel)));
        let countdown = settings.timeout_ms.filter(|timeout| *timeout > 0).map({
            let remaining_seconds = Rc::clone(&remaining_seconds);
            let on_cancel = Rc::clone(&on_cancel);
            move |timeout| {
                CountdownTimer::new(
                    timeout,
                    move |seconds| remaining_seconds.set(Some(seconds)),
                    move || (on_cancel.borrow_mut())(),
                )
            }
        });
        Self {
            base_title: title.into(),
            options,
            selected_index: 0,
            on_select: Box::new(on_select),
            on_cancel,
            on_toggle_tools_expanded: settings.on_toggle_tools_expanded.take(),
            countdown,
            remaining_seconds,
        }
    }

    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    #[must_use]
    pub fn title(&self) -> String {
        self.remaining_seconds.get().map_or_else(
            || self.base_title.clone(),
            |s| format!("{} ({s}s)", self.base_title),
        )
    }

    pub fn tick_timeout(&mut self) {
        if let Some(countdown) = &mut self.countdown {
            countdown.tick();
        }
    }

    pub fn dispose(&mut self) {
        if let Some(countdown) = &mut self.countdown {
            countdown.dispose();
        }
    }
}

impl Component for ExtensionSelectorComponent {
    fn render(&self, width: usize) -> Vec<String> {
        let mut lines = vec![
            "─".repeat(width.max(1)),
            String::new(),
            format!(" {}", self.title()),
            String::new(),
        ];
        lines.extend(self.options.iter().enumerate().map(|(index, option)| {
            format!(
                "{}{}",
                if index == self.selected_index {
                    " → "
                } else {
                    "   "
                },
                option
            )
        }));
        lines.extend([
            String::new(),
            " ↑↓ navigate  enter select  escape cancel".into(),
            String::new(),
            "─".repeat(width.max(1)),
        ]);
        lines
    }

    fn handle_input(&mut self, data: &str) {
        let (toggle, up, down, confirm, cancel) = {
            let keybindings = get_keybindings().lock().unwrap();
            (
                keybindings.matches(data, "app.tools.expand"),
                keybindings.matches(data, "tui.select.up") || data == "k",
                keybindings.matches(data, "tui.select.down") || data == "j",
                keybindings.matches(data, "tui.select.confirm") || data == "\n",
                keybindings.matches(data, "tui.select.cancel"),
            )
        };
        if toggle {
            if let Some(callback) = &mut self.on_toggle_tools_expanded {
                callback();
            }
        } else if up {
            self.selected_index = self.selected_index.saturating_sub(1);
        } else if down {
            self.selected_index = self
                .selected_index
                .saturating_add(1)
                .min(self.options.len().saturating_sub(1));
        } else if confirm {
            if let Some(selected) = self.options.get(self.selected_index) {
                (self.on_select)(selected.clone());
            }
        } else if cancel {
            (self.on_cancel.borrow_mut())();
        }
    }
}
