use std::cmp::Ordering;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use zedflow_agent::types::{
    AgentCallbackError, AgentFuture, AgentTool, AgentToolExecuteFn, AgentToolResult,
    AgentToolResultContent, Tool, ToolSchema,
};
use zedflow_ai::{AbortSignal, TextContent, TextContentType};

use super::path_utils::resolve_to_cwd;
use super::read::truncation_details;
use super::truncate::{
    DEFAULT_MAX_BYTES, TruncationOptions, TruncationResult, format_size, truncate_head,
};

pub const DEFAULT_LS_LIMIT: usize = 500;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LsToolInput {
    pub path: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LsToolDetails {
    pub truncation: Option<TruncationResult>,
    pub entry_limit_reached: Option<usize>,
}

pub type LsToolResult = AgentToolResult<Option<LsToolDetails>>;

#[derive(Clone, Debug)]
pub struct LsTool {
    cwd: PathBuf,
}

impl LsTool {
    pub fn new(cwd: impl AsRef<Path>) -> Self {
        Self {
            cwd: cwd.as_ref().to_path_buf(),
        }
    }

    pub async fn execute(&self, input: LsToolInput) -> io::Result<LsToolResult> {
        self.execute_with_signal(input, None).await
    }

    async fn execute_with_signal(
        &self,
        input: LsToolInput,
        signal: Option<&AbortSignal>,
    ) -> io::Result<LsToolResult> {
        check_aborted(signal)?;
        let directory = resolve_to_cwd(input.path.as_deref().unwrap_or("."), &self.cwd)?;
        let effective_limit = input.limit.unwrap_or(DEFAULT_LS_LIMIT);
        let metadata = tokio::fs::metadata(&directory).await.map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Path not found: {}", directory.display()),
                )
            } else {
                error
            }
        })?;
        if !metadata.is_dir() {
            return Err(io::Error::other(format!(
                "Not a directory: {}",
                directory.display()
            )));
        }

        let mut reader = tokio::fs::read_dir(&directory).await.map_err(|error| {
            io::Error::new(error.kind(), format!("Cannot read directory: {error}"))
        })?;
        let mut entries = Vec::new();
        while let Some(entry) = reader.next_entry().await? {
            entries.push(entry.file_name().to_string_lossy().into_owned());
        }
        entries.sort_by(|left, right| {
            let order = left.to_lowercase().cmp(&right.to_lowercase());
            if order == Ordering::Equal {
                left.cmp(right)
            } else {
                order
            }
        });

        let mut results = Vec::new();
        let mut entry_limit_reached = None;
        for entry in entries {
            check_aborted(signal)?;
            if results.len() >= effective_limit {
                entry_limit_reached = Some(effective_limit);
                break;
            }
            let full_path = directory.join(&entry);
            let Ok(metadata) = tokio::fs::metadata(full_path).await else {
                continue;
            };
            results.push(if metadata.is_dir() {
                format!("{entry}/")
            } else {
                entry
            });
        }

        if results.is_empty() {
            return Ok(AgentToolResult {
                content: vec![text("(empty directory)")],
                details: None,
                terminate: None,
            });
        }

        let raw_output = results.join("\n");
        let truncation = truncate_head(
            &raw_output,
            TruncationOptions {
                max_lines: usize::MAX,
                max_bytes: DEFAULT_MAX_BYTES,
            },
        );
        let mut output = truncation.content.clone();
        let mut notices = Vec::new();
        if let Some(limit) = entry_limit_reached {
            notices.push(format!(
                "{limit} entries limit reached. Use limit={} for more",
                limit.saturating_mul(2)
            ));
        }
        if truncation.truncated {
            notices.push(format!("{} limit reached", format_size(DEFAULT_MAX_BYTES)));
        }
        if !notices.is_empty() {
            output.push_str("\n\n[");
            output.push_str(&notices.join(". "));
            output.push(']');
        }
        let details = if entry_limit_reached.is_some() || truncation.truncated {
            Some(LsToolDetails {
                truncation: truncation.truncated.then_some(truncation),
                entry_limit_reached,
            })
        } else {
            None
        };

        Ok(AgentToolResult {
            content: vec![text(output)],
            details,
            terminate: None,
        })
    }
}

pub fn create_ls_tool(cwd: impl AsRef<Path>) -> AgentTool {
    let tool = LsTool::new(cwd);
    let execute: AgentToolExecuteFn = Arc::new(move |_tool_call_id, args, signal, _on_update| {
        let tool = tool.clone();
        Box::pin(async move {
            let input = LsToolInput {
                path: args
                    .get("path")
                    .and_then(|value| value.as_str())
                    .map(str::to_owned),
                limit: args
                    .get("limit")
                    .and_then(|value| value.as_u64())
                    .and_then(|value| usize::try_from(value).ok()),
            };
            let result = tool
                .execute_with_signal(input, signal.as_ref())
                .await
                .map_err(|error| Box::new(error) as AgentCallbackError)?;
            let details = result
                .details
                .map(|details| {
                    let mut value = ToolSchema::Object(Default::default());
                    if let Some(truncation) = details.truncation {
                        value["truncation"] = truncation_details(&truncation);
                    }
                    if let Some(limit) = details.entry_limit_reached {
                        value["entryLimitReached"] = limit.into();
                    }
                    value
                })
                .unwrap_or(ToolSchema::Null);
            Ok(AgentToolResult {
                content: result.content,
                details,
                terminate: result.terminate,
            })
        }) as AgentFuture<'_, _>
    });

    AgentTool {
        tool: Tool {
            name: "ls".into(),
            description: format!(
                "List directory contents. Returns entries sorted alphabetically, with '/' suffix for directories. Includes dotfiles. Output is truncated to {DEFAULT_LS_LIMIT} entries or {}KB (whichever is hit first).",
                DEFAULT_MAX_BYTES / 1024
            ),
            parameters: serde_yaml::from_str(
                r#"{"type":"object","properties":{"path":{"type":"string","description":"Directory to list (default: current directory)"},"limit":{"type":"number","description":"Maximum number of entries to return (default: 500)"}}}"#,
            )
            .expect("valid ls schema"),
        },
        label: "ls".into(),
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
