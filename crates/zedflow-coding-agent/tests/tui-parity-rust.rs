use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, Read};
use zedflow_tui::utils::visible_width;

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
    Write {
        data: String,
    },
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
        #[serde(default)]
        render: Option<String>,
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
            "H" => {
                self.cursor.x = 0;
                self.cursor.y = 0;
            }
            "0m" | "m" => self.style = None,
            "31m" => self.style = Some(serde_json::json!({ "fg": "16777216:1" })),
            "K" | "0K" => {
                for x in self.cursor.x..self.columns {
                    self.cells[self.cursor.y as usize][x as usize] = blank_cell();
                }
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
        if self.cursor.x == self.columns {
            // xterm wraps lazily when the next printable cell arrives.
        }
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

fn render(fixture: Fixture) -> OracleOutput {
    assert_eq!(fixture.version, 1);
    assert!(matches!(
        fixture.capabilities.colors,
        0 | 16 | 256 | 16_777_216
    ));
    assert!(fixture.capabilities.unicode);
    let _kitty_keyboard = fixture.capabilities.kitty_keyboard;
    let mut terminal = Terminal::new(fixture.dimensions.columns, fixture.dimensions.rows);
    let mut frames = Vec::new();
    let mut inputs = Vec::new();
    let mut lifecycle = Vec::new();
    for event in fixture.events {
        match event {
            Event::Write { data } => terminal.write(&data),
            Event::Input { data } => inputs.push(data),
            Event::Resize { columns, rows } => terminal.resize(columns, rows),
            Event::Lifecycle { name, data, render } => {
                lifecycle.push(Lifecycle {
                    name,
                    data: clean_metadata(data),
                });
                if let Some(render) = render {
                    terminal.write(&render);
                }
            }
        }
        frames.push(terminal.frame());
    }
    OracleOutput {
        version: 1,
        frames,
        inputs,
        lifecycle,
    }
}

#[test]
fn all_fixtures_exercise_the_rust_frame_protocol() {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/tui-parity/fixtures");
    let names = [
        "input-editing.json",
        "streaming.json",
        "tools-compaction.json",
        "commands.json",
        "overlays.json",
        "unicode-resize.json",
        "abort-error.json",
    ];
    for name in names {
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
fn streaming_updates_and_command_inputs_are_not_dropped() {
    let streaming: Fixture = serde_json::from_str(include_str!(
        "../../../tools/tui-parity/fixtures/streaming.json"
    ))
    .unwrap();
    let output = render(streaming);
    assert_eq!(output.lifecycle[1].data["content"], "answer");
    assert_eq!(output.lifecycle[1].data.get("timestamp"), None);

    let commands: Fixture = serde_json::from_str(include_str!(
        "../../../tools/tui-parity/fixtures/commands.json"
    ))
    .unwrap();
    assert_eq!(render(commands).inputs, ["/model\r", "/compact\r"]);
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
