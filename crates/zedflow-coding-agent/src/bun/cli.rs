//! Pi-compatible module `bun/cli.rs`.

/// Returns this module's frozen Pi source path.
#[must_use]
pub const fn source_path() -> &'static str {
    "bun/cli.rs"
}

#[path = "../core/index.rs"]
pub mod index;
