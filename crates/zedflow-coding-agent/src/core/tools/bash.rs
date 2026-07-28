//! Bash tool and pluggable local command execution backend.

use std::{
    future::Future,
    io,
    path::{Path, PathBuf},
    pin::Pin,
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
    sync::mpsc,
    task::JoinHandle,
    time::Instant,
};
use zedflow_agent::types::{
    AgentCallbackError, AgentFuture, AgentTool, AgentToolExecuteFn, AgentToolResult,
    AgentToolResultContent, Tool, ToolSchema,
};
use zedflow_ai::{AbortSignal, TextContent, TextContentType};

use crate::{
    output_accumulator::{
        OutputAccumulator, OutputAccumulatorOptions, OutputSnapshot, OutputSnapshotOptions,
    },
    read::truncation_details,
    truncate::{DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, TruncatedBy, format_size},
    utils::shell::{
        get_shell_config, kill_process_tree, track_detached_child_pid, untrack_detached_child_pid,
    },
};

const MAX_TIMEOUT_MS: f64 = 2_147_483_647.0;

pub type BashOperationFuture =
    Pin<Box<dyn Future<Output = io::Result<Option<i32>>> + Send + 'static>>;
pub type BashDataCallback = Arc<dyn Fn(&[u8]) + Send + Sync>;

#[derive(Clone)]
pub struct BashOperationOptions {
    pub on_data: BashDataCallback,
    pub signal: Option<AbortSignal>,
    pub timeout: Option<f64>,
    pub env: Option<Vec<(String, String)>>,
}

pub trait BashOperations: Send + Sync {
    fn exec(
        &self,
        command: String,
        cwd: PathBuf,
        options: BashOperationOptions,
    ) -> BashOperationFuture;
}

#[derive(Clone, Debug, Default)]
pub struct LocalBashOperations {
    shell_path: Option<PathBuf>,
}

pub fn create_local_bash_operations(shell_path: Option<PathBuf>) -> LocalBashOperations {
    LocalBashOperations { shell_path }
}

impl BashOperations for LocalBashOperations {
    fn exec(
        &self,
        command: String,
        cwd: PathBuf,
        options: BashOperationOptions,
    ) -> BashOperationFuture {
        let shell_path = self.shell_path.clone();
        Box::pin(async move { execute_local(command, cwd, shell_path.as_deref(), options).await })
    }
}

#[derive(Clone)]
pub struct BashSpawnContext {
    pub command: String,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
}

pub type BashSpawnHook = Arc<dyn Fn(BashSpawnContext) -> BashSpawnContext + Send + Sync>;

#[derive(Clone, Default)]
pub struct BashToolOptions {
    pub operations: Option<Arc<dyn BashOperations>>,
    pub command_prefix: Option<String>,
    pub shell_path: Option<PathBuf>,
    pub spawn_hook: Option<BashSpawnHook>,
}

pub fn create_bash_tool(cwd: impl AsRef<Path>) -> AgentTool {
    create_bash_tool_with_options(cwd, BashToolOptions::default())
}

pub fn create_bash_tool_with_options(cwd: impl AsRef<Path>, options: BashToolOptions) -> AgentTool {
    let cwd = cwd.as_ref().to_path_buf();
    let operations: Arc<dyn BashOperations> = options
        .operations
        .unwrap_or_else(|| Arc::new(create_local_bash_operations(options.shell_path.clone())));
    let command_prefix = options.command_prefix;
    let spawn_hook = options.spawn_hook;
    let execute: AgentToolExecuteFn = Arc::new(move |_tool_call_id, args, signal, on_update| {
        let cwd = cwd.clone();
        let operations = operations.clone();
        let command_prefix = command_prefix.clone();
        let spawn_hook = spawn_hook.clone();
        Box::pin(async move {
            let command = args
                .get("command")
                .and_then(ToolSchema::as_str)
                .unwrap_or_default();
            let timeout = args.get("timeout").and_then(ToolSchema::as_f64);
            resolve_timeout(timeout).map_err(|error| Box::new(error) as AgentCallbackError)?;
            let command = command_prefix.map_or_else(
                || command.to_owned(),
                |prefix| format!("{prefix}\n{command}"),
            );
            let context = BashSpawnContext {
                command,
                cwd,
                env: std::env::vars().collect(),
            };
            let context = spawn_hook.map_or(context.clone(), |hook| hook(context));
            execute_tool(operations.as_ref(), context, timeout, signal, on_update)
                .await
                .map_err(|error| Box::new(error) as AgentCallbackError)
        }) as AgentFuture<'_, _>
    });

    AgentTool {
        tool: Tool {
            name: "bash".into(),
            description: format!(
                "Execute a bash command in the current working directory. Returns stdout and stderr. Output is truncated to last {DEFAULT_MAX_LINES} lines or {}KB (whichever is hit first). If truncated, full output is saved to a temp file. Optionally provide a timeout in seconds.",
                DEFAULT_MAX_BYTES / 1024
            ),
            parameters: serde_yaml::from_str(
                r#"{"type":"object","properties":{"command":{"type":"string","description":"Bash command to execute"},"timeout":{"type":"number","description":"Timeout in seconds (optional, no default timeout)"}},"required":["command"]}"#,
            )
            .expect("valid bash schema"),
        },
        label: "bash".into(),
        prepare_arguments: None,
        execute,
        execution_mode: None,
    }
}

async fn execute_tool(
    operations: &dyn BashOperations,
    context: BashSpawnContext,
    timeout: Option<f64>,
    signal: Option<AbortSignal>,
    on_update: Option<zedflow_agent::types::AgentToolUpdateCallback>,
) -> io::Result<AgentToolResult> {
    let output = Arc::new(Mutex::new(OutputAccumulator::new(
        OutputAccumulatorOptions {
            temp_file_prefix: "pi-bash".into(),
            ..Default::default()
        },
    )));
    let output_error = Arc::new(Mutex::new(None::<String>));
    if let Some(update) = &on_update {
        update(result("", ToolSchema::Null));
    }
    let callback_output = output.clone();
    let callback_error = output_error.clone();
    let callback_update = on_update.clone();
    let on_data = Arc::new(move |data: &[u8]| {
        let mut output = callback_output.lock().unwrap();
        if let Err(error) = output.append(data) {
            *callback_error.lock().unwrap() = Some(error.to_string());
            return;
        }
        if let Some(update) = &callback_update
            && let Ok(snapshot) = output.snapshot(OutputSnapshotOptions {
                persist_if_truncated: true,
            })
        {
            update(snapshot_result(&snapshot));
        }
    });

    let execution = operations
        .exec(
            context.command,
            context.cwd,
            BashOperationOptions {
                on_data,
                signal: signal.clone(),
                timeout,
                env: Some(context.env),
            },
        )
        .await;
    if let Some(error) = output_error.lock().unwrap().take() {
        return Err(io::Error::other(error));
    }
    let snapshot = {
        let mut output = output.lock().unwrap();
        output.finish()?;
        output.snapshot(OutputSnapshotOptions {
            persist_if_truncated: true,
        })?
    };
    let (text, details) = format_output(&snapshot);

    match execution {
        Ok(Some(0) | None) => Ok(result(&text, details)),
        Ok(Some(code)) => Err(io::Error::other(append_status(
            &text,
            &format!("Command exited with code {code}"),
        ))),
        Err(error)
            if error.to_string() == "aborted"
                || signal.as_ref().is_some_and(AbortSignal::aborted) =>
        {
            Err(io::Error::other(append_status(&text, "Command aborted")))
        }
        Err(error) if error.to_string().starts_with("timeout:") => {
            let message = error.to_string();
            let seconds = message.split_once(':').map_or("", |(_, value)| value);
            Err(io::Error::other(append_status(
                &text,
                &format!("Command timed out after {seconds} seconds"),
            )))
        }
        Err(error) => Err(error),
    }
}

fn snapshot_result(snapshot: &OutputSnapshot) -> AgentToolResult {
    let details = if snapshot.truncation.truncated {
        details(snapshot)
    } else {
        ToolSchema::Null
    };
    result(&snapshot.content, details)
}

fn format_output(snapshot: &OutputSnapshot) -> (String, ToolSchema) {
    let mut text = if snapshot.content.is_empty() {
        "(no output)".to_owned()
    } else {
        snapshot.content.clone()
    };
    if !snapshot.truncation.truncated {
        return (text, ToolSchema::Null);
    }
    let path = snapshot
        .full_output_path
        .as_ref()
        .map_or_else(String::new, |path| path.display().to_string());
    let truncation = &snapshot.truncation;
    let start = truncation
        .total_lines
        .saturating_sub(truncation.output_lines)
        + 1;
    let end = truncation.total_lines;
    let notice = if truncation.last_line_partial {
        format!(
            "Showing last {} of line {end}. Full output: {path}",
            format_size(truncation.output_bytes)
        )
    } else if truncation.truncated_by == Some(TruncatedBy::Lines) {
        format!(
            "Showing lines {start}-{end} of {}. Full output: {path}",
            truncation.total_lines
        )
    } else {
        format!(
            "Showing lines {start}-{end} of {} ({} limit). Full output: {path}",
            truncation.total_lines,
            format_size(DEFAULT_MAX_BYTES)
        )
    };
    text.push_str(&format!("\n\n[{notice}]"));
    (text, details(snapshot))
}

fn details(snapshot: &OutputSnapshot) -> ToolSchema {
    let mut value = ToolSchema::Object(Default::default());
    value["truncation"] = truncation_details(&snapshot.truncation);
    value["fullOutputPath"] = snapshot
        .full_output_path
        .as_ref()
        .map_or(ToolSchema::Null, |path| path.display().to_string().into());
    value
}

fn result(text: &str, details: ToolSchema) -> AgentToolResult {
    AgentToolResult {
        content: vec![AgentToolResultContent::Text(TextContent {
            content_type: TextContentType::Text,
            text: text.to_owned(),
            text_signature: None,
        })],
        details,
        terminate: None,
    }
}

fn append_status(text: &str, status: &str) -> String {
    if text.is_empty() {
        status.to_owned()
    } else {
        format!("{text}\n\n{status}")
    }
}

fn resolve_timeout(timeout: Option<f64>) -> io::Result<Option<Duration>> {
    let Some(seconds) = timeout else {
        return Ok(None);
    };
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid timeout: must be a finite number of seconds",
        ));
    }
    let milliseconds = seconds * 1000.0;
    if milliseconds > MAX_TIMEOUT_MS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Invalid timeout: maximum is {} seconds",
                MAX_TIMEOUT_MS / 1000.0
            ),
        ));
    }
    Ok(Some(Duration::from_secs_f64(seconds)))
}

async fn execute_local(
    command: String,
    cwd: PathBuf,
    shell_path: Option<&Path>,
    options: BashOperationOptions,
) -> io::Result<Option<i32>> {
    let timeout = resolve_timeout(options.timeout)?;
    if options.signal.as_ref().is_some_and(AbortSignal::aborted) {
        return Err(io::Error::other("aborted"));
    }
    if !cwd.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Working directory does not exist: {}\nCannot execute bash commands.",
                cwd.display()
            ),
        ));
    }
    let shell = get_shell_config(shell_path).map_err(io::Error::other)?;
    let from_stdin = shell.command_transport_stdin;
    let mut process = Command::new(&shell.shell);
    process
        .args(&shell.args)
        .args((!from_stdin).then_some(command.as_str()))
        .current_dir(cwd)
        .env_clear()
        .envs(options.env.unwrap_or_else(|| std::env::vars().collect()))
        .stdin(if from_stdin {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = process.spawn()?;
    if from_stdin {
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(command.as_bytes()).await?;
        }
    }
    let pid = child.id();
    if let Some(pid) = pid {
        track_detached_child_pid(pid);
    }
    let (tx, mut rx) = mpsc::unbounded_channel();
    let stdout_task = pipe_chunks(child.stdout.take().expect("piped stdout"), tx.clone());
    let stderr_task = pipe_chunks(child.stderr.take().expect("piped stderr"), tx);
    let deadline = timeout.map(|value| Instant::now() + value);
    let mut failure = None;
    let status = loop {
        while let Ok(data) = rx.try_recv() {
            (options.on_data)(&data);
        }
        if options.signal.as_ref().is_some_and(AbortSignal::aborted) {
            failure = Some(io::Error::other("aborted"));
        } else if deadline.is_some_and(|value| Instant::now() >= value) {
            failure = Some(io::Error::other(format!(
                "timeout:{}",
                options.timeout.unwrap()
            )));
        }
        if failure.is_some() {
            if let Some(pid) = pid {
                kill_process_tree(pid);
            }
            let _ = child.kill().await;
            break child.wait().await?;
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        tokio::select! {
            Some(data) = rx.recv() => (options.on_data)(&data),
            () = tokio::time::sleep(Duration::from_millis(10)) => {}
        }
    };
    tokio::task::yield_now().await;
    while let Ok(data) = rx.try_recv() {
        (options.on_data)(&data);
    }
    stdout_task.abort();
    stderr_task.abort();
    if let Some(pid) = pid {
        untrack_detached_child_pid(pid);
    }
    if let Some(error) = failure {
        return Err(error);
    }
    Ok(status.code())
}

fn pipe_chunks(
    mut reader: impl AsyncRead + Unpin + Send + 'static,
    tx: mpsc::UnboundedSender<Vec<u8>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut buffer = vec![0; 8192];
        loop {
            match reader.read(&mut buffer).await {
                Ok(0) | Err(_) => break,
                Ok(length) => {
                    if tx.send(buffer[..length].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_timeout_before_spawn() {
        assert!(resolve_timeout(Some(0.0)).is_err());
        assert!(resolve_timeout(Some(f64::INFINITY)).is_err());
        assert!(resolve_timeout(Some(MAX_TIMEOUT_MS / 1000.0 + 1.0)).is_err());
    }
}
