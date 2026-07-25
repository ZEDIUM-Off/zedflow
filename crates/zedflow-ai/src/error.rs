//! Error and placeholder conventions for the Pi AI package port.

use std::error::Error as StdError;
use std::fmt;

/// Canonical reason text for unresolved third-party replacements.
pub const PORT_PLACEHOLDER_REASON: &str = "no Rust replacement selected yet";

/// Exact marker required on every source-level port placeholder.
pub const PORT_PLACEHOLDER_MARKER: &str = r#"/// PORT PLACEHOLDER:
/// Original dependency: `<npm package / API>`.
/// Reason: no Rust replacement selected yet.
/// Required behavior: `<exact Pi behavior to preserve>`.
/// Replacement decision needed before production use."#;

/// Convenient result type for ported AI package code.
pub type Result<T> = std::result::Result<T, Error>;

/// Common error type for AI porting infrastructure.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// A code path reached a documented `PORT PLACEHOLDER` instead of a real port.
    PortPlaceholder(PortPlaceholderError),
}

impl Error {
    /// Creates an error for a documented `PORT PLACEHOLDER`.
    #[must_use]
    pub const fn port_placeholder(placeholder: PortPlaceholderError) -> Self {
        Self::PortPlaceholder(placeholder)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PortPlaceholder(error) => error.fmt(f),
        }
    }
}

impl StdError for Error {}

/// Error returned when a documented placeholder is intentionally left unimplemented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortPlaceholderError {
    original_dependency: &'static str,
    required_behavior: &'static str,
}

impl PortPlaceholderError {
    /// Creates a placeholder error with Pi parity context.
    #[must_use]
    pub const fn new(original_dependency: &'static str, required_behavior: &'static str) -> Self {
        Self {
            original_dependency,
            required_behavior,
        }
    }

    /// Original npm package or API that has no selected Rust replacement yet.
    #[must_use]
    pub const fn original_dependency(&self) -> &'static str {
        self.original_dependency
    }

    /// Reason the placeholder exists.
    #[must_use]
    pub const fn reason(&self) -> &'static str {
        PORT_PLACEHOLDER_REASON
    }

    /// Exact Pi behavior the future replacement must preserve.
    #[must_use]
    pub const fn required_behavior(&self) -> &'static str {
        self.required_behavior
    }
}

impl fmt::Display for PortPlaceholderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "port placeholder for {}: {}; required behavior: {}",
            self.original_dependency,
            self.reason(),
            self.required_behavior
        )
    }
}

impl StdError for PortPlaceholderError {}

impl From<PortPlaceholderError> for Error {
    fn from(value: PortPlaceholderError) -> Self {
        Self::PortPlaceholder(value)
    }
}

/// Builds a placeholder error for a documented missing replacement.
#[must_use]
pub const fn placeholder(
    original_dependency: &'static str,
    required_behavior: &'static str,
) -> PortPlaceholderError {
    PortPlaceholderError::new(original_dependency, required_behavior)
}

/// Returns an error for a documented missing replacement.
///
/// # Errors
///
/// Always returns [`Error::PortPlaceholder`] with the supplied Pi parity context.
pub fn unsupported<T>(
    original_dependency: &'static str,
    required_behavior: &'static str,
) -> Result<T> {
    Err(placeholder(original_dependency, required_behavior).into())
}
