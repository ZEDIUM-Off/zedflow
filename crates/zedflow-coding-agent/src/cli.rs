//! CLI support modules ported from Pi's coding-agent package.

#[path = "cli/args.rs"]
pub mod args;
#[path = "cli/file-processor.rs"]
pub mod file_processor;
#[path = "cli/initial-message.rs"]
pub mod initial_message;

pub use args::{
    Args, Diagnostic, DiagnosticKind, Mode, help_text, is_valid_thinking_level, parse_args,
};
pub use file_processor::{ProcessFileOptions, ProcessedFiles, process_file_arguments};
pub use initial_message::{InitialMessageResult, build_initial_message};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Interactive,
    Print,
    Json,
    Rpc,
}

pub fn resolve_app_mode(parsed: &Args, stdin_is_tty: bool, stdout_is_tty: bool) -> AppMode {
    match parsed.mode {
        Some(Mode::Rpc) => AppMode::Rpc,
        Some(Mode::Json) => AppMode::Json,
        _ if parsed.print || !stdin_is_tty || !stdout_is_tty => AppMode::Print,
        _ => AppMode::Interactive,
    }
}
