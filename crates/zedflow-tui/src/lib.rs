#![forbid(unsafe_code)]

//! Dependency-light TUI runtime primitives.

pub mod keys;
#[path = "native-modifiers.rs"]
pub mod native_modifiers;
pub mod primitives;
#[path = "stdin-buffer.rs"]
pub mod stdin_buffer;
pub mod terminal;
#[path = "terminal-colors.rs"]
pub mod terminal_colors;
#[path = "terminal-image.rs"]
pub mod terminal_image;
pub mod tui;
#[path = "undo-stack.rs"]
pub mod undo_stack;
pub mod utils;
#[path = "word-navigation.rs"]
pub mod word_navigation;

pub use keys::*;
pub use primitives::*;
pub use tui::*;
pub use utils::{slice_by_column, truncate_to_width, visible_width, wrap_text_with_ansi};

pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
