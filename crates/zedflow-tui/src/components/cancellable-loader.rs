use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crate::{Component, components::Loader, keybindings::get_keybindings};

#[derive(Clone)]
pub struct CancellationSignal(Arc<AtomicBool>);

impl CancellationSignal {
    pub fn aborted(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// A loader that exposes cancellation through the global select-cancel binding.
pub struct CancellableLoader {
    pub loader: Loader,
    signal: CancellationSignal,
    pub on_abort: Option<Box<dyn FnMut()>>,
}

impl CancellableLoader {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            loader: Loader::new(message),
            signal: CancellationSignal(Arc::new(AtomicBool::new(false))),
            on_abort: None,
        }
    }

    pub fn signal(&self) -> CancellationSignal {
        self.signal.clone()
    }

    pub fn abort(&mut self) {
        if !self.signal.0.swap(true, Ordering::AcqRel) {
            if let Some(callback) = &mut self.on_abort {
                callback();
            }
        }
    }

    pub fn aborted(&self) -> bool {
        self.signal.aborted()
    }

    pub fn stop(&mut self) {}

    pub fn dispose(&mut self) {
        self.stop();
    }
}

impl Component for CancellableLoader {
    fn render(&self, width: usize) -> Vec<String> {
        self.loader.render(width)
    }

    fn handle_input(&mut self, data: &str) {
        if get_keybindings()
            .lock()
            .unwrap()
            .matches(data, "tui.select.cancel")
        {
            self.abort();
        }
    }
}
