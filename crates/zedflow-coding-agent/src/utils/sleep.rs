use std::time::Duration;
use zedflow_ai::utils::abort_signals::AbortSignal;

/// Sleeps for `ms`, returning an error if the optional signal aborts.
pub async fn sleep(ms: u64, signal: Option<&AbortSignal>) -> Result<(), &'static str> {
    if signal.is_some_and(AbortSignal::aborted) {
        return Err("Aborted");
    }
    if let Some(signal) = signal {
        tokio::select! {
            () = tokio::time::sleep(Duration::from_millis(ms)) => Ok(()),
            () = signal.cancelled() => Err("Aborted"),
        }
    } else {
        tokio::time::sleep(Duration::from_millis(ms)).await;
        Ok(())
    }
}
