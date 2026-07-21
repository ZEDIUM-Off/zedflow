use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use zedflow_agent::types::{
    AgentCallbackError, AgentFuture, AgentTool, AgentToolExecuteFn, AgentToolResult,
    AgentToolResultContent, Tool, ToolSchema,
};
use zedflow_ai::{AbortSignal, TextContent, TextContentType};

use super::file_mutation_queue::with_file_mutation_queue;
use super::path_utils::resolve_to_cwd;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WriteToolInput {
    pub path: String,
    pub content: String,
}

pub type WriteToolResult = AgentToolResult<()>;

#[derive(Clone, Debug)]
pub struct WriteTool {
    cwd: PathBuf,
}

impl WriteTool {
    pub fn new(cwd: impl AsRef<Path>) -> Self {
        Self {
            cwd: cwd.as_ref().to_path_buf(),
        }
    }

    pub async fn execute(&self, input: WriteToolInput) -> io::Result<WriteToolResult> {
        self.execute_with_signal(input, None).await
    }

    async fn execute_with_signal(
        &self,
        input: WriteToolInput,
        signal: Option<&AbortSignal>,
    ) -> io::Result<WriteToolResult> {
        let absolute_path = resolve_to_cwd(&input.path, &self.cwd)?;
        let directory = absolute_path
            .parent()
            .ok_or_else(|| io::Error::other("write path has no parent directory"))?
            .to_path_buf();
        let path_display = input.path.clone();
        let content = input.content;
        let write_path = absolute_path.clone();

        with_file_mutation_queue(&absolute_path, || async move {
            check_aborted(signal)?;
            tokio::fs::create_dir_all(directory).await?;
            check_aborted(signal)?;
            tokio::fs::write(write_path, content.as_bytes()).await?;
            check_aborted(signal)?;

            Ok(AgentToolResult {
                content: vec![text(format!(
                    "Successfully wrote {} bytes to {path_display}",
                    content.encode_utf16().count()
                ))],
                details: (),
                terminate: None,
            })
        })
        .await?
    }
}

pub fn create_write_tool(cwd: impl AsRef<Path>) -> AgentTool {
    let tool = WriteTool::new(cwd);
    let execute: AgentToolExecuteFn = Arc::new(move |_tool_call_id, args, signal, _on_update| {
        let tool = tool.clone();
        Box::pin(async move {
            let input = WriteToolInput {
                path: args
                    .get("path")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_owned(),
                content: args
                    .get("content")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_owned(),
            };
            let result = tool
                .execute_with_signal(input, signal.as_ref())
                .await
                .map_err(|error| Box::new(error) as AgentCallbackError)?;
            Ok(AgentToolResult {
                content: result.content,
                details: ToolSchema::Null,
                terminate: result.terminate,
            })
        }) as AgentFuture<'_, _>
    });

    AgentTool {
        tool: Tool {
            name: "write".into(),
            description: "Write content to a file. Creates the file if it doesn't exist, overwrites if it does. Automatically creates parent directories.".into(),
            parameters: serde_yaml::from_str(
                r#"{"type":"object","properties":{"path":{"type":"string","description":"Path to the file to write (relative or absolute)"},"content":{"type":"string","description":"Content to write to the file"}},"required":["path","content"]}"#,
            )
            .expect("valid write schema"),
        },
        label: "write".into(),
        prepare_arguments: None,
        execute,
        execution_mode: None,
    }
}

fn check_aborted(signal: Option<&AbortSignal>) -> io::Result<()> {
    if signal.is_some_and(AbortSignal::aborted) {
        Err(io::Error::other("Operation aborted"))
    } else {
        Ok(())
    }
}

fn text(value: impl Into<String>) -> AgentToolResultContent {
    AgentToolResultContent::Text(TextContent {
        content_type: TextContentType::Text,
        text: value.into(),
        text_signature: None,
    })
}
