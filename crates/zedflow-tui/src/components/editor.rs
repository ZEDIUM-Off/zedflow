use crate::{
    AutocompleteProvider, AutocompleteSuggestions, CURSOR_MARKER, Component, Focusable, KillRing,
    SelectItem, UndoStack, decode_printable_key, find_word_backward, find_word_forward,
    get_keybindings, matches_key,
    utils::{truncate_to_width, visible_width},
};
use icu_segmenter::GraphemeClusterSegmenter;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextChunk {
    pub text: String,
    pub start_index: usize,
    pub end_index: usize,
}

/// Pi's word-aware wrapping. Byte offsets are retained so cursor placement is lossless.
pub fn word_wrap_line(line: &str, max_width: usize) -> Vec<TextChunk> {
    if line.is_empty() || max_width == 0 {
        return vec![TextChunk {
            text: String::new(),
            start_index: 0,
            end_index: 0,
        }];
    }
    if visible_width(line) <= max_width {
        return vec![TextChunk {
            text: line.into(),
            start_index: 0,
            end_index: line.len(),
        }];
    }
    let boundaries: Vec<usize> = GraphemeClusterSegmenter::new().segment_str(line).collect();
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut width = 0;
    let mut wrap: Option<(usize, usize)> = None;
    for pair in boundaries.windows(2) {
        let at = pair[0];
        let end = pair[1];
        let grapheme = &line[at..end];
        let grapheme_width = visible_width(grapheme);
        if width + grapheme_width > max_width {
            if let Some((wrap_at, wrap_width)) = wrap
                && width.saturating_sub(wrap_width) + grapheme_width <= max_width
            {
                chunks.push(TextChunk {
                    text: line[start..wrap_at].into(),
                    start_index: start,
                    end_index: wrap_at,
                });
                start = wrap_at;
                width = width.saturating_sub(wrap_width);
            } else if start < at {
                chunks.push(TextChunk {
                    text: line[start..at].into(),
                    start_index: start,
                    end_index: at,
                });
                start = at;
                width = 0;
            }
            wrap = None;
        }
        width += grapheme_width;
        let next = line.get(end..).and_then(|s| s.chars().next());
        if grapheme.chars().all(char::is_whitespace) && next.is_some_and(|c| !c.is_whitespace()) {
            wrap = Some((end, width));
        }
    }
    chunks.push(TextChunk {
        text: line[start..].into(),
        start_index: start,
        end_index: line.len(),
    });
    chunks
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EditorState {
    lines: Vec<String>,
    cursor_line: usize,
    cursor_col: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LastAction {
    Kill,
    Yank,
    TypeWord,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JumpMode {
    Forward,
    Backward,
}

pub struct Editor {
    state: EditorState,
    pub focused: bool,
    pub on_submit: Option<Box<dyn FnMut(&str)>>,
    pub on_change: Option<Box<dyn FnMut(&str)>>,
    pub disable_submit: bool,
    padding_x: usize,
    autocomplete_max_visible: usize,
    autocomplete_provider: Option<Box<dyn AutocompleteProvider>>,
    autocomplete: Option<AutocompleteSuggestions>,
    autocomplete_selected: usize,
    history: Vec<String>,
    history_index: Option<usize>,
    history_draft: Option<EditorState>,
    kill_ring: KillRing,
    last_action: Option<LastAction>,
    undo: UndoStack<EditorState>,
    paste_buffer: String,
    in_paste: bool,
    pastes: HashMap<usize, String>,
    paste_counter: usize,
    preferred_col: Option<usize>,
    jump_mode: Option<JumpMode>,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    pub fn new() -> Self {
        Self {
            state: EditorState {
                lines: vec![String::new()],
                cursor_line: 0,
                cursor_col: 0,
            },
            focused: false,
            on_submit: None,
            on_change: None,
            disable_submit: false,
            padding_x: 0,
            autocomplete_max_visible: 5,
            autocomplete_provider: None,
            autocomplete: None,
            autocomplete_selected: 0,
            history: Vec::new(),
            history_index: None,
            history_draft: None,
            kill_ring: KillRing::default(),
            last_action: None,
            undo: UndoStack::default(),
            paste_buffer: String::new(),
            in_paste: false,
            pastes: HashMap::new(),
            paste_counter: 0,
            preferred_col: None,
            jump_mode: None,
        }
    }

    pub fn word_wrap_line(line: &str, max_width: usize) -> Vec<TextChunk> {
        word_wrap_line(line, max_width)
    }
    pub fn get_text(&self) -> String {
        self.state.lines.join("\n")
    }
    pub fn text(&self) -> String {
        self.get_text()
    }
    pub fn get_lines(&self) -> Vec<String> {
        self.state.lines.clone()
    }
    pub fn get_cursor(&self) -> (usize, usize) {
        (self.state.cursor_line, self.state.cursor_col)
    }
    pub fn get_expanded_text(&self) -> String {
        let mut result = self.text();
        for (id, value) in &self.pastes {
            for marker in [
                format!("[paste #{id} +{} lines]", value.split('\n').count()),
                format!("[paste #{id} {} chars]", value.len()),
            ] {
                result = result.replace(&marker, value);
            }
        }
        result
    }
    pub fn set_text(&mut self, text: impl AsRef<str>) {
        self.cancel_autocomplete();
        self.exit_history();
        self.last_action = None;
        let normalized = normalize(text.as_ref());
        if self.text() != normalized {
            self.undo.push(&self.state);
        }
        self.set_text_internal(&normalized, false);
    }
    pub fn insert_text_at_cursor(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.undo.push(&self.state);
        self.exit_history();
        self.insert_internal(&normalize(text));
    }
    pub fn add_to_history(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() || self.history.first().is_some_and(|v| v == text) {
            return;
        }
        self.history.insert(0, text.into());
        self.history.truncate(100);
    }
    pub fn set_autocomplete_provider(&mut self, provider: Box<dyn AutocompleteProvider>) {
        self.cancel_autocomplete();
        self.autocomplete_provider = Some(provider);
    }
    pub fn set_padding_x(&mut self, padding: usize) {
        self.padding_x = padding;
    }
    pub fn get_padding_x(&self) -> usize {
        self.padding_x
    }
    pub fn set_autocomplete_max_visible(&mut self, max: usize) {
        self.autocomplete_max_visible = max.clamp(3, 20);
    }
    pub fn get_autocomplete_max_visible(&self) -> usize {
        self.autocomplete_max_visible
    }
    pub fn is_showing_autocomplete(&self) -> bool {
        self.autocomplete.is_some()
    }

    fn changed(&mut self) {
        let text = self.text();
        if let Some(callback) = &mut self.on_change {
            callback(&text);
        }
    }
    fn set_text_internal(&mut self, text: &str, at_start: bool) {
        self.state.lines = text.split('\n').map(str::to_owned).collect();
        if self.state.lines.is_empty() {
            self.state.lines.push(String::new());
        }
        self.state.cursor_line = if at_start {
            0
        } else {
            self.state.lines.len() - 1
        };
        self.state.cursor_col = if at_start {
            0
        } else {
            self.state.lines[self.state.cursor_line].len()
        };
        self.preferred_col = None;
        self.changed();
    }
    fn current(&self) -> &str {
        &self.state.lines[self.state.cursor_line]
    }
    fn push_undo(&mut self) {
        self.undo.push(&self.state);
    }
    fn exit_history(&mut self) {
        self.history_index = None;
        self.history_draft = None;
    }
    fn cancel_autocomplete(&mut self) {
        self.autocomplete = None;
        self.autocomplete_selected = 0;
    }
    fn update_autocomplete(&mut self, force: bool) {
        self.autocomplete = self.autocomplete_provider.as_ref().and_then(|provider| {
            provider.get_suggestions(
                &self.state.lines,
                self.state.cursor_line,
                self.state.cursor_col,
                force,
            )
        });
        self.autocomplete_selected = self
            .autocomplete
            .as_ref()
            .and_then(|suggestions| {
                suggestions
                    .items
                    .iter()
                    .position(|item| item.value == suggestions.prefix)
                    .or_else(|| {
                        suggestions
                            .items
                            .iter()
                            .position(|item| item.value.starts_with(&suggestions.prefix))
                    })
            })
            .unwrap_or(0);
    }
    fn navigate_history(&mut self, up: bool) {
        if self.history.is_empty() {
            return;
        }
        let next = match (self.history_index, up) {
            (None, true) => Some(0),
            (Some(i), true) if i + 1 < self.history.len() => Some(i + 1),
            (Some(0), false) => None,
            (Some(i), false) => Some(i - 1),
            _ => return,
        };
        if self.history_index.is_none() && next.is_some() {
            self.push_undo();
            self.history_draft = Some(self.state.clone());
        }
        self.history_index = next;
        if let Some(i) = next {
            let value = self.history[i].clone();
            self.set_text_internal(&value, up);
        } else if let Some(draft) = self.history_draft.take() {
            self.state = draft;
            self.changed();
        }
    }
    fn insert_internal(&mut self, text: &str) {
        let inserted: Vec<&str> = text.split('\n').collect();
        let line = self.current().to_owned();
        let before = &line[..self.state.cursor_col];
        let after = &line[self.state.cursor_col..];
        if inserted.len() == 1 {
            self.state.lines[self.state.cursor_line] = format!("{before}{text}{after}");
            self.state.cursor_col += text.len();
        } else {
            let mut replacement = Vec::with_capacity(inserted.len());
            replacement.push(format!("{before}{}", inserted[0]));
            replacement.extend(
                inserted[1..inserted.len() - 1]
                    .iter()
                    .map(|s| (*s).to_owned()),
            );
            replacement.push(format!("{}{after}", inserted.last().unwrap()));
            let count = replacement.len();
            self.state
                .lines
                .splice(self.state.cursor_line..=self.state.cursor_line, replacement);
            self.state.cursor_line += count - 1;
            self.state.cursor_col = inserted.last().unwrap().len();
        }
        self.changed();
    }
    fn insert(&mut self, text: &str) {
        self.preferred_col = None;
        self.exit_history();
        if text.chars().any(char::is_whitespace) || self.last_action != Some(LastAction::TypeWord) {
            self.push_undo();
        }
        self.last_action = Some(LastAction::TypeWord);
        self.insert_internal(text);
        self.update_autocomplete(false);
    }
    fn previous_grapheme(text: &str, cursor: usize) -> usize {
        let points: Vec<_> = GraphemeClusterSegmenter::new()
            .segment_str(&text[..cursor])
            .collect();
        points
            .get(points.len().saturating_sub(2))
            .copied()
            .unwrap_or(0)
    }
    fn next_grapheme(text: &str, cursor: usize) -> usize {
        GraphemeClusterSegmenter::new()
            .segment_str(&text[cursor..])
            .nth(1)
            .map_or(text.len(), |n| cursor + n)
    }
    fn marker_span_at(&self, line: &str, cursor: usize, backward: bool) -> Option<(usize, usize)> {
        for id in self.pastes.keys() {
            let needle = format!("[paste #{id} ");
            for (start, _) in line.match_indices(&needle) {
                let end = start + line[start..].find(']')? + 1;
                if (backward && cursor == end)
                    || (!backward && cursor == start)
                    || (cursor > start && cursor < end)
                {
                    return Some((start, end));
                }
            }
        }
        None
    }
    fn previous_position(&self) -> usize {
        self.marker_span_at(self.current(), self.state.cursor_col, true)
            .map_or_else(
                || Self::previous_grapheme(self.current(), self.state.cursor_col),
                |(start, _)| start,
            )
    }
    fn next_position(&self) -> usize {
        self.marker_span_at(self.current(), self.state.cursor_col, false)
            .map_or_else(
                || Self::next_grapheme(self.current(), self.state.cursor_col),
                |(_, end)| end,
            )
    }
    fn jump(&mut self, needle: &str, mode: JumpMode) {
        if mode == JumpMode::Forward {
            for line in self.state.cursor_line..self.state.lines.len() {
                let from = if line == self.state.cursor_line {
                    self.state.cursor_col.saturating_add(1)
                } else {
                    0
                };
                if let Some(offset) = self.state.lines[line]
                    .get(from..)
                    .and_then(|s| s.find(needle))
                {
                    self.state.cursor_line = line;
                    self.state.cursor_col = from + offset;
                    return;
                }
            }
        } else {
            for line in (0..=self.state.cursor_line).rev() {
                let to = if line == self.state.cursor_line {
                    self.state.cursor_col
                } else {
                    self.state.lines[line].len()
                };
                if let Some(offset) = self.state.lines[line][..to].rfind(needle) {
                    self.state.cursor_line = line;
                    self.state.cursor_col = offset;
                    return;
                }
            }
        }
    }
    fn backspace(&mut self) {
        self.preferred_col = None;
        self.exit_history();
        self.last_action = None;
        if self.state.cursor_col > 0 {
            self.push_undo();
            let start = self.previous_position();
            let end = self.state.cursor_col;
            self.state.lines[self.state.cursor_line].replace_range(start..end, "");
            self.state.cursor_col = start;
        } else if self.state.cursor_line > 0 {
            self.push_undo();
            let current = self.state.lines.remove(self.state.cursor_line);
            self.state.cursor_line -= 1;
            self.state.cursor_col = self.current().len();
            self.state.lines[self.state.cursor_line].push_str(&current);
        }
        self.changed();
        self.update_autocomplete(false);
    }
    fn delete_forward(&mut self) {
        self.preferred_col = None;
        self.exit_history();
        self.last_action = None;
        if self.state.cursor_col < self.current().len() {
            self.push_undo();
            let end = self.next_position();
            self.state.lines[self.state.cursor_line].replace_range(self.state.cursor_col..end, "");
        } else if self.state.cursor_line + 1 < self.state.lines.len() {
            self.push_undo();
            let next = self.state.lines.remove(self.state.cursor_line + 1);
            self.state.lines[self.state.cursor_line].push_str(&next);
        }
        self.changed();
    }
    fn kill_range(&mut self, start: usize, end: usize, prepend: bool) {
        if start == end {
            return;
        }
        self.push_undo();
        let killed = self.current()[start..end].to_owned();
        self.kill_ring
            .push(&killed, prepend, self.last_action == Some(LastAction::Kill));
        self.state.lines[self.state.cursor_line].replace_range(start..end, "");
        self.state.cursor_col = start;
        self.last_action = Some(LastAction::Kill);
        self.changed();
    }
    fn delete_word_backward(&mut self) {
        if self.state.cursor_col == 0 {
            self.kill_newline(true);
        } else {
            let start = find_word_backward(self.current(), self.state.cursor_col);
            self.kill_range(start, self.state.cursor_col, true);
        }
    }
    fn delete_word_forward(&mut self) {
        if self.state.cursor_col == self.current().len() {
            self.kill_newline(false);
        } else {
            let end = find_word_forward(self.current(), self.state.cursor_col);
            self.kill_range(self.state.cursor_col, end, false);
        }
    }
    fn kill_newline(&mut self, backward: bool) {
        if backward && self.state.cursor_line > 0 {
            self.push_undo();
            self.kill_ring
                .push("\n", true, self.last_action == Some(LastAction::Kill));
            let current = self.state.lines.remove(self.state.cursor_line);
            self.state.cursor_line -= 1;
            self.state.cursor_col = self.current().len();
            self.state.lines[self.state.cursor_line].push_str(&current);
            self.last_action = Some(LastAction::Kill);
            self.changed();
        } else if !backward && self.state.cursor_line + 1 < self.state.lines.len() {
            self.push_undo();
            self.kill_ring
                .push("\n", false, self.last_action == Some(LastAction::Kill));
            let next = self.state.lines.remove(self.state.cursor_line + 1);
            self.state.lines[self.state.cursor_line].push_str(&next);
            self.last_action = Some(LastAction::Kill);
            self.changed();
        }
    }
    fn add_newline(&mut self) {
        self.cancel_autocomplete();
        self.exit_history();
        self.last_action = None;
        self.push_undo();
        let tail = self.state.lines[self.state.cursor_line].split_off(self.state.cursor_col);
        self.state.cursor_line += 1;
        self.state.cursor_col = 0;
        self.state.lines.insert(self.state.cursor_line, tail);
        self.changed();
    }
    fn move_horizontal(&mut self, right: bool) {
        self.last_action = None;
        self.preferred_col = None;
        if right {
            if self.state.cursor_col < self.current().len() {
                self.state.cursor_col = self.next_position();
            } else if self.state.cursor_line + 1 < self.state.lines.len() {
                self.state.cursor_line += 1;
                self.state.cursor_col = 0;
            }
        } else if self.state.cursor_col > 0 {
            self.state.cursor_col = self.previous_position();
        } else if self.state.cursor_line > 0 {
            self.state.cursor_line -= 1;
            self.state.cursor_col = self.current().len();
        }
        if self.autocomplete.is_some() {
            self.update_autocomplete(false);
        }
    }
    fn move_vertical(&mut self, down: bool) {
        let target = if down {
            self.state
                .cursor_line
                .checked_add(1)
                .filter(|i| *i < self.state.lines.len())
        } else {
            self.state.cursor_line.checked_sub(1)
        };
        if let Some(line) = target {
            let wanted = self.preferred_col.unwrap_or(self.state.cursor_col);
            self.state.cursor_line = line;
            let end = self.current().len();
            self.state.cursor_col = wanted.min(end);
            self.preferred_col = (wanted > end).then_some(wanted);
        }
    }
    fn paste(&mut self, text: &str) {
        self.cancel_autocomplete();
        self.exit_history();
        self.last_action = None;
        self.push_undo();
        let mut decoded = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(start) = rest.find("\x1b[") {
            decoded.push_str(&rest[..start]);
            let sequence = &rest[start + 2..];
            if let Some(end) = sequence.find(";5u")
                && let Ok(code) = sequence[..end].parse::<u8>()
                && code.is_ascii_alphabetic()
            {
                decoded.push((code & 0x1f) as char);
                rest = &sequence[end + 3..];
                continue;
            }
            decoded.push('\x1b');
            rest = &rest[start + 1..];
        }
        decoded.push_str(rest);
        let mut filtered: String = normalize(&decoded)
            .chars()
            .filter(|c| *c == '\n' || !c.is_control())
            .collect();
        if filtered.starts_with(['/', '~', '.'])
            && self.state.cursor_col > 0
            && self.current()[..self.state.cursor_col]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            filtered.insert(0, ' ');
        }
        let lines = filtered.split('\n').count();
        if lines > 10 || filtered.len() > 1000 {
            self.paste_counter += 1;
            self.pastes.insert(self.paste_counter, filtered.clone());
            let marker = if lines > 10 {
                format!("[paste #{} +{} lines]", self.paste_counter, lines)
            } else {
                format!("[paste #{} {} chars]", self.paste_counter, filtered.len())
            };
            self.insert_internal(&marker);
        } else {
            self.insert_internal(&filtered);
        }
    }
    fn submit(&mut self) {
        self.cancel_autocomplete();
        let value = self.get_expanded_text().trim().to_owned();
        self.state = EditorState {
            lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
        };
        self.pastes.clear();
        self.paste_counter = 0;
        self.exit_history();
        self.undo.clear();
        self.last_action = None;
        self.changed();
        if let Some(callback) = &mut self.on_submit {
            callback(&value);
        }
    }
    fn apply_selected_completion(&mut self) -> bool {
        let Some(suggestions) = &self.autocomplete else {
            return false;
        };
        let Some(item) = suggestions.items.get(self.autocomplete_selected) else {
            return false;
        };
        let result = self
            .autocomplete_provider
            .as_ref()
            .unwrap()
            .apply_completion(
                &self.state.lines,
                self.state.cursor_line,
                self.state.cursor_col,
                item,
                &suggestions.prefix,
            );
        self.push_undo();
        self.state.lines = result.lines;
        self.state.cursor_line = result.cursor_line;
        self.state.cursor_col = result.cursor_col;
        self.cancel_autocomplete();
        self.changed();
        true
    }

    fn handle(&mut self, mut data: &str) {
        if let Some(start) = data.find("\x1b[200~") {
            self.in_paste = true;
            self.paste_buffer.clear();
            data = &data[start + 6..];
        }
        if self.in_paste {
            self.paste_buffer.push_str(data);
            if let Some(end) = self.paste_buffer.find("\x1b[201~") {
                let paste = self.paste_buffer[..end].to_owned();
                let rest = self.paste_buffer[end + 6..].to_owned();
                self.paste_buffer.clear();
                self.in_paste = false;
                self.paste(&paste);
                if !rest.is_empty() {
                    self.handle(&rest);
                }
            }
            return;
        }
        let matches = |name| get_keybindings().lock().unwrap().matches(data, name);
        if let Some(mode) = self.jump_mode {
            if matches("tui.editor.jumpForward") || matches("tui.editor.jumpBackward") {
                self.jump_mode = None;
                return;
            }
            self.jump_mode = None;
            if let Some(printable) = decode_printable_key(data)
                .or_else(|| (!data.chars().any(char::is_control)).then(|| data.to_owned()))
            {
                self.jump(&printable, mode);
                self.last_action = None;
                self.preferred_col = None;
                return;
            }
        }
        if self.autocomplete.is_some() {
            if matches("tui.select.cancel") {
                self.cancel_autocomplete();
                return;
            }
            if matches("tui.select.up") {
                let n = self.autocomplete.as_ref().unwrap().items.len();
                if n > 0 {
                    self.autocomplete_selected =
                        self.autocomplete_selected.checked_sub(1).unwrap_or(n - 1);
                }
                return;
            }
            if matches("tui.select.down") {
                let n = self.autocomplete.as_ref().unwrap().items.len();
                if n > 0 {
                    self.autocomplete_selected = (self.autocomplete_selected + 1) % n;
                }
                return;
            }
            if matches("tui.input.tab") {
                self.apply_selected_completion();
                return;
            }
            if matches("tui.select.confirm") && self.apply_selected_completion() {
                if !data.ends_with('\r') {
                    return;
                }
            }
        }
        if matches("tui.input.copy") {
            return;
        }
        if matches("tui.editor.undo") {
            if let Some(state) = self.undo.pop() {
                self.state = state;
                self.last_action = None;
                self.changed();
            }
            return;
        }
        if matches("tui.input.tab") {
            self.update_autocomplete(true);
            if self
                .autocomplete
                .as_ref()
                .is_some_and(|s| s.items.len() == 1)
            {
                self.apply_selected_completion();
            }
            return;
        }
        if matches("tui.editor.deleteToLineEnd") {
            let end = self.current().len();
            if self.state.cursor_col == end {
                self.kill_newline(false)
            } else {
                self.kill_range(self.state.cursor_col, end, false)
            }
            return;
        }
        if matches("tui.editor.deleteToLineStart") {
            if self.state.cursor_col == 0 {
                self.kill_newline(true)
            } else {
                self.kill_range(0, self.state.cursor_col, true)
            }
            return;
        }
        if matches("tui.editor.deleteWordBackward") {
            self.delete_word_backward();
            return;
        }
        if matches("tui.editor.deleteWordForward") {
            self.delete_word_forward();
            return;
        }
        if matches("tui.editor.deleteCharBackward") || matches_key(data, "shift+backspace") {
            self.backspace();
            return;
        }
        if matches("tui.editor.deleteCharForward") || matches_key(data, "shift+delete") {
            self.delete_forward();
            return;
        }
        if matches("tui.editor.yank") {
            if let Some(text) = self.kill_ring.peek().map(str::to_owned) {
                self.push_undo();
                self.insert_internal(&text);
                self.last_action = Some(LastAction::Yank);
            }
            return;
        }
        if matches("tui.editor.yankPop") {
            if self.last_action == Some(LastAction::Yank) && self.kill_ring.len() > 1 {
                let old = self.kill_ring.peek().unwrap().to_owned();
                for _ in 0..old.chars().count() {
                    self.backspace();
                }
                self.kill_ring.rotate();
                let text = self.kill_ring.peek().unwrap().to_owned();
                self.insert_internal(&text);
                self.last_action = Some(LastAction::Yank);
            }
            return;
        }
        if matches("tui.editor.cursorLineStart") {
            self.state.cursor_col = 0;
            self.preferred_col = None;
            self.last_action = None;
            return;
        }
        if matches("tui.editor.cursorLineEnd") {
            self.state.cursor_col = self.current().len();
            self.preferred_col = None;
            self.last_action = None;
            return;
        }
        if matches("tui.editor.cursorWordLeft") {
            self.state.cursor_col = find_word_backward(self.current(), self.state.cursor_col);
            self.preferred_col = None;
            self.last_action = None;
            return;
        }
        if matches("tui.editor.cursorWordRight") {
            self.state.cursor_col = find_word_forward(self.current(), self.state.cursor_col);
            self.preferred_col = None;
            self.last_action = None;
            return;
        }
        if matches("tui.input.newLine") || data == "\n" || data == "\x1b\r" {
            self.add_newline();
            return;
        }
        if matches("tui.input.submit") {
            if self.disable_submit {
                return;
            }
            if self.state.cursor_col > 0
                && self.current().as_bytes()[self.state.cursor_col - 1] == b'\\'
            {
                self.backspace();
                self.add_newline();
            } else {
                self.submit();
            }
            return;
        }
        if matches("tui.editor.cursorUp") {
            if self.state.cursor_line == 0 {
                if self.state.cursor_col == 0
                    || self.current().is_empty()
                    || self.history_index.is_some()
                {
                    self.navigate_history(true)
                } else {
                    self.state.cursor_col = 0
                }
            } else {
                self.move_vertical(false)
            }
            return;
        }
        if matches("tui.editor.cursorDown") {
            if self.state.cursor_line + 1 == self.state.lines.len() && self.history_index.is_some()
            {
                self.navigate_history(false)
            } else if self.state.cursor_line + 1 == self.state.lines.len() {
                self.state.cursor_col = self.current().len()
            } else {
                self.move_vertical(true)
            }
            return;
        }
        if matches("tui.editor.cursorRight") {
            self.move_horizontal(true);
            return;
        }
        if matches("tui.editor.cursorLeft") {
            self.move_horizontal(false);
            return;
        }
        if matches("tui.editor.pageUp") {
            for _ in 0..5 {
                self.move_vertical(false);
            }
            return;
        }
        if matches("tui.editor.pageDown") {
            for _ in 0..5 {
                self.move_vertical(true);
            }
            return;
        }
        if matches("tui.editor.jumpForward") {
            self.jump_mode = Some(JumpMode::Forward);
            return;
        }
        if matches("tui.editor.jumpBackward") {
            self.jump_mode = Some(JumpMode::Backward);
            return;
        }
        if matches_key(data, "shift+space") {
            self.insert(" ");
            return;
        }
        if let Some(printable) = decode_printable_key(data) {
            self.insert(&printable);
            return;
        }
        if !data.chars().any(char::is_control) {
            self.insert(data);
        }
    }
}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\t', "    ")
}

impl Component for Editor {
    fn render(&self, width: usize) -> Vec<String> {
        let padding = self.padding_x.min(width.saturating_sub(1) / 2);
        let content = width.saturating_sub(padding * 2).max(1);
        let layout = content.saturating_sub(usize::from(padding == 0)).max(1);
        let mut out = vec!["─".repeat(width)];
        let side = " ".repeat(padding);
        for (line_index, line) in self.state.lines.iter().enumerate() {
            for chunk in word_wrap_line(line, layout) {
                let mut display = chunk.text.clone();
                let mut line_width = visible_width(&display);
                if line_index == self.state.cursor_line
                    && self.state.cursor_col >= chunk.start_index
                    && self.state.cursor_col <= chunk.end_index
                {
                    let at = self
                        .state
                        .cursor_col
                        .saturating_sub(chunk.start_index)
                        .min(display.len());
                    let marker = if self.focused { CURSOR_MARKER } else { "" };
                    let end = Self::next_grapheme(&display, at);
                    let cursor: String = if at == display.len() {
                        line_width += 1;
                        " ".into()
                    } else {
                        display[at..end].into()
                    };
                    display = format!(
                        "{}{marker}\x1b[7m{cursor}\x1b[0m{}",
                        &display[..at],
                        &display[end..]
                    );
                }
                out.push(format!(
                    "{side}{display}{}{side}",
                    " ".repeat(content.saturating_sub(line_width))
                ));
            }
        }
        out.push("─".repeat(width));
        if let Some(suggestions) = &self.autocomplete {
            let start = self
                .autocomplete_selected
                .saturating_sub(self.autocomplete_max_visible / 2)
                .min(
                    suggestions
                        .items
                        .len()
                        .saturating_sub(self.autocomplete_max_visible),
                );
            for (i, item) in suggestions.items
                [start..(start + self.autocomplete_max_visible).min(suggestions.items.len())]
                .iter()
                .enumerate()
            {
                let prefix = if start + i == self.autocomplete_selected {
                    "→ "
                } else {
                    "  "
                };
                let text = format!("{prefix}{}", item.label);
                out.push(format!(
                    "{side}{}{side}",
                    truncate_to_width(&text, content, "", false)
                ));
            }
        }
        out
    }
    fn handle_input(&mut self, data: &str) {
        self.handle(data)
    }
    fn set_focused(&mut self, v: bool) {
        self.focused = v
    }
    fn is_focused(&self) -> bool {
        self.focused
    }
}
impl Focusable for Editor {
    fn set_focused(&mut self, v: bool) {
        self.focused = v
    }
    fn is_focused(&self) -> bool {
        self.focused
    }
}
impl crate::EditorComponent for Editor {
    fn get_text(&self) -> String {
        Editor::get_text(self)
    }
    fn set_text(&mut self, text: &str) {
        Editor::set_text(self, text)
    }
    fn handle_input(&mut self, data: &str) {
        self.handle(data)
    }
    fn add_to_history(&mut self, text: &str) {
        Editor::add_to_history(self, text)
    }
    fn insert_text_at_cursor(&mut self, text: &str) {
        Editor::insert_text_at_cursor(self, text)
    }
    fn get_expanded_text(&self) -> String {
        Editor::get_expanded_text(self)
    }
    fn set_autocomplete_provider(&mut self, provider: Box<dyn AutocompleteProvider>) {
        Editor::set_autocomplete_provider(self, provider)
    }
    fn set_padding_x(&mut self, padding: usize) {
        Editor::set_padding_x(self, padding)
    }
    fn set_autocomplete_max_visible(&mut self, max: usize) {
        Editor::set_autocomplete_max_visible(self, max)
    }
}

impl From<&crate::AutocompleteItem> for SelectItem {
    fn from(item: &crate::AutocompleteItem) -> Self {
        Self {
            value: item.value.clone(),
            label: item.label.clone(),
            description: item.description.clone(),
        }
    }
}
