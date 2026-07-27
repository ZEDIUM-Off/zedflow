#![forbid(unsafe_code)]

//! Dependency-light TUI runtime primitives.

pub mod autocomplete;
pub mod components {
    #[path = "../components/box.rs"]
    pub mod r#box;
    #[path = "../components/cancellable_loader.rs"]
    pub mod cancellable_loader;
    #[path = "../components/editor.rs"]
    pub mod editor;
    #[path = "../components/image.rs"]
    pub mod image;
    #[path = "../components/input.rs"]
    pub mod input;
    #[path = "../components/loader.rs"]
    pub mod loader;
    #[path = "../components/markdown.rs"]
    pub mod markdown;
    #[path = "../components/select_list.rs"]
    pub mod select_list;
    #[path = "../components/settings_list.rs"]
    pub mod settings_list;
    #[path = "../components/spacer.rs"]
    pub mod spacer;
    #[path = "../components/text.rs"]
    pub mod text;
    #[path = "../components/truncated_text.rs"]
    pub mod truncated_text;
    pub use r#box::Box;
    pub use cancellable_loader::CancellableLoader;
    pub use editor::Editor;
    pub use image::Image;
    pub use input::Input;
    pub use loader::Loader;
    pub use markdown::Markdown;
    pub use select_list::SelectList;
    pub use settings_list::SettingsList;
    pub use spacer::Spacer;
    pub use text::Text;
    pub use truncated_text::TruncatedText;
}
#[path = "editor-component.rs"]
pub mod editor_component;
pub mod fuzzy;
pub mod index;
#[path = "keybindings.rs"]
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
pub mod utils {
    include!("utils.rs");

    pub fn truncate_to_width(text: &str, width: usize) -> String {
        if visible_width(text) <= width {
            return text.to_owned();
        }
        let mut out = String::new();
        let mut used = 0;
        for c in text.chars() {
            let cw = visible_width(&c.to_string());
            if used + cw > width {
                break;
            }
            out.push(c);
            used += cw;
        }
        out
    }

    pub fn wrap_text_with_ansi(text: &str, width: usize) -> Vec<String> {
        if width == 0 {
            return vec![String::new()];
        }
        text.lines()
            .flat_map(|line| {
                if line.is_empty() {
                    return vec![String::new()];
                }
                let mut rows = Vec::new();
                let mut row = String::new();
                for word in line.split_whitespace() {
                    let candidate = if row.is_empty() {
                        word.to_owned()
                    } else {
                        format!("{row} {word}")
                    };
                    if visible_width(&candidate) > width && !row.is_empty() {
                        rows.push(row);
                        row = word.to_owned();
                    } else {
                        row = candidate;
                    }
                }
                if !row.is_empty() {
                    rows.push(row);
                }
                rows
            })
            .collect()
    }
}
#[path = "word-navigation.rs"]
pub mod word_navigation;
pub use keys::*;
pub use primitives::*;

/// Zero-width marker emitted at the logical cursor position.
pub const CURSOR_MARKER: &str = "\x1b_pi:c\x07";

/// A renderable TUI component.
pub trait Component {
    fn render(&self, width: usize) -> Vec<String>;
    fn invalidate(&mut self) {}
    fn handle_input(&mut self, _data: &str) {}
    fn wants_key_release(&self) -> bool {
        false
    }
}

/// Components which can expose a cursor to the terminal.
pub trait Focusable {
    fn set_focused(&mut self, focused: bool);
    fn is_focused(&self) -> bool;
}

/// A component that renders children in insertion order.
pub struct Container {
    children: Vec<Box<dyn Component>>,
}

impl Container {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }
    pub fn add_child(&mut self, child: impl Component + 'static) {
        self.children.push(Box::new(child));
    }
    pub fn remove_child(&mut self, index: usize) -> Option<Box<dyn Component>> {
        (index < self.children.len()).then(|| self.children.remove(index))
    }
    pub fn clear(&mut self) {
        self.children.clear();
    }
    pub fn len(&self) -> usize {
        self.children.len()
    }
    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }
}

impl Default for Container {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Container {
    fn render(&self, width: usize) -> Vec<String> {
        self.children
            .iter()
            .flat_map(|child| child.render(width))
            .collect()
    }
    fn invalidate(&mut self) {
        for child in &mut self.children {
            child.invalidate();
        }
    }
    fn handle_input(&mut self, data: &str) {
        if let Some(child) = self.children.last_mut() {
            child.handle_input(data);
        }
    }
}

/// Minimal runtime state for composing a base component and modal overlays.
pub struct Tui {
    pub root: Container,
    overlays: Vec<Box<dyn Component>>,
    focused: Option<usize>,
}

impl Tui {
    pub fn new() -> Self {
        Self {
            root: Container::new(),
            overlays: Vec::new(),
            focused: None,
        }
    }
    pub fn show_overlay(&mut self, overlay: impl Component + 'static) -> usize {
        self.overlays.push(Box::new(overlay));
        let id = self.overlays.len() - 1;
        self.focused = Some(id);
        id
    }
    pub fn hide_overlay(&mut self, id: usize) -> Option<Box<dyn Component>> {
        if id >= self.overlays.len() {
            return None;
        }
        let removed = self.overlays.remove(id);
        self.focused = self.overlays.len().checked_sub(1);
        Some(removed)
    }
    pub fn render(&self, width: usize) -> Vec<String> {
        let mut lines = self.root.render(width);
        if let Some(overlay) = self.overlays.last() {
            lines.extend(overlay.render(width));
        }
        lines
    }
    pub fn dispatch_input(&mut self, data: &str) {
        if let Some(id) = self.focused {
            if let Some(overlay) = self.overlays.get_mut(id) {
                overlay.handle_input(data);
                return;
            }
        }
        self.root.handle_input(data);
    }
    pub fn overlay_count(&self) -> usize {
        self.overlays.len()
    }
}

impl Default for Tui {
    fn default() -> Self {
        Self::new()
    }
}

pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
