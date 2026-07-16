use serde_json::json;
use zedflow_ai::api::google_shared::{Tool, convert_tools};

fn tool(parameters: serde_json::Value) -> Tool {
    Tool {
        name: "test_tool".into(),
        description: "A test tool".into(),
        parameters,
    }
}

#[test]
fn recursively_strips_openapi_meta_keys_without_mutating_or_removing_ref() {
    let parameters = json!({
        "$schema": "draft-07", "$id": "root", "$defs": { "x": { "type": "string" } },
        "type": "object", "properties": { "deep": { "$comment": "meta", "$ref": "#/$defs/x", "type": "string" } }
    });
    let original = parameters.clone();
    let converted = convert_tools(&[tool(parameters.clone())], true).expect("declarations");
    assert_eq!(parameters, original);
    assert_eq!(
        converted[0].function_declarations[0].parameters,
        Some(json!({
            "type": "object", "properties": { "deep": { "$ref": "#/$defs/x", "type": "string" } }
        }))
    );
    assert!(
        converted[0].function_declarations[0]
            .parameters_json_schema
            .is_none()
    );
}

#[test]
fn selects_json_schema_field_and_empty_tools_are_omitted() {
    let schema = json!({ "$schema": "draft-07", "type": "object" });
    let converted = convert_tools(&[tool(schema.clone())], false).expect("declarations");
    let declaration = &converted[0].function_declarations[0];
    assert_eq!(declaration.parameters_json_schema, Some(schema));
    assert!(declaration.parameters.is_none());
    assert!(convert_tools(&[], true).is_none());
}
