#![deny(unsafe_code)]

//! Dependency-light TUI runtime primitives.

pub mod autocomplete;
pub mod components;
#[path = "editor-component.rs"]
pub mod editor_component;
pub mod fuzzy;
pub mod index;
pub mod keybindings;
pub mod keys;
#[path = "kill-ring.rs"]
pub mod kill_ring;
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

pub use autocomplete::*;
pub use components::*;
pub use editor_component::*;
pub use fuzzy::*;
pub use keybindings::*;
pub use keys::*;
pub use kill_ring::KillRing;
pub use terminal::{ProcessTerminal, Terminal, TerminalEvent};
pub use terminal_colors::*;
pub use tui::*;
pub use undo_stack::UndoStack;
pub use utils::{slice_by_column, truncate_to_width, visible_width, wrap_text_with_ansi};
pub use word_navigation::{find_word_backward, find_word_forward};

pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
