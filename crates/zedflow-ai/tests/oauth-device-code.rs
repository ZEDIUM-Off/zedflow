use std::collections::VecDeque;
use std::convert::Infallible;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use zedflow_ai::utils::abort_signals::{AbortController, AbortSignal};
use zedflow_ai::utils::oauth::device_code::{
    OAuthDeviceCodeFlowError, OAuthDeviceCodePollOptions, OAuthDeviceCodePollResult,
    poll_oauth_device_code_flow_with_runtime,
};

fn fake_clock() -> Arc<AtomicU64> {
    Arc::new(AtomicU64::new(0))
}

fn now(clock: &Arc<AtomicU64>) -> impl Fn() -> Duration + use<> {
    let clock = Arc::clone(clock);
    move || Duration::from_millis(clock.load(Ordering::SeqCst))
}

fn sleep(
    clock: &Arc<AtomicU64>,
) -> impl FnMut(
    Duration,
    Option<AbortSignal>,
) -> std::future::Ready<Result<(), OAuthDeviceCodeFlowError>>
+ use<> {
    let clock = Arc::clone(clock);
    move |duration, signal| {
        if signal.as_ref().is_some_and(AbortSignal::aborted) {
            return std::future::ready(Err(OAuthDeviceCodeFlowError::Cancelled));
        }
        clock.fetch_add(duration.as_millis() as u64, Ordering::SeqCst);
        std::future::ready(Ok(()))
    }
}

#[tokio::test]
async fn polls_immediately_and_can_wait_before_the_first_poll() {
    for (wait_before_first_poll, expected) in [(false, vec![0, 2_000]), (true, vec![2_000, 4_000])]
    {
        let clock = fake_clock();
        let poll_times = Arc::new(Mutex::new(Vec::new()));
        let results = Arc::new(Mutex::new(VecDeque::from([
            OAuthDeviceCodePollResult::Pending,
            OAuthDeviceCodePollResult::Complete { value: "token" },
        ])));
        let value = poll_oauth_device_code_flow_with_runtime(
            OAuthDeviceCodePollOptions {
                interval_seconds: Some(2.0),
                expires_in_seconds: Some(30.0),
                wait_before_first_poll,
                poll: {
                    let clock = Arc::clone(&clock);
                    let poll_times = Arc::clone(&poll_times);
                    let results = Arc::clone(&results);
                    move || {
                        poll_times
                            .lock()
                            .unwrap()
                            .push(clock.load(Ordering::SeqCst));
                        let result = results.lock().unwrap().pop_front().unwrap();
                        async move { Ok::<_, Infallible>(result) }
                    }
                },
                signal: None,
            },
            now(&clock),
            sleep(&clock),
        )
        .await
        .unwrap();
        assert_eq!(value, "token");
        assert_eq!(*poll_times.lock().unwrap(), expected);
    }
}

#[tokio::test]
async fn applies_rfc_slow_down_and_server_intervals_exactly() {
    for (server_interval, expected_second_poll) in [(None, 7_000), (Some(30.0), 30_000)] {
        let clock = fake_clock();
        let poll_times = Arc::new(Mutex::new(Vec::new()));
        let results = Arc::new(Mutex::new(VecDeque::from([
            OAuthDeviceCodePollResult::SlowDown {
                interval_seconds: server_interval,
            },
            OAuthDeviceCodePollResult::Complete { value: "token" },
        ])));
        let value = poll_oauth_device_code_flow_with_runtime(
            OAuthDeviceCodePollOptions {
                interval_seconds: Some(2.0),
                expires_in_seconds: Some(900.0),
                wait_before_first_poll: false,
                poll: {
                    let clock = Arc::clone(&clock);
                    let poll_times = Arc::clone(&poll_times);
                    let results = Arc::clone(&results);
                    move || {
                        poll_times
                            .lock()
                            .unwrap()
                            .push(clock.load(Ordering::SeqCst));
                        let result = results.lock().unwrap().pop_front().unwrap();
                        async move { Ok::<_, Infallible>(result) }
                    }
                },
                signal: None,
            },
            now(&clock),
            sleep(&clock),
        )
        .await
        .unwrap();
        assert_eq!(value, "token");
        assert_eq!(*poll_times.lock().unwrap(), vec![0, expected_second_poll]);
    }
}

#[tokio::test]
async fn expiration_and_abort_match_pi_errors_without_waiting() {
    let clock = fake_clock();
    let timeout = poll_oauth_device_code_flow_with_runtime::<(), _, _, Infallible, _, _, _>(
        OAuthDeviceCodePollOptions {
            interval_seconds: Some(5.0),
            expires_in_seconds: Some(3.0),
            wait_before_first_poll: false,
            poll: || async { Ok(OAuthDeviceCodePollResult::Pending) },
            signal: None,
        },
        now(&clock),
        sleep(&clock),
    )
    .await
    .unwrap_err();
    assert_eq!(clock.load(Ordering::SeqCst), 3_000);
    assert_eq!(timeout.to_string(), "Device flow timed out");

    let clock = fake_clock();
    let controller = AbortController::new();
    let signal = controller.signal();
    let cancelled = poll_oauth_device_code_flow_with_runtime::<(), _, _, Infallible, _, _, _>(
        OAuthDeviceCodePollOptions {
            interval_seconds: Some(5.0),
            expires_in_seconds: Some(30.0),
            wait_before_first_poll: false,
            poll: move || {
                controller.abort();
                async { Ok(OAuthDeviceCodePollResult::Pending) }
            },
            signal: Some(signal),
        },
        now(&clock),
        sleep(&clock),
    )
    .await
    .unwrap_err();
    assert_eq!(clock.load(Ordering::SeqCst), 0);
    assert_eq!(cancelled.to_string(), "Login cancelled");
}
