use std::path::{Path, PathBuf};
use std::sync::Arc;

use zedflow_agent::types::{AgentCallbackError, AgentTool, AgentToolResult, ToolSchema};
use zedflow_ai::Tool;

use crate::file_mutation_queue::with_file_mutation_queue;
use crate::path_utils::resolve_to_cwd;
use crate::read::{check_aborted, required_string, schema, text_content};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WriteToolInput {
    pub path: String,
    pub content: String,
}

pub fn create_write_tool(cwd: impl Into<PathBuf>) -> AgentTool {
    let cwd = cwd.into();
    AgentTool {
        tool: Tool {
            name: "write".into(),
            description: "Write content to a file. Creates the file if it doesn't exist, overwrites if it does. Automatically creates parent directories.".into(),
            parameters: schema(
                r#"{
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Path to the file to write (relative or absolute)" },
                        "content": { "type": "string", "description": "Content to write to the file" }
                    },
                    "required": ["path", "content"]
                }"#,
            ),
        },
        label: "write".into(),
        prepare_arguments: None,
        execution_mode: None,
        execute: Arc::new(move |_tool_call_id, arguments, signal, _on_update| {
            let cwd = cwd.clone();
            Box::pin(async move {
                let input = WriteToolInput {
                    path: required_string(&arguments, "path")?,
                    content: required_string(&arguments, "content")?,
                };
                execute_write(&cwd, input, signal).await
            })
        }),
    }
}

pub async fn execute_write(
    cwd: &Path,
    input: WriteToolInput,
    signal: Option<zedflow_ai::AbortSignal>,
) -> Result<AgentToolResult, AgentCallbackError> {
    let absolute_path = resolve_to_cwd(&input.path, cwd)?;
    let parent = absolute_path.parent().unwrap_or(cwd).to_path_buf();
    let display_path = input.path.clone();
    let content = input.content;
    let written_units = content.encode_utf16().count();

    with_file_mutation_queue(&absolute_path, || async {
        check_aborted(signal.as_ref())?;
        tokio::fs::create_dir_all(parent).await?;
        check_aborted(signal.as_ref())?;
        tokio::fs::write(&absolute_path, content).await?;
        check_aborted(signal.as_ref())?;
        Ok::<_, AgentCallbackError>(AgentToolResult {
            content: vec![text_content(format!(
                "Successfully wrote {written_units} bytes to {display_path}"
            ))],
            details: ToolSchema::Null,
            terminate: None,
        })
    })
    .await?
}
