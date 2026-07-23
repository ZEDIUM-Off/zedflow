//! Library-facing CLI entry point.

pub use crate::cli::{parse_args, Args, Diagnostic, DiagnosticType, Mode, UnknownFlagValue};

#[must_use]
pub fn parse_environment_args(args: &[String]) -> Args { parse_args(args.iter().cloned()) }
