//! Shared Pi-parity test harness.
//!
//! Mock/fake behavior belongs here or in dedicated integration tests, never in
//! `src/` modules.

#![allow(dead_code)]

pub mod http_capture;
pub mod live_credentials;
pub mod oauth_fixture;
pub mod sse_fixture;
pub mod ws_fixture;
