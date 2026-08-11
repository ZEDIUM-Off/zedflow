//! Single-line input dialog exposed to extensions.

use std::{cell::Cell, rc::Rc};

use crate::modes_interactive_components_index::countdown_timer::CountdownTimer;
use zedflow_tui::{Component, Focusable, Input, get_keybindings};

#[derive(Debug, Clone, Copy, Default)]
pub struct ExtensionInputOptions {
    pub timeout_ms: Option<u64>,
}

pub struct ExtensionInputComponent {
    base_title: String,
    input: Input,
    on_submit: Box<dyn FnMut(String)>,
    on_cancel: Rc<std::cell::RefCell<Box<dyn FnMut()>>>,
    countdown: Option<CountdownTimer>,
    remaining_seconds: Rc<Cell<Option<u64>>>,
    focused: bool,
}

impl ExtensionInputComponent {
    pub fn new(
        title: impl Into<String>,
        _placeholder: Option<&str>,
        on_submit: impl FnMut(String) + 'static,
        on_cancel: impl FnMut() + 'static,
        options: ExtensionInputOptions,
    ) -> Self {
        let remaining_seconds = Rc::new(Cell::new(None));
        let on_cancel: Rc<std::cell::RefCell<Box<dyn FnMut()>>> =
            Rc::new(std::cell::RefCell::new(Box::new(on_cancel)));
        let countdown = options.timeout_ms.filter(|timeout| *timeout > 0).map({
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
            input: Input::default(),
            on_submit: Box::new(on_submit),
            on_cancel,
            countdown,
            remaining_seconds,
            focused: false,
        }
    }

    #[must_use]
    pub fn title(&self) -> String {
        self.remaining_seconds.get().map_or_else(
            || self.base_title.clone(),
            |s| format!("{} ({s}s)", self.base_title),
        )
    }

    #[must_use]
    pub fn value(&self) -> &str {
        self.input.get_value()
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

impl Component for ExtensionInputComponent {
    fn render(&self, width: usize) -> Vec<String> {
        let mut lines = vec![
            "─".repeat(width.max(1)),
            String::new(),
            format!(" {}", self.title()),
            String::new(),
        ];
        lines.extend(self.input.render(width));
        lines.extend([
            String::new(),
            " enter submit  escape cancel".into(),
            String::new(),
            "─".repeat(width.max(1)),
        ]);
        lines
    }

    fn handle_input(&mut self, data: &str) {
        let (submit, cancel) = {
            let keybindings = get_keybindings().lock().unwrap();
            (
                keybindings.matches(data, "tui.select.confirm") || data == "\n",
                keybindings.matches(data, "tui.select.cancel"),
            )
        };
        if submit {
            (self.on_submit)(self.input.get_value().to_owned());
        } else if cancel {
            (self.on_cancel.borrow_mut())();
        } else {
            self.input.handle_input(data);
        }
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        Focusable::set_focused(&mut self.input, focused);
    }

    fn is_focused(&self) -> bool {
        self.focused
    }
}
