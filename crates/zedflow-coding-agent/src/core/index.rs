//! Pi-compatible module `core/index.rs`.

/// Returns this module's frozen Pi source path.
#[must_use]
pub const fn source_path() -> &'static str {
    "core/index.rs"
}

#[path = "../modes/index.rs"]
pub mod index;
