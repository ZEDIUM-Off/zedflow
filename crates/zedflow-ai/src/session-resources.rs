//! Session-scoped cleanup registry ported from Pi's `packages/ai/src/session-resources.ts`.

use std::any::Any;
use std::error::Error as StdError;
use std::fmt;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

/// Boxed cleanup error returned by a session resource cleanup callback.
pub type SessionResourceCleanupBoxError = Box<dyn StdError + Send + Sync + 'static>;

/// Result returned by a session resource cleanup callback.
pub type SessionResourceCleanupResult = Result<(), SessionResourceCleanupBoxError>;

/// Callback registered to clean up resources for an optional session id.
pub type SessionResourceCleanup =
    Arc<dyn Fn(Option<&str>) -> SessionResourceCleanupResult + Send + Sync + 'static>;

static SESSION_RESOURCE_CLEANUPS: LazyLock<Mutex<Vec<Option<SessionResourceCleanup>>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

/// Registers a session resource cleanup callback and returns an unregister function.
///
/// Registering the same [`Arc`] callback more than once keeps one registry entry, matching
/// JavaScript `Set` semantics in the Pi implementation. Calling the returned function removes
/// the callback from future cleanup runs.
pub fn register_session_resource_cleanup(
    cleanup: SessionResourceCleanup,
) -> impl FnOnce() + Send + 'static {
    {
        let mut cleanups = lock_cleanups();
        if !cleanups
            .iter()
            .flatten()
            .any(|existing| Arc::ptr_eq(existing, &cleanup))
        {
            cleanups.push(Some(cleanup.clone()));
        }
    }

    move || unregister_session_resource_cleanup(&cleanup)
}

/// Runs all registered cleanup callbacks for an optional session id.
///
/// Like Pi, every registered cleanup is attempted. If one or more callbacks fail or panic, the
/// returned error aggregates all failures after iteration completes.
///
/// # Errors
///
/// Returns [`SessionResourceCleanupError`] when at least one registered cleanup callback returns
/// an error or panics.
pub fn cleanup_session_resources(
    session_id: Option<&str>,
) -> Result<(), SessionResourceCleanupError> {
    let mut failures = Vec::new();
    let mut index = 0;

    loop {
        let cleanup = {
            let cleanups = lock_cleanups();
            let Some(entry) = cleanups.get(index) else {
                break;
            };
            index += 1;
            entry.clone()
        };

        let Some(cleanup) = cleanup else {
            continue;
        };

        match panic::catch_unwind(AssertUnwindSafe(|| cleanup(session_id))) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => failures.push(SessionResourceCleanupFailure::Error(error)),
            Err(payload) => failures.push(SessionResourceCleanupFailure::Panic(
                panic_payload_to_string(&payload),
            )),
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(SessionResourceCleanupError::new(failures))
    }
}

/// Aggregate failure returned by [`cleanup_session_resources`].
#[derive(Debug)]
pub struct SessionResourceCleanupError {
    failures: Vec<SessionResourceCleanupFailure>,
}

impl SessionResourceCleanupError {
    /// Creates an aggregate cleanup error from callback failures.
    #[must_use]
    pub fn new(failures: Vec<SessionResourceCleanupFailure>) -> Self {
        Self { failures }
    }

    /// Returns the cleanup failures collected during one cleanup run.
    #[must_use]
    pub fn failures(&self) -> &[SessionResourceCleanupFailure] {
        &self.failures
    }
}

impl fmt::Display for SessionResourceCleanupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Failed to cleanup session resources")
    }
}

impl StdError for SessionResourceCleanupError {}

/// One failure captured while cleaning up session resources.
#[derive(Debug)]
pub enum SessionResourceCleanupFailure {
    /// A cleanup callback returned an error.
    Error(SessionResourceCleanupBoxError),
    /// A cleanup callback panicked with the contained message.
    Panic(String),
}

impl fmt::Display for SessionResourceCleanupFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error(error) => error.fmt(f),
            Self::Panic(message) => write!(f, "cleanup callback panicked: {message}"),
        }
    }
}

impl StdError for SessionResourceCleanupFailure {}

fn unregister_session_resource_cleanup(cleanup: &SessionResourceCleanup) {
    let mut cleanups = lock_cleanups();
    for entry in cleanups.iter_mut() {
        if entry
            .as_ref()
            .is_some_and(|existing| Arc::ptr_eq(existing, cleanup))
        {
            *entry = None;
            break;
        }
    }
}

fn lock_cleanups() -> MutexGuard<'static, Vec<Option<SessionResourceCleanup>>> {
    match SESSION_RESOURCE_CLEANUPS.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn panic_payload_to_string(payload: &Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_owned()
    }
}

#[cfg(test)]
fn clear_session_resource_cleanups_for_test() {
    lock_cleanups().clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::LazyLock;

    static TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    #[derive(Debug)]
    struct TestCleanupError;

    impl fmt::Display for TestCleanupError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("test cleanup failed")
        }
    }

    impl StdError for TestCleanupError {}

    #[test]
    fn cleanup_runs_registered_callbacks_and_unregisters_them() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        clear_session_resource_cleanups_for_test();

        let seen = Arc::new(Mutex::new(Vec::new()));
        let cleanup_seen = Arc::clone(&seen);
        let cleanup: SessionResourceCleanup = Arc::new(move |session_id| {
            cleanup_seen
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(session_id.map(str::to_owned));
            Ok(())
        });

        let unregister = register_session_resource_cleanup(cleanup);
        cleanup_session_resources(Some("session-1")).expect("cleanup should succeed");
        unregister();
        cleanup_session_resources(Some("session-2")).expect("cleanup should still succeed");

        assert_eq!(
            *seen.lock().unwrap_or_else(|poisoned| poisoned.into_inner()),
            vec![Some("session-1".to_owned())]
        );
        clear_session_resource_cleanups_for_test();
    }

    #[test]
    fn cleanup_aggregates_errors_and_continues() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        clear_session_resource_cleanups_for_test();

        let calls = Arc::new(Mutex::new(0));
        let cleanup_calls = Arc::clone(&calls);
        let failing: SessionResourceCleanup = Arc::new(|_| Err(Box::new(TestCleanupError)));
        let succeeding: SessionResourceCleanup = Arc::new(move |_| {
            *cleanup_calls
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) += 1;
            Ok(())
        });

        let _unregister_failing = register_session_resource_cleanup(failing);
        let _unregister_succeeding = register_session_resource_cleanup(succeeding);

        let error = cleanup_session_resources(None).expect_err("one cleanup should fail");

        assert_eq!(error.to_string(), "Failed to cleanup session resources");
        assert_eq!(error.failures().len(), 1);
        assert_eq!(
            *calls
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
            1
        );
        clear_session_resource_cleanups_for_test();
    }
}
