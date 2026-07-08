//! OAuth device-code polling helpers ported from Pi's `packages/ai/src/utils/oauth/device-code.ts`.

use std::error::Error as StdError;
use std::fmt;
use std::future::Future;
use std::time::{Duration, Instant};

use futures::channel::oneshot;

use crate::utils::abort_signals::AbortSignal;

const CANCEL_MESSAGE: &str = "Login cancelled";
const TIMEOUT_MESSAGE: &str = "Device flow timed out";
const SLOW_DOWN_TIMEOUT_MESSAGE: &str = "Device flow timed out after one or more slow_down responses. This is often caused by clock drift in WSL or VM environments. Please sync or restart the VM clock and try again.";
const MINIMUM_INTERVAL: Duration = Duration::from_millis(1000);
// RFC 8628 section 3.2: if the authorization server omits `interval`, the client must use 5 seconds.
const DEFAULT_POLL_INTERVAL_SECONDS: f64 = 5.0;
// RFC 8628 section 3.5: `slow_down` means the polling interval must increase by 5 seconds.
const SLOW_DOWN_INTERVAL_INCREMENT: Duration = Duration::from_millis(5000);
const ABORT_CHECK_INTERVAL: Duration = Duration::from_millis(50);

/// Result alias for OAuth device-code polling.
pub type OAuthDeviceCodeFlowResult<T> = Result<T, OAuthDeviceCodeFlowError>;

/// Errors returned while polling an OAuth device-code flow.
#[derive(Debug)]
#[non_exhaustive]
pub enum OAuthDeviceCodeFlowError {
    /// The login flow was cancelled by an abort signal.
    Cancelled,
    /// The device-code deadline elapsed before completion.
    TimedOut {
        /// Whether at least one `slow_down` response was received before timeout.
        after_slow_down: bool,
    },
    /// The authorization server returned a terminal failure message.
    Failed(String),
    /// The caller-provided poll operation failed.
    Poll(Box<dyn StdError + Send + Sync>),
}

impl fmt::Display for OAuthDeviceCodeFlowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str(CANCEL_MESSAGE),
            Self::TimedOut {
                after_slow_down: true,
            } => formatter.write_str(SLOW_DOWN_TIMEOUT_MESSAGE),
            Self::TimedOut {
                after_slow_down: false,
            } => formatter.write_str(TIMEOUT_MESSAGE),
            Self::Failed(message) => formatter.write_str(message),
            Self::Poll(error) => write!(formatter, "device code poll failed: {error}"),
        }
    }
}

impl StdError for OAuthDeviceCodeFlowError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Poll(error) => Some(error.as_ref()),
            Self::Cancelled | Self::TimedOut { .. } | Self::Failed(_) => None,
        }
    }
}

/// Status returned by one OAuth device-code polling attempt.
#[derive(Debug, Clone, PartialEq)]
pub enum OAuthDeviceCodePollResult<T> {
    /// Authorization is still pending.
    Pending,
    /// The server requested a slower polling interval.
    SlowDown {
        /// Server-provided replacement polling interval, in seconds.
        interval_seconds: Option<f64>,
    },
    /// The flow failed with a terminal message.
    Failed {
        /// Error message to return to the caller.
        message: String,
    },
    /// The flow completed successfully.
    Complete {
        /// Completed value returned by the flow.
        value: T,
    },
}

/// Options for [`poll_oauth_device_code_flow`].
#[derive(Debug, Clone)]
pub struct OAuthDeviceCodePollOptions<P> {
    /// Initial polling interval in seconds. Defaults to five seconds.
    pub interval_seconds: Option<f64>,
    /// Device-code expiration duration in seconds. `None` means no deadline.
    pub expires_in_seconds: Option<f64>,
    /// Whether to wait one interval before the first poll attempt.
    pub wait_before_first_poll: bool,
    /// Async operation that performs one poll attempt.
    pub poll: P,
    /// Optional signal used to cancel the flow.
    pub signal: Option<AbortSignal>,
}

/// Polls an OAuth device-code flow until it completes, fails, is cancelled, or expires.
///
/// This preserves Pi's RFC 8628 behavior: a missing interval defaults to five seconds,
/// intervals are never below one second, and `slow_down` either adopts the server interval
/// or adds five seconds to the current interval.
///
/// # Errors
///
/// Returns [`OAuthDeviceCodeFlowError::Cancelled`] when the signal aborts,
/// [`OAuthDeviceCodeFlowError::TimedOut`] when the deadline elapses,
/// [`OAuthDeviceCodeFlowError::Failed`] for terminal poll failures, or
/// [`OAuthDeviceCodeFlowError::Poll`] when the caller-provided poll operation errors.
pub async fn poll_oauth_device_code_flow<T, P, Fut, E>(
    options: OAuthDeviceCodePollOptions<P>,
) -> OAuthDeviceCodeFlowResult<T>
where
    P: FnMut() -> Fut,
    Fut: Future<Output = Result<OAuthDeviceCodePollResult<T>, E>>,
    E: StdError + Send + Sync + 'static,
{
    let OAuthDeviceCodePollOptions {
        interval_seconds,
        expires_in_seconds,
        wait_before_first_poll,
        mut poll,
        signal,
    } = options;

    let deadline = deadline_from_now(expires_in_seconds);
    let mut interval = interval_duration(interval_seconds);
    let mut slow_down_responses = 0_usize;

    if wait_before_first_poll {
        let Some(sleep_for) = sleep_duration(interval, deadline) else {
            return Err(timeout_error(slow_down_responses));
        };
        abortable_sleep(sleep_for, signal.as_ref()).await?;
    }

    while !is_expired(deadline) {
        if is_aborted(signal.as_ref()) {
            return Err(OAuthDeviceCodeFlowError::Cancelled);
        }

        match poll()
            .await
            .map_err(|error| OAuthDeviceCodeFlowError::Poll(Box::new(error)))?
        {
            OAuthDeviceCodePollResult::Complete { value } => return Ok(value),
            OAuthDeviceCodePollResult::Failed { message } => {
                return Err(OAuthDeviceCodeFlowError::Failed(message));
            }
            OAuthDeviceCodePollResult::Pending => {}
            OAuthDeviceCodePollResult::SlowDown { interval_seconds } => {
                slow_down_responses = slow_down_responses.saturating_add(1);
                interval = interval_seconds.map_or_else(
                    || interval.saturating_add(SLOW_DOWN_INTERVAL_INCREMENT),
                    |seconds| interval_duration(Some(seconds)),
                );
            }
        }

        let Some(sleep_for) = sleep_duration(interval, deadline) else {
            break;
        };
        abortable_sleep(sleep_for, signal.as_ref()).await?;
    }

    Err(timeout_error(slow_down_responses))
}

fn deadline_from_now(expires_in_seconds: Option<f64>) -> Option<Instant> {
    match expires_in_seconds {
        None => None,
        Some(seconds) if seconds.is_infinite() && seconds.is_sign_positive() => None,
        Some(seconds) if seconds.is_finite() && seconds > 0.0 => {
            Instant::now().checked_add(Duration::from_secs_f64(seconds))
        }
        Some(_) => Some(Instant::now()),
    }
}

fn interval_duration(interval_seconds: Option<f64>) -> Duration {
    let seconds = interval_seconds.unwrap_or(DEFAULT_POLL_INTERVAL_SECONDS);
    if !seconds.is_finite() {
        return MINIMUM_INTERVAL;
    }

    let millis = (seconds * 1000.0)
        .floor()
        .max(MINIMUM_INTERVAL.as_millis() as f64)
        .min(u64::MAX as f64) as u64;
    Duration::from_millis(millis)
}

fn sleep_duration(interval: Duration, deadline: Option<Instant>) -> Option<Duration> {
    let Some(deadline) = deadline else {
        return Some(interval);
    };

    deadline
        .checked_duration_since(Instant::now())
        .map(|remaining| interval.min(remaining))
        .filter(|duration| !duration.is_zero())
}

fn is_expired(deadline: Option<Instant>) -> bool {
    deadline.is_some_and(|deadline| Instant::now() >= deadline)
}

fn timeout_error(slow_down_responses: usize) -> OAuthDeviceCodeFlowError {
    OAuthDeviceCodeFlowError::TimedOut {
        after_slow_down: slow_down_responses > 0,
    }
}

async fn abortable_sleep(
    duration: Duration,
    signal: Option<&AbortSignal>,
) -> OAuthDeviceCodeFlowResult<()> {
    if is_aborted(signal) {
        return Err(OAuthDeviceCodeFlowError::Cancelled);
    }

    let signal = signal.cloned();
    let thread_signal = signal.clone();
    let (sender, receiver) = oneshot::channel();
    let spawn_result = std::thread::Builder::new()
        .name("oauth-device-code-sleep".to_owned())
        .spawn(move || {
            let completed = sleep_until_or_abort(duration, thread_signal.as_ref());
            let _ = sender.send(completed);
        });

    if spawn_result.is_err() {
        return blocking_sleep_until_or_abort(duration, signal.as_ref());
    }

    match receiver.await {
        Ok(true) => Ok(()),
        Ok(false) => Err(OAuthDeviceCodeFlowError::Cancelled),
        Err(_) => Ok(()),
    }
}

fn blocking_sleep_until_or_abort(
    duration: Duration,
    signal: Option<&AbortSignal>,
) -> OAuthDeviceCodeFlowResult<()> {
    if sleep_until_or_abort(duration, signal) {
        Ok(())
    } else {
        Err(OAuthDeviceCodeFlowError::Cancelled)
    }
}

fn sleep_until_or_abort(duration: Duration, signal: Option<&AbortSignal>) -> bool {
    let Some(deadline) = Instant::now().checked_add(duration) else {
        return true;
    };

    loop {
        if is_aborted(signal) {
            return false;
        }

        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return true;
        };
        if remaining.is_zero() {
            return true;
        }

        std::thread::sleep(remaining.min(ABORT_CHECK_INTERVAL));
    }
}

fn is_aborted(signal: Option<&AbortSignal>) -> bool {
    signal.is_some_and(AbortSignal::aborted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, mpsc};
    use std::thread;

    #[derive(Debug)]
    struct PollError;

    impl fmt::Display for PollError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("poll failed")
        }
    }

    impl StdError for PollError {}

    fn recorded_count(poll_times: &Arc<Mutex<Vec<Instant>>>) -> usize {
        poll_times.lock().expect("poll times lock poisoned").len()
    }

    fn recorded_elapsed(poll_times: &Arc<Mutex<Vec<Instant>>>, start: Instant) -> Vec<Duration> {
        poll_times
            .lock()
            .expect("poll times lock poisoned")
            .iter()
            .map(|time| time.duration_since(start))
            .collect()
    }

    fn wait_for_count(
        poll_times: &Arc<Mutex<Vec<Instant>>>,
        expected_count: usize,
        timeout: Duration,
    ) {
        let deadline = Instant::now()
            .checked_add(timeout)
            .expect("test timeout overflowed");

        while Instant::now() < deadline {
            if recorded_count(poll_times) >= expected_count {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }

        panic!("expected {expected_count} polls before timeout");
    }

    #[test]
    fn polls_immediately_and_returns_the_completed_value() {
        let start = Instant::now();
        let poll_times = Arc::new(Mutex::new(Vec::new()));
        let poll_count = Arc::new(AtomicUsize::new(0));
        let thread_poll_times = Arc::clone(&poll_times);
        let thread_poll_count = Arc::clone(&poll_count);

        let handle = thread::spawn(move || {
            block_on(poll_oauth_device_code_flow(OAuthDeviceCodePollOptions {
                interval_seconds: Some(2.0),
                expires_in_seconds: Some(30.0),
                wait_before_first_poll: false,
                poll: move || {
                    thread_poll_times
                        .lock()
                        .expect("poll times lock poisoned")
                        .push(Instant::now());
                    let poll_number = thread_poll_count.fetch_add(1, Ordering::SeqCst) + 1;
                    async move {
                        Ok::<_, PollError>(if poll_number == 1 {
                            OAuthDeviceCodePollResult::Pending
                        } else {
                            OAuthDeviceCodePollResult::Complete { value: "token" }
                        })
                    }
                },
                signal: None,
            }))
        });

        wait_for_count(&poll_times, 1, Duration::from_secs(1));

        thread::sleep(Duration::from_millis(1_950));
        let result = handle
            .join()
            .expect("poll thread panicked")
            .expect("polling should complete");

        assert_eq!(result, "token");
        let elapsed = recorded_elapsed(&poll_times, start);
        assert_eq!(elapsed.len(), 2);
        assert!(elapsed[0] < Duration::from_millis(100));
        assert!(elapsed[1] >= Duration::from_secs(2));
    }

    #[test]
    fn can_wait_before_the_first_poll() {
        let start = Instant::now();
        let poll_times = Arc::new(Mutex::new(Vec::new()));
        let thread_poll_times = Arc::clone(&poll_times);

        let handle = thread::spawn(move || {
            block_on(poll_oauth_device_code_flow(OAuthDeviceCodePollOptions {
                interval_seconds: Some(2.0),
                expires_in_seconds: Some(30.0),
                wait_before_first_poll: true,
                poll: move || {
                    thread_poll_times
                        .lock()
                        .expect("poll times lock poisoned")
                        .push(Instant::now());
                    async {
                        Ok::<_, PollError>(OAuthDeviceCodePollResult::Complete { value: "token" })
                    }
                },
                signal: None,
            }))
        });

        thread::sleep(Duration::from_millis(100));
        assert_eq!(recorded_count(&poll_times), 0);

        let result = handle
            .join()
            .expect("poll thread panicked")
            .expect("polling should complete");

        assert_eq!(result, "token");
        let elapsed = recorded_elapsed(&poll_times, start);
        assert_eq!(elapsed.len(), 1);
        assert!(elapsed[0] >= Duration::from_secs(2));
    }

    #[test]
    fn increases_the_interval_by_5_seconds_after_slow_down_without_a_server_interval() {
        let start = Instant::now();
        let poll_times = Arc::new(Mutex::new(Vec::new()));
        let results = Arc::new(Mutex::new(VecDeque::from([
            OAuthDeviceCodePollResult::SlowDown {
                interval_seconds: None,
            },
            OAuthDeviceCodePollResult::Complete { value: "token" },
        ])));
        let thread_poll_times = Arc::clone(&poll_times);
        let thread_results = Arc::clone(&results);

        let handle = thread::spawn(move || {
            block_on(poll_oauth_device_code_flow(OAuthDeviceCodePollOptions {
                interval_seconds: Some(2.0),
                expires_in_seconds: Some(900.0),
                wait_before_first_poll: false,
                poll: move || {
                    thread_poll_times
                        .lock()
                        .expect("poll times lock poisoned")
                        .push(Instant::now());
                    let result = thread_results
                        .lock()
                        .expect("results lock poisoned")
                        .pop_front()
                        .expect("unexpected extra poll");
                    async move { Ok::<_, PollError>(result) }
                },
                signal: None,
            }))
        });

        wait_for_count(&poll_times, 1, Duration::from_secs(1));

        thread::sleep(Duration::from_millis(6_950));
        let result = handle
            .join()
            .expect("poll thread panicked")
            .expect("polling should complete");

        assert_eq!(result, "token");
        let elapsed = recorded_elapsed(&poll_times, start);
        assert_eq!(elapsed.len(), 2);
        assert!(elapsed[1] >= Duration::from_secs(7));
    }

    #[test]
    fn honors_a_server_provided_slow_down_interval() {
        let start = Instant::now();
        let poll_times = Arc::new(Mutex::new(Vec::new()));
        let results = Arc::new(Mutex::new(VecDeque::from([
            OAuthDeviceCodePollResult::SlowDown {
                interval_seconds: Some(1.0),
            },
            OAuthDeviceCodePollResult::Complete { value: "token" },
        ])));
        let thread_poll_times = Arc::clone(&poll_times);
        let thread_results = Arc::clone(&results);

        let handle = thread::spawn(move || {
            block_on(poll_oauth_device_code_flow(OAuthDeviceCodePollOptions {
                interval_seconds: Some(2.0),
                expires_in_seconds: Some(900.0),
                wait_before_first_poll: false,
                poll: move || {
                    thread_poll_times
                        .lock()
                        .expect("poll times lock poisoned")
                        .push(Instant::now());
                    let result = thread_results
                        .lock()
                        .expect("results lock poisoned")
                        .pop_front()
                        .expect("unexpected extra poll");
                    async move { Ok::<_, PollError>(result) }
                },
                signal: None,
            }))
        });

        wait_for_count(&poll_times, 1, Duration::from_secs(1));

        thread::sleep(Duration::from_millis(950));
        let result = handle
            .join()
            .expect("poll thread panicked")
            .expect("polling should complete");

        assert_eq!(result, "token");
        let elapsed = recorded_elapsed(&poll_times, start);
        assert_eq!(elapsed.len(), 2);
        assert!(elapsed[1] >= Duration::from_secs(1));
        assert!(elapsed[1] < Duration::from_secs(7));
    }

    #[test]
    fn cancels_an_in_flight_wait() {
        let controller = crate::utils::abort_signals::AbortController::new();
        let (poll_started_sender, poll_started_receiver) = mpsc::channel();
        let mut poll_started_sender = Some(poll_started_sender);
        let signal = controller.signal();

        let handle = thread::spawn(move || {
            block_on(poll_oauth_device_code_flow::<(), _, _, _>(
                OAuthDeviceCodePollOptions {
                    interval_seconds: Some(5.0),
                    expires_in_seconds: Some(30.0),
                    wait_before_first_poll: false,
                    poll: move || {
                        if let Some(sender) = poll_started_sender.take() {
                            sender.send(()).expect("poll-start notification failed");
                        }
                        async { Ok::<_, PollError>(OAuthDeviceCodePollResult::Pending) }
                    },
                    signal: Some(signal),
                },
            ))
        });

        poll_started_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("poll should start");
        controller.abort();

        let error = handle
            .join()
            .expect("poll thread panicked")
            .expect_err("aborted wait should error");

        assert_eq!(error.to_string(), CANCEL_MESSAGE);
    }

    #[test]
    fn returns_failed_message() {
        let error = block_on(poll_oauth_device_code_flow::<(), _, _, _>(
            OAuthDeviceCodePollOptions {
                interval_seconds: None,
                expires_in_seconds: Some(30.0),
                wait_before_first_poll: false,
                poll: || async {
                    Ok::<_, PollError>(OAuthDeviceCodePollResult::Failed {
                        message: "denied".to_owned(),
                    })
                },
                signal: None,
            },
        ))
        .expect_err("failed poll result should error");

        assert_eq!(error.to_string(), "denied");
    }

    #[test]
    fn cancelled_signal_errors_before_polling() {
        let controller = crate::utils::abort_signals::AbortController::new();
        controller.abort();

        let error = block_on(poll_oauth_device_code_flow::<(), _, _, _>(
            OAuthDeviceCodePollOptions {
                interval_seconds: None,
                expires_in_seconds: Some(30.0),
                wait_before_first_poll: false,
                poll: || async { Ok::<_, PollError>(OAuthDeviceCodePollResult::Pending) },
                signal: Some(controller.signal()),
            },
        ))
        .expect_err("aborted signal should error");

        assert_eq!(error.to_string(), CANCEL_MESSAGE);
    }
}
