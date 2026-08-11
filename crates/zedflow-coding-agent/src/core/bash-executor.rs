//! Bash execution with streaming, cancellation, sanitization, and bounded output.

use std::{io, path::PathBuf, sync::Arc};

use zedflow_ai::AbortSignal;

use crate::{
    bash_tool::{BashOperationOptions, BashOperations},
    output_accumulator::{OutputAccumulator, OutputAccumulatorOptions, OutputSnapshotOptions},
    truncate::DEFAULT_MAX_BYTES,
    utils::{ansi::strip_ansi, shell::sanitize_binary_output},
};

#[derive(Clone, Default)]
pub struct BashExecutorOptions {
    pub on_chunk: Option<Arc<dyn Fn(String) + Send + Sync>>,
    pub signal: Option<AbortSignal>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BashResult {
    pub output: String,
    pub exit_code: Option<i32>,
    pub cancelled: bool,
    pub truncated: bool,
    pub full_output_path: Option<PathBuf>,
}

pub async fn execute_bash_with_operations(
    command: &str,
    cwd: &str,
    operations: &dyn BashOperations,
    options: BashExecutorOptions,
) -> io::Result<BashResult> {
    let output = Arc::new(std::sync::Mutex::new(OutputAccumulator::new(
        OutputAccumulatorOptions {
            max_bytes: DEFAULT_MAX_BYTES,
            temp_file_prefix: "pi-bash".into(),
            ..Default::default()
        },
    )));
    let callback_output = output.clone();
    let callback = options.on_chunk.clone();
    let on_data = Arc::new(move |data: &[u8]| {
        let text =
            sanitize_binary_output(&strip_ansi(&String::from_utf8_lossy(data))).replace('\r', "");
        let _ = callback_output.lock().unwrap().append(text.as_bytes());
        if let Some(callback) = &callback {
            callback(text);
        }
    });

    let execution = operations
        .exec(
            command.to_owned(),
            PathBuf::from(cwd),
            BashOperationOptions {
                on_data,
                signal: options.signal.clone(),
                timeout: None,
                env: None,
            },
        )
        .await;
    let cancelled = options.signal.as_ref().is_some_and(AbortSignal::aborted);
    let mut output = output.lock().unwrap();
    output.finish()?;
    let snapshot = output.snapshot(OutputSnapshotOptions {
        persist_if_truncated: true,
    })?;

    match execution {
        Ok(exit_code) => Ok(BashResult {
            output: snapshot.content,
            exit_code: if cancelled { None } else { exit_code },
            cancelled,
            truncated: snapshot.truncation.truncated,
            full_output_path: snapshot.full_output_path,
        }),
        Err(_) if cancelled => Ok(BashResult {
            output: snapshot.content,
            exit_code: None,
            cancelled: true,
            truncated: snapshot.truncation.truncated,
            full_output_path: snapshot.full_output_path,
        }),
        Err(error) => Err(error),
    }
}
