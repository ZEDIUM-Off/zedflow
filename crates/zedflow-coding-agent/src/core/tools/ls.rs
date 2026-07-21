use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use zedflow_agent::types::{AgentCallbackError, AgentTool, AgentToolResult, ToolSchema};
use zedflow_ai::Tool;

use crate::read::{check_aborted, object, optional_usize, schema, text_content, truncation_value};
use crate::truncate::{DEFAULT_MAX_BYTES, TruncationOptions, format_size, truncate_head};

const DEFAULT_LIMIT: usize = 500;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LsToolInput {
    pub path: Option<String>,
    pub limit: Option<usize>,
}

pub fn create_ls_tool(cwd: impl Into<PathBuf>) -> AgentTool {
    let cwd = cwd.into();
    AgentTool {
        tool: Tool {
            name: "ls".into(),
            description: format!(
                "List directory contents. Returns entries sorted alphabetically, with '/' suffix for directories. Includes dotfiles. Output is truncated to {DEFAULT_LIMIT} entries or {}KB (whichever is hit first).",
                DEFAULT_MAX_BYTES / 1024
            ),
            parameters: schema(
                r#"{
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Directory to list (default: current directory)" },
                        "limit": { "type": "number", "description": "Maximum number of entries to return (default: 500)" }
                    }
                }"#,
            ),
        },
        label: "ls".into(),
        prepare_arguments: None,
        execution_mode: None,
        execute: Arc::new(move |_tool_call_id, arguments, signal, _on_update| {
            let cwd = cwd.clone();
            Box::pin(async move {
                let input = LsToolInput {
                    path: arguments
                        .get("path")
                        .and_then(ToolSchema::as_str)
                        .map(str::to_owned),
                    limit: optional_usize(&arguments, "limit")?,
                };
                execute_ls(&cwd, input, signal).await
            })
        }),
    }
}

pub async fn execute_ls(
    cwd: &Path,
    input: LsToolInput,
    signal: Option<zedflow_ai::AbortSignal>,
) -> Result<AgentToolResult, AgentCallbackError> {
    check_aborted(signal.as_ref())?;
    let dir_path = crate::path_utils::resolve_to_cwd(input.path.as_deref().unwrap_or("."), cwd)?;
    let metadata = match tokio::fs::metadata(&dir_path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!("Path not found: {}", dir_path.display()).into());
        }
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_dir() {
        return Err(format!("Not a directory: {}", dir_path.display()).into());
    }

    let mut reader = tokio::fs::read_dir(&dir_path)
        .await
        .map_err(|error| format!("Cannot read directory: {error}"))?;
    let mut entries = Vec::new();
    while let Some(entry) = reader
        .next_entry()
        .await
        .map_err(|error| format!("Cannot read directory: {error}"))?
    {
        entries.push(entry.file_name().to_string_lossy().into_owned());
    }
    entries.sort_by(|left, right| {
        let folded = left.to_lowercase().cmp(&right.to_lowercase());
        if folded == Ordering::Equal {
            left.cmp(right)
        } else {
            folded
        }
    });

    let limit = input.limit.unwrap_or(DEFAULT_LIMIT);
    let mut results = Vec::new();
    let mut entry_limit_reached = false;
    for entry in entries {
        if results.len() >= limit {
            entry_limit_reached = true;
            break;
        }
        let path = dir_path.join(&entry);
        let Ok(metadata) = tokio::fs::metadata(path).await else {
            continue;
        };
        results.push(if metadata.is_dir() {
            format!("{entry}/")
        } else {
            entry
        });
    }
    check_aborted(signal.as_ref())?;

    if results.is_empty() {
        return Ok(AgentToolResult {
            content: vec![text_content("(empty directory)")],
            details: ToolSchema::Null,
            terminate: None,
        });
    }

    let truncation = truncate_head(
        &results.join("\n"),
        TruncationOptions {
            max_lines: usize::MAX,
            max_bytes: DEFAULT_MAX_BYTES,
        },
    );
    let mut output = truncation.content.clone();
    let mut details = Vec::new();
    let mut notices = Vec::new();
    if entry_limit_reached {
        notices.push(format!(
            "{limit} entries limit reached. Use limit={} for more",
            limit.saturating_mul(2)
        ));
        details.push(("entryLimitReached", limit.into()));
    }
    if truncation.truncated {
        notices.push(format!("{} limit reached", format_size(DEFAULT_MAX_BYTES)));
        details.push(("truncation", truncation_value(&truncation)));
    }
    if !notices.is_empty() {
        output.push_str(&format!("\n\n[{}]", notices.join(". ")));
    }

    let details = match details.as_slice() {
        [] => ToolSchema::Null,
        [(key, value)] => object([(*key, value.clone())]),
        [(first_key, first), (second_key, second)] => {
            object([(*first_key, first.clone()), (*second_key, second.clone())])
        }
        _ => unreachable!(),
    };
    Ok(AgentToolResult {
        content: vec![text_content(output)],
        details,
        terminate: None,
    })
}
