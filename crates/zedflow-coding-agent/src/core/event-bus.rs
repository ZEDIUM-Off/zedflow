use std::{
    any::Any,
    collections::HashMap,
    panic::AssertUnwindSafe,
    sync::{Arc, Mutex},
};

pub type EventData = Arc<dyn Any + Send + Sync>;
type Handler = Arc<dyn Fn(EventData) + Send + Sync>;

#[derive(Clone, Default)]
pub struct EventBus {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    next_id: u64,
    handlers: HashMap<String, Vec<(u64, Handler)>>,
}

impl EventBus {
    pub fn emit(&self, channel: &str, data: EventData) {
        let handlers = self
            .inner
            .lock()
            .unwrap()
            .handlers
            .get(channel)
            .cloned()
            .unwrap_or_default();
        for (_, handler) in handlers {
            if std::panic::catch_unwind(AssertUnwindSafe(|| handler(data.clone()))).is_err() {
                eprintln!("Event handler error ({channel})");
            }
        }
    }

    pub fn on<F>(&self, channel: impl Into<String>, handler: F) -> impl FnOnce()
    where
        F: Fn(EventData) + Send + Sync + 'static,
    {
        let channel = channel.into();
        let id = {
            let mut inner = self.inner.lock().unwrap();
            let id = inner.next_id;
            inner.next_id += 1;
            inner
                .handlers
                .entry(channel.clone())
                .or_default()
                .push((id, Arc::new(handler)));
            id
        };
        let inner = self.inner.clone();
        move || {
            if let Some(handlers) = inner.lock().unwrap().handlers.get_mut(&channel) {
                handlers.retain(|(handler_id, _)| *handler_id != id);
            }
        }
    }

    pub fn clear(&self) {
        self.inner.lock().unwrap().handlers.clear();
    }
}

pub fn create_event_bus() -> EventBus {
    EventBus::default()
}
