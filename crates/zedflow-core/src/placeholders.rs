//! Canonical placeholder helpers for Pi dependencies without selected Rust replacements.

use crate::error::{PortPlaceholderError, Result};

pub use crate::error::PORT_PLACEHOLDER_REASON;

/// Exact marker required on every source-level port placeholder.
pub const PORT_PLACEHOLDER_MARKER: &str = r#"/// PORT PLACEHOLDER:
/// Original dependency: `<npm package / API>`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `<exact Pi behavior to preserve>`.
/// Replacement decision needed before production use."#;

/// Builds a placeholder error for a documented missing replacement.
#[must_use]
pub const fn error(
    original_dependency: &'static str,
    required_behavior: &'static str,
) -> PortPlaceholderError {
    PortPlaceholderError::new(original_dependency, required_behavior)
}

/// Returns an error for a documented missing replacement.
///
/// # Errors
///
/// Always returns [`crate::error::Error::PortPlaceholder`] with the supplied Pi parity context.
pub fn unsupported<T>(
    original_dependency: &'static str,
    required_behavior: &'static str,
) -> Result<T> {
    Err(error(original_dependency, required_behavior).into())
}
