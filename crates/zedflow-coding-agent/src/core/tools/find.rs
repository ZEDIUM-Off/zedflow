use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use zedflow_agent::types::{AgentCallbackError, AgentTool, AgentToolResult, ToolSchema};
use zedflow_ai::Tool;

use crate::read::{
    check_aborted, object, optional_usize, required_string, schema, text_content, truncation_value,
};
use crate::truncate::{DEFAULT_MAX_BYTES, TruncationOptions, format_size, truncate_head};

const DEFAULT_LIMIT: usize = 1_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FindToolInput {
    pub pattern: String,
    pub path: Option<String>,
    pub limit: Option<usize>,
}

pub fn create_find_tool(cwd: impl Into<PathBuf>) -> AgentTool {
    let cwd = cwd.into();
    AgentTool {
        tool: Tool {
            name: "find".into(),
            description: format!(
                "Search for files by glob pattern. Returns matching file paths relative to the search directory. Respects .gitignore. Output is truncated to {DEFAULT_LIMIT} results or {}KB (whichever is hit first).",
                DEFAULT_MAX_BYTES / 1024
            ),
            parameters: schema(
                r#"{
                    "type": "object",
                    "properties": {
                        "pattern": { "type": "string", "description": "Glob pattern to match files, e.g. '*.ts' or 'src/**/*.rs'" },
                        "path": { "type": "string", "description": "Directory to search in (default: current directory)" },
                        "limit": { "type": "number", "description": "Maximum number of results (default: 1000)" }
                    },
                    "required": ["pattern"]
                }"#,
            ),
        },
        label: "find".into(),
        prepare_arguments: None,
        execution_mode: None,
        execute: Arc::new(move |_tool_call_id, arguments, signal, _on_update| {
            let cwd = cwd.clone();
            Box::pin(async move {
                let input = FindToolInput {
                    pattern: required_string(&arguments, "pattern")?,
                    path: arguments
                        .get("path")
                        .and_then(ToolSchema::as_str)
                        .map(str::to_owned),
                    limit: optional_usize(&arguments, "limit")?,
                };
                execute_find(&cwd, input, signal).await
            })
        }),
    }
}

pub async fn execute_find(
    cwd: &Path,
    input: FindToolInput,
    signal: Option<zedflow_ai::AbortSignal>,
) -> Result<AgentToolResult, AgentCallbackError> {
    check_aborted(signal.as_ref())?;
    let search_path = crate::path_utils::resolve_to_cwd(input.path.as_deref().unwrap_or("."), cwd)?;
    if !search_path.try_exists()? {
        return Err(format!("Path not found: {}", search_path.display()).into());
    }

    let limit = input.limit.unwrap_or(DEFAULT_LIMIT);
    let mut arguments = vec!["--glob", "--color=never", "--hidden"];
    if !inside_git_repository(&search_path) {
        arguments.push("--no-require-git");
    }
    arguments.push("--max-results");
    let limit_argument = limit.to_string();
    arguments.push(&limit_argument);

    let mut effective_pattern = input.pattern.clone();
    if input.pattern.contains('/') {
        arguments.push("--full-path");
        if !input.pattern.starts_with('/')
            && !input.pattern.starts_with("**/")
            && input.pattern != "**"
        {
            effective_pattern = format!("**/{}", input.pattern);
        }
    }
    arguments.extend(["--", &effective_pattern]);
    let search_argument = search_path.to_string_lossy();
    arguments.push(&search_argument);

    let output = Command::new("fd")
        .args(arguments)
        .output()
        .map_err(|error| format!("fd is not available and could not be executed: {error}"))?;
    check_aborted(signal.as_ref())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() && stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_owned().into());
    }

    let mut results = stdout
        .lines()
        .filter_map(|line| {
            let line = line.trim_end_matches('\r').trim();
            if line.is_empty() {
                return None;
            }
            let trailing_slash = line.ends_with(['/', '\\']);
            let path = Path::new(line);
            let relative = path.strip_prefix(&search_path).unwrap_or(path);
            let mut relative = relative
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");
            if trailing_slash && !relative.ends_with('/') {
                relative.push('/');
            }
            Some(relative)
        })
        .collect::<Vec<_>>();
    results.sort();

    if results.is_empty() {
        return Ok(AgentToolResult {
            content: vec![text_content("No files found matching pattern")],
            details: ToolSchema::Null,
            terminate: None,
        });
    }

    let result_limit_reached = results.len() >= limit;
    let truncation = truncate_head(
        &results.join("\n"),
        TruncationOptions {
            max_lines: usize::MAX,
            max_bytes: DEFAULT_MAX_BYTES,
        },
    );
    let mut result_output = truncation.content.clone();
    let mut details = Vec::new();
    let mut notices = Vec::new();
    if result_limit_reached {
        notices.push(format!(
            "{limit} results limit reached. Use limit={} for more, or refine pattern",
            limit.saturating_mul(2)
        ));
        details.push(("resultLimitReached", limit.into()));
    }
    if truncation.truncated {
        notices.push(format!("{} limit reached", format_size(DEFAULT_MAX_BYTES)));
        details.push(("truncation", truncation_value(&truncation)));
    }
    if !notices.is_empty() {
        result_output.push_str(&format!("\n\n[{}]", notices.join(". ")));
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
        content: vec![text_content(result_output)],
        details,
        terminate: None,
    })
}

fn inside_git_repository(search_path: &Path) -> bool {
    search_path
        .ancestors()
        .any(|directory| directory.join(".git").try_exists().unwrap_or(false))
}
