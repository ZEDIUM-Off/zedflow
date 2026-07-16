use std::sync::{Arc, Mutex};

use crate::harness::types::{
    CreateTempFileOptions, ExecutionEnv, ExecutionError, ExecutionErrorCode, FileContent,
    ShellExecOptions,
};

use super::truncate::{DEFAULT_MAX_BYTES, truncate_tail};

/// Options for shell execution with captured output.
#[derive(Clone, Default)]
pub struct ShellCaptureOptions {
    /// Working directory for the command.
    pub cwd: Option<String>,
    /// Additional environment variables.
    pub env: Option<std::collections::HashMap<String, String>>,
    /// Timeout in seconds.
    pub timeout: Option<u64>,
    /// Abort signal for cancellation.
    pub abort_signal: Option<zedflow_ai::AbortSignal>,
    /// Output chunk callback.
    pub on_chunk: Option<Arc<dyn Fn(String) + Send + Sync>>,
}

/// Captured shell result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShellCaptureResult {
    /// Truncated output shown to the model/user.
    pub output: String,
    /// Exit code, absent when cancelled.
    pub exit_code: Option<i32>,
    /// Whether execution was cancelled.
    pub cancelled: bool,
    /// Whether output was truncated.
    pub truncated: bool,
    /// Path to full output, when available.
    pub full_output_path: Option<String>,
}

#[derive(Default)]
struct CaptureState {
    chunks: Vec<String>,
    total_bytes: usize,
}

/// Remove binary/control characters from shell output while preserving tabs and newlines.
#[must_use]
pub fn sanitize_binary_output(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            let code = *character as u32;
            code == 0x09
                || code == 0x0a
                || code == 0x0d
                || (code > 0x1f && !(0xfff9..=0xfffb).contains(&code))
        })
        .collect()
}

/// Execute a shell command, capture combined output, truncate the visible tail, and spill full output when needed.
///
/// # Errors
///
/// Returns execution errors from the environment or full-output spill writes.
pub async fn execute_shell_with_capture(
    env: &dyn ExecutionEnv,
    command: &str,
    options: Option<ShellCaptureOptions>,
) -> Result<ShellCaptureResult, ExecutionError> {
    let options = options.unwrap_or_default();
    let state = Arc::new(Mutex::new(CaptureState::default()));
    let on_chunk = options.on_chunk.clone();
    let stdout_state = Arc::clone(&state);
    let stderr_state = Arc::clone(&state);

    let capture = move |chunk: String, state: &Arc<Mutex<CaptureState>>| {
        let text = sanitize_binary_output(&chunk).replace('\r', "");
        if let Ok(mut state) = state.lock() {
            state.total_bytes = state.total_bytes.saturating_add(chunk.len());
            state.chunks.push(text.clone());
            let mut output_bytes: usize = state.chunks.iter().map(String::len).sum();
            let max_output_bytes = DEFAULT_MAX_BYTES * 2;
            while output_bytes > max_output_bytes && state.chunks.len() > 1 {
                let removed = state.chunks.remove(0);
                output_bytes = output_bytes.saturating_sub(removed.len());
            }
        }
        if let Some(on_chunk) = &on_chunk {
            on_chunk(text);
        }
    };

    let stdout_capture = Arc::new({
        let capture = capture.clone();
        move |chunk: String| capture(chunk, &stdout_state)
    });
    let stderr_capture = Arc::new(move |chunk: String| capture(chunk, &stderr_state));

    let exec_result = env
        .exec(
            command,
            Some(ShellExecOptions {
                cwd: options.cwd.clone(),
                env: options.env.clone(),
                timeout: options.timeout,
                abort_signal: options.abort_signal.clone(),
                on_stdout: Some(stdout_capture),
                on_stderr: Some(stderr_capture),
            }),
        )
        .await;

    let (tail_output, total_bytes) = {
        let state = state.lock().map_err(|_| {
            ExecutionError::new(
                ExecutionErrorCode::Unknown,
                "failed to capture shell output",
                None,
            )
        })?;
        (state.chunks.join(""), state.total_bytes)
    };
    let truncation = truncate_tail(&tail_output, None);
    let output = if truncation.truncated {
        truncation.content.clone()
    } else {
        tail_output.clone()
    };
    let needs_full_output = total_bytes > DEFAULT_MAX_BYTES || truncation.truncated;
    let full_output_path = if needs_full_output {
        Some(write_full_output(env, &tail_output, options.abort_signal.clone()).await?)
    } else {
        None
    };

    match exec_result {
        Ok(result) => {
            let cancelled = options
                .abort_signal
                .as_ref()
                .is_some_and(|signal| signal.aborted());
            Ok(ShellCaptureResult {
                output,
                exit_code: if cancelled {
                    None
                } else {
                    Some(result.exit_code)
                },
                cancelled,
                truncated: truncation.truncated,
                full_output_path,
            })
        }
        Err(error)
            if error.code == ExecutionErrorCode::Aborted
                || options
                    .abort_signal
                    .as_ref()
                    .is_some_and(|signal| signal.aborted()) =>
        {
            Ok(ShellCaptureResult {
                output,
                exit_code: None,
                cancelled: true,
                truncated: truncation.truncated,
                full_output_path,
            })
        }
        Err(error) => Err(error),
    }
}

async fn write_full_output(
    env: &dyn ExecutionEnv,
    output: &str,
    abort_signal: Option<zedflow_ai::AbortSignal>,
) -> Result<String, ExecutionError> {
    let path = env
        .create_temp_file(CreateTempFileOptions {
            prefix: Some("bash-".to_string()),
            suffix: Some(".log".to_string()),
            abort_signal: abort_signal.clone(),
        })
        .await
        .map_err(to_execution_error)?;
    env.append_file(&path, FileContent::Text(output.to_string()), abort_signal)
        .await
        .map_err(to_execution_error)?;
    Ok(path)
}

fn to_execution_error(error: impl std::fmt::Display) -> ExecutionError {
    ExecutionError::new(ExecutionErrorCode::Unknown, error.to_string(), None)
}
