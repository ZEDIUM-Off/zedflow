use serde_json::json;
use zedflow_ai::api::anthropic_messages::{from_claude_code_tool_name, to_claude_code_tool_name};
use zedflow_ai::types::Tool;

fn tool(name: &str, description: &str, property_name: &str, property_description: &str) -> Tool {
    Tool {
        name: name.to_owned(),
        description: description.to_owned(),
        parameters: json!({
            "type": "object",
            "properties": {
                property_name: {
                    "type": "string",
                    "description": property_description,
                }
            },
            "required": [property_name],
        }),
    }
}

fn round_trip_tool_name(name: &str, tools: &[Tool]) -> String {
    let outbound = to_claude_code_tool_name(name);
    from_claude_code_tool_name(&outbound, Some(tools)).into_owned()
}

#[test]
fn normalizes_user_defined_tool_matching_cc_name_todowrite_round_trip() {
    let todo_tool = tool("todowrite", "Write a todo item", "task", "The task to add");
    let tools = [todo_tool];

    assert_eq!(to_claude_code_tool_name("todowrite"), "TodoWrite");
    assert_eq!(round_trip_tool_name("todowrite", &tools), "todowrite");
}

#[test]
fn handles_pi_builtin_tools_read_write_edit_bash() {
    for (name, canonical) in [
        ("read", "Read"),
        ("write", "Write"),
        ("edit", "Edit"),
        ("bash", "Bash"),
    ] {
        let tools = [tool(name, "Pi built-in tool", "path", "File path")];

        assert_eq!(to_claude_code_tool_name(name), canonical);
        assert_eq!(round_trip_tool_name(name, &tools), name);
    }
}

#[test]
fn does_not_map_find_to_glob() {
    let find_tool = tool("find", "Find files by pattern", "pattern", "Glob pattern");
    let tools = [find_tool];

    assert_eq!(to_claude_code_tool_name("find"), "find");
    assert_eq!(from_claude_code_tool_name("Glob", Some(&tools)), "Glob");
    assert_eq!(round_trip_tool_name("find", &tools), "find");
}

#[test]
fn handles_custom_tools_that_do_not_match_cc_tool_names() {
    let custom_tool = tool("my_custom_tool", "A custom tool", "input", "Input value");
    let tools = [custom_tool];

    assert_eq!(to_claude_code_tool_name("my_custom_tool"), "my_custom_tool");
    assert_eq!(
        round_trip_tool_name("my_custom_tool", &tools),
        "my_custom_tool"
    );
}
