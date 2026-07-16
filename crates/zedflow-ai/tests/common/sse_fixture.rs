//! SSE fixture helpers for Pi stream parity tests.

use std::fmt;

use serde::Serialize;

/// One Server-Sent Events frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseFrame {
    pub event: Option<String>,
    pub data: Vec<String>,
    pub id: Option<String>,
    pub retry_ms: Option<u64>,
}

impl SseFrame {
    #[must_use]
    pub fn data(data: impl Into<String>) -> Self {
        Self {
            event: None,
            data: vec![data.into()],
            id: None,
            retry_ms: None,
        }
    }

    #[must_use]
    pub fn event(event: impl Into<String>, data: impl Into<String>) -> Self {
        Self::data(data).with_event(event)
    }

    /// Builds an SSE frame from JSON data.
    ///
    /// # Panics
    ///
    /// Panics when `value` cannot be serialized, which is a test bug.
    #[must_use]
    pub fn json<T: Serialize>(event: impl Into<String>, value: &T) -> Self {
        Self::event(
            event,
            serde_json::to_string(value).expect("test SSE JSON should serialize"),
        )
    }

    #[must_use]
    pub fn done() -> Self {
        Self::data("[DONE]")
    }

    #[must_use]
    pub fn with_event(mut self, event: impl Into<String>) -> Self {
        self.event = Some(event.into());
        self
    }

    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    #[must_use]
    pub fn with_retry_ms(mut self, retry_ms: u64) -> Self {
        self.retry_ms = Some(retry_ms);
        self
    }

    #[must_use]
    pub fn push_data_line(mut self, data: impl Into<String>) -> Self {
        self.data.push(data.into());
        self
    }

    #[must_use]
    pub fn data_text(&self) -> String {
        self.data.join("\n")
    }
}

impl fmt::Display for SseFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(event) = &self.event {
            writeln!(f, "event: {event}")?;
        }
        if let Some(id) = &self.id {
            writeln!(f, "id: {id}")?;
        }
        if let Some(retry_ms) = self.retry_ms {
            writeln!(f, "retry: {retry_ms}")?;
        }
        for data in &self.data {
            for line in data.lines() {
                writeln!(f, "data: {line}")?;
            }
            if data.is_empty() {
                writeln!(f, "data:")?;
            }
        }
        writeln!(f)
    }
}

/// A complete SSE response body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SseFixture {
    frames: Vec<SseFrame>,
}

impl SseFixture {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn from_frames(frames: impl IntoIterator<Item = SseFrame>) -> Self {
        Self {
            frames: frames.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn event(mut self, event: impl Into<String>, data: impl Into<String>) -> Self {
        self.frames.push(SseFrame::event(event, data));
        self
    }

    #[must_use]
    pub fn data(mut self, data: impl Into<String>) -> Self {
        self.frames.push(SseFrame::data(data));
        self
    }

    /// Adds a JSON event frame.
    ///
    /// # Panics
    ///
    /// Panics when `value` cannot be serialized, which is a test bug.
    #[must_use]
    pub fn json<T: Serialize>(mut self, event: impl Into<String>, value: &T) -> Self {
        self.frames.push(SseFrame::json(event, value));
        self
    }

    #[must_use]
    pub fn done(mut self) -> Self {
        self.frames.push(SseFrame::done());
        self
    }

    #[must_use]
    pub fn frames(&self) -> &[SseFrame] {
        &self.frames
    }

    #[must_use]
    pub fn into_frames(self) -> Vec<SseFrame> {
        self.frames
    }
}

impl fmt::Display for SseFixture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for frame in &self.frames {
            write!(f, "{frame}")?;
        }
        Ok(())
    }
}

#[must_use]
pub fn parse_sse(input: &str) -> Vec<SseFrame> {
    let mut frames = Vec::new();
    let mut current = SseFrame {
        event: None,
        data: Vec::new(),
        id: None,
        retry_ms: None,
    };
    let mut has_field = false;

    for line in input.lines() {
        if line.is_empty() {
            if has_field {
                frames.push(current);
                current = SseFrame {
                    event: None,
                    data: Vec::new(),
                    id: None,
                    retry_ms: None,
                };
                has_field = false;
            }
            continue;
        }
        if line.starts_with(':') {
            continue;
        }
        has_field = true;
        let (field, value) = line.split_once(':').map_or((line, ""), |(field, value)| {
            (field, value.strip_prefix(' ').unwrap_or(value))
        });
        match field {
            "event" => current.event = Some(value.to_owned()),
            "data" => current.data.push(value.to_owned()),
            "id" => current.id = Some(value.to_owned()),
            "retry" => current.retry_ms = value.parse().ok(),
            _ => {}
        }
    }

    if has_field {
        frames.push(current);
    }

    frames
}
