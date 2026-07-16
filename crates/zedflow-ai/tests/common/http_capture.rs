//! Pi-style fetch mock helpers for integration tests.
//!
//! These helpers intentionally live under `tests/common`: they are test-only
//! replacements for Vitest `vi.fn(fetch)` handlers.

use std::collections::{BTreeMap, VecDeque};
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;
use serde_json::Value;

const REDACTED: &str = "<redacted>";

/// A captured HTTP request, matching the parts Pi tests assert on.
#[derive(Clone, PartialEq, Eq)]
pub struct CapturedRequest {
    pub method: String,
    pub url: String,
    pub headers: BTreeMap<String, String>,
    pub body: Option<Vec<u8>>,
}

impl CapturedRequest {
    #[must_use]
    pub fn new(method: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            url: url.into(),
            headers: BTreeMap::new(),
            body: None,
        }
    }

    #[must_use]
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers
            .insert(normalize_header_name(name), value.into());
        self
    }

    #[must_use]
    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Serializes a JSON request body for tests that mirror `fetch(..., { body: JSON.stringify(...) })`.
    ///
    /// # Panics
    ///
    /// Panics when `value` cannot be serialized, which is a test bug.
    #[must_use]
    pub fn json_body<T: Serialize>(self, value: &T) -> Self {
        self.body(serde_json::to_vec(value).expect("test JSON body should serialize"))
            .header("content-type", "application/json")
    }

    #[must_use]
    pub fn body_text(&self) -> Option<String> {
        self.body
            .as_ref()
            .map(|body| String::from_utf8_lossy(body).into_owned())
    }

    #[must_use]
    pub fn body_json(&self) -> Option<Value> {
        self.body
            .as_ref()
            .and_then(|body| serde_json::from_slice(body).ok())
    }

    #[must_use]
    pub fn redacted_headers(&self) -> BTreeMap<String, String> {
        redact_headers(&self.headers)
    }
}

impl fmt::Debug for CapturedRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CapturedRequest")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("headers", &self.redacted_headers())
            .field("body", &self.body_text())
            .finish()
    }
}

/// A queued fake HTTP response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

impl FixtureResponse {
    #[must_use]
    pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            headers: BTreeMap::new(),
            body: body.into(),
        }
    }

    #[must_use]
    pub fn text(status: u16, body: impl Into<String>) -> Self {
        Self::new(status, body.into().into_bytes())
            .header("content-type", "text/plain; charset=utf-8")
    }

    /// Builds a JSON response.
    ///
    /// # Panics
    ///
    /// Panics when `value` cannot be serialized, which is a test bug.
    #[must_use]
    pub fn json<T: Serialize>(status: u16, value: &T) -> Self {
        Self::new(
            status,
            serde_json::to_vec(value).expect("test JSON response should serialize"),
        )
        .header("content-type", "application/json")
    }

    #[must_use]
    pub fn sse(body: impl Into<String>) -> Self {
        Self::new(200, body.into().into_bytes()).header("content-type", "text/event-stream")
    }

    #[must_use]
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers
            .insert(normalize_header_name(name), value.into());
        self
    }

    #[must_use]
    pub fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    #[must_use]
    pub fn body_json(&self) -> Option<Value> {
        serde_json::from_slice(&self.body).ok()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpCaptureError {
    UnexpectedRequest(CapturedRequest),
    PendingResponses(usize),
    MissingRequest,
}

impl fmt::Display for HttpCaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedRequest(request) => write!(f, "unexpected HTTP request: {request:?}"),
            Self::PendingResponses(count) => {
                write!(f, "{count} queued HTTP response(s) were not consumed")
            }
            Self::MissingRequest => write!(f, "expected an HTTP request, but none was captured"),
        }
    }
}

impl Error for HttpCaptureError {}

/// Sequenced fake transport: each request is captured, then the next response is returned.
#[derive(Debug, Clone, Default)]
pub struct HttpCapture {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Debug, Default)]
struct Inner {
    requests: Vec<CapturedRequest>,
    responses: VecDeque<FixtureResponse>,
}

impl HttpCapture {
    #[must_use]
    pub fn new(responses: impl IntoIterator<Item = FixtureResponse>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                requests: Vec::new(),
                responses: responses.into_iter().collect(),
            })),
        }
    }

    pub fn push_response(&self, response: FixtureResponse) {
        self.inner().responses.push_back(response);
    }

    pub fn request(&self, request: CapturedRequest) -> Result<FixtureResponse, HttpCaptureError> {
        let mut inner = self.inner();
        inner.requests.push(request.clone());
        inner
            .responses
            .pop_front()
            .ok_or(HttpCaptureError::UnexpectedRequest(request))
    }

    #[must_use]
    pub fn requests(&self) -> Vec<CapturedRequest> {
        self.inner().requests.clone()
    }

    pub fn take_requests(&self) -> Vec<CapturedRequest> {
        std::mem::take(&mut self.inner().requests)
    }

    pub fn next_request(&self) -> Result<CapturedRequest, HttpCaptureError> {
        self.take_requests()
            .into_iter()
            .next()
            .ok_or(HttpCaptureError::MissingRequest)
    }

    pub fn assert_no_pending_responses(&self) -> Result<(), HttpCaptureError> {
        let pending = self.inner().responses.len();
        if pending == 0 {
            Ok(())
        } else {
            Err(HttpCaptureError::PendingResponses(pending))
        }
    }

    #[must_use]
    pub fn pending_response_count(&self) -> usize {
        self.inner().responses.len()
    }

    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[must_use]
pub fn normalize_header_name(name: impl Into<String>) -> String {
    name.into().to_ascii_lowercase()
}

#[must_use]
pub fn redact_headers(headers: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    headers
        .iter()
        .map(|(name, value)| {
            let redacted = if is_secret_header(name) {
                REDACTED.to_owned()
            } else {
                value.clone()
            };
            (name.clone(), redacted)
        })
        .collect()
}

#[must_use]
pub fn is_secret_header(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    matches!(
        name.as_str(),
        "authorization"
            | "proxy-authorization"
            | "cookie"
            | "set-cookie"
            | "x-api-key"
            | "api-key"
            | "anthropic-api-key"
            | "openai-api-key"
            | "x-goog-api-key"
            | "x-stainless-api-key"
            | "copilot-authorization"
            | "x-copilot-authorization"
    )
}
