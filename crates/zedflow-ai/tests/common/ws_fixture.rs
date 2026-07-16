//! WebSocket frame fixtures for Codex transport parity tests.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;
use serde_json::Value;

use super::http_capture::{normalize_header_name, redact_headers};

/// Server-side WebSocket event delivered to the code under test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsEvent {
    Open,
    Text(String),
    Error(String),
    Close { code: u16, reason: String },
}

impl WsEvent {
    #[must_use]
    pub fn text(data: impl Into<String>) -> Self {
        Self::Text(data.into())
    }

    /// Builds a JSON text frame.
    ///
    /// # Panics
    ///
    /// Panics when `value` cannot be serialized, which is a test bug.
    #[must_use]
    pub fn json<T: Serialize>(value: &T) -> Self {
        Self::Text(serde_json::to_string(value).expect("test websocket JSON should serialize"))
    }

    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error(message.into())
    }

    #[must_use]
    pub fn close(code: u16, reason: impl Into<String>) -> Self {
        Self::Close {
            code,
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsConnection {
    pub url: String,
    pub headers: BTreeMap<String, String>,
}

impl WsConnection {
    #[must_use]
    pub fn redacted_headers(&self) -> BTreeMap<String, String> {
        redact_headers(&self.headers)
    }
}

#[derive(Debug, Clone, Default)]
pub struct WebSocketFixture {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Debug, Default)]
struct Inner {
    connection: Option<WsConnection>,
    server_events: VecDeque<WsEvent>,
    sent_text: Vec<String>,
    closed: bool,
}

impl WebSocketFixture {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_events(events: impl IntoIterator<Item = WsEvent>) -> Self {
        let fixture = Self::new();
        {
            fixture.inner().server_events = events.into_iter().collect();
        }
        fixture
    }

    pub fn connect<I, K, V>(&self, url: impl Into<String>, headers: I)
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let headers = headers
            .into_iter()
            .map(|(name, value)| (normalize_header_name(name), value.into()))
            .collect();
        self.inner().connection = Some(WsConnection {
            url: url.into(),
            headers,
        });
    }

    pub fn push_event(&self, event: WsEvent) {
        self.inner().server_events.push_back(event);
    }

    pub fn send_text(&self, data: impl Into<String>) {
        self.inner().sent_text.push(data.into());
    }

    /// Sends a JSON client frame.
    ///
    /// # Panics
    ///
    /// Panics when `value` cannot be serialized, which is a test bug.
    pub fn send_json<T: Serialize>(&self, value: &T) {
        self.send_text(serde_json::to_string(value).expect("test websocket JSON should serialize"));
    }

    #[must_use]
    pub fn next_event(&self) -> Option<WsEvent> {
        self.inner().server_events.pop_front()
    }

    pub fn close(&self) {
        self.inner().closed = true;
    }

    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.inner().closed
    }

    #[must_use]
    pub fn connection(&self) -> Option<WsConnection> {
        self.inner().connection.clone()
    }

    #[must_use]
    pub fn sent_texts(&self) -> Vec<String> {
        self.inner().sent_text.clone()
    }

    #[must_use]
    pub fn sent_json(&self) -> Vec<Value> {
        self.sent_texts()
            .into_iter()
            .filter_map(|text| serde_json::from_str(&text).ok())
            .collect()
    }

    #[must_use]
    pub fn pending_event_count(&self) -> usize {
        self.inner().server_events.len()
    }

    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
