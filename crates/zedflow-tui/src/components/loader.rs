use std::sync::Arc;

use crate::{Component, components::Text};

type Style = Arc<dyn Fn(&str) -> String + Send + Sync>;

const DEFAULT_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

#[derive(Debug, Clone, Default)]
pub struct LoaderIndicatorOptions {
    /// Empty frames hide the indicator.
    pub frames: Option<Vec<String>>,
    pub interval_ms: Option<u64>,
}

/// Loader state. Animation is advanced by the owner thread with `advance_frame`.
pub struct Loader {
    pub text: Text,
    pub frames: Vec<String>,
    pub frame: usize,
    pub interval_ms: u64,
    message: String,
    spinner_style: Style,
    message_style: Style,
    verbatim_indicator: bool,
    running: bool,
}

impl Loader {
    pub fn new(message: impl Into<String>) -> Self {
        Self::with_styles(message, Arc::new(str::to_owned), Arc::new(str::to_owned))
    }

    pub fn with_styles(
        message: impl Into<String>,
        spinner_style: Style,
        message_style: Style,
    ) -> Self {
        let mut loader = Self {
            text: Text::new("", 1, 0),
            frames: DEFAULT_FRAMES.iter().map(|frame| (*frame).into()).collect(),
            frame: 0,
            interval_ms: 80,
            message: message.into(),
            spinner_style,
            message_style,
            verbatim_indicator: false,
            running: true,
        };
        loader.update_display();
        loader
    }

    pub fn start(&mut self) {
        self.running = true;
        self.update_display();
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    pub fn is_running(&self) -> bool {
        self.running
    }

    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.update_display();
    }

    pub fn set_indicator(&mut self, indicator: Option<LoaderIndicatorOptions>) {
        self.verbatim_indicator = indicator.is_some();
        self.frames = indicator
            .as_ref()
            .and_then(|options| options.frames.clone())
            .unwrap_or_else(|| DEFAULT_FRAMES.iter().map(|frame| (*frame).into()).collect());
        self.interval_ms = indicator
            .and_then(|options| options.interval_ms)
            .filter(|interval| *interval > 0)
            .unwrap_or(80);
        self.frame = 0;
        self.start();
    }

    /// Advance one deterministic animation frame on the TUI owner thread.
    pub fn advance_frame(&mut self) {
        if self.running && self.frames.len() > 1 {
            self.frame = (self.frame + 1) % self.frames.len();
            self.update_display();
        }
    }

    fn update_display(&mut self) {
        let frame = self
            .frames
            .get(self.frame)
            .map(String::as_str)
            .unwrap_or("");
        let indicator = if frame.is_empty() {
            String::new()
        } else if self.verbatim_indicator {
            format!("{frame} ")
        } else {
            format!("{} ", (self.spinner_style)(frame))
        };
        self.text.set_text(format!(
            "{indicator}{}",
            (self.message_style)(&self.message)
        ));
    }
}

impl Component for Loader {
    fn render(&self, width: usize) -> Vec<String> {
        let mut lines = vec![String::new()];
        lines.extend(self.text.render(width));
        lines
    }
}
