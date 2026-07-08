//! Abort-signal composition helpers ported from Pi's `packages/ai/src/utils/abort-signals.ts`.

use std::any::Any;
use std::fmt;
use std::mem;
use std::sync::{Arc, Mutex, MutexGuard, Weak};

/// Opaque reason carried by an aborted signal.
#[derive(Clone)]
pub struct AbortReason {
    value: Arc<dyn Any + Send + Sync>,
}

impl AbortReason {
    /// Wraps an abort reason value.
    #[must_use]
    pub fn new<T>(value: T) -> Self
    where
        T: Any + Send + Sync,
    {
        Self {
            value: Arc::new(value),
        }
    }

    /// Returns the reason value when it has type `T`.
    #[must_use]
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.value.downcast_ref::<T>()
    }
}

impl fmt::Debug for AbortReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AbortReason(..)")
    }
}

#[derive(Default)]
struct AbortSignalInner {
    state: Mutex<AbortSignalState>,
}

#[derive(Default)]
struct AbortSignalState {
    aborted: bool,
    reason: Option<AbortReason>,
    next_listener_id: u64,
    listeners: Vec<AbortListenerEntry>,
}

struct AbortListenerEntry {
    id: u64,
    listener: Box<dyn FnOnce() + Send>,
}

/// An abort signal compatible with Pi's DOM `AbortSignal` usage.
#[derive(Clone)]
pub struct AbortSignal {
    inner: Arc<AbortSignalInner>,
}

impl AbortSignal {
    fn new() -> Self {
        Self {
            inner: Arc::new(AbortSignalInner::default()),
        }
    }

    /// Returns whether this signal has already been aborted.
    #[must_use]
    pub fn aborted(&self) -> bool {
        lock_state(&self.inner).aborted
    }

    /// Returns the abort reason, if one was supplied.
    #[must_use]
    pub fn reason(&self) -> Option<AbortReason> {
        lock_state(&self.inner).reason.clone()
    }

    /// Returns true when both handles point at the same signal.
    #[must_use]
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    fn abort(&self, reason: Option<AbortReason>) {
        let listeners = {
            let mut state = lock_state(&self.inner);
            if state.aborted {
                return;
            }
            state.aborted = true;
            state.reason = reason;
            mem::take(&mut state.listeners)
        };

        for listener in listeners {
            (listener.listener)();
        }
    }

    fn add_abort_listener_once(&self, listener: impl FnOnce() + Send + 'static) -> AbortListener {
        let mut state = lock_state(&self.inner);
        if state.aborted {
            return AbortListener::detached();
        }

        state.next_listener_id = state.next_listener_id.saturating_add(1);
        let id = state.next_listener_id;
        state.listeners.push(AbortListenerEntry {
            id,
            listener: Box::new(listener),
        });

        AbortListener {
            signal: Arc::downgrade(&self.inner),
            id,
        }
    }
}

impl fmt::Debug for AbortSignal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AbortSignal")
            .field("aborted", &self.aborted())
            .field("reason", &self.reason())
            .finish()
    }
}

impl PartialEq for AbortSignal {
    fn eq(&self, other: &Self) -> bool {
        self.ptr_eq(other)
    }
}

impl Eq for AbortSignal {}

/// Controller used to abort an [`AbortSignal`].
#[derive(Debug, Clone)]
pub struct AbortController {
    signal: AbortSignal,
}

impl AbortController {
    /// Creates a controller with a fresh, non-aborted signal.
    #[must_use]
    pub fn new() -> Self {
        Self {
            signal: AbortSignal::new(),
        }
    }

    /// Returns the signal controlled by this controller.
    #[must_use]
    pub fn signal(&self) -> AbortSignal {
        self.signal.clone()
    }

    /// Aborts the signal without a reason.
    pub fn abort(&self) {
        self.signal.abort(None);
    }

    /// Aborts the signal with `reason`.
    pub fn abort_with_reason<T>(&self, reason: T)
    where
        T: Any + Send + Sync,
    {
        self.signal.abort(Some(AbortReason::new(reason)));
    }
}

impl Default for AbortController {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct AbortListener {
    signal: Weak<AbortSignalInner>,
    id: u64,
}

impl AbortListener {
    fn detached() -> Self {
        Self {
            signal: Weak::new(),
            id: 0,
        }
    }

    fn remove(self) {
        let Some(signal) = self.signal.upgrade() else {
            return;
        };
        lock_state(&signal)
            .listeners
            .retain(|entry| entry.id != self.id);
    }
}

/// Result of combining optional abort signals.
#[derive(Debug)]
pub struct CombinedAbortSignal {
    /// Combined signal, absent when the input contained no active signals.
    pub signal: Option<AbortSignal>,
    listeners: Vec<AbortListener>,
}

impl CombinedAbortSignal {
    /// Removes listeners installed while combining multiple signals.
    pub fn cleanup(&mut self) {
        for listener in self.listeners.drain(..) {
            listener.remove();
        }
    }
}

/// Combines optional abort signals using Pi's `combineAbortSignals` behavior.
#[must_use]
pub fn combine_abort_signals(signals: &[Option<AbortSignal>]) -> CombinedAbortSignal {
    let active_signals: Vec<_> = signals.iter().filter_map(Clone::clone).collect();

    if active_signals.is_empty() {
        return CombinedAbortSignal {
            signal: None,
            listeners: Vec::new(),
        };
    }

    if active_signals.len() == 1 {
        return CombinedAbortSignal {
            signal: Some(active_signals[0].clone()),
            listeners: Vec::new(),
        };
    }

    let controller = AbortController::new();
    let controller_signal = controller.signal();
    let mut listeners = Vec::new();

    for signal in active_signals {
        if signal.aborted() {
            abort_from_signal(&controller_signal, &signal);
            break;
        }

        let combined_signal = controller_signal.clone();
        let source_signal = signal.clone();
        listeners.push(signal.add_abort_listener_once(move || {
            abort_from_signal(&combined_signal, &source_signal);
        }));
    }

    CombinedAbortSignal {
        signal: Some(controller_signal),
        listeners,
    }
}

fn abort_from_signal(combined_signal: &AbortSignal, source_signal: &AbortSignal) {
    if !combined_signal.aborted() {
        combined_signal.abort(source_signal.reason());
    }
}

fn lock_state(inner: &AbortSignalInner) -> MutexGuard<'_, AbortSignalState> {
    inner
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combines_no_signals_as_absent_signal() {
        let mut combined = combine_abort_signals(&[]);

        assert!(combined.signal.is_none());
        combined.cleanup();
    }

    #[test]
    fn returns_single_signal_without_wrapping() {
        let controller = AbortController::new();
        let signal = controller.signal();

        let combined = combine_abort_signals(&[Some(signal.clone())]);

        assert_eq!(combined.signal.as_ref(), Some(&signal));
    }

    #[test]
    fn aborts_combined_signal_with_first_source_reason() {
        let first = AbortController::new();
        let second = AbortController::new();
        let combined = combine_abort_signals(&[Some(first.signal()), Some(second.signal())]);
        let signal = combined
            .signal
            .expect("combined signal exists for multiple sources");

        second.abort_with_reason(String::from("second"));
        first.abort_with_reason(String::from("first"));

        let reason = signal.reason().expect("abort reason is propagated");
        assert_eq!(
            reason.downcast_ref::<String>().map(String::as_str),
            Some("second")
        );
    }

    #[test]
    fn cleanup_removes_registered_listeners() {
        let first = AbortController::new();
        let second = AbortController::new();
        let mut combined = combine_abort_signals(&[Some(first.signal()), Some(second.signal())]);
        let signal = combined
            .signal
            .clone()
            .expect("combined signal exists for multiple sources");

        combined.cleanup();
        first.abort();
        second.abort();

        assert!(!signal.aborted());
    }
}
