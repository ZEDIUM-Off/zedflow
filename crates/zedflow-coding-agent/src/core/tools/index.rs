//! Built-in coding tool exports and grouped constructors.

use std::path::Path;

use zedflow_agent::types::AgentTool;

pub use crate::{
    bash_tool::{
        BashOperationOptions, BashOperations, BashSpawnContext, BashSpawnHook, BashToolOptions,
        LocalBashOperations, create_bash_tool, create_bash_tool_with_options,
        create_local_bash_operations,
    },
    edit::{
        EditOperations, EditToolDetails, EditToolInput, create_edit_tool,
        create_edit_tool_with_operations,
    },
    file_mutation_queue::with_file_mutation_queue,
    find::{
        FindOperations, FindToolDetails, FindToolInput, create_find_tool,
        create_find_tool_with_operations,
    },
    grep::{
        GrepOperations, GrepToolDetails, GrepToolInput, create_grep_tool,
        create_grep_tool_with_operations,
    },
    ls::{
        LsOperations, LsToolDetails, LsToolInput, LsToolOptions, create_ls_tool,
        create_ls_tool_with_operations, create_ls_tool_with_options,
    },
    read::{
        ReadOperations, ReadToolDetails, ReadToolInput, create_read_tool,
        create_read_tool_with_operations,
    },
    truncate::{
        DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, TruncationOptions, TruncationResult, format_size,
        truncate_head, truncate_line, truncate_tail,
    },
    write::{
        WriteOperations, WriteToolInput, WriteToolOptions, create_write_tool,
        create_write_tool_with_operations, create_write_tool_with_options,
    },
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ToolName {
    Read,
    Bash,
    Edit,
    Write,
    Grep,
    Find,
    Ls,
}

pub const ALL_TOOL_NAMES: [ToolName; 7] = [
    ToolName::Read,
    ToolName::Bash,
    ToolName::Edit,
    ToolName::Write,
    ToolName::Grep,
    ToolName::Find,
    ToolName::Ls,
];

pub fn create_tool(tool_name: ToolName, cwd: impl AsRef<Path>) -> AgentTool {
    match tool_name {
        ToolName::Read => create_read_tool(cwd),
        ToolName::Bash => create_bash_tool(cwd),
        ToolName::Edit => create_edit_tool(cwd),
        ToolName::Write => create_write_tool(cwd),
        ToolName::Grep => create_grep_tool(cwd),
        ToolName::Find => create_find_tool(cwd),
        ToolName::Ls => create_ls_tool(cwd),
    }
}

pub fn create_coding_tools(cwd: impl AsRef<Path>) -> Vec<AgentTool> {
    let cwd = cwd.as_ref();
    vec![
        create_read_tool(cwd),
        create_bash_tool(cwd),
        create_edit_tool(cwd),
        create_write_tool(cwd),
    ]
}

pub fn create_read_only_tools(cwd: impl AsRef<Path>) -> Vec<AgentTool> {
    let cwd = cwd.as_ref();
    vec![
        create_read_tool(cwd),
        create_grep_tool(cwd),
        create_find_tool(cwd),
        create_ls_tool(cwd),
    ]
}

pub fn create_all_tools(cwd: impl AsRef<Path>) -> Vec<(ToolName, AgentTool)> {
    let cwd = cwd.as_ref();
    ALL_TOOL_NAMES
        .into_iter()
        .map(|name| (name, create_tool(name, cwd)))
        .collect()
}
