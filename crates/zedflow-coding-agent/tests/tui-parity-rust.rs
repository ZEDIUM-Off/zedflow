use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::VecDeque,
    io::{self, Read},
    sync::{Arc, Mutex},
    time::Duration,
};
use zedflow_agent::{
    harness::types::AgentHarnessEvent,
    types::{AgentEvent, AgentMessage},
};
use zedflow_coding_agent::modes::interactive::InteractiveMode;
use zedflow_tui::{Terminal as TuiTerminal, TerminalEvent, utils::visible_width};

#[derive(Debug, Deserialize)]
struct Fixture {
    version: u8,
    dimensions: Dimensions,
    capabilities: Capabilities,
    events: Vec<Event>,
}

#[derive(Debug, Deserialize)]
struct Dimensions {
    columns: u16,
    rows: u16,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Capabilities {
    colors: u32,
    unicode: bool,
    kitty_keyboard: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Event {
    Input {
        data: String,
    },
    Resize {
        columns: u16,
        rows: u16,
    },
    Lifecycle {
        name: String,
        #[serde(default)]
        data: Value,
    },
}

#[derive(Debug, Serialize, PartialEq)]
struct OracleOutput {
    version: u8,
    frames: Vec<Frame>,
    inputs: Vec<String>,
    lifecycle: Vec<Lifecycle>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
struct Frame {
    cells: Vec<Vec<Cell>>,
    cursor: Cursor,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
struct Cell {
    text: String,
    width: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<Value>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
struct Cursor {
    x: u16,
    y: u16,
    visible: bool,
}

#[derive(Debug, Serialize, PartialEq)]
struct Lifecycle {
    name: String,
    data: Value,
}

struct Screen {
    columns: u16,
    rows: u16,
    cells: Vec<Vec<Cell>>,
    cursor: Cursor,
    style: Option<Value>,
}

impl Screen {
    fn new(columns: u16, rows: u16) -> Self {
        Self {
            columns,
            rows,
            cells: blank_screen(columns, rows),
            cursor: Cursor {
                x: 0,
                y: 0,
                visible: true,
            },
            style: None,
        }
    }

    fn write(&mut self, data: &str) {
        let mut chars = data.chars().peekable();
        while let Some(character) = chars.next() {
            match character {
                '\x1b' if chars.next_if_eq(&'[').is_some() => {
                    let mut sequence = String::new();
                    while let Some(next) = chars.next() {
                        sequence.push(next);
                        if next.is_ascii_alphabetic() {
                            break;
                        }
                    }
                    self.control(&sequence);
                }
                '\x1b' if chars.next_if_eq(&']').is_some() || chars.next_if_eq(&'_').is_some() => {
                    for next in chars.by_ref() {
                        if next == '\x07' {
                            break;
                        }
                    }
                }
                '\r' => self.cursor.x = 0,
                '\n' => self.newline(),
                c if !c.is_control() => self.put(c),
                _ => {}
            }
        }
    }

    fn control(&mut self, sequence: &str) {
        match sequence {
            "?25l" => self.cursor.visible = false,
            "?25h" => self.cursor.visible = true,
            "2J" => self.cells = blank_screen(self.columns, self.rows),
            "0m" | "m" | "27m" => self.style = None,
            "7m" => self.style = Some(serde_json::json!({ "inverse": true })),
            "H" => {
                self.cursor.x = 0;
                self.cursor.y = 0;
            }
            _ => {}
        }
    }

    fn newline(&mut self) {
        if self.cursor.y + 1 < self.rows {
            self.cursor.y += 1;
        } else {
            self.cells.remove(0);
            self.cells.push(blank_row(self.columns));
        }
    }

    fn put(&mut self, character: char) {
        let text = character.to_string();
        let width = visible_width(&text) as u8;
        if width == 0 {
            if self.cursor.x > 0 {
                self.cells[self.cursor.y as usize][self.cursor.x as usize - 1]
                    .text
                    .push(character);
            }
            return;
        }
        if self.cursor.x + u16::from(width) > self.columns {
            self.cursor.x = 0;
            self.newline();
        }
        let y = self.cursor.y as usize;
        let x = self.cursor.x as usize;
        self.cells[y][x] = Cell {
            text,
            width,
            style: self.style.clone(),
        };
        if width == 2 && self.cursor.x + 1 < self.columns {
            self.cells[y][x + 1] = Cell {
                text: String::new(),
                width: 0,
                style: self.style.clone(),
            };
        }
        self.cursor.x += u16::from(width);
    }

    fn resize(&mut self, columns: u16, rows: u16) {
        self.cells.resize_with(rows as usize, || blank_row(columns));
        for row in &mut self.cells {
            row.resize_with(columns as usize, blank_cell);
            row.truncate(columns as usize);
        }
        self.columns = columns;
        self.rows = rows;
        self.cursor.x = self.cursor.x.min(columns.saturating_sub(1));
        self.cursor.y = self.cursor.y.min(rows.saturating_sub(1));
    }

    fn frame(&self) -> Frame {
        let mut cells = self.cells.clone();
        for row in &mut cells {
            while row.last().is_some_and(|cell| {
                (cell.text.is_empty() || cell.text == " ")
                    && cell.width == 1
                    && cell.style.is_none()
            }) {
                row.pop();
            }
        }
        Frame {
            cells,
            cursor: self.cursor.clone(),
        }
    }
}

fn blank_cell() -> Cell {
    Cell {
        text: String::new(),
        width: 1,
        style: None,
    }
}
fn blank_row(columns: u16) -> Vec<Cell> {
    (0..columns).map(|_| blank_cell()).collect()
}
fn blank_screen(columns: u16, rows: u16) -> Vec<Vec<Cell>> {
    (0..rows).map(|_| blank_row(columns)).collect()
}

fn clean_metadata(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(clean_metadata).collect()),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .filter(|(key, _)| !matches!(key.as_str(), "timestamp" | "cwd" | "path" | "query"))
                .map(|(key, value)| (key, clean_metadata(value)))
                .collect(),
        ),
        value => value,
    }
}

#[derive(Default)]
struct MemoryTerminalState {
    columns: u16,
    rows: u16,
    events: VecDeque<TerminalEvent>,
    writes: Vec<String>,
}

struct MemoryTerminal(Arc<Mutex<MemoryTerminalState>>);

impl TuiTerminal for MemoryTerminal {
    fn start(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn poll_event(&mut self, _: Duration) -> io::Result<Option<TerminalEvent>> {
        Ok(self.0.lock().unwrap().events.pop_front())
    }
    fn stop(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn drain_input(&mut self, _: u64, _: u64) {}
    fn write(&mut self, data: &str) -> io::Result<()> {
        self.0.lock().unwrap().writes.push(data.into());
        Ok(())
    }
    fn columns(&self) -> u16 {
        self.0.lock().unwrap().columns
    }
    fn rows(&self) -> u16 {
        self.0.lock().unwrap().rows
    }
    fn kitty_protocol_active(&self) -> bool {
        true
    }
    fn move_by(&self, _: i32) -> io::Result<()> {
        Ok(())
    }
    fn hide_cursor(&self) -> io::Result<()> {
        Ok(())
    }
    fn show_cursor(&self) -> io::Result<()> {
        Ok(())
    }
    fn clear_line(&self) -> io::Result<()> {
        Ok(())
    }
    fn clear_from_cursor(&self) -> io::Result<()> {
        Ok(())
    }
    fn clear_screen(&self) -> io::Result<()> {
        Ok(())
    }
    fn set_title(&self, _: &str) -> io::Result<()> {
        Ok(())
    }
    fn set_progress(&mut self, _: bool) -> io::Result<()> {
        Ok(())
    }
}

fn assistant_message(
    content: &str,
    stop_reason: &str,
    error_message: Option<&str>,
) -> AgentMessage {
    AgentMessage::Custom(serde_json::json!({
        "role": "assistant",
        "content": if content.is_empty() { Value::Array(vec![]) } else { serde_json::json!([{"type":"text","text":content}]) },
        "stopReason": stop_reason,
        "errorMessage": error_message,
    }))
}

fn apply_lifecycle(mode: &mut InteractiveMode, name: &str, data: &Value) {
    match name {
        "message_start" => mode.apply_session_event(AgentHarnessEvent::Agent(AgentEvent::MessageStart {
            message: assistant_message("", "complete", None),
        })),
        "message_update" => {
            let message = assistant_message(data.get("content").and_then(Value::as_str).unwrap_or(""), "complete", None);
            let assistant_message_event = serde_json::from_value(serde_json::json!({
                "type":"text_delta", "contentIndex":0, "delta":"", "partial": {
                    "role":"assistant", "content":[], "api":"openai-completions", "provider":"openai", "model":"oracle",
                    "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0.0,"output":0.0,"cacheRead":0.0,"cacheWrite":0.0,"total":0.0}},
                    "stopReason":"stop", "timestamp":0
                }
            })).unwrap();
            mode.apply_session_event(AgentHarnessEvent::Agent(AgentEvent::MessageUpdate { message, assistant_message_event }));
        }
        "message_end" => mode.apply_session_event(AgentHarnessEvent::Agent(AgentEvent::MessageEnd {
            message: assistant_message(data.get("content").and_then(Value::as_str).unwrap_or(""), "complete", None),
        })),
        "tool_start" => mode.apply_session_event(AgentHarnessEvent::Agent(AgentEvent::ToolExecutionStart {
            tool_call_id: "oracle".into(), tool_name: data.get("tool").and_then(Value::as_str).unwrap_or("tool").into(), args: Value::Object(Default::default()),
        })),
        "tool_update" => mode.apply_session_event(AgentHarnessEvent::Agent(AgentEvent::ToolExecutionUpdate {
            tool_call_id: "oracle".into(), tool_name: "tool".into(), args: Value::Object(Default::default()), partial_result: serde_json::json!({"content":[{"type":"text","text":data.get("content").and_then(Value::as_str).unwrap_or("")}]}),
        })),
        "tool_end" => mode.apply_session_event(AgentHarnessEvent::Agent(AgentEvent::ToolExecutionEnd {
            tool_call_id: "oracle".into(), tool_name: "tool".into(), result: serde_json::json!({"content":[{"type":"text","text":data.get("content").and_then(Value::as_str).unwrap_or("")}]}), is_error: false,
        })),
        "abort" => mode.apply_session_event(AgentHarnessEvent::Agent(AgentEvent::MessageEnd {
            message: assistant_message("", "aborted", Some("Operation aborted")),
        })),
        "error" => mode.apply_session_event(AgentHarnessEvent::Agent(AgentEvent::MessageEnd {
            message: assistant_message("", "error", data.get("message").and_then(Value::as_str)),
        })),
        _ => {}
    }
}

fn render(fixture: Fixture) -> OracleOutput {
    assert_eq!(fixture.version, 2);
    assert!(matches!(
        fixture.capabilities.colors,
        0 | 16 | 256 | 16_777_216
    ));
    assert!(fixture.capabilities.unicode);
    let _kitty_keyboard = fixture.capabilities.kitty_keyboard;
    let terminal = Arc::new(Mutex::new(MemoryTerminalState {
        columns: fixture.dimensions.columns,
        rows: fixture.dimensions.rows,
        ..Default::default()
    }));
    let mut mode = InteractiveMode::with_terminal(MemoryTerminal(Arc::clone(&terminal)));
    mode.run().unwrap();
    let mut screen = Screen::new(fixture.dimensions.columns, fixture.dimensions.rows);
    let mut frames = Vec::new();
    let mut inputs = Vec::new();
    let mut lifecycle = Vec::new();

    for event in fixture.events {
        match event {
            Event::Input { data } => {
                inputs.push(data.clone());
                terminal
                    .lock()
                    .unwrap()
                    .events
                    .push_back(TerminalEvent::Input(data));
                mode.pump_events(Duration::ZERO).unwrap();
            }
            Event::Resize { columns, rows } => {
                let mut state = terminal.lock().unwrap();
                state.columns = columns;
                state.rows = rows;
                state.events.push_back(TerminalEvent::Resize);
                drop(state);
                screen.resize(columns, rows);
                mode.pump_events(Duration::ZERO).unwrap();
            }
            Event::Lifecycle { name, data } => {
                let data = clean_metadata(data);
                lifecycle.push(Lifecycle {
                    name: name.clone(),
                    data: data.clone(),
                });
                apply_lifecycle(&mut mode, &name, &data);
            }
        }
        let state = terminal.lock().unwrap();
        let lines = mode.render_current_frame(state.columns as usize, state.rows as usize);
        let columns = state.columns;
        let rows = state.rows;
        drop(state);
        screen.write(&format!("\x1b[2J\x1b[H{}", lines.join("\r\n")));
        screen.cursor.visible = mode.tui_mut().overlay_count() == 0;
        frames.push(screen.frame());
        assert_eq!(screen.columns, columns);
        assert_eq!(screen.rows, rows);
    }
    mode.stop().unwrap();
    OracleOutput {
        version: 2,
        frames,
        inputs,
        lifecycle,
    }
}

#[test]
fn all_fixtures_use_component_oracle_without_fixture_rendering() {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/tui-parity/fixtures");
    for name in [
        "input-editing.json",
        "streaming.json",
        "tools-compaction.json",
        "commands.json",
        "overlays.json",
        "unicode-resize.json",
        "abort-error.json",
    ] {
        let fixture: Fixture =
            serde_json::from_slice(&std::fs::read(directory.join(name)).unwrap()).unwrap();
        let event_count = fixture.events.len();
        let output = render(fixture);
        assert_eq!(output.frames.len(), event_count, "{name}");
        assert!(
            output
                .lifecycle
                .iter()
                .all(|event| event.name != "received"),
            "{name}"
        );
    }
}

#[test]
fn streaming_commands_and_editor_input_remain_component_observable() {
    let streaming: Fixture = serde_json::from_str(include_str!(
        "../../../tools/tui-parity/fixtures/streaming.json"
    ))
    .unwrap();
    let streaming_event_count = streaming.events.len();
    let output = render(streaming);
    assert_eq!(output.lifecycle[1].data["content"], "answer");
    assert_eq!(output.lifecycle[1].data.get("timestamp"), None);
    assert_eq!(output.frames.len(), streaming_event_count);

    let commands: Fixture = serde_json::from_str(include_str!(
        "../../../tools/tui-parity/fixtures/commands.json"
    ))
    .unwrap();
    let output = render(commands);
    assert_eq!(
        output.inputs,
        ["/settings", "\r", "\x1b[B", "\x1b", "/compact", "\r"]
    );
    let submitted_frames = [1, 5].map(|index| {
        output.frames[index]
            .cells
            .iter()
            .flatten()
            .map(|cell| cell.text.as_str())
            .collect::<String>()
    });
    assert!(
        submitted_frames
            .iter()
            .all(|visible| !visible.contains("/settings") && !visible.contains("/compact")),
        "submitted built-ins must not leak into the mounted CustomEditor"
    );
}

#[test]
#[ignore = "invoked by tools/tui-parity/run.py"]
fn rust_oracle_subprocess() {
    let mut fixture = String::new();
    io::stdin().read_to_string(&mut fixture).unwrap();
    let output = render(serde_json::from_str(&fixture).unwrap());
    println!(
        "ZEDFLOW_TUI_ORACLE:{}",
        serde_json::to_string(&output).unwrap()
    );
}
