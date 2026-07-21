#![forbid(unsafe_code)]

//! Zedflow coding-agent crate.

/// Deterministic utilities ported from Pi's coding-agent package.
pub mod utils;

/// Crate identity, useful while the clean workspace skeleton is being filled.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
