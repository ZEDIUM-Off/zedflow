//! Crate-private Tokio worker support for synchronous stream entrypoints.

use std::future::Future;

/// Runs a sendable task on the current Tokio runtime, or on a private fallback runtime.
pub(crate) fn spawn_worker(task: impl Future<Output = ()> + Send + 'static) {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(task);
    } else {
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Tokio current-thread runtime construction must succeed")
                .block_on(task);
        });
    }
}
