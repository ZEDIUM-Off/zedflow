//! Pi-compatible module `modes/index.rs`.

/// Returns this module's frozen Pi source path.
#[must_use]
pub const fn source_path() -> &'static str {
    "modes/index.rs"
}

#[path = "../core/export-html/index.rs"]
pub mod index;
