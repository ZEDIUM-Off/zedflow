//! Tool argument validation helpers ported from Pi's `packages/ai/src/utils/validation.ts`.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use jsonschema::error::ValidationErrorKind;
use serde_json::{Map, Number, Value};

use crate::types::{Tool, ToolCall};

/// Error returned when a tool call cannot be validated against its tool schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolValidationError {
    /// No tool with the requested name exists.
    ToolNotFound {
        /// Requested tool name.
        name: String,
    },
    /// The tool schema could not be compiled.
    InvalidSchema {
        /// Tool name whose schema failed compilation.
        tool_name: String,
        /// Schema compiler error message.
        message: String,
    },
    /// Arguments failed schema validation.
    ValidationFailed {
        /// Tool name whose arguments failed validation.
        tool_name: String,
        /// Formatted validation failures.
        errors: String,
        /// Original arguments received from the tool call.
        received_arguments: Value,
    },
}

impl fmt::Display for ToolValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ToolNotFound { name } => write!(f, "Tool \"{name}\" not found"),
            Self::InvalidSchema { tool_name, message } => {
                write!(
                    f,
                    "Validation schema failed for tool \"{tool_name}\": {message}"
                )
            }
            Self::ValidationFailed {
                tool_name,
                errors,
                received_arguments,
            } => {
                let received = serde_json::to_string_pretty(received_arguments)
                    .unwrap_or_else(|_| received_arguments.to_string());
                write!(
                    f,
                    "Validation failed for tool \"{tool_name}\":\n{errors}\n\nReceived arguments:\n{received}"
                )
            }
        }
    }
}

impl Error for ToolValidationError {}

/// Finds a tool by name and validates the tool call arguments against its schema.
///
/// # Errors
///
/// Returns [`ToolValidationError::ToolNotFound`] if no tool has the call name,
/// [`ToolValidationError::InvalidSchema`] if the tool schema cannot be compiled, or
/// [`ToolValidationError::ValidationFailed`] if the arguments do not satisfy the schema.
pub fn validate_tool_call(
    tools: &[Tool],
    tool_call: &ToolCall,
) -> Result<Value, ToolValidationError> {
    let tool = tools
        .iter()
        .find(|tool| tool.name == tool_call.name)
        .ok_or_else(|| ToolValidationError::ToolNotFound {
            name: tool_call.name.clone(),
        })?;

    validate_tool_arguments(tool, tool_call)
}

/// Validates tool call arguments against a tool's schema and returns the coerced arguments.
///
/// # Errors
///
/// Returns [`ToolValidationError::InvalidSchema`] if the tool schema cannot be compiled, or
/// [`ToolValidationError::ValidationFailed`] if the arguments do not satisfy the schema.
pub fn validate_tool_arguments(
    tool: &Tool,
    tool_call: &ToolCall,
) -> Result<Value, ToolValidationError> {
    let original_args = arguments_to_value(&tool_call.arguments);
    let args = coerce_with_json_schema(original_args.clone(), &tool.parameters);
    let validator = jsonschema::validator_for(&tool.parameters).map_err(|error| {
        ToolValidationError::InvalidSchema {
            tool_name: tool.name.clone(),
            message: error.to_string(),
        }
    })?;

    if validator.is_valid(&args) {
        return Ok(args);
    }

    let errors = validator
        .iter_errors(&args)
        .map(|error| format!("  - {}: {}", format_validation_path(&error), error))
        .collect::<Vec<_>>()
        .join("\n");

    Err(ToolValidationError::ValidationFailed {
        tool_name: tool_call.name.clone(),
        errors: if errors.is_empty() {
            "Unknown validation error".to_string()
        } else {
            errors
        },
        received_arguments: original_args,
    })
}

fn arguments_to_value(arguments: &HashMap<String, Value>) -> Value {
    arguments
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Map<_, _>>()
        .into()
}

fn get_schema_types(schema: &Value) -> Vec<&str> {
    let Some(schema) = schema.as_object() else {
        return Vec::new();
    };

    match schema.get("type") {
        Some(Value::String(schema_type)) => vec![schema_type.as_str()],
        Some(Value::Array(schema_types)) => schema_types
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    }
}

fn matches_json_type(value: &Value, schema_type: &str) -> bool {
    match schema_type {
        "number" => value.is_number(),
        "integer" => is_integer(value),
        "boolean" => value.is_boolean(),
        "string" => value.is_string(),
        "null" => value.is_null(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        _ => false,
    }
}

fn is_integer(value: &Value) -> bool {
    value
        .as_i64()
        .map(|_| true)
        .or_else(|| value.as_u64().map(|_| true))
        .unwrap_or_else(|| value.as_f64().is_some_and(|number| number.fract() == 0.0))
}

fn coerce_primitive_by_type(value: &Value, schema_type: &str) -> Value {
    match schema_type {
        "number" => coerce_number(value, false),
        "integer" => coerce_number(value, true),
        "boolean" => coerce_boolean(value),
        "string" => coerce_string(value),
        "null" => coerce_null(value),
        _ => value.clone(),
    }
}

fn coerce_number(value: &Value, integer_only: bool) -> Value {
    if integer_only {
        return match value {
            Value::Null => Number::from(0).into(),
            Value::String(text) if !text.trim().is_empty() => text
                .parse::<i64>()
                .ok()
                .map(Number::from)
                .map_or_else(|| value.clone(), Value::Number),
            Value::Bool(flag) => Number::from(u8::from(*flag)).into(),
            _ => value.clone(),
        };
    }

    match value {
        Value::Null => Number::from_f64(0.0).map_or_else(|| value.clone(), Value::Number),
        Value::String(text) if !text.trim().is_empty() => text
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite())
            .and_then(Number::from_f64)
            .map_or_else(|| value.clone(), Value::Number),
        Value::Bool(flag) => Number::from_f64(f64::from(u8::from(*flag)))
            .map_or_else(|| value.clone(), Value::Number),
        _ => value.clone(),
    }
}

fn coerce_boolean(value: &Value) -> Value {
    match value {
        Value::Null => Value::Bool(false),
        Value::String(text) if text == "true" => Value::Bool(true),
        Value::String(text) if text == "false" => Value::Bool(false),
        Value::Number(number) if number.as_i64() == Some(1) || number.as_u64() == Some(1) => {
            Value::Bool(true)
        }
        Value::Number(number) if number.as_i64() == Some(0) || number.as_u64() == Some(0) => {
            Value::Bool(false)
        }
        _ => value.clone(),
    }
}

fn coerce_string(value: &Value) -> Value {
    match value {
        Value::Null => Value::String(String::new()),
        Value::Number(number) => Value::String(number.to_string()),
        Value::Bool(flag) => Value::String(flag.to_string()),
        _ => value.clone(),
    }
}

fn coerce_null(value: &Value) -> Value {
    match value {
        Value::String(text) if text.is_empty() => Value::Null,
        Value::Number(number) if number.as_i64() == Some(0) || number.as_u64() == Some(0) => {
            Value::Null
        }
        Value::Bool(false) => Value::Null,
        _ => value.clone(),
    }
}

fn apply_schema_object_coercion(value: &mut Map<String, Value>, schema: &Map<String, Value>) {
    let properties = schema.get("properties").and_then(Value::as_object);
    let defined_keys = properties
        .map(|properties| properties.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();

    if let Some(properties) = properties {
        for (key, property_schema) in properties {
            if let Some(property_value) = value.get_mut(key) {
                *property_value = coerce_with_json_schema(property_value.clone(), property_schema);
            }
        }
    }

    let Some(additional_properties) = schema
        .get("additionalProperties")
        .and_then(Value::as_object)
    else {
        return;
    };

    for (key, property_value) in value {
        if defined_keys.contains(key) {
            continue;
        }
        *property_value = coerce_with_json_schema(
            property_value.clone(),
            &Value::Object(additional_properties.clone()),
        );
    }
}

fn apply_schema_array_coercion(value: &mut [Value], schema: &Map<String, Value>) {
    match schema.get("items") {
        Some(Value::Array(items)) => {
            for (item_value, item_schema) in value.iter_mut().zip(items) {
                *item_value = coerce_with_json_schema(item_value.clone(), item_schema);
            }
        }
        Some(item_schema @ Value::Object(_)) => {
            for item_value in value {
                *item_value = coerce_with_json_schema(item_value.clone(), item_schema);
            }
        }
        _ => {}
    }
}

fn coerce_with_union_schema(value: &Value, schemas: &[Value]) -> Value {
    for schema in schemas {
        let coerced = coerce_with_json_schema(value.clone(), schema);
        let Ok(validator) = jsonschema::validator_for(schema) else {
            continue;
        };
        if validator.is_valid(&coerced) {
            return coerced;
        }
    }
    value.clone()
}

fn coerce_with_json_schema(value: Value, schema: &Value) -> Value {
    let mut next_value = value;
    let Some(schema_object) = schema.as_object() else {
        return next_value;
    };

    if let Some(all_of) = schema_object.get("allOf").and_then(Value::as_array) {
        for nested in all_of {
            next_value = coerce_with_json_schema(next_value, nested);
        }
    }

    if let Some(any_of) = schema_object.get("anyOf").and_then(Value::as_array) {
        next_value = coerce_with_union_schema(&next_value, any_of);
    }

    if let Some(one_of) = schema_object.get("oneOf").and_then(Value::as_array) {
        next_value = coerce_with_union_schema(&next_value, one_of);
    }

    let schema_types = get_schema_types(schema);
    let matches_union_member = schema_types.len() > 1
        && schema_types
            .iter()
            .any(|schema_type| matches_json_type(&next_value, schema_type));
    if !schema_types.is_empty() && !matches_union_member {
        for schema_type in &schema_types {
            let candidate = coerce_primitive_by_type(&next_value, schema_type);
            if candidate != next_value {
                next_value = candidate;
                break;
            }
        }
    }

    if schema_types.contains(&"object")
        && let Value::Object(object) = &mut next_value
    {
        apply_schema_object_coercion(object, schema_object);
    }

    if schema_types.contains(&"array")
        && let Value::Array(array) = &mut next_value
    {
        apply_schema_array_coercion(array, schema_object);
    }

    next_value
}

fn format_validation_path(error: &jsonschema::ValidationError<'_>) -> String {
    if let ValidationErrorKind::Required { property } = error.kind()
        && let Some(required_property) = property.as_str()
    {
        let base_path = location_to_dot_path(&error.instance_path().to_string());
        return if base_path.is_empty() {
            required_property.to_string()
        } else {
            format!("{base_path}.{required_property}")
        };
    }

    let path = location_to_dot_path(&error.instance_path().to_string());
    if path.is_empty() {
        "root".to_string()
    } else {
        path
    }
}

fn location_to_dot_path(path: &str) -> String {
    path.trim_start_matches('/')
        .replace("/", ".")
        .replace("~1", "/")
        .replace("~0", "~")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use super::*;
    use crate::types::ToolCallType;

    #[test]
    fn validates_and_coerces_tool_arguments() {
        let tool = Tool {
            name: "sum".to_string(),
            description: "Add numbers".to_string(),
            parameters: json!({
                "type": "object",
                "properties": { "count": { "type": "integer" } },
                "required": ["count"]
            }),
        };
        let tool_call = ToolCall {
            content_type: ToolCallType::ToolCall,
            id: "call-1".to_string(),
            name: "sum".to_string(),
            arguments: HashMap::from([("count".to_string(), json!("3"))]),
            thought_signature: None,
        };

        assert_eq!(
            validate_tool_call(&[tool], &tool_call).expect("valid coerced arguments"),
            json!({ "count": 3 })
        );
    }

    fn create_tool_call_with_plain_schema(schema: Value, value: Value) -> (Tool, ToolCall) {
        let tool = Tool {
            name: "echo".to_string(),
            description: "Echo tool".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "value": schema,
                },
                "required": ["value"],
            }),
        };
        let tool_call = ToolCall {
            content_type: ToolCallType::ToolCall,
            id: "tool-1".to_string(),
            name: "echo".to_string(),
            arguments: HashMap::from([("value".to_string(), value)]),
            thought_signature: None,
        };

        (tool, tool_call)
    }

    #[test]
    fn reports_missing_tool() {
        let tool_call = ToolCall {
            content_type: ToolCallType::ToolCall,
            id: "call-1".to_string(),
            name: "missing".to_string(),
            arguments: HashMap::new(),
            thought_signature: None,
        };

        assert_eq!(
            validate_tool_call(&[], &tool_call),
            Err(ToolValidationError::ToolNotFound {
                name: "missing".to_string()
            })
        );
    }

    #[test]
    fn still_validates_when_function_constructor_is_unavailable() {
        // Rust jsonschema validation does not use JavaScript's Function constructor.
        let tool = Tool {
            name: "echo".to_string(),
            description: "Echo tool".to_string(),
            parameters: json!({
                "type": "object",
                "properties": { "count": { "type": "number" } },
                "required": ["count"],
            }),
        };
        let tool_call = ToolCall {
            content_type: ToolCallType::ToolCall,
            id: "tool-1".to_string(),
            name: "echo".to_string(),
            arguments: HashMap::from([("count".to_string(), json!("42"))]),
            thought_signature: None,
        };

        assert_eq!(
            validate_tool_arguments(&tool, &tool_call).expect("valid coerced arguments"),
            json!({ "count": 42.0 })
        );
    }

    #[test]
    fn coerces_serialized_plain_json_schemas_with_ajv_compatible_primitive_rules() {
        let passing_cases = [
            (json!({ "type": "number" }), json!("42"), json!(42.0)),
            (json!({ "type": "number" }), json!(true), json!(1.0)),
            (json!({ "type": "number" }), Value::Null, json!(0.0)),
            (json!({ "type": "integer" }), json!("42"), json!(42)),
            (json!({ "type": "boolean" }), json!("true"), json!(true)),
            (json!({ "type": "boolean" }), json!("false"), json!(false)),
            (json!({ "type": "boolean" }), json!(1), json!(true)),
            (json!({ "type": "boolean" }), json!(0), json!(false)),
            (json!({ "type": "string" }), Value::Null, json!("")),
            (json!({ "type": "string" }), json!(true), json!("true")),
            (json!({ "type": "null" }), json!(""), Value::Null),
            (json!({ "type": "null" }), json!(0), Value::Null),
            (json!({ "type": "null" }), json!(false), Value::Null),
            (
                json!({ "type": ["number", "string"] }),
                json!("1"),
                json!("1"),
            ),
            (
                json!({ "type": ["boolean", "number"] }),
                json!("1"),
                json!(1.0),
            ),
        ];

        for (schema, input, expected) in passing_cases {
            let (tool, tool_call) = create_tool_call_with_plain_schema(schema, input);
            assert_eq!(
                validate_tool_arguments(&tool, &tool_call).expect("valid coerced arguments"),
                json!({ "value": expected })
            );
        }
    }

    #[test]
    fn rejects_invalid_coercions_for_serialized_plain_json_schemas() {
        let failing_cases = [
            (json!({ "type": "boolean" }), json!("1")),
            (json!({ "type": "boolean" }), json!("0")),
            (json!({ "type": "null" }), json!("null")),
            (json!({ "type": "integer" }), json!("42.1")),
        ];

        for (schema, input) in failing_cases {
            let (tool, tool_call) = create_tool_call_with_plain_schema(schema, input);
            assert!(
                validate_tool_arguments(&tool, &tool_call)
                    .expect_err("invalid coerced arguments")
                    .to_string()
                    .contains("Validation failed")
            );
        }
    }

    #[test]
    fn coerces_nested_arrays_objects_and_additional_properties() {
        let tool = Tool {
            name: "shape".to_string(),
            description: "Nested schema".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "items": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": { "count": { "type": "integer" } },
                            "additionalProperties": { "type": "boolean" }
                        }
                    }
                },
                "required": ["items"]
            }),
        };
        let tool_call = ToolCall {
            content_type: ToolCallType::ToolCall,
            id: "tool-1".to_string(),
            name: "shape".to_string(),
            arguments: HashMap::from([(
                "items".to_string(),
                json!([{ "count": "3", "enabled": "true" }]),
            )]),
            thought_signature: None,
        };

        assert_eq!(
            validate_tool_arguments(&tool, &tool_call).expect("valid nested coerced arguments"),
            json!({ "items": [{ "count": 3, "enabled": true }] })
        );
    }

    #[test]
    fn reports_required_property_path_like_pi() {
        let tool = Tool {
            name: "nested".to_string(),
            description: "Nested schema".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "outer": {
                        "type": "object",
                        "properties": { "count": { "type": "integer" } },
                        "required": ["count"]
                    }
                },
                "required": ["outer"]
            }),
        };
        let tool_call = ToolCall {
            content_type: ToolCallType::ToolCall,
            id: "tool-1".to_string(),
            name: "nested".to_string(),
            arguments: HashMap::from([("outer".to_string(), json!({}))]),
            thought_signature: None,
        };

        assert!(
            validate_tool_arguments(&tool, &tool_call)
                .expect_err("missing nested property")
                .to_string()
                .contains("outer.count")
        );
    }
}
