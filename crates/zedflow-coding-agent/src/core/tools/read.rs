use std::fmt;
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use zedflow_agent::types::{
    AgentCallbackError, AgentFuture, AgentTool, AgentToolExecuteFn, AgentToolResult,
    AgentToolResultContent, Tool, ToolSchema,
};
use zedflow_ai::{AbortSignal, ImageContent, ImageContentType, TextContent, TextContentType};

use super::path_utils::resolve_read_path_async;
use super::truncate::{
    DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, TruncatedBy, TruncationResult, format_size, truncate_head,
};
use crate::utils::image_process::{ProcessImageOptions, process_image};
use crate::utils::mime::detect_supported_image_mime_type_from_file;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadToolInput {
    pub path: String,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadToolDetails {
    pub truncation: TruncationResult,
}

pub type ReadToolResult = AgentToolResult<Option<ReadToolDetails>>;
pub type ReadOperationFuture<T> = Pin<Box<dyn Future<Output = io::Result<T>> + Send>>;
pub type ReadAccessOperation = Arc<dyn Fn(PathBuf) -> ReadOperationFuture<()> + Send + Sync>;
pub type ReadFileOperation = Arc<dyn Fn(PathBuf) -> ReadOperationFuture<Vec<u8>> + Send + Sync>;
pub type ReadDetectImageMimeTypeOperation =
    Arc<dyn Fn(PathBuf) -> ReadOperationFuture<Option<String>> + Send + Sync>;

#[derive(Clone)]
pub struct ReadOperations {
    pub access: ReadAccessOperation,
    pub read_file: ReadFileOperation,
    pub detect_image_mime_type: Option<ReadDetectImageMimeTypeOperation>,
}

impl Default for ReadOperations {
    fn default() -> Self {
        Self {
            access: Arc::new(|path| {
                Box::pin(async move { tokio::fs::File::open(path).await.map(|_| ()) })
            }),
            read_file: Arc::new(|path| Box::pin(tokio::fs::read(path))),
            detect_image_mime_type: Some(Arc::new(|path| {
                Box::pin(async move {
                    tokio::task::spawn_blocking(move || {
                        detect_supported_image_mime_type_from_file(path)
                            .map(|mime_type| mime_type.map(str::to_owned))
                    })
                    .await
                    .map_err(|error| io::Error::other(error.to_string()))?
                })
            })),
        }
    }
}

impl fmt::Debug for ReadOperations {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReadOperations")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub struct ReadTool {
    cwd: PathBuf,
    operations: ReadOperations,
}

impl ReadTool {
    pub fn new(cwd: impl AsRef<Path>) -> Self {
        Self::with_operations(cwd, ReadOperations::default())
    }

    pub fn with_operations(cwd: impl AsRef<Path>, operations: ReadOperations) -> Self {
        Self {
            cwd: cwd.as_ref().to_path_buf(),
            operations,
        }
    }

    pub async fn execute(&self, input: ReadToolInput) -> io::Result<ReadToolResult> {
        self.execute_with_signal(input, None).await
    }

    async fn execute_with_signal(
        &self,
        input: ReadToolInput,
        signal: Option<&AbortSignal>,
    ) -> io::Result<ReadToolResult> {
        check_aborted(signal)?;
        let absolute_path = resolve_read_path_async(&input.path, &self.cwd).await?;
        check_aborted(signal)?;
        (self.operations.access)(absolute_path.clone()).await?;
        check_aborted(signal)?;
        let mime_type =
            if let Some(detect_image_mime_type) = &self.operations.detect_image_mime_type {
                detect_image_mime_type(absolute_path.clone()).await?
            } else {
                None
            };
        check_aborted(signal)?;
        let bytes = (self.operations.read_file)(absolute_path).await?;
        check_aborted(signal)?;

        if let Some(mime_type) = mime_type {
            let processed_mime_type = mime_type.clone();
            let processed = tokio::task::spawn_blocking(move || {
                process_image(&bytes, &processed_mime_type, ProcessImageOptions::default())
            })
            .await
            .map_err(|error| io::Error::other(error.to_string()))?;
            check_aborted(signal)?;
            let content = match processed {
                Ok(image) => {
                    let mut note = format!("Read image file [{}]", image.mime_type);
                    if !image.hints.is_empty() {
                        note.push('\n');
                        note.push_str(&image.hints.join("\n"));
                    }
                    vec![
                        text(note),
                        AgentToolResultContent::Image(ImageContent {
                            content_type: ImageContentType::Image,
                            data: image.data,
                            mime_type: image.mime_type,
                        }),
                    ]
                }
                Err(message) => vec![text(format!("Read image file [{mime_type}]\n{message}"))],
            };
            return Ok(AgentToolResult {
                content,
                details: None,
                terminate: None,
            });
        }

        let text_content = String::from_utf8_lossy(&bytes);
        let all_lines: Vec<&str> = text_content.split('\n').collect();
        let total_file_lines = all_lines.len();
        let start_line = input.offset.unwrap_or(1).saturating_sub(1);
        let start_line_display = start_line + 1;
        if start_line >= total_file_lines {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Offset {} is beyond end of file ({} lines total)",
                    input.offset.unwrap_or(1),
                    total_file_lines
                ),
            ));
        }

        let (selected_content, user_limited_lines) = if let Some(limit) = input.limit {
            let end_line = start_line.saturating_add(limit).min(total_file_lines);
            (
                all_lines[start_line..end_line].join("\n"),
                Some(end_line - start_line),
            )
        } else {
            (all_lines[start_line..].join("\n"), None)
        };
        let truncation = truncate_head(&selected_content, Default::default());

        let (output, details) = if truncation.first_line_exceeds_limit {
            let first_line_size = format_size(all_lines[start_line].len());
            (
                format!(
                    "[Line {start_line_display} is {first_line_size}, exceeds {} limit. Use bash: sed -n '{start_line_display}p' {} | head -c {DEFAULT_MAX_BYTES}]",
                    format_size(DEFAULT_MAX_BYTES),
                    input.path
                ),
                Some(ReadToolDetails {
                    truncation: truncation.clone(),
                }),
            )
        } else if truncation.truncated {
            let end_line_display = start_line_display + truncation.output_lines - 1;
            let next_offset = end_line_display + 1;
            let byte_notice = if truncation.truncated_by == Some(TruncatedBy::Bytes) {
                format!(" ({} limit)", format_size(DEFAULT_MAX_BYTES))
            } else {
                String::new()
            };
            (
                format!(
                    "{}\n\n[Showing lines {start_line_display}-{end_line_display} of {total_file_lines}{byte_notice}. Use offset={next_offset} to continue.]",
                    truncation.content
                ),
                Some(ReadToolDetails {
                    truncation: truncation.clone(),
                }),
            )
        } else if let Some(limited) = user_limited_lines
            && start_line + limited < total_file_lines
        {
            let remaining = total_file_lines - (start_line + limited);
            let next_offset = start_line + limited + 1;
            (
                format!(
                    "{}\n\n[{remaining} more lines in file. Use offset={next_offset} to continue.]",
                    truncation.content
                ),
                None,
            )
        } else {
            (truncation.content.clone(), None)
        };

        Ok(AgentToolResult {
            content: vec![text(output)],
            details,
            terminate: None,
        })
    }
}

pub fn create_read_tool(cwd: impl AsRef<Path>) -> AgentTool {
    create_read_tool_with_operations(cwd, ReadOperations::default())
}

pub fn create_read_tool_with_operations(
    cwd: impl AsRef<Path>,
    operations: ReadOperations,
) -> AgentTool {
    let tool = ReadTool::with_operations(cwd, operations);
    let execute: AgentToolExecuteFn = Arc::new(move |_tool_call_id, args, signal, _on_update| {
        let tool = tool.clone();
        Box::pin(async move {
            let input = ReadToolInput {
                path: args
                    .get("path")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_owned(),
                offset: args
                    .get("offset")
                    .and_then(|value| value.as_u64())
                    .and_then(|value| usize::try_from(value).ok()),
                limit: args
                    .get("limit")
                    .and_then(|value| value.as_u64())
                    .and_then(|value| usize::try_from(value).ok()),
            };
            let result = tool
                .execute_with_signal(input, signal.as_ref())
                .await
                .map_err(|error| Box::new(error) as AgentCallbackError)?;
            Ok(AgentToolResult {
                content: result.content,
                details: result
                    .details
                    .map(|details| {
                        let mut value = ToolSchema::Object(Default::default());
                        value["truncation"] = truncation_details(&details.truncation);
                        value
                    })
                    .unwrap_or(ToolSchema::Null),
                terminate: result.terminate,
            })
        }) as AgentFuture<'_, _>
    });

    AgentTool {
        tool: Tool {
            name: "read".into(),
            description: format!(
                "Read the contents of a file. Supports text files and images (jpg, png, gif, webp, bmp). Images are sent as attachments. For text files, output is truncated to {DEFAULT_MAX_LINES} lines or {}KB (whichever is hit first). Use offset/limit for large files. When you need the full file, continue with offset until complete.",
                DEFAULT_MAX_BYTES / 1024
            ),
            parameters: serde_yaml::from_str(
                r#"{"type":"object","properties":{"path":{"type":"string","description":"Path to the file to read (relative or absolute)"},"offset":{"type":"number","description":"Line number to start reading from (1-indexed)"},"limit":{"type":"number","description":"Maximum number of lines to read"}},"required":["path"]}"#,
            )
            .expect("valid read schema"),
        },
        label: "read".into(),
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

pub(crate) fn truncation_details(truncation: &TruncationResult) -> ToolSchema {
    let mut value = ToolSchema::Object(Default::default());
    value["content"] = truncation.content.clone().into();
    value["truncated"] = truncation.truncated.into();
    value["truncatedBy"] = match truncation.truncated_by {
        Some(TruncatedBy::Lines) => "lines".into(),
        Some(TruncatedBy::Bytes) => "bytes".into(),
        None => ToolSchema::Null,
    };
    value["totalLines"] = truncation.total_lines.into();
    value["totalBytes"] = truncation.total_bytes.into();
    value["outputLines"] = truncation.output_lines.into();
    value["outputBytes"] = truncation.output_bytes.into();
    value["lastLinePartial"] = truncation.last_line_partial.into();
    value["firstLineExceedsLimit"] = truncation.first_line_exceeds_limit.into();
    value["maxLines"] = truncation.max_lines.into();
    value["maxBytes"] = truncation.max_bytes.into();
    value
}

fn text(value: impl Into<String>) -> AgentToolResultContent {
    AgentToolResultContent::Text(TextContent {
        content_type: TextContentType::Text,
        text: value.into(),
        text_signature: None,
    })
}
