//! OAuth/device-code fixtures and fake timing for Pi parity tests.

use std::collections::VecDeque;
use std::error::Error;
use std::fmt;

use base64::Engine as _;
use serde::Serialize;
use serde_json::{Value, json};

/// Device-code details Pi passes to `onDeviceCode` callbacks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCodeInfo {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub instructions: Option<String>,
    pub interval_seconds: u64,
    pub expires_in_seconds: u64,
}

impl DeviceCodeInfo {
    #[must_use]
    pub fn new(
        device_code: impl Into<String>,
        user_code: impl Into<String>,
        verification_uri: impl Into<String>,
    ) -> Self {
        Self {
            device_code: device_code.into(),
            user_code: user_code.into(),
            verification_uri: verification_uri.into(),
            instructions: None,
            interval_seconds: 5,
            expires_in_seconds: 900,
        }
    }

    #[must_use]
    pub fn interval_seconds(mut self, seconds: u64) -> Self {
        self.interval_seconds = seconds;
        self
    }

    #[must_use]
    pub fn expires_in_seconds(mut self, seconds: u64) -> Self {
        self.expires_in_seconds = seconds;
        self
    }

    #[must_use]
    pub fn instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = Some(instructions.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OAuthPoll<T> {
    Pending,
    SlowDown { interval_seconds: Option<u64> },
    Complete(T),
    Error(String),
}

impl<T> OAuthPoll<T> {
    #[must_use]
    pub fn slow_down() -> Self {
        Self::SlowDown {
            interval_seconds: None,
        }
    }

    #[must_use]
    pub fn slow_down_to(interval_seconds: u64) -> Self {
        Self::SlowDown {
            interval_seconds: Some(interval_seconds),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OAuthFixtureError {
    Expired,
    Cancelled,
    Failed(String),
    MissingPollResponse,
}

impl fmt::Display for OAuthFixtureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Expired => write!(f, "OAuth device-code flow expired"),
            Self::Cancelled => write!(f, "Login cancelled"),
            Self::Failed(message) => write!(f, "OAuth device-code flow failed: {message}"),
            Self::MissingPollResponse => write!(f, "OAuth fixture ran out of poll responses"),
        }
    }
}

impl Error for OAuthFixtureError {}

/// Fake clock used by OAuth polling tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FakeClock {
    now_ms: u64,
}

impl FakeClock {
    #[must_use]
    pub fn at_ms(now_ms: u64) -> Self {
        Self { now_ms }
    }

    #[must_use]
    pub fn now_ms(self) -> u64 {
        self.now_ms
    }

    pub fn advance_ms(&mut self, millis: u64) {
        self.now_ms = self.now_ms.saturating_add(millis);
    }
}

/// Deterministic simulation of Pi's OAuth device-code polling loop.
#[derive(Debug, Clone)]
pub struct DeviceCodePollingFixture<T> {
    interval_seconds: u64,
    expires_in_seconds: u64,
    wait_before_first_poll: bool,
    clock: FakeClock,
    poll_responses: VecDeque<OAuthPoll<T>>,
    poll_times_ms: Vec<u64>,
    cancelled: bool,
}

impl<T> DeviceCodePollingFixture<T> {
    #[must_use]
    pub fn new(interval_seconds: u64, expires_in_seconds: u64, start_ms: u64) -> Self {
        Self {
            interval_seconds,
            expires_in_seconds,
            wait_before_first_poll: false,
            clock: FakeClock::at_ms(start_ms),
            poll_responses: VecDeque::new(),
            poll_times_ms: Vec::new(),
            cancelled: false,
        }
    }

    #[must_use]
    pub fn wait_before_first_poll(mut self) -> Self {
        self.wait_before_first_poll = true;
        self
    }

    #[must_use]
    pub fn responses(mut self, responses: impl IntoIterator<Item = OAuthPoll<T>>) -> Self {
        self.poll_responses = responses.into_iter().collect();
        self
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    pub fn poll_until_complete(&mut self) -> Result<T, OAuthFixtureError> {
        let start_ms = self.clock.now_ms();
        let expires_at_ms = start_ms.saturating_add(self.expires_in_seconds.saturating_mul(1000));
        let mut next_interval_seconds = self.interval_seconds;

        if self.wait_before_first_poll {
            self.clock
                .advance_ms(next_interval_seconds.saturating_mul(1000));
        }

        loop {
            if self.cancelled {
                return Err(OAuthFixtureError::Cancelled);
            }
            if self.clock.now_ms() > expires_at_ms {
                return Err(OAuthFixtureError::Expired);
            }

            self.poll_times_ms.push(self.clock.now_ms());
            match self
                .poll_responses
                .pop_front()
                .ok_or(OAuthFixtureError::MissingPollResponse)?
            {
                OAuthPoll::Complete(value) => return Ok(value),
                OAuthPoll::Error(message) => return Err(OAuthFixtureError::Failed(message)),
                OAuthPoll::Pending => {}
                OAuthPoll::SlowDown { interval_seconds } => {
                    next_interval_seconds =
                        interval_seconds.unwrap_or(next_interval_seconds.saturating_add(5));
                }
            }

            self.clock
                .advance_ms(next_interval_seconds.saturating_mul(1000));
        }
    }

    #[must_use]
    pub fn poll_times_ms(&self) -> &[u64] {
        &self.poll_times_ms
    }

    #[must_use]
    pub fn now_ms(&self) -> u64 {
        self.clock.now_ms()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OAuthTokenFixture {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub token_type: String,
}

impl OAuthTokenFixture {
    #[must_use]
    pub fn bearer(access_token: impl Into<String>, refresh_token: impl Into<String>) -> Self {
        Self {
            access_token: access_token.into(),
            refresh_token: refresh_token.into(),
            expires_in: 3600,
            token_type: "Bearer".to_owned(),
        }
    }
}

#[must_use]
pub fn openai_codex_access_token(account_id: &str) -> String {
    unsigned_jwt(
        json!({ "alg": "none" }),
        json!({
            "https://api.openai.com/auth": { "chatgpt_account_id": account_id }
        }),
    )
}

#[must_use]
pub fn unsigned_jwt(header: Value, payload: Value) -> String {
    let engine = base64::engine::general_purpose::STANDARD;
    format!(
        "{}.{}.signature",
        engine.encode(serde_json::to_vec(&header).expect("test JWT header should serialize")),
        engine.encode(serde_json::to_vec(&payload).expect("test JWT payload should serialize"))
    )
}

#[must_use]
pub fn device_authorization_pending_body() -> Value {
    json!({
        "error": {
            "message": "Device authorization is pending. Please try again.",
            "type": "invalid_request_error",
            "param": null,
            "code": "deviceauth_authorization_pending"
        }
    })
}
