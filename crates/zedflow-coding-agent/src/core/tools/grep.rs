use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use zedflow_agent::types::{
    AgentCallbackError, AgentFuture, AgentTool, AgentToolExecuteFn, AgentToolResult,
    AgentToolResultContent, Tool, ToolSchema,
};
use zedflow_ai::{AbortSignal, TextContent, TextContentType};

use super::path_utils::resolve_to_cwd;
use super::read::truncation_details;
use super::truncate::{
    DEFAULT_MAX_BYTES, GREP_MAX_LINE_LENGTH, TruncationOptions, TruncationResult, format_size,
    truncate_head, truncate_line,
};
use crate::utils::tools_manager::ensure_tool;

pub const DEFAULT_GREP_LIMIT: usize = 100;

#[derive(Clone, Debug, PartialEq)]
pub struct GrepToolInput {
    pub pattern: String,
    pub path: Option<String>,
    pub glob: Option<String>,
    pub ignore_case: Option<bool>,
    pub literal: Option<bool>,
    pub context: Option<usize>,
    pub limit: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GrepToolDetails {
    pub truncation: Option<TruncationResult>,
    pub match_limit_reached: Option<f64>,
    pub lines_truncated: bool,
}

pub type GrepToolResult = AgentToolResult<Option<GrepToolDetails>>;
pub type GrepOperationFuture<T> = Pin<Box<dyn Future<Output = io::Result<T>> + Send>>;
pub type GrepIsDirectoryOperation = Arc<dyn Fn(PathBuf) -> GrepOperationFuture<bool> + Send + Sync>;
pub type GrepReadFileOperation = Arc<dyn Fn(PathBuf) -> GrepOperationFuture<String> + Send + Sync>;

#[derive(Clone)]
pub struct GrepOperations {
    pub is_directory: GrepIsDirectoryOperation,
    pub read_file: GrepReadFileOperation,
}

impl Default for GrepOperations {
    fn default() -> Self {
        Self {
            is_directory: Arc::new(|path| {
                Box::pin(async move { Ok(tokio::fs::metadata(path).await?.is_dir()) })
            }),
            read_file: Arc::new(|path| Box::pin(tokio::fs::read_to_string(path))),
        }
    }
}

impl fmt::Debug for GrepOperations {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrepOperations")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub struct GrepTool {
    cwd: PathBuf,
    operations: GrepOperations,
}

#[derive(Debug)]
struct Match {
    file_path: PathBuf,
    line_number: usize,
    line_text: Option<String>,
}

impl GrepTool {
    pub fn new(cwd: impl AsRef<Path>) -> Self {
        Self::with_operations(cwd, GrepOperations::default())
    }

    pub fn with_operations(cwd: impl AsRef<Path>, operations: GrepOperations) -> Self {
        Self {
            cwd: cwd.as_ref().to_path_buf(),
            operations,
        }
    }

    pub async fn execute(&self, input: GrepToolInput) -> io::Result<GrepToolResult> {
        self.execute_with_signal(input, None).await
    }

    async fn execute_with_signal(
        &self,
        input: GrepToolInput,
        signal: Option<&AbortSignal>,
    ) -> io::Result<GrepToolResult> {
        check_aborted(signal)?;
        let ensure_rg = ensure_tool("rg", true);
        let rg_path = if let Some(signal) = signal {
            tokio::select! {
                path = ensure_rg => path,
                () = signal.cancelled() => return Err(io::Error::other("Operation aborted")),
            }
        } else {
            ensure_rg.await
        }
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "ripgrep (rg) is not available and could not be downloaded",
            )
        })?;
        let search_path = resolve_to_cwd(input.path.as_deref().unwrap_or("."), &self.cwd)?;
        let is_directory = (self.operations.is_directory)(search_path.clone())
            .await
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Path not found: {}", search_path.display()),
                )
            })?;
        let context = input.context.filter(|value| *value > 0).unwrap_or(0);
        let limit = input.limit.unwrap_or(DEFAULT_GREP_LIMIT as f64).max(1.0);

        let mut arguments = vec![
            "--json".to_owned(),
            "--line-number".to_owned(),
            "--color=never".to_owned(),
            "--hidden".to_owned(),
        ];
        if input.ignore_case.unwrap_or(false) {
            arguments.push("--ignore-case".to_owned());
        }
        if input.literal.unwrap_or(false) {
            arguments.push("--fixed-strings".to_owned());
        }
        if let Some(glob) = input.glob {
            arguments.extend(["--glob".to_owned(), glob]);
        }
        arguments.extend([
            "--".to_owned(),
            input.pattern,
            search_path.to_string_lossy().into_owned(),
        ]);

        let mut command = Command::new(rg_path);
        command
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(|error| {
            io::Error::new(error.kind(), format!("Failed to run ripgrep: {error}"))
        })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("Failed to read ripgrep stdout"))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("Failed to read ripgrep stderr"))?;
        let stderr_task = tokio::spawn(async move {
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes).await.map(|_| bytes)
        });

        let mut stdout = BufReader::new(stdout);
        let mut line = Vec::new();
        let mut matches = Vec::new();
        let mut match_count = 0;
        let mut killed_due_to_limit = false;
        loop {
            line.clear();
            let bytes_read = if let Some(signal) = signal {
                tokio::select! {
                    result = stdout.read_until(b'\n', &mut line) => result?,
                    () = signal.cancelled() => {
                        let _ = child.start_kill();
                        let _ = child.wait().await;
                        stderr_task.abort();
                        return Err(io::Error::other("Operation aborted"));
                    },
                }
            } else {
                stdout.read_until(b'\n', &mut line).await?
            };
            if bytes_read == 0 {
                break;
            }
            let line = String::from_utf8_lossy(&line);
            let Ok(event) = serde_yaml::from_str::<ToolSchema>(&line) else {
                continue;
            };
            if event.get("type").and_then(ToolSchema::as_str) != Some("match") {
                continue;
            }
            match_count += 1;
            if let (Some(file_path), Some(line_number)) = (
                event
                    .pointer("/data/path/text")
                    .and_then(ToolSchema::as_str),
                event
                    .pointer("/data/line_number")
                    .and_then(ToolSchema::as_u64)
                    .and_then(|value| usize::try_from(value).ok()),
            ) {
                matches.push(Match {
                    file_path: PathBuf::from(file_path),
                    line_number,
                    line_text: event
                        .pointer("/data/lines/text")
                        .and_then(ToolSchema::as_str)
                        .map(str::to_owned),
                });
            }
            if match_count as f64 >= limit {
                killed_due_to_limit = true;
                let _ = child.start_kill();
                break;
            }
        }

        let status = if let Some(signal) = signal {
            tokio::select! {
                result = child.wait() => result?,
                () = signal.cancelled() => {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    stderr_task.abort();
                    return Err(io::Error::other("Operation aborted"));
                },
            }
        } else {
            child.wait().await?
        };
        let stderr = stderr_task.await.map_err(io::Error::other)??;
        check_aborted(signal)?;
        if !killed_due_to_limit && !matches!(status.code(), Some(0 | 1)) {
            let stderr = String::from_utf8_lossy(&stderr).trim().to_owned();
            return Err(io::Error::other(if stderr.is_empty() {
                format!("ripgrep exited with code {status}")
            } else {
                stderr
            }));
        }

        if match_count == 0 {
            return Ok(AgentToolResult {
                content: vec![text("No matches found")],
                details: None,
                terminate: None,
            });
        }

        let match_limit_reached = killed_due_to_limit.then_some(limit);
        let mut cache: HashMap<PathBuf, Vec<String>> = HashMap::new();
        let mut output_lines = Vec::new();
        let mut lines_truncated = false;
        for found in matches {
            let display = display_path(&found.file_path, &search_path, is_directory);
            if context == 0
                && let Some(line_text) = found.line_text
            {
                let normalized = line_text.replace("\r\n", "\n").replace('\r', "");
                let normalized = normalized.strip_suffix('\n').unwrap_or(&normalized);
                let (line, truncated) = truncate_line(&normalized, GREP_MAX_LINE_LENGTH);
                lines_truncated |= truncated;
                output_lines.push(format!("{display}:{}: {line}", found.line_number));
                continue;
            }

            let lines = if let Some(lines) = cache.get(&found.file_path) {
                lines
            } else {
                let lines = (self.operations.read_file)(found.file_path.clone())
                    .await
                    .map(|content| {
                        content
                            .replace("\r\n", "\n")
                            .replace('\r', "\n")
                            .split('\n')
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default();
                cache.insert(found.file_path.clone(), lines);
                cache.get(&found.file_path).expect("inserted grep file")
            };
            if lines.is_empty() {
                output_lines.push(format!(
                    "{display}:{}: (unable to read file)",
                    found.line_number
                ));
                continue;
            }
            let start = found.line_number.saturating_sub(context).max(1);
            let end = found.line_number.saturating_add(context).min(lines.len());
            for current in start..=end {
                let (line, truncated) = truncate_line(&lines[current - 1], GREP_MAX_LINE_LENGTH);
                lines_truncated |= truncated;
                if current == found.line_number {
                    output_lines.push(format!("{display}:{current}: {line}"));
                } else {
                    output_lines.push(format!("{display}-{current}- {line}"));
                }
            }
        }

        let raw_output = output_lines.join("\n");
        let truncation = truncate_head(
            &raw_output,
            TruncationOptions {
                max_lines: usize::MAX,
                max_bytes: DEFAULT_MAX_BYTES,
            },
        );
        let mut result_output = truncation.content.clone();
        let mut notices = Vec::new();
        if let Some(limit) = match_limit_reached {
            notices.push(format!(
                "{limit} matches limit reached. Use limit={} for more, or refine pattern",
                limit * 2.0
            ));
        }
        if truncation.truncated {
            notices.push(format!("{} limit reached", format_size(DEFAULT_MAX_BYTES)));
        }
        if lines_truncated {
            notices.push(format!(
                "Some lines truncated to {GREP_MAX_LINE_LENGTH} chars. Use read tool to see full lines"
            ));
        }
        if !notices.is_empty() {
            result_output.push_str("\n\n[");
            result_output.push_str(&notices.join(". "));
            result_output.push(']');
        }
        let details = if match_limit_reached.is_some() || truncation.truncated || lines_truncated {
            Some(GrepToolDetails {
                truncation: truncation.truncated.then_some(truncation),
                match_limit_reached,
                lines_truncated,
            })
        } else {
            None
        };

        Ok(AgentToolResult {
            content: vec![text(result_output)],
            details,
            terminate: None,
        })
    }
}

pub fn create_grep_tool(cwd: impl AsRef<Path>) -> AgentTool {
    create_grep_tool_with_operations(cwd, GrepOperations::default())
}

pub fn create_grep_tool_with_operations(
    cwd: impl AsRef<Path>,
    operations: GrepOperations,
) -> AgentTool {
    let tool = GrepTool::with_operations(cwd, operations);
    let execute: AgentToolExecuteFn = Arc::new(move |_tool_call_id, args, signal, _on_update| {
        let tool = tool.clone();
        Box::pin(async move {
            let input = GrepToolInput {
                pattern: args
                    .get("pattern")
                    .and_then(ToolSchema::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                path: args
                    .get("path")
                    .and_then(ToolSchema::as_str)
                    .map(str::to_owned),
                glob: args
                    .get("glob")
                    .and_then(ToolSchema::as_str)
                    .map(str::to_owned),
                ignore_case: args.get("ignoreCase").and_then(ToolSchema::as_bool),
                literal: args.get("literal").and_then(ToolSchema::as_bool),
                context: number(&args, "context"),
                limit: args.get("limit").and_then(ToolSchema::as_f64),
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
                    if let Some(limit) = details.match_limit_reached {
                        value["matchLimitReached"] = limit.into();
                    }
                    if details.lines_truncated {
                        value["linesTruncated"] = true.into();
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
            name: "grep".into(),
            description: format!(
                "Search file contents for a pattern. Returns matching lines with file paths and line numbers. Respects .gitignore. Output is truncated to {DEFAULT_GREP_LIMIT} matches or {}KB (whichever is hit first). Long lines are truncated to {GREP_MAX_LINE_LENGTH} chars.",
                DEFAULT_MAX_BYTES / 1024
            ),
            parameters: serde_yaml::from_str(
                r#"{"type":"object","properties":{"pattern":{"type":"string","description":"Search pattern (regex or literal string)"},"path":{"type":"string","description":"Directory or file to search (default: current directory)"},"glob":{"type":"string","description":"Filter files by glob pattern"},"ignoreCase":{"type":"boolean"},"literal":{"type":"boolean"},"context":{"type":"number"},"limit":{"type":"number"}},"required":["pattern"]}"#,
            )
            .expect("valid grep schema"),
        },
        label: "grep".into(),
        prepare_arguments: None,
        execute,
        execution_mode: None,
    }
}

fn display_path(file_path: &Path, search_path: &Path, is_directory: bool) -> String {
    if is_directory
        && let Ok(relative) = file_path.strip_prefix(search_path)
        && !relative.as_os_str().is_empty()
    {
        return relative
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
    }
    file_path
        .file_name()
        .unwrap_or(file_path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

fn number(args: &ToolSchema, key: &str) -> Option<usize> {
    args.get(key).and_then(|value| {
        value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .or_else(|| {
                value
                    .as_i64()
                    .map(|value| usize::try_from(value).unwrap_or(0))
            })
    })
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
