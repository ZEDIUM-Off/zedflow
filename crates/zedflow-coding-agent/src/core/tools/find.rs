use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::process::Command;
use zedflow_agent::types::{
    AgentCallbackError, AgentFuture, AgentTool, AgentToolExecuteFn, AgentToolResult,
    AgentToolResultContent, Tool, ToolSchema,
};
use zedflow_ai::{AbortSignal, TextContent, TextContentType};

use super::path_utils::{path_exists_async, resolve_to_cwd};
use super::read::truncation_details;
use super::truncate::{
    DEFAULT_MAX_BYTES, TruncationOptions, TruncationResult, format_size, truncate_head,
};
use crate::utils::tools_manager::ensure_tool;

pub const DEFAULT_FIND_LIMIT: usize = 1_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FindToolInput {
    pub pattern: String,
    pub path: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FindToolDetails {
    pub truncation: Option<TruncationResult>,
    pub result_limit_reached: Option<usize>,
}

pub type FindToolResult = AgentToolResult<Option<FindToolDetails>>;

#[derive(Clone, Debug)]
pub struct FindTool {
    cwd: PathBuf,
}

impl FindTool {
    pub fn new(cwd: impl AsRef<Path>) -> Self {
        Self {
            cwd: cwd.as_ref().to_path_buf(),
        }
    }

    pub async fn execute(&self, input: FindToolInput) -> io::Result<FindToolResult> {
        self.execute_with_signal(input, None).await
    }

    async fn execute_with_signal(
        &self,
        input: FindToolInput,
        signal: Option<&AbortSignal>,
    ) -> io::Result<FindToolResult> {
        check_aborted(signal)?;
        let search_path = resolve_to_cwd(input.path.as_deref().unwrap_or("."), &self.cwd)?;
        let effective_limit = input.limit.unwrap_or(DEFAULT_FIND_LIMIT);
        let ensure_fd = ensure_tool("fd", true);
        let fd_path = if let Some(signal) = signal {
            tokio::select! {
                path = ensure_fd => path,
                () = signal.cancelled() => return Err(io::Error::other("Operation aborted")),
            }
        } else {
            ensure_fd.await
        }
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "fd is not available and could not be downloaded",
            )
        })?;

        let mut inside_git_repo = false;
        let mut current = Some(search_path.as_path());
        while let Some(directory) = current {
            if path_exists_async(directory.join(".git")).await {
                inside_git_repo = true;
                break;
            }
            current = directory.parent();
        }

        let mut arguments = vec![
            "--glob".to_owned(),
            "--color=never".to_owned(),
            "--hidden".to_owned(),
        ];
        if !inside_git_repo {
            arguments.push("--no-require-git".to_owned());
        }
        arguments.extend(["--max-results".to_owned(), effective_limit.to_string()]);

        let mut effective_pattern = input.pattern.clone();
        if input.pattern.contains('/') {
            arguments.push("--full-path".to_owned());
            if !input.pattern.starts_with('/')
                && !input.pattern.starts_with("**/")
                && input.pattern != "**"
            {
                effective_pattern = format!("**/{}", input.pattern);
            }
        }
        arguments.extend([
            "--".to_owned(),
            effective_pattern,
            search_path.to_string_lossy().into_owned(),
        ]);

        let mut command = Command::new(fd_path);
        command.args(arguments).kill_on_drop(true);
        let output = if let Some(signal) = signal {
            tokio::select! {
                result = command.output() => result?,
                () = signal.cancelled() => return Err(io::Error::other("Operation aborted")),
            }
        } else {
            command.output().await?
        };
        check_aborted(signal)?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if !output.status.success() && stdout.is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(io::Error::other(if stderr.is_empty() {
                format!("fd exited with code {}", output.status)
            } else {
                stderr
            }));
        }

        let mut paths = Vec::new();
        for raw_line in stdout.lines() {
            let line = raw_line.trim_end_matches('\r').trim();
            if line.is_empty() {
                continue;
            }
            let trailing_slash = line.ends_with('/') || line.ends_with('\\');
            let path = Path::new(line);
            let relative = path.strip_prefix(&search_path).unwrap_or(path);
            let mut display = relative
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");
            if trailing_slash && !display.ends_with('/') {
                display.push('/');
            }
            paths.push(display);
        }

        if paths.is_empty() {
            return Ok(AgentToolResult {
                content: vec![text("No files found matching pattern")],
                details: None,
                terminate: None,
            });
        }

        let result_limit_reached = (paths.len() >= effective_limit).then_some(effective_limit);
        let raw_output = paths.join("\n");
        let truncation = truncate_head(
            &raw_output,
            TruncationOptions {
                max_lines: usize::MAX,
                max_bytes: DEFAULT_MAX_BYTES,
            },
        );
        let mut result_output = truncation.content.clone();
        let mut notices = Vec::new();
        if let Some(limit) = result_limit_reached {
            notices.push(format!(
                "{limit} results limit reached. Use limit={} for more, or refine pattern",
                limit.saturating_mul(2)
            ));
        }
        if truncation.truncated {
            notices.push(format!("{} limit reached", format_size(DEFAULT_MAX_BYTES)));
        }
        if !notices.is_empty() {
            result_output.push_str("\n\n[");
            result_output.push_str(&notices.join(". "));
            result_output.push(']');
        }
        let details = if result_limit_reached.is_some() || truncation.truncated {
            Some(FindToolDetails {
                truncation: truncation.truncated.then_some(truncation),
                result_limit_reached,
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

pub fn create_find_tool(cwd: impl AsRef<Path>) -> AgentTool {
    let tool = FindTool::new(cwd);
    let execute: AgentToolExecuteFn = Arc::new(move |_tool_call_id, args, signal, _on_update| {
        let tool = tool.clone();
        Box::pin(async move {
            let input = FindToolInput {
                pattern: args
                    .get("pattern")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_owned(),
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
                    if let Some(limit) = details.result_limit_reached {
                        value["resultLimitReached"] = limit.into();
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
            name: "find".into(),
            description: format!(
                "Search for files by glob pattern. Returns matching file paths relative to the search directory. Respects .gitignore. Output is truncated to {DEFAULT_FIND_LIMIT} results or {}KB (whichever is hit first).",
                DEFAULT_MAX_BYTES / 1024
            ),
            parameters: serde_yaml::from_str(
                r#"{"type":"object","properties":{"pattern":{"type":"string"},"path":{"type":"string","description":"Directory to search in (default: current directory)"},"limit":{"type":"number","description":"Maximum number of results (default: 1000)"}},"required":["pattern"]}"#,
            )
            .expect("valid find schema"),
        },
        label: "find".into(),
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
