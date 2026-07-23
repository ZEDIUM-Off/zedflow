use super::types::ToolInfo;

#[must_use]
pub fn wrap_registered_tool(tool: ToolInfo) -> ToolInfo {
    tool
}

#[must_use]
pub fn wrap_registered_tools(tools: impl IntoIterator<Item = ToolInfo>) -> Vec<ToolInfo> {
    tools.into_iter().map(wrap_registered_tool).collect()
}
