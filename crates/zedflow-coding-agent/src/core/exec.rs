//! Shared command execution utilities for extensions and custom tools.

use std::{io, path::PathBuf, process::Stdio, time::Duration};

use tokio::{io::AsyncReadExt, process::Command, sync::mpsc, time::Instant};
use zedflow_ai::AbortSignal;

#[derive(Clone, Debug, Default)]
pub struct ExecOptions {
    pub signal: Option<AbortSignal>,
    pub timeout: Option<Duration>,
    pub cwd: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
    pub killed: bool,
}

/// Executes one program directly (never through a shell), collecting both output streams.
pub async fn exec_command(
    command: &str,
    args: &[String],
    cwd: impl Into<PathBuf>,
    options: ExecOptions,
) -> io::Result<ExecResult> {
    let mut process = Command::new(command);
    process
        .args(args)
        .current_dir(options.cwd.unwrap_or_else(|| cwd.into()))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = process.spawn()?;
    let mut child_stdout = child.stdout.take().expect("piped stdout");
    let mut child_stderr = child.stderr.take().expect("piped stderr");
    let (tx, mut rx) = mpsc::unbounded_channel();
    let stdout_tx = tx.clone();
    let stdout_task = tokio::spawn(async move {
        let mut bytes = vec![0; 8192];
        loop {
            let length = child_stdout.read(&mut bytes).await?;
            if length == 0 || stdout_tx.send((true, bytes[..length].to_vec())).is_err() {
                return Ok::<(), io::Error>(());
            }
        }
    });
    let stderr_task = tokio::spawn(async move {
        let mut bytes = vec![0; 8192];
        loop {
            let length = child_stderr.read(&mut bytes).await?;
            if length == 0 || tx.send((false, bytes[..length].to_vec())).is_err() {
                return Ok::<(), io::Error>(());
            }
        }
    });

    let deadline = options
        .timeout
        .filter(|value| !value.is_zero())
        .map(|value| Instant::now() + value);
    let mut killed = false;
    let status = loop {
        if options.signal.as_ref().is_some_and(AbortSignal::aborted)
            || deadline.is_some_and(|value| Instant::now() >= value)
        {
            killed = true;
            let _ = child.kill().await;
            break child.wait().await;
        }
        if let Some(status) = child.try_wait()? {
            break Ok(status);
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }?;

    // Detached descendants can inherit these pipes. Collect promptly available
    // output, but do not let those inherited handles keep command completion open.
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let drain_deadline = Instant::now() + Duration::from_millis(20);
    loop {
        tokio::select! {
            message = rx.recv() => match message {
                Some((true, bytes)) => stdout.extend(bytes),
                Some((false, bytes)) => stderr.extend(bytes),
                None => break,
            },
            () = tokio::time::sleep_until(drain_deadline) => break,
        }
    }
    stdout_task.abort();
    stderr_task.abort();
    Ok(ExecResult {
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
        code: status.code().unwrap_or(if killed { 0 } else { 1 }),
        killed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn timeout_marks_process_killed() {
        let result = exec_command(
            "sh",
            &["-c".into(), "printf ready; sleep 1".into()],
            ".",
            ExecOptions {
                timeout: Some(Duration::from_millis(20)),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert!(result.killed);
        assert_eq!(result.stdout, "ready");
    }
}
