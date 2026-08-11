use crate::utils::{slice_by_column, visible_width};
use crate::{
    CURSOR_MARKER, Component, Focusable, KillRing, UndoStack, decode_kitty_printable,
    find_word_backward, find_word_forward, get_keybindings,
};
use icu_segmenter::GraphemeClusterSegmenter;

#[derive(Clone, Debug, PartialEq, Eq)]
struct InputState {
    value: String,
    cursor: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LastAction {
    Kill,
    Yank,
    TypeWord,
}

/// Single-line input with Pi-compatible editing, paste, kill/yank, undo, and scrolling.
pub struct Input {
    value: String,
    cursor: usize,
    pub focused: bool,
    pub on_submit: Option<Box<dyn FnMut(&str)>>,
    pub on_escape: Option<Box<dyn FnMut()>>,
    paste_buffer: String,
    in_paste: bool,
    kill_ring: KillRing,
    last_action: Option<LastAction>,
    undo_stack: UndoStack<InputState>,
}

impl Default for Input {
    fn default() -> Self {
        Self::new("")
    }
}

impl Input {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        Self {
            cursor: value.len(),
            value,
            focused: false,
            on_submit: None,
            on_escape: None,
            paste_buffer: String::new(),
            in_paste: false,
            kill_ring: KillRing::default(),
            last_action: None,
            undo_stack: UndoStack::default(),
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }
    pub fn get_value(&self) -> &str {
        &self.value
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.cursor = self.cursor.min(self.value.len());
        while !self.value.is_char_boundary(self.cursor) {
            self.cursor -= 1;
        }
    }

    fn matches(data: &str, binding: &str) -> bool {
        get_keybindings().lock().unwrap().matches(data, binding)
    }

    fn push_undo(&mut self) {
        self.undo_stack.push(&InputState {
            value: self.value.clone(),
            cursor: self.cursor,
        });
    }

    fn insert(&mut self, text: &str) {
        if text.chars().any(char::is_whitespace) || self.last_action != Some(LastAction::TypeWord) {
            self.push_undo();
        }
        self.last_action = Some(LastAction::TypeWord);
        self.value.insert_str(self.cursor, text);
        self.cursor += text.len();
    }

    fn previous_grapheme(&self) -> usize {
        let boundaries: Vec<_> = GraphemeClusterSegmenter::new()
            .segment_str(&self.value[..self.cursor])
            .collect();
        boundaries
            .get(boundaries.len().saturating_sub(2))
            .copied()
            .unwrap_or(0)
    }

    fn next_grapheme(&self) -> usize {
        GraphemeClusterSegmenter::new()
            .segment_str(&self.value[self.cursor..])
            .nth(1)
            .map_or(self.value.len(), |n| self.cursor + n)
    }

    fn delete_range_to_ring(&mut self, start: usize, end: usize, prepend: bool) {
        if start == end {
            return;
        }
        self.push_undo();
        let killed = self.value[start..end].to_owned();
        self.kill_ring
            .push(&killed, prepend, self.last_action == Some(LastAction::Kill));
        self.value.replace_range(start..end, "");
        self.cursor = start;
        self.last_action = Some(LastAction::Kill);
    }

    fn paste(&mut self, text: &str) {
        self.last_action = None;
        self.push_undo();
        let clean = text
            .replace("\r\n", "")
            .replace(['\r', '\n'], "")
            .replace('\t', "    ");
        self.value.insert_str(self.cursor, &clean);
        self.cursor += clean.len();
    }

    fn handle(&mut self, data: &str) {
        let paste_data;
        let data = if let Some(start) = data.find("\x1b[200~") {
            self.in_paste = true;
            self.paste_buffer.clear();
            paste_data = format!("{}{}", &data[..start], &data[start + 6..]);
            paste_data.as_str()
        } else {
            data
        };
        if self.in_paste {
            self.paste_buffer.push_str(data);
            if let Some(end) = self.paste_buffer.find("\x1b[201~") {
                let paste = self.paste_buffer[..end].to_owned();
                let remaining = self.paste_buffer[end + 6..].to_owned();
                self.paste_buffer.clear();
                self.in_paste = false;
                self.paste(&paste);
                if !remaining.is_empty() {
                    self.handle(&remaining);
                }
            }
            return;
        }

        if Self::matches(data, "tui.select.cancel") {
            if let Some(callback) = &mut self.on_escape {
                callback();
            }
        } else if Self::matches(data, "tui.editor.undo") {
            if let Some(state) = self.undo_stack.pop() {
                self.value = state.value;
                self.cursor = state.cursor;
                self.last_action = None;
            }
        } else if Self::matches(data, "tui.input.submit") || data == "\n" {
            if let Some(callback) = &mut self.on_submit {
                callback(&self.value);
            }
        } else if Self::matches(data, "tui.editor.deleteCharBackward") {
            self.last_action = None;
            if self.cursor > 0 {
                self.push_undo();
                let start = self.previous_grapheme();
                self.value.replace_range(start..self.cursor, "");
                self.cursor = start;
            }
        } else if Self::matches(data, "tui.editor.deleteCharForward") {
            self.last_action = None;
            if self.cursor < self.value.len() {
                self.push_undo();
                let end = self.next_grapheme();
                self.value.replace_range(self.cursor..end, "");
            }
        } else if Self::matches(data, "tui.editor.deleteWordBackward") {
            let start = find_word_backward(&self.value, self.cursor);
            self.delete_range_to_ring(start, self.cursor, true);
        } else if Self::matches(data, "tui.editor.deleteWordForward") {
            let end = find_word_forward(&self.value, self.cursor);
            self.delete_range_to_ring(self.cursor, end, false);
        } else if Self::matches(data, "tui.editor.deleteToLineStart") {
            self.delete_range_to_ring(0, self.cursor, true);
        } else if Self::matches(data, "tui.editor.deleteToLineEnd") {
            self.delete_range_to_ring(self.cursor, self.value.len(), false);
        } else if Self::matches(data, "tui.editor.yank") {
            if let Some(text) = self.kill_ring.peek().map(str::to_owned) {
                self.push_undo();
                self.value.insert_str(self.cursor, &text);
                self.cursor += text.len();
                self.last_action = Some(LastAction::Yank);
            }
        } else if Self::matches(data, "tui.editor.yankPop") {
            if self.last_action == Some(LastAction::Yank) && self.kill_ring.len() > 1 {
                self.push_undo();
                let old = self.kill_ring.peek().unwrap().len();
                self.value.replace_range(self.cursor - old..self.cursor, "");
                self.cursor -= old;
                self.kill_ring.rotate();
                let text = self.kill_ring.peek().unwrap().to_owned();
                self.value.insert_str(self.cursor, &text);
                self.cursor += text.len();
            }
        } else if Self::matches(data, "tui.editor.cursorLeft") {
            self.last_action = None;
            if self.cursor > 0 {
                self.cursor = self.previous_grapheme();
            }
        } else if Self::matches(data, "tui.editor.cursorRight") {
            self.last_action = None;
            if self.cursor < self.value.len() {
                self.cursor = self.next_grapheme();
            }
        } else if Self::matches(data, "tui.editor.cursorLineStart") {
            self.last_action = None;
            self.cursor = 0;
        } else if Self::matches(data, "tui.editor.cursorLineEnd") {
            self.last_action = None;
            self.cursor = self.value.len();
        } else if Self::matches(data, "tui.editor.cursorWordLeft") {
            self.last_action = None;
            self.cursor = find_word_backward(&self.value, self.cursor);
        } else if Self::matches(data, "tui.editor.cursorWordRight") {
            self.last_action = None;
            self.cursor = find_word_forward(&self.value, self.cursor);
        } else if let Some(printable) = decode_kitty_printable(data) {
            self.insert(&printable);
        } else if !data.chars().any(|c| c.is_control()) {
            self.insert(data);
        }
    }
}

impl Component for Input {
    fn render(&self, width: usize) -> Vec<String> {
        let prompt = "> ";
        let available = width.saturating_sub(2);
        if available == 0 {
            return vec![prompt.into()];
        }
        let total = visible_width(&self.value);
        let (visible, cursor) = if total < available {
            (self.value.clone(), self.cursor)
        } else {
            let scroll_width = if self.cursor == self.value.len() {
                available.saturating_sub(1)
            } else {
                available
            };
            let cursor_col = visible_width(&self.value[..self.cursor]);
            let half = scroll_width / 2;
            let start = if cursor_col < half {
                0
            } else if cursor_col > total.saturating_sub(half) {
                total.saturating_sub(scroll_width)
            } else {
                cursor_col - half
            };
            let visible = slice_by_column(&self.value, start, scroll_width, true);
            let before =
                slice_by_column(&self.value, start, cursor_col.saturating_sub(start), true);
            let cursor = before.len();
            (visible, cursor)
        };
        let end = GraphemeClusterSegmenter::new()
            .segment_str(&visible[cursor..])
            .nth(1)
            .unwrap_or(visible.len() - cursor)
            + cursor;
        let at = if cursor == visible.len() {
            " "
        } else {
            &visible[cursor..end]
        };
        let marker = if self.focused { CURSOR_MARKER } else { "" };
        let mut line = format!(
            "> {}{}\x1b[7m{}\x1b[27m{}",
            &visible[..cursor],
            marker,
            at,
            &visible[end..]
        );
        line.push_str(&" ".repeat(available.saturating_sub(visible_width(&line) - 2)));
        vec![line]
    }
    fn handle_input(&mut self, data: &str) {
        self.handle(data);
    }
    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
    fn is_focused(&self) -> bool {
        self.focused
    }
}
impl Focusable for Input {
    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
    fn is_focused(&self) -> bool {
        self.focused
    }
}
