//! Loader wrapped in Pi's dynamic borders and optional cancel hint.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use zedflow_tui::{CancellableLoader, Component, Loader};

use crate::{keybinding_hints::key_hint, modes_interactive_theme_theme::Theme};

#[derive(Clone)]
pub struct BorderedLoaderSignal(Arc<AtomicBool>);
impl BorderedLoaderSignal {
    #[must_use]
    pub fn aborted(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

enum Inner {
    Cancellable(CancellableLoader),
    Plain(Loader),
}

pub struct BorderedLoader {
    inner: Inner,
    cancellable: bool,
    signal: BorderedLoaderSignal,
    border_ansi: String,
}

impl BorderedLoader {
    #[must_use]
    pub fn new(theme: &Theme, message: impl Into<String>, cancellable: bool) -> Self {
        let signal = BorderedLoaderSignal(Arc::new(AtomicBool::new(false)));
        let inner = if cancellable {
            Inner::Cancellable(CancellableLoader::new(message))
        } else {
            Inner::Plain(Loader::new(message))
        };
        Self {
            inner,
            cancellable,
            signal,
            border_ansi: theme.fg_ansi("border").unwrap_or("").into(),
        }
    }

    #[must_use]
    pub fn signal(&self) -> BorderedLoaderSignal {
        self.signal.clone()
    }

    pub fn set_on_abort(&mut self, callback: impl FnMut() + 'static) {
        if let Inner::Cancellable(loader) = &mut self.inner {
            loader.on_abort = Some(Box::new(callback));
        }
    }

    pub fn dispose(&mut self) {
        match &mut self.inner {
            Inner::Cancellable(loader) => loader.dispose(),
            Inner::Plain(loader) => loader.stop(),
        }
    }
}

impl Component for BorderedLoader {
    fn render(&self, width: usize) -> Vec<String> {
        let border = format!("{}{}\x1b[39m", self.border_ansi, "─".repeat(width.max(1)));
        let mut lines = vec![border.clone()];
        lines.extend(match &self.inner {
            Inner::Cancellable(loader) => loader.render(width),
            Inner::Plain(loader) => loader.render(width),
        });
        if self.cancellable {
            lines.push(String::new());
            lines.push(key_hint("tui.select.cancel", "cancel"));
        }
        lines.push(String::new());
        lines.push(border);
        lines
    }

    fn handle_input(&mut self, data: &str) {
        if let Inner::Cancellable(loader) = &mut self.inner {
            loader.handle_input(data);
            if loader.aborted() {
                self.signal.0.store(true, Ordering::Release);
            }
        }
    }
}
