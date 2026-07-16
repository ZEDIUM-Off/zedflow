use std::collections::HashMap;

use serde_json::{Value, json};
use zedflow_ai::types::{Tool, ToolCall, ToolCallType};
use zedflow_ai::utils::validation::{validate_tool_arguments, validate_tool_call};

fn call(schema: Value, value: Value) -> (Tool, ToolCall) {
    (
        Tool {
            name: "echo".into(),
            description: "Echo tool".into(),
            parameters: json!({
                "type": "object",
                "properties": { "value": schema },
                "required": ["value"]
            }),
        },
        ToolCall {
            content_type: ToolCallType::ToolCall,
            id: "tool-1".into(),
            name: "echo".into(),
            arguments: HashMap::from([("value".into(), value)]),
            thought_signature: None,
        },
    )
}

#[test]
fn coerces_plain_json_schema_primitives_like_pi() {
    let passing = [
        (json!({"type":"number"}), json!("42"), json!(42.0)),
        (json!({"type":"number"}), json!(true), json!(1.0)),
        (json!({"type":"number"}), Value::Null, json!(0.0)),
        (json!({"type":"integer"}), json!("42"), json!(42)),
        (json!({"type":"boolean"}), json!("true"), json!(true)),
        (json!({"type":"boolean"}), json!("false"), json!(false)),
        (json!({"type":"boolean"}), json!(1), json!(true)),
        (json!({"type":"boolean"}), json!(0), json!(false)),
        (json!({"type":"string"}), Value::Null, json!("")),
        (json!({"type":"string"}), json!(true), json!("true")),
        (json!({"type":"null"}), json!(""), Value::Null),
        (json!({"type":"null"}), json!(0), Value::Null),
        (json!({"type":"null"}), json!(false), Value::Null),
        (json!({"type":["number","string"]}), json!("1"), json!("1")),
        (json!({"type":["boolean","number"]}), json!("1"), json!(1.0)),
    ];
    for (schema, input, expected) in passing {
        let (tool, tool_call) = call(schema, input);
        assert_eq!(
            validate_tool_arguments(&tool, &tool_call).unwrap(),
            json!({"value": expected})
        );
    }
}

#[test]
fn rejects_invalid_plain_schema_coercions_with_pi_error_shape() {
    for (schema, input) in [
        (json!({"type":"boolean"}), json!("1")),
        (json!({"type":"boolean"}), json!("0")),
        (json!({"type":"null"}), json!("null")),
        (json!({"type":"integer"}), json!("42.1")),
    ] {
        let (tool, tool_call) = call(schema, input);
        let error = validate_tool_arguments(&tool, &tool_call)
            .unwrap_err()
            .to_string();
        assert!(
            error.starts_with("Validation failed for tool \"echo\":"),
            "{error}"
        );
        assert!(error.contains("Received arguments:"), "{error}");
    }
}

#[test]
fn handles_nested_values_required_paths_and_additional_properties() {
    let tool = Tool {
        name: "shape".into(),
        description: "Nested schema".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": { "count": { "type": "integer" } },
                        "required": ["count"],
                        "additionalProperties": { "type": "boolean" }
                    }
                }
            },
            "required": ["items"]
        }),
    };
    let valid = ToolCall {
        content_type: ToolCallType::ToolCall,
        id: "tool-1".into(),
        name: "shape".into(),
        arguments: HashMap::from([("items".into(), json!([{"count":"3","enabled":"true"}]))]),
        thought_signature: None,
    };
    assert_eq!(
        validate_tool_arguments(&tool, &valid).unwrap(),
        json!({"items":[{"count":3,"enabled":true}]})
    );

    let missing = ToolCall {
        arguments: HashMap::from([("items".into(), json!([{}]))]),
        ..valid
    };
    assert!(
        validate_tool_arguments(&tool, &missing)
            .unwrap_err()
            .to_string()
            .contains("items.0.count")
    );
}

#[test]
fn reports_unknown_tools() {
    let (_, tool_call) = call(json!({"type":"string"}), json!("ok"));
    assert_eq!(
        validate_tool_call(&[], &tool_call).unwrap_err().to_string(),
        "Tool \"echo\" not found"
    );
}
