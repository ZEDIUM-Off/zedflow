//! Pi coding-agent status indicators for the interactive mode.

use zedflow_tui::Component;

const DEFAULT_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const DEFAULT_INTERVAL_MS: u64 = 80;
const INTERRUPT_KEY: &str = "escape";

/// Pi's active interactive status indicator variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusIndicatorKind {
    Working,
    Retry,
    Compaction,
    BranchSummary,
}

/// Extension-provided spinner configuration for the working indicator.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkingIndicatorOptions {
    pub frames: Option<Vec<String>>,
    pub interval_ms: Option<u64>,
}

/// Shared spinner and message state for Pi's status indicators.
#[derive(Debug, Clone)]
pub struct StatusIndicator {
    pub kind: StatusIndicatorKind,
    message: String,
    frames: Vec<String>,
    interval_ms: u64,
    current_frame: usize,
    running: bool,
}

impl StatusIndicator {
    #[must_use]
    pub fn new(
        kind: StatusIndicatorKind,
        message: impl Into<String>,
        indicator: Option<WorkingIndicatorOptions>,
    ) -> Self {
        let mut status = Self {
            kind,
            message: message.into(),
            frames: DEFAULT_FRAMES.iter().map(|frame| (*frame).into()).collect(),
            interval_ms: DEFAULT_INTERVAL_MS,
            current_frame: 0,
            running: false,
        };
        status.set_indicator(indicator);
        status
    }

    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn interval_ms(&self) -> u64 {
        self.interval_ms
    }

    pub fn set_indicator(&mut self, indicator: Option<WorkingIndicatorOptions>) {
        self.frames = indicator
            .as_ref()
            .and_then(|options| options.frames.clone())
            .unwrap_or_else(|| DEFAULT_FRAMES.iter().map(|frame| (*frame).into()).collect());
        self.interval_ms = indicator
            .and_then(|options| options.interval_ms)
            .filter(|interval| *interval > 0)
            .unwrap_or(DEFAULT_INTERVAL_MS);
        self.current_frame = 0;
        self.running = true;
    }

    /// Advance the animation once; the interactive runtime calls this per frame interval.
    pub fn advance_frame(&mut self) {
        if self.running && self.frames.len() > 1 {
            self.current_frame = (self.current_frame + 1) % self.frames.len();
        }
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    pub fn dispose(&mut self) {
        self.stop();
    }

    fn display(&self) -> String {
        let frame = self
            .frames
            .get(self.current_frame)
            .map_or("", String::as_str);
        if frame.is_empty() {
            self.message.clone()
        } else {
            format!("{frame} {}", self.message)
        }
    }
}

impl Component for StatusIndicator {
    fn render(&self, width: usize) -> Vec<String> {
        let line = self.display();
        let content_width = width.saturating_sub(2);
        let content: String = line.chars().take(content_width).collect();
        let rendered = format!(" {content}");
        vec!["".into(), format!("{rendered:<width$}")]
    }
}

/// The streaming-work indicator.
#[derive(Debug, Clone)]
pub struct WorkingStatusIndicator(pub StatusIndicator);

impl WorkingStatusIndicator {
    #[must_use]
    pub fn new(message: impl Into<String>, indicator: Option<WorkingIndicatorOptions>) -> Self {
        Self(StatusIndicator::new(
            StatusIndicatorKind::Working,
            message,
            indicator,
        ))
    }
}

impl Component for WorkingStatusIndicator {
    fn render(&self, width: usize) -> Vec<String> {
        self.0.render(width)
    }
}

/// The retry-wait indicator. `tick` is driven by the runtime once per second.
#[derive(Debug, Clone)]
pub struct RetryStatusIndicator {
    pub status: StatusIndicator,
    attempt: u32,
    max_attempts: u32,
    remaining_seconds: Option<u64>,
}

impl RetryStatusIndicator {
    #[must_use]
    pub fn new(attempt: u32, max_attempts: u32, delay_ms: u64) -> Self {
        let seconds = delay_ms.div_ceil(1_000);
        Self {
            status: StatusIndicator::new(
                StatusIndicatorKind::Retry,
                retry_message(attempt, max_attempts, seconds),
                None,
            ),
            attempt,
            max_attempts,
            remaining_seconds: Some(seconds),
        }
    }

    /// Applies one CountdownTimer interval and returns whether the timer remains active.
    pub fn tick(&mut self) -> bool {
        let Some(remaining) = self.remaining_seconds else {
            return false;
        };
        let remaining = remaining.saturating_sub(1);
        self.status
            .set_message(retry_message(self.attempt, self.max_attempts, remaining));
        if remaining == 0 {
            self.remaining_seconds = None;
            false
        } else {
            self.remaining_seconds = Some(remaining);
            true
        }
    }

    pub fn dispose(&mut self) {
        self.remaining_seconds = None;
        self.status.dispose();
    }
}

impl Component for RetryStatusIndicator {
    fn render(&self, width: usize) -> Vec<String> {
        self.status.render(width)
    }
}

/// The reason that triggered context compaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactionStatusReason {
    Manual,
    Threshold,
    Overflow,
}

/// The context-compaction indicator.
#[derive(Debug, Clone)]
pub struct CompactionStatusIndicator(pub StatusIndicator);

impl CompactionStatusIndicator {
    #[must_use]
    pub fn new(reason: CompactionStatusReason) -> Self {
        let cancel_hint = format!("({INTERRUPT_KEY} to cancel)");
        let message = match reason {
            CompactionStatusReason::Manual => format!("Compacting context... {cancel_hint}"),
            CompactionStatusReason::Threshold => format!("Auto-compacting... {cancel_hint}"),
            CompactionStatusReason::Overflow => {
                format!("Context overflow detected, Auto-compacting... {cancel_hint}")
            }
        };
        Self(StatusIndicator::new(
            StatusIndicatorKind::Compaction,
            message,
            None,
        ))
    }
}

impl Component for CompactionStatusIndicator {
    fn render(&self, width: usize) -> Vec<String> {
        self.0.render(width)
    }
}

/// The branch-summary indicator.
#[derive(Debug, Clone)]
pub struct BranchSummaryStatusIndicator(pub StatusIndicator);

impl BranchSummaryStatusIndicator {
    #[must_use]
    pub fn new() -> Self {
        Self(StatusIndicator::new(
            StatusIndicatorKind::BranchSummary,
            format!("Summarizing branch... ({INTERRUPT_KEY} to cancel)"),
            None,
        ))
    }
}

impl Default for BranchSummaryStatusIndicator {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for BranchSummaryStatusIndicator {
    fn render(&self, width: usize) -> Vec<String> {
        self.0.render(width)
    }
}

/// A two-line placeholder that prevents status-area height changes while idle.
#[derive(Debug, Default, Clone, Copy)]
pub struct IdleStatus;

impl Component for IdleStatus {
    fn render(&self, width: usize) -> Vec<String> {
        let empty_line = " ".repeat(width);
        vec![empty_line.clone(), empty_line]
    }
}

fn retry_message(attempt: u32, max_attempts: u32, seconds: u64) -> String {
    format!("Retrying ({attempt}/{max_attempts}) in {seconds}s... ({INTERRUPT_KEY} to cancel)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_height_and_retry_disposal_match_pi() {
        assert_eq!(IdleStatus.render(20), vec![" ".repeat(20), " ".repeat(20)]);

        let mut retry = RetryStatusIndicator::new(1, 3, 1_000);
        retry.dispose();
        assert!(!retry.tick());
        assert_eq!(
            retry.status.message(),
            "Retrying (1/3) in 1s... (escape to cancel)"
        );
    }
}
