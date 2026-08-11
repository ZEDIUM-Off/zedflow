//! Pi-compatible module `core/telemetry.rs`.

/// Returns this module's frozen Pi source path.
#[must_use]
pub const fn source_path() -> &'static str {
    "core/telemetry.rs"
}

#[path = "extensions/index.rs"]
pub mod index;
