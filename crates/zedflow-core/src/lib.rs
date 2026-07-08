#![forbid(unsafe_code)]

//! Shared substrate for Rust ports of Pi TypeScript packages.

pub mod error;
pub mod placeholders;

/// Crate identity, useful while the clean workspace skeleton is being filled.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
