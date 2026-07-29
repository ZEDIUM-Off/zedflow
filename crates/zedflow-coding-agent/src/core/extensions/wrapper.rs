use serde_json::Value;

use super::{
    runner::ExtensionRunner,
    types::{ExtensionError, ToolInfo},
};

#[must_use]
pub fn wrap_registered_tool(tool: ToolInfo) -> ToolInfo {
    tool
}

#[must_use]
pub fn wrap_registered_tools(tools: impl IntoIterator<Item = ToolInfo>) -> Vec<ToolInfo> {
    tools.into_iter().map(wrap_registered_tool).collect()
}

/// Small native equivalent of Pi's tool wrapper: tool calls always receive the runner's
/// current context rather than a context captured during registration.
pub fn execute_registered_tool(
    runner: &mut ExtensionRunner,
    name: &str,
    arguments: Value,
) -> Result<Value, ExtensionError> {
    runner.invoke_tool(name, arguments)
}
