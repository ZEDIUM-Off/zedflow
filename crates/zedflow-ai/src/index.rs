//! Core side-effect-free exports ported from Pi's `packages/ai/src/index.ts`.
//!
//! Rust exposes the same crate modules directly from `lib.rs`; this module is a marker
//! for the TypeScript package entrypoint row.

pub use crate::auth::context;
pub use crate::auth::types;
pub use crate::images_models;

/// Index entrypoint name from the source package.
pub const INDEX_ENTRYPOINT: &str = "@earendil-works/pi-ai";
