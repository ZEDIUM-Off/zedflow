//! Conversion between coding-agent tool definitions and core agent tools.
//!
//! Rust uses the same `AgentTool` value for both contracts, so wrapping is an
//! identity operation rather than the adapter required by TypeScript.

use zedflow_agent::types::AgentTool;

pub type ToolDefinition = AgentTool;

pub fn wrap_tool_definition(definition: ToolDefinition) -> AgentTool {
    definition
}

pub fn wrap_tool_definitions(definitions: Vec<ToolDefinition>) -> Vec<AgentTool> {
    definitions
}

pub fn create_tool_definition_from_agent_tool(tool: AgentTool) -> ToolDefinition {
    tool
}
