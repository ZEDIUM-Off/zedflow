use std::io;
use std::path::{Path, PathBuf};
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
use crate::utils::mime::detect_supported_image_mime_type;

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

#[derive(Clone, Debug)]
pub struct ReadTool {
    cwd: PathBuf,
}

impl ReadTool {
    pub fn new(cwd: impl AsRef<Path>) -> Self {
        Self {
            cwd: cwd.as_ref().to_path_buf(),
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
        let bytes = tokio::fs::read(absolute_path).await?;
        check_aborted(signal)?;

        if let Some(mime_type) = detect_supported_image_mime_type(&bytes) {
            return Ok(AgentToolResult {
                content: vec![
                    text(format!("Read image file [{mime_type}]")),
                    AgentToolResultContent::Image(ImageContent {
                        content_type: ImageContentType::Image,
                        data: encode_base64(&bytes),
                        mime_type: mime_type.to_owned(),
                    }),
                ],
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
    let tool = ReadTool::new(cwd);
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

fn encode_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        encoded.push(ALPHABET[((value >> 18) & 63) as usize] as char);
        encoded.push(ALPHABET[((value >> 12) & 63) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            ALPHABET[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            ALPHABET[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

fn text(value: impl Into<String>) -> AgentToolResultContent {
    AgentToolResultContent::Text(TextContent {
        content_type: TextContentType::Text,
        text: value.into(),
        text_signature: None,
    })
}
