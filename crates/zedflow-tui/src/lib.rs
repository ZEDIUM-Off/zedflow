#![forbid(unsafe_code)]

//! Zedflow tui crate.

pub mod keys;
pub mod primitives;
pub use keys::*;
pub use primitives::*;

/// Crate identity, useful while the clean workspace skeleton is being filled.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
