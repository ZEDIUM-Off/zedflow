use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, Read};
use zedflow_coding_agent::{
    keybindings::KeybindingsManager,
    modes_interactive_components_index::{
        assistant_message::{StopReason, StreamingAssistantMessage},
        custom_editor::CustomEditor,
    },
    tool_execution::ToolExecutionComponent,
};
use zedflow_tui::{
    Component, SelectItem, SelectList, SelectListTheme, Text, Tui, utils::visible_width,
};

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

struct Terminal {
    columns: u16,
    rows: u16,
    cells: Vec<Vec<Cell>>,
    cursor: Cursor,
    style: Option<Value>,
}

impl Terminal {
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
                '\x1b' if chars.next_if_eq(&']').is_some() => {
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
            while row
                .last()
                .is_some_and(|cell| cell.text.is_empty() && cell.width == 1)
            {
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

struct Lines(Vec<String>);
impl Component for Lines {
    fn render(&self, _: usize) -> Vec<String> {
        self.0.clone()
    }
}

fn plain_select_theme() -> SelectListTheme {
    let plain = std::sync::Arc::new(|text: &str| text.to_owned());
    SelectListTheme {
        selected_prefix: plain.clone(),
        selected_text: plain.clone(),
        description: plain.clone(),
        scroll_info: plain.clone(),
        no_match: plain,
    }
}

fn frame_lines(
    columns: usize,
    rows: usize,
    assistant: &StreamingAssistantMessage,
    show_assistant: bool,
    tool: &ToolExecutionComponent,
    show_tool: bool,
    compacting: bool,
    editor: &CustomEditor,
    overlay: bool,
    selected: usize,
) -> Vec<String> {
    let mut tui = Tui::new();
    if show_assistant {
        tui.root.add_child(Lines(assistant.render(columns)));
    }
    if show_tool {
        tui.root.add_child(Lines(tool.render(columns)));
    }
    if compacting {
        tui.root.add_child(Text::new("Compacting...", 1, 0));
    }
    tui.root.add_child(Lines(editor.render(columns)));
    if overlay {
        let mut selector = SelectList::new(
            vec![
                SelectItem {
                    value: "model".into(),
                    label: "model".into(),
                    description: None,
                },
                SelectItem {
                    value: "session".into(),
                    label: "session".into(),
                    description: None,
                },
            ],
            5,
            plain_select_theme(),
        );
        selector.set_selected_index(selected);
        tui.root.add_child(Lines(selector.render(columns)));
    }
    tui.render_frame(columns, rows)
}

fn render(fixture: Fixture) -> OracleOutput {
    assert_eq!(fixture.version, 2);
    assert!(matches!(
        fixture.capabilities.colors,
        0 | 16 | 256 | 16_777_216
    ));
    assert!(fixture.capabilities.unicode);
    let _kitty_keyboard = fixture.capabilities.kitty_keyboard;
    let mut columns = fixture.dimensions.columns;
    let mut rows = fixture.dimensions.rows;
    let mut terminal = Terminal::new(columns, rows);
    let mut editor = CustomEditor::new(KeybindingsManager::new(Default::default(), None));
    let mut assistant = StreamingAssistantMessage::default();
    let mut tool = ToolExecutionComponent::new("tool", "");
    let mut show_assistant = false;
    let mut show_tool = false;
    let mut compacting = false;
    let mut overlay = false;
    let mut selected = 0;
    let mut frames = Vec::new();
    let mut inputs = Vec::new();
    let mut lifecycle = Vec::new();

    for event in fixture.events {
        match event {
            Event::Input { data } => {
                inputs.push(data.clone());
                if overlay {
                    if data == "\x1b[B" || data == "\x1b[A" {
                        selected = (selected + 1) % 2;
                    }
                    if data == "\x1b" || data == "\r" {
                        overlay = false;
                    }
                } else if data == "/model\r" || data == "/session\r" {
                    overlay = true;
                } else if data != "/compact\r" {
                    editor.handle_input(&data);
                }
            }
            Event::Resize {
                columns: next_columns,
                rows: next_rows,
            } => {
                columns = next_columns;
                rows = next_rows;
                terminal.resize(columns, rows);
            }
            Event::Lifecycle { name, data } => {
                let data = clean_metadata(data);
                lifecycle.push(Lifecycle {
                    name: name.clone(),
                    data: data.clone(),
                });
                match name.as_str() {
                    "message_start" => {
                        assistant = StreamingAssistantMessage::default();
                        show_assistant = true;
                    }
                    "message_update" | "message_end" => {
                        assistant.update_content(
                            "",
                            data.get("content").and_then(Value::as_str).unwrap_or(""),
                        );
                        show_assistant = true;
                    }
                    "tool_start" => {
                        tool = ToolExecutionComponent::new(
                            data.get("tool").and_then(Value::as_str).unwrap_or("tool"),
                            "",
                        );
                        tool.mark_execution_started();
                        show_tool = true;
                    }
                    "tool_update" => tool.update_result(
                        data.get("content").and_then(Value::as_str).unwrap_or(""),
                        false,
                    ),
                    "tool_end" => tool.update_result(
                        data.get("content").and_then(Value::as_str).unwrap_or(""),
                        false,
                    ),
                    "compaction_start" => compacting = true,
                    "compaction_end" => compacting = false,
                    "session" => overlay = true,
                    "abort" => {
                        assistant.set_stop(StopReason::Aborted, Some("Operation aborted".into()));
                        show_assistant = true;
                    }
                    "error" => {
                        assistant.set_stop(
                            StopReason::Error,
                            data.get("message")
                                .and_then(Value::as_str)
                                .map(str::to_owned),
                        );
                        show_assistant = true;
                    }
                    _ => {}
                }
            }
        }
        let lines = frame_lines(
            columns as usize,
            rows as usize,
            &assistant,
            show_assistant,
            &tool,
            show_tool,
            compacting,
            &editor,
            overlay,
            selected,
        );
        terminal.write(&format!("\x1b[2J\x1b[H{}", lines.join("\r\n")));
        terminal.cursor.visible = !overlay;
        frames.push(terminal.frame());
    }
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
    let output = render(streaming);
    assert_eq!(output.lifecycle[1].data["content"], "answer");
    assert_eq!(output.lifecycle[1].data.get("timestamp"), None);
    assert!(
        output.frames[1]
            .cells
            .iter()
            .flatten()
            .map(|cell| cell.text.as_str())
            .collect::<String>()
            .contains("answer")
    );

    let commands: Fixture = serde_json::from_str(include_str!(
        "../../../tools/tui-parity/fixtures/commands.json"
    ))
    .unwrap();
    let output = render(commands);
    assert_eq!(output.inputs, ["/model\r", "\x1b[B", "\x1b", "/compact\r"]);
    assert!(
        output
            .frames
            .iter()
            .all(|frame| frame.cells.iter().flatten().all(|cell| cell.text != "/")),
        "built-ins must not leak into the editor"
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
