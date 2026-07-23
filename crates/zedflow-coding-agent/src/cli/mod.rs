#[path = "args.rs"]
pub mod args;
#[path = "file-processor.rs"]
pub mod file_processor;
#[path = "initial-message.rs"]
pub mod initial_message;

/// CLI helpers kept intentionally side-effect free; UI-specific commands are
/// implemented by the host application.
#[path = "config-selector.rs"]
pub mod config_selector;
#[path = "list-models.rs"]
pub mod list_models;
#[path = "project-trust.rs"]
pub mod project_trust;
#[path = "session-picker.rs"]
pub mod session_picker;
#[path = "startup-ui.rs"]
pub mod startup_ui;

pub use args::{Args, Diagnostic, DiagnosticType, Mode, UnknownFlagValue, parse_args};
pub use initial_message::{InitialMessageResult, build_initial_message};
