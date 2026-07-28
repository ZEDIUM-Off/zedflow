use crate::keys::is_key_release;
use crate::terminal::{Terminal, TerminalEvent};
use crate::terminal_image::{delete_kitty_image, is_image_line};
use crate::utils::{normalize_terminal_output, slice_by_column, slice_with_width, visible_width};
use std::{collections::BTreeSet, io, time::Duration};

/// Zero-width marker emitted by focused components at the logical cursor.
pub const CURSOR_MARKER: &str = "\x1b_pi:c\x07";
const SEGMENT_RESET: &str = "\x1b[0m\x1b]8;;\x07";

pub trait Component {
    fn render(&self, width: usize) -> Vec<String>;
    fn invalidate(&mut self) {}
    fn handle_input(&mut self, _data: &str) {}
    fn wants_key_release(&self) -> bool {
        false
    }
    fn set_focused(&mut self, _focused: bool) {}
    fn is_focused(&self) -> bool {
        false
    }
}

pub trait Focusable {
    fn set_focused(&mut self, focused: bool);
    fn is_focused(&self) -> bool;
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverlayAnchor {
    #[default]
    Center,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    TopCenter,
    BottomCenter,
    LeftCenter,
    RightCenter,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SizeValue {
    Cells(usize),
    Percent(f32),
}

impl From<usize> for SizeValue {
    fn from(value: usize) -> Self {
        Self::Cells(value)
    }
}

impl SizeValue {
    fn resolve(self, reference: usize) -> usize {
        match self {
            Self::Cells(value) => value,
            Self::Percent(value) => ((reference as f32 * value) / 100.0).floor().max(0.0) as usize,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OverlayMargin {
    pub top: usize,
    pub right: usize,
    pub bottom: usize,
    pub left: usize,
}

pub struct OverlayOptions {
    pub width: Option<SizeValue>,
    pub min_width: Option<usize>,
    pub max_height: Option<SizeValue>,
    pub anchor: OverlayAnchor,
    pub offset_x: isize,
    pub offset_y: isize,
    pub row: Option<SizeValue>,
    pub col: Option<SizeValue>,
    pub margin: OverlayMargin,
    pub visible: Option<Box<dyn Fn(usize, usize) -> bool>>,
    pub non_capturing: bool,
}

impl Default for OverlayOptions {
    fn default() -> Self {
        Self {
            width: None,
            min_width: None,
            max_height: None,
            anchor: OverlayAnchor::Center,
            offset_x: 0,
            offset_y: 0,
            row: None,
            col: None,
            margin: OverlayMargin::default(),
            visible: None,
            non_capturing: false,
        }
    }
}

struct Overlay {
    id: usize,
    component: Box<dyn Component>,
    options: OverlayOptions,
    hidden: bool,
    focus_order: usize,
}

/// Pi-compatible, line-oriented TUI runtime. It leaves scrollback intact and
/// redraws only changed lines unless a terminal resize requires a full redraw.
pub struct Tui {
    pub root: Container,
    terminal: Option<Box<dyn Terminal>>,
    overlays: Vec<Overlay>,
    focused: Option<usize>,
    next_overlay_id: usize,
    focus_order: usize,
    previous_lines: Vec<String>,
    previous_kitty_image_ids: BTreeSet<u32>,
    previous_width: usize,
    previous_height: usize,
    hardware_cursor_row: usize,
    full_redraws: usize,
    started: bool,
    show_hardware_cursor: bool,
    clear_on_shrink: bool,
}

impl Tui {
    pub fn new() -> Self {
        Self::from_terminal(None)
    }

    pub fn with_terminal(terminal: impl Terminal + 'static) -> Self {
        Self::from_terminal(Some(Box::new(terminal)))
    }

    fn from_terminal(terminal: Option<Box<dyn Terminal>>) -> Self {
        Self {
            root: Container::new(),
            terminal,
            overlays: Vec::new(),
            focused: None,
            next_overlay_id: 0,
            focus_order: 0,
            previous_lines: Vec::new(),
            previous_kitty_image_ids: BTreeSet::new(),
            previous_width: 0,
            previous_height: 0,
            hardware_cursor_row: 0,
            full_redraws: 0,
            started: false,
            show_hardware_cursor: std::env::var_os("PI_HARDWARE_CURSOR").is_some_and(|v| v == "1"),
            clear_on_shrink: std::env::var_os("PI_CLEAR_ON_SHRINK").is_some_and(|v| v == "1"),
        }
    }

    pub fn full_redraws(&self) -> usize {
        self.full_redraws
    }

    pub fn set_clear_on_shrink(&mut self, enabled: bool) {
        self.clear_on_shrink = enabled;
    }

    pub fn show_overlay(&mut self, overlay: impl Component + 'static) -> usize {
        self.show_overlay_with_options(overlay, OverlayOptions::default())
    }

    pub fn show_overlay_with_options(
        &mut self,
        mut component: impl Component + 'static,
        options: OverlayOptions,
    ) -> usize {
        let id = self.next_overlay_id;
        self.next_overlay_id += 1;
        self.focus_order += 1;
        if !options.non_capturing {
            self.unfocus_current();
            component.set_focused(true);
            self.focused = Some(id);
        }
        self.overlays.push(Overlay {
            id,
            component: Box::new(component),
            options,
            hidden: false,
            focus_order: self.focus_order,
        });
        id
    }

    /// Remove an overlay by its current stack index (the original Rust API).
    pub fn hide_overlay(&mut self, index: usize) -> Option<Box<dyn Component>> {
        let id = self.overlays.get(index)?.id;
        self.hide_overlay_by_id(id)
    }

    /// Remove an overlay by the stable id returned from `show_overlay`.
    pub fn hide_overlay_by_id(&mut self, id: usize) -> Option<Box<dyn Component>> {
        let index = self.overlays.iter().position(|overlay| overlay.id == id)?;
        let was_focused = self.focused == Some(id);
        let mut removed = self.overlays.remove(index);
        removed.component.set_focused(false);
        if was_focused {
            self.focused = self.top_capturing_overlay_id();
            self.focus_current();
        }
        Some(removed.component)
    }

    pub fn set_overlay_hidden(&mut self, id: usize, hidden: bool) -> bool {
        let Some(index) = self.overlays.iter().position(|overlay| overlay.id == id) else {
            return false;
        };
        self.overlays[index].hidden = hidden;
        if hidden && self.focused == Some(id) {
            self.overlays[index].component.set_focused(false);
            self.focused = self.top_capturing_overlay_id();
            self.focus_current();
        }
        true
    }

    pub fn focus_overlay(&mut self, id: usize) -> bool {
        let Some(index) = self
            .overlays
            .iter()
            .position(|overlay| overlay.id == id && !overlay.hidden)
        else {
            return false;
        };
        self.unfocus_current();
        self.focus_order += 1;
        self.overlays[index].focus_order = self.focus_order;
        self.overlays[index].component.set_focused(true);
        self.focused = Some(id);
        true
    }

    pub fn unfocus_overlay(&mut self, id: usize) -> bool {
        if self.focused != Some(id) {
            return false;
        }
        self.unfocus_current();
        self.focused = self.top_capturing_overlay_id();
        self.focus_current();
        true
    }

    pub fn is_overlay_focused(&self, id: usize) -> bool {
        self.focused == Some(id)
    }

    fn overlay_visible(overlay: &Overlay, width: usize, height: usize) -> bool {
        !overlay.hidden
            && overlay
                .options
                .visible
                .as_ref()
                .is_none_or(|visible| visible(width, height))
    }

    fn top_capturing_overlay_id(&self) -> Option<usize> {
        self.overlays
            .iter()
            .filter(|overlay| !overlay.hidden && !overlay.options.non_capturing)
            .max_by_key(|overlay| overlay.focus_order)
            .map(|overlay| overlay.id)
    }

    fn unfocus_current(&mut self) {
        if let Some(id) = self.focused
            && let Some(overlay) = self.overlays.iter_mut().find(|overlay| overlay.id == id)
        {
            overlay.component.set_focused(false);
        }
    }

    fn focus_current(&mut self) {
        if let Some(id) = self.focused
            && let Some(overlay) = self.overlays.iter_mut().find(|overlay| overlay.id == id)
        {
            overlay.component.set_focused(true);
        }
    }

    pub fn overlay_count(&self) -> usize {
        self.overlays.len()
    }

    /// Compatibility rendering used by component composition. Runtime terminal
    /// rendering uses [`Self::render_frame`] so overlays are screen-relative.
    pub fn render(&self, width: usize) -> Vec<String> {
        let mut lines = self.root.render(width);
        if let Some(overlay) = self
            .overlays
            .iter()
            .filter(|overlay| !overlay.hidden)
            .max_by_key(|overlay| overlay.focus_order)
        {
            lines.extend(overlay.component.render(width));
        }
        lines
    }

    pub fn render_frame(&self, width: usize, height: usize) -> Vec<String> {
        let mut lines = self.root.render(width);
        let mut overlays: Vec<&Overlay> = self
            .overlays
            .iter()
            .filter(|overlay| Self::overlay_visible(overlay, width, height))
            .collect();
        overlays.sort_by_key(|overlay| overlay.focus_order);
        if overlays.is_empty() {
            return lines;
        }
        lines.resize(lines.len().max(height), String::new());
        let viewport_start = lines.len().saturating_sub(height);
        for overlay in overlays {
            let (overlay_width, max_height) = Self::overlay_size(&overlay.options, width, height);
            let mut rendered = overlay.component.render(overlay_width);
            rendered.truncate(max_height.unwrap_or(usize::MAX));
            let (row, col) = Self::overlay_position(
                &overlay.options,
                overlay_width,
                rendered.len(),
                width,
                height,
            );
            let needed = viewport_start + row + rendered.len();
            lines.resize(lines.len().max(needed), String::new());
            for (offset, overlay_line) in rendered.iter().enumerate() {
                let line = viewport_start + row + offset;
                lines[line] =
                    composite_line_at(&lines[line], overlay_line, col, overlay_width, width);
            }
        }
        lines
    }

    fn overlay_size(
        options: &OverlayOptions,
        width: usize,
        height: usize,
    ) -> (usize, Option<usize>) {
        let available_width = width
            .saturating_sub(options.margin.left + options.margin.right)
            .max(1);
        let available_height = height
            .saturating_sub(options.margin.top + options.margin.bottom)
            .max(1);
        let mut overlay_width = options
            .width
            .map_or(80.min(available_width), |value| value.resolve(width));
        overlay_width = overlay_width
            .max(options.min_width.unwrap_or(1))
            .min(available_width);
        let max_height = options
            .max_height
            .map(|value| value.resolve(height).max(1).min(available_height));
        (overlay_width, max_height)
    }

    fn overlay_position(
        options: &OverlayOptions,
        overlay_width: usize,
        overlay_height: usize,
        width: usize,
        height: usize,
    ) -> (usize, usize) {
        let m = options.margin;
        let available_width = width.saturating_sub(m.left + m.right);
        let available_height = height.saturating_sub(m.top + m.bottom);
        let max_row = available_height.saturating_sub(overlay_height);
        let max_col = available_width.saturating_sub(overlay_width);
        let anchor_row = match options.anchor {
            OverlayAnchor::TopLeft | OverlayAnchor::TopCenter | OverlayAnchor::TopRight => 0,
            OverlayAnchor::BottomLeft
            | OverlayAnchor::BottomCenter
            | OverlayAnchor::BottomRight => max_row,
            _ => max_row / 2,
        };
        let anchor_col = match options.anchor {
            OverlayAnchor::TopLeft | OverlayAnchor::LeftCenter | OverlayAnchor::BottomLeft => 0,
            OverlayAnchor::TopRight | OverlayAnchor::RightCenter | OverlayAnchor::BottomRight => {
                max_col
            }
            _ => max_col / 2,
        };
        let row = options.row.map_or(anchor_row, |value| match value {
            SizeValue::Cells(value) => value.saturating_sub(m.top),
            SizeValue::Percent(value) => ((max_row as f32 * value) / 100.0).floor() as usize,
        });
        let col = options.col.map_or(anchor_col, |value| match value {
            SizeValue::Cells(value) => value.saturating_sub(m.left),
            SizeValue::Percent(value) => ((max_col as f32 * value) / 100.0).floor() as usize,
        });
        (
            (m.top as isize + row as isize + options.offset_y)
                .clamp(m.top as isize, (m.top + max_row) as isize) as usize,
            (m.left as isize + col as isize + options.offset_x)
                .clamp(m.left as isize, (m.left + max_col) as isize) as usize,
        )
    }

    pub fn dispatch_input(&mut self, data: &str) {
        if let Some(id) = self.focused
            && let Some(overlay) = self.overlays.iter_mut().find(|overlay| overlay.id == id)
        {
            if !is_key_release(data) || overlay.component.wants_key_release() {
                overlay.component.handle_input(data);
            }
            return;
        }
        self.root.handle_input(data);
    }

    pub fn start(&mut self) -> io::Result<()> {
        if self.started {
            return Ok(());
        }
        if let Some(terminal) = &mut self.terminal {
            terminal.start()?;
            if let Err(error) = terminal.hide_cursor() {
                let _ = terminal.stop();
                return Err(error);
            }
        }
        self.started = true;
        if let Err(error) = self.request_render(false) {
            self.started = false;
            if let Some(terminal) = &mut self.terminal {
                let _ = terminal.show_cursor();
                let _ = terminal.stop();
            }
            return Err(error);
        }
        Ok(())
    }

    /// Wait for one terminal event, drain the rest, then dispatch and render on
    /// the thread that owns this `Tui`. Returns immediately while stopped.
    pub fn pump_events(&mut self, timeout: Duration) -> io::Result<usize> {
        if !self.started {
            return Ok(0);
        }
        let Some(terminal) = &mut self.terminal else {
            return Ok(0);
        };
        let mut events = Vec::new();
        if let Some(event) = terminal.poll_event(timeout)? {
            events.push(event);
        }
        while let Some(event) = terminal.poll_event(Duration::ZERO)? {
            events.push(event);
        }
        let count = events.len();
        for event in events {
            if let TerminalEvent::Input(data) = event {
                self.dispatch_input(&data);
            }
        }
        if count != 0 {
            self.request_render(false)?;
        }
        Ok(count)
    }

    pub fn stop(&mut self) -> io::Result<()> {
        if !self.started {
            return Ok(());
        }
        self.started = false;
        let Some(terminal) = &mut self.terminal else {
            return Ok(());
        };
        let show_cursor = terminal.show_cursor();
        let stop = terminal.stop();
        show_cursor.and(stop)
    }

    /// Render immediately. Pi coalesces this operation on a 16ms timer; Rust
    /// callers retain deterministic control and can perform their own scheduling.
    pub fn request_render(&mut self, force: bool) -> io::Result<()> {
        if !self.started {
            return Ok(());
        }
        let Some(terminal) = self.terminal.as_ref() else {
            return Ok(());
        };
        let width = terminal.columns() as usize;
        let height = terminal.rows() as usize;
        let mut lines = self.render_frame(width, height);
        let terminal = self.terminal.as_mut().expect("terminal checked above");
        let cursor = extract_cursor_position(&mut lines, height);
        for line in &mut lines {
            if !is_image_line(line) {
                *line = normalize_terminal_output(line) + SEGMENT_RESET;
            }
        }
        let size_changed = self.previous_width != 0
            && (self.previous_width != width || self.previous_height != height);
        let shrinking = self.clear_on_shrink && lines.len() < self.previous_lines.len();
        if force || size_changed || shrinking {
            self.full_redraws += 1;
            let mut output = String::from("\x1b[?2026h");
            output.push_str(&delete_kitty_images(&self.previous_kitty_image_ids));
            output.push_str("\x1b[2J\x1b[H\x1b[3J");
            output.push_str(&render_full_lines(&lines, height));
            output.push_str("\x1b[?2026l");
            terminal.write(&output)?;
            self.hardware_cursor_row = lines.len().saturating_sub(1);
        } else if self.previous_lines.is_empty() {
            terminal.write(&format!(
                "\x1b[?2026h{}\x1b[?2026l",
                render_full_lines(&lines, height)
            ))?;
            self.hardware_cursor_row = lines.len().saturating_sub(1);
        } else {
            let max = lines.len().max(self.previous_lines.len());
            let first = (0..max).find(|&index| lines.get(index) != self.previous_lines.get(index));
            if let Some(first) = first {
                let last = (first..max)
                    .rev()
                    .find(|&index| lines.get(index) != self.previous_lines.get(index))
                    .unwrap_or(first);
                let (first, last) = expand_changed_range_for_kitty_images(
                    first,
                    last,
                    &self.previous_lines,
                    &lines,
                );
                let append_start = first == self.previous_lines.len() && first > 0;
                let move_target = if append_start { first - 1 } else { first };
                let delta = move_target as isize - self.hardware_cursor_row as isize;
                let mut output = String::from("\x1b[?2026h");
                output.push_str(&delete_changed_kitty_images(
                    &self.previous_lines,
                    first,
                    last,
                ));
                if delta > 0 {
                    output.push_str(&format!("\x1b[{delta}B"));
                } else if delta < 0 {
                    output.push_str(&format!("\x1b[{}A", -delta));
                }
                output.push_str(if append_start { "\r\n" } else { "\r" });
                let render_end = last.min(lines.len().saturating_sub(1));
                let mut index = first;
                while index <= last {
                    if index > first {
                        output.push_str("\r\n");
                    }
                    output.push_str("\x1b[2K");
                    let Some(line) = lines.get(index) else {
                        index += 1;
                        continue;
                    };
                    let reserved = if is_image_line(line) {
                        kitty_image_reserved_rows(&lines, index, render_end)
                    } else {
                        1
                    };
                    if reserved > 1 {
                        for _ in 1..reserved {
                            output.push_str("\r\n\x1b[2K");
                        }
                        output.push_str(&format!("\x1b[{}A", reserved - 1));
                        output.push_str(line);
                        output.push_str(&format!("\x1b[{}B", reserved - 1));
                        index += reserved;
                    } else {
                        output.push_str(line);
                        index += 1;
                    }
                }
                output.push_str("\x1b[?2026l");
                terminal.write(&output)?;
                self.hardware_cursor_row = last.min(lines.len().saturating_sub(1));
            }
        }
        position_hardware_cursor(
            terminal.as_mut(),
            cursor,
            &mut self.hardware_cursor_row,
            self.show_hardware_cursor,
        )?;
        self.previous_kitty_image_ids = collect_kitty_image_ids(&lines);
        self.previous_lines = lines;
        self.previous_width = width;
        self.previous_height = height;
        Ok(())
    }
}

impl Default for Tui {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_kitty_image_header(line: &str) -> (Vec<u32>, usize) {
    let Some((_, sequence)) = line.split_once("\x1b_G") else {
        return (Vec::new(), 1);
    };
    let Some((parameters, _)) = sequence.split_once(';') else {
        return (Vec::new(), 1);
    };
    let mut ids = Vec::new();
    let mut rows = 1;
    for parameter in parameters.split(',') {
        let Some((key, value)) = parameter.split_once('=') else {
            continue;
        };
        let Ok(value) = value.parse::<u32>() else {
            continue;
        };
        if value == 0 {
            continue;
        }
        match key {
            "i" => ids.push(value),
            "r" => rows = value as usize,
            _ => {}
        }
    }
    (ids, rows)
}

fn collect_kitty_image_ids(lines: &[String]) -> BTreeSet<u32> {
    lines
        .iter()
        .flat_map(|line| parse_kitty_image_header(line).0)
        .collect()
}

fn delete_kitty_images(ids: &BTreeSet<u32>) -> String {
    ids.iter().map(|id| delete_kitty_image(*id)).collect()
}

fn delete_changed_kitty_images(lines: &[String], first: usize, last: usize) -> String {
    let ids = lines
        .get(first..=last.min(lines.len().saturating_sub(1)))
        .unwrap_or_default()
        .iter()
        .flat_map(|line| parse_kitty_image_header(line).0)
        .collect();
    delete_kitty_images(&ids)
}

fn kitty_image_reserved_rows(lines: &[String], index: usize, max_index: usize) -> usize {
    let rows = parse_kitty_image_header(lines.get(index).map_or("", String::as_str)).1;
    if rows <= 1 {
        return 1;
    }
    let max_rows = rows
        .min(max_index.saturating_sub(index) + 1)
        .min(lines.len().saturating_sub(index));
    let mut reserved = 1;
    while reserved < max_rows {
        let line = &lines[index + reserved];
        if is_image_line(line) || visible_width(line) > 0 {
            break;
        }
        reserved += 1;
    }
    reserved
}

fn expand_changed_range_for_kitty_images(
    first: usize,
    last: usize,
    previous_lines: &[String],
    new_lines: &[String],
) -> (usize, usize) {
    let mut expanded = (first, last);
    for lines in [previous_lines, new_lines] {
        for (index, line) in lines.iter().enumerate() {
            if parse_kitty_image_header(line).0.is_empty() {
                continue;
            }
            let block_end = index + kitty_image_reserved_rows(lines, index, lines.len() - 1) - 1;
            if index >= first || (index <= last && block_end >= first) {
                expanded.0 = expanded.0.min(index);
                expanded.1 = expanded.1.max(block_end);
            }
        }
    }
    expanded
}

fn render_full_lines(lines: &[String], height: usize) -> String {
    let mut output = String::new();
    let mut index = 0;
    while index < lines.len() {
        if index > 0 {
            output.push_str("\r\n");
        }
        let line = &lines[index];
        let reserved = if is_image_line(line) {
            kitty_image_reserved_rows(lines, index, lines.len() - 1)
        } else {
            1
        };
        if reserved > 1 && reserved <= height {
            output.push_str(&"\r\n".repeat(reserved - 1));
            output.push_str(&format!("\x1b[{}A", reserved - 1));
            output.push_str(line);
            output.push_str(&format!("\x1b[{}B", reserved - 1));
            index += reserved;
        } else {
            output.push_str(line);
            index += 1;
        }
    }
    output
}

pub fn composite_line_at(
    base_line: &str,
    overlay_line: &str,
    start_col: usize,
    overlay_width: usize,
    total_width: usize,
) -> String {
    if is_image_line(base_line) {
        return base_line.to_owned();
    }
    let before = slice_with_width(base_line, 0, start_col, true);
    let after_start = start_col.saturating_add(overlay_width);
    let after = slice_with_width(
        base_line,
        after_start,
        total_width.saturating_sub(after_start),
        true,
    );
    let overlay = slice_with_width(overlay_line, 0, overlay_width, true);
    let mut result = before.0;
    result.push_str(&" ".repeat(start_col.saturating_sub(before.1)));
    result.push_str(SEGMENT_RESET);
    result.push_str(&overlay.0);
    result.push_str(&" ".repeat(overlay_width.saturating_sub(overlay.1)));
    result.push_str(SEGMENT_RESET);
    result.push_str(&after.0);
    let target_after = total_width.saturating_sub(start_col + overlay_width);
    result.push_str(&" ".repeat(target_after.saturating_sub(after.1)));
    if visible_width(&result) > total_width {
        slice_by_column(&result, 0, total_width, true)
    } else {
        result
    }
}

pub fn extract_cursor_position(lines: &mut [String], height: usize) -> Option<(usize, usize)> {
    let viewport_top = lines.len().saturating_sub(height);
    for row in (viewport_top..lines.len()).rev() {
        if let Some(index) = lines[row].find(CURSOR_MARKER) {
            let col = visible_width(&lines[row][..index]);
            lines[row].replace_range(index..index + CURSOR_MARKER.len(), "");
            return Some((row, col));
        }
    }
    None
}

fn position_hardware_cursor(
    terminal: &mut dyn Terminal,
    cursor: Option<(usize, usize)>,
    hardware_row: &mut usize,
    show: bool,
) -> io::Result<()> {
    let Some((row, col)) = cursor else {
        return terminal.hide_cursor();
    };
    let delta = row as isize - *hardware_row as isize;
    if delta != 0 {
        terminal.write(&if delta > 0 {
            format!("\x1b[{delta}B")
        } else {
            format!("\x1b[{}A", -delta)
        })?;
    }
    terminal.write(&format!("\x1b[{}G", col + 1))?;
    *hardware_row = row;
    if show {
        terminal.show_cursor()
    } else {
        terminal.hide_cursor()
    }
}
