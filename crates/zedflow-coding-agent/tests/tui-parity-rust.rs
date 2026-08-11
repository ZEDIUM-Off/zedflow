use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Debug, Serialize)]
struct OracleOutput {
    version: u8,
    frames: Vec<Frame>,
    inputs: Vec<String>,
    lifecycle: Vec<Lifecycle>,
}

#[derive(Debug, Serialize)]
struct Frame {
    cells: Vec<Vec<Cell>>,
    cursor: Cursor,
}

#[derive(Debug, Serialize)]
struct Cell {
    text: String,
    width: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<Value>,
}

#[derive(Debug, Serialize)]
struct Cursor {
    x: u16,
    y: u16,
    visible: bool,
}

#[derive(Debug, Serialize)]
struct Lifecycle {
    name: String,
    data: Value,
}

#[test]
fn consumes_fixture_and_emits_normalized_frame_protocol() {
    let fixture: Fixture = serde_json::from_str(
        r#"{
          "version":1,
          "dimensions":{"columns":8,"rows":2},
          "capabilities":{"colors":256,"unicode":true,"kittyKeyboard":true},
          "events":[
            {"type":"write","data":"Pi"},
            {"type":"input","data":"x"},
            {"type":"resize","columns":10,"rows":3},
            {"type":"lifecycle","name":"message_end","data":null}
          ]
        }"#,
    )
    .unwrap();

    assert_eq!(fixture.version, 1);
    assert_eq!(
        (fixture.dimensions.columns, fixture.dimensions.rows),
        (8, 2)
    );
    assert_eq!(
        (
            fixture.capabilities.colors,
            fixture.capabilities.unicode,
            fixture.capabilities.kitty_keyboard,
        ),
        (256, true, true)
    );
    match &fixture.events[..] {
        [
            Event::Write { data },
            Event::Input { data: input },
            Event::Resize { columns, rows },
            Event::Lifecycle {
                name,
                data: Value::Null,
                render: None,
            },
        ] => {
            assert_eq!(
                (
                    data.as_str(),
                    input.as_str(),
                    *columns,
                    *rows,
                    name.as_str()
                ),
                ("Pi", "x", 10, 3, "message_end")
            );
        }
        events => panic!("unexpected events: {events:?}"),
    }

    let output = OracleOutput {
        version: 1,
        frames: vec![Frame {
            cells: vec![vec![Cell {
                text: "Pi".into(),
                width: 1,
                style: None,
            }]],
            cursor: Cursor {
                x: 2,
                y: 0,
                visible: true,
            },
        }],
        inputs: vec!["x".into()],
        lifecycle: vec![Lifecycle {
            name: "message_end".into(),
            data: Value::Null,
        }],
    };
    let encoded = serde_json::to_value(output).unwrap();
    assert_eq!(encoded["frames"][0]["cursor"]["x"], 2);
    assert_eq!(encoded["frames"][0]["cells"][0][0]["text"], "Pi");

    let schema: Value = serde_json::from_str(include_str!(
        "../../../tools/tui-parity/fixtures/schema.json"
    ))
    .unwrap();
    assert_eq!(schema["properties"]["version"]["const"], 1);
}
