//! Pi-compatible module `bun/register-bedrock.rs`.

/// Returns this module's frozen Pi source path.
#[must_use]
pub const fn source_path() -> &'static str {
    "bun/register-bedrock.rs"
}

#[path = "../core/telemetry.rs"]
pub mod telemetry;
