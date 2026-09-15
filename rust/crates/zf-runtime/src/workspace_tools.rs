//! ADK file tools and a cancellable, observable shell rooted in a run's cwd.
//!
//! Shell behavior follows Pi 0.85.1: no implicit timeout, bounded tail preview,
//! complete output on disk, and cancellation of the command's process group.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::{Duration, Instant},
};

use adk_core::Tool;
use adk_devtools::{EditFileTool, ReadFileTool, Workspace, WriteFileTool};
use adk_tool::SimpleToolContext;
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
    sync::Mutex,
};
use tokio_util::sync::CancellationToken;

pub type ProgressSink = Arc<dyn Fn(Value) + Send + Sync>;
/// Called in observed pipe order; completion means the bytes are durable.
pub enum OutputEvent {
    Opened(PathBuf),
    Bytes {
        stream: &'static str,
        bytes: Vec<u8>,
    },
}
pub type OutputSink =
    Arc<dyn Fn(OutputEvent) -> futures::future::BoxFuture<'static, Result<()>> + Send + Sync>;

const MAX_LINES: usize = 2_000;
const MAX_BYTES: usize = 50 * 1024;

/// One instance per run: ADK's read tracking is shared by all file operations.
pub struct WorkspaceTools {
    cwd: PathBuf,
    data_dir: PathBuf,
    files: HashMap<String, Arc<dyn Tool>>,
    mutations: Mutex<()>,
}

fn file_tools() -> HashMap<String, Arc<dyn Tool>> {
    // Access is deliberately host-wide, as with Pi. cwd resolves relative paths;
    // it is not a containment boundary. The OS still enforces its permissions.
    let workspace = Workspace::new(Path::new("/"));
    HashMap::from([
        (
            "read".into(),
            Arc::new(ReadFileTool::new(workspace.clone())) as Arc<dyn Tool>,
        ),
        (
            "write".into(),
            Arc::new(WriteFileTool::new(workspace.clone())) as Arc<dyn Tool>,
        ),
        (
            "edit".into(),
            Arc::new(EditFileTool::new(workspace)) as Arc<dyn Tool>,
        ),
    ])
}

pub fn declarations() -> HashMap<String, Value> {
    let mut definitions = HashMap::new();
    for (name, tool) in file_tools() {
        let mut parameters = tool
            .parameters_schema()
            .unwrap_or_else(|| json!({"type":"object"}));
        parameters["properties"]["path"]["description"] =
            json!("File path, absolute or relative to the run working directory. Supports ~/.");
        let description = match name.as_str() {
            "read" => {
                "Read a UTF-8 text file with line numbers. Supports offset (1-based) and limit. Output is bounded to 2000 lines / 50 KiB. Continue with the returned nextOffset when truncated."
            }
            "write" => {
                "Create or overwrite a UTF-8 file, creating parent directories. Use for new files or complete rewrites."
            }
            _ => {
                "Replace old_string with new_string in a file previously read with read (or created with write) in this run. The exact target must be unique unless replace_all=true."
            }
        };
        definitions.insert(
            name,
            json!({"description":description,"parameters":parameters}),
        );
    }
    definitions.insert("exec".into(), json!({
        "description":"Execute a bash command in the run working directory. Returns combined stdout/stderr, exit status and a complete output file. Preview is limited to the last 2000 lines / 50 KiB. Optional timeout in seconds; no default timeout. Commands are cancellable and do not provide managed background sessions.",
        "parameters":{"type":"object","properties":{
            "command":{"type":"string","description":"Bash command to execute"},
            "timeout":{"type":"number","exclusiveMinimum":0,"description":"Optional timeout in seconds"}
        },"required":["command"]}
    }));
    definitions
}

impl WorkspaceTools {
    pub fn new(cwd: PathBuf, data_dir: PathBuf) -> Result<Self> {
        let cwd = std::fs::canonicalize(&cwd)
            .with_context(|| format!("cannot open working directory {}", cwd.display()))?;
        ensure!(
            cwd.is_dir(),
            "working directory is not a directory: {}",
            cwd.display()
        );
        let data_dir = if data_dir.is_absolute() {
            data_dir
        } else {
            std::env::current_dir()
                .context("cannot resolve tool output directory")?
                .join(data_dir)
        };
        Ok(Self {
            cwd,
            data_dir,
            files: file_tools(),
            mutations: Mutex::new(()),
        })
    }

    pub async fn execute(
        &self,
        name: &str,
        args: Value,
        call_id: &str,
        cancel: CancellationToken,
        progress: ProgressSink,
    ) -> Result<Value> {
        self.execute_with_output_sink(name, args, call_id, cancel, progress, None)
            .await
    }
    pub async fn execute_with_output_sink(
        &self,
        name: &str,
        mut args: Value,
        call_id: &str,
        cancel: CancellationToken,
        progress: ProgressSink,
        output_sink: Option<OutputSink>,
    ) -> Result<Value> {
        ensure!(
            !cancel.is_cancelled(),
            "operation cancelled before execution"
        );
        if name == "exec" {
            return self
                .exec(args, call_id, cancel, progress, output_sink)
                .await;
        }
        let tool = self
            .files
            .get(name)
            .with_context(|| format!("unknown workspace tool {name}"))?;
        let path = args
            .get("path")
            .and_then(Value::as_str)
            .context("path must be a string")?;
        args["path"] = json!(resolve_path(path, &self.cwd)?);
        if name == "read" {
            for field in ["offset", "limit"] {
                if let Some(value) = args.get(field) {
                    ensure!(
                        value.as_u64().is_some_and(|n| n > 0),
                        "{field} must be a positive integer"
                    );
                }
            }
            let requested = args
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(MAX_LINES as u64);
            args["limit"] = json!(requested.min(MAX_LINES as u64));
        }
        if name == "edit" {
            ensure!(
                args.get("old_string")
                    .and_then(Value::as_str)
                    .is_some_and(|s| !s.is_empty()),
                "old_string must be a non-empty string"
            );
        }
        // Do not race a mutating filesystem future against cancellation: a Tokio
        // filesystem operation can still complete after its future is dropped.
        let _mutation = if name == "read" {
            None
        } else {
            Some(tokio::select! {
                guard = self.mutations.lock() => guard,
                () = cancel.cancelled() => bail!("operation cancelled before execution"),
            })
        };
        ensure!(
            !cancel.is_cancelled(),
            "operation cancelled before execution"
        );
        let context = Arc::new(SimpleToolContext::new(name).with_function_call_id(call_id));
        let mut result = tool
            .execute(context, args.clone())
            .await
            .with_context(|| format!("{name} failed for {}", args["path"]))?;
        // ADK displays paths relative to its root (/); retain the resolved path
        // in the result so the user and model see unambiguous provenance.
        result["path"] = args["path"].clone();
        if name == "read" {
            let content = result["content"].as_str().unwrap_or_default();
            if content
                .split_inclusive('\n')
                .next()
                .is_some_and(|line| line.len() > MAX_BYTES)
            {
                let offset = args["offset"].as_u64().unwrap_or(1);
                return Ok(json!({
                    "path":args["path"],
                    "content":format!("Line {offset} exceeds the 50 KiB preview limit. Use exec with a byte-range command to inspect this line."),
                    "truncated":true,"nextOffset":offset,"returned_lines":0
                }));
            }
            let (preview, truncated) = truncate_head(content);
            let lines = preview.lines().count();
            let offset = args["offset"].as_u64().unwrap_or(1);
            let observed_lines = result["total_lines"].as_u64().unwrap_or_default();
            ensure!(
                offset == 1 || observed_lines >= offset,
                "offset {offset} is beyond the end of the file"
            );
            let has_more = observed_lines > offset.saturating_sub(1).saturating_add(lines as u64);
            result["content"] = json!(preview);
            result["truncated"] = json!(truncated || has_more);
            result["nextOffset"] = json!(offset.saturating_add(lines as u64));
            result["returned_lines"] = json!(lines);
            // ADK's counter stops at the slice boundary and is not the total
            // file length. Do not expose that counter as a complete count.
            if let Some(object) = result.as_object_mut() {
                object.remove("total_lines");
            }
        }
        Ok(result)
    }

    async fn exec(
        &self,
        args: Value,
        call_id: &str,
        cancel: CancellationToken,
        progress: ProgressSink,
        output_sink: Option<OutputSink>,
    ) -> Result<Value> {
        let command = args
            .get("command")
            .and_then(Value::as_str)
            .context("command must be a string")?;
        let duration = args
            .get("timeout")
            .map(|value| {
                let seconds = value
                    .as_f64()
                    .context("timeout must be a positive number of seconds")?;
                ensure!(
                    seconds.is_finite() && seconds > 0.0 && seconds <= 2_147_483.647,
                    "timeout must be between 0 and 2147483.647 seconds"
                );
                Duration::try_from_secs_f64(seconds).context("invalid timeout")
            })
            .transpose()?;
        let log_dir = self.data_dir.join("tool-output");
        tokio::fs::create_dir_all(&log_dir)
            .await
            .context("cannot create tool output directory")?;
        let log_path = log_dir.join(format!("{}.log", uuid::Uuid::new_v4()));
        let mut log = tokio::fs::File::create(&log_path)
            .await
            .context("cannot create command output log")?;
        if let Some(sink) = &output_sink {
            sink(OutputEvent::Opened(log_path.clone()))
                .await
                .context("cannot register command output")?;
        }
        let mut process = Command::new("bash");
        process
            .arg("-c")
            .arg(command)
            .current_dir(&self.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            process.as_std_mut().process_group(0);
        }
        // Preparing the output log awaits filesystem work; an abort received
        // during those awaits must still prevent spawning the command.
        ensure!(!cancel.is_cancelled(), "command cancelled before execution");
        let start = Instant::now();
        let mut child = process.spawn().context("cannot start bash")?;
        let mut group =
            ProcessGroup::new(child.id().context("spawned command has no process id")?)?;
        let mut stdout = child.stdout.take().context("command stdout unavailable")?;
        let mut stderr = child.stderr.take().context("command stderr unavailable")?;
        let mut stdout_buffer = [0_u8; 8192];
        let mut stderr_buffer = [0_u8; 8192];
        let mut stdout_done = false;
        let mut stderr_done = false;
        let mut status = None;
        let mut failure = None;
        let mut output = Tail::default();
        let deadline = async {
            match duration {
                Some(duration) => tokio::time::sleep(duration).await,
                None => std::future::pending::<()>().await,
            }
        };
        tokio::pin!(deadline);
        let mut drain_deadline = None;
        let mut last_update = Instant::now();
        let mut emitted_output = false;
        (progress)(
            json!({"callId":call_id,"tool":"exec","status":"running","content":"","fullOutputPath":log_path}),
        );
        loop {
            if status.is_some() && stdout_done && stderr_done {
                break;
            }
            let next_drain_deadline = drain_deadline;
            let drain = async move {
                match next_drain_deadline {
                    Some(deadline) => tokio::time::sleep_until(deadline).await,
                    None => std::future::pending::<()>().await,
                }
            };
            tokio::pin!(drain);
            tokio::select! {
                count = stdout.read(&mut stdout_buffer), if !stdout_done => {
                    let count = count.context("cannot read command stdout")?;
                    stdout_done = count == 0;
                    if count>0 && let Some(sink)=&output_sink {sink(OutputEvent::Bytes {stream:"stdout",bytes:stdout_buffer[..count].to_vec()}).await.context("cannot commit command stdout")?;}
                    log.write_all(&stdout_buffer[..count]).await.context("cannot save command output")?;
                    output.append(&stdout_buffer[..count]);
                }
                count = stderr.read(&mut stderr_buffer), if !stderr_done => {
                    let count = count.context("cannot read command stderr")?;
                    stderr_done = count == 0;
                    if count>0 && let Some(sink)=&output_sink {sink(OutputEvent::Bytes {stream:"stderr",bytes:stderr_buffer[..count].to_vec()}).await.context("cannot commit command stderr")?;}
                    log.write_all(&stderr_buffer[..count]).await.context("cannot save command output")?;
                    output.append(&stderr_buffer[..count]);
                }
                result = child.wait(), if status.is_none() => {
                    status = Some(result.context("cannot wait for command")?);
                    // Background descendants may inherit pipe descriptors. They
                    // are not managed sessions and must not outlive this call.
                    group.terminate()?;
                    drain_deadline = Some(tokio::time::Instant::now() + Duration::from_secs(1));
                }
                () = cancel.cancelled(), if status.is_none() => {
                    failure = Some("command cancelled".to_owned());
                    group.terminate()?;
                    status = Some(child.wait().await.context("cannot reap cancelled command")?);
                    drain_deadline = Some(tokio::time::Instant::now() + Duration::from_secs(1));
                }
                () = &mut deadline, if status.is_none() => {
                    failure = Some(format!("command timed out after {} seconds", duration.map(|v| v.as_secs_f64()).unwrap_or_default()));
                    group.terminate()?;
                    status = Some(child.wait().await.context("cannot reap timed-out command")?);
                    drain_deadline = Some(tokio::time::Instant::now() + Duration::from_secs(1));
                }
                () = &mut drain => break,
            }
            if !emitted_output || last_update.elapsed() >= Duration::from_millis(80) {
                (progress)(
                    json!({"callId":call_id,"tool":"exec","status":"running","content":output.text(),"truncated":output.truncated,"fullOutputPath":log_path}),
                );
                last_update = Instant::now();
                emitted_output = true;
            }
        }
        log.flush().await.context("cannot flush command output")?;
        let status = status.context("command ended without an exit status")?;
        let result = json!({"content":output.text(),"exitCode":status.code(),"durationMs":start.elapsed().as_millis(),"cwd":self.cwd,"fullOutputPath":log_path,"truncated":output.truncated});
        (progress)(
            json!({"callId":call_id,"tool":"exec","status":if failure.is_some() || !status.success() {"error"} else {"completed"},"result":result}),
        );
        if let Some(failure) = failure {
            bail!(
                "{failure}\n{}\nFull output: {}",
                output.text(),
                log_path.display()
            );
        }
        ensure!(
            status.success(),
            "command exited with {status}\n{}\nFull output: {}",
            output.text(),
            log_path.display()
        );
        Ok(result)
    }
}

fn resolve_path(path: &str, cwd: &Path) -> Result<PathBuf> {
    let path = if path == "~" || path.starts_with("~/") {
        let home = std::env::var_os("HOME").context("HOME is unavailable for ~/ expansion")?;
        PathBuf::from(home).join(path.strip_prefix("~/").unwrap_or(""))
    } else {
        PathBuf::from(path)
    };
    Ok(if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    })
}

fn truncate_head(content: &str) -> (String, bool) {
    let mut end = 0;
    for line in content.split_inclusive('\n').take(MAX_LINES) {
        if end + line.len() > MAX_BYTES {
            break;
        }
        end += line.len();
    }
    if end == 0 && !content.is_empty() {
        end = content.floor_char_boundary(MAX_BYTES.min(content.len()));
    }
    (content[..end].to_owned(), end < content.len())
}

#[derive(Default)]
struct Tail {
    bytes: Vec<u8>,
    truncated: bool,
}

impl Tail {
    fn append(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
        let byte_cut = self.bytes.len().saturating_sub(MAX_BYTES);
        let mut newline_count = 0;
        let mut line_cut = 0;
        for (index, byte) in self.bytes.iter().enumerate().rev() {
            if *byte == b'\n' {
                newline_count += 1;
                // A trailing newline closes the current last line.
                let allowed = MAX_LINES + usize::from(self.bytes.last() == Some(&b'\n'));
                if newline_count == allowed {
                    line_cut = index + 1;
                    break;
                }
            }
        }
        let cut = byte_cut.max(line_cut);
        if cut > 0 {
            self.bytes.drain(..cut);
            self.truncated = true;
        }
    }
    fn text(&self) -> String {
        // A byte-bounded tail can begin inside a valid UTF-8 code point. Drop
        // that incomplete prefix instead of fabricating a replacement character
        // for an otherwise valid command output. Raw bytes remain in the log.
        let prefix = if self.truncated {
            self.bytes
                .iter()
                .take_while(|byte| (**byte & 0xc0) == 0x80)
                .count()
        } else {
            0
        };
        let text = String::from_utf8_lossy(&self.bytes[prefix..]);
        let start = text.ceil_char_boundary(text.len().saturating_sub(MAX_BYTES));
        text[start..].to_owned()
    }
}

struct ProcessGroup {
    pid: Option<i32>,
}

impl ProcessGroup {
    fn new(pid: u32) -> Result<Self> {
        Ok(Self {
            pid: Some(i32::try_from(pid).context("invalid process id")?),
        })
    }
    fn terminate(&mut self) -> Result<()> {
        let Some(pid) = self.pid.take() else {
            return Ok(());
        };
        #[cfg(unix)]
        match nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(pid),
            nix::sys::signal::Signal::SIGKILL,
        ) {
            Ok(()) | Err(nix::errno::Errno::ESRCH) => Ok(()),
            Err(error) => Err(error).context("cannot stop command process group"),
        }
        #[cfg(not(unix))]
        {
            let _ = pid;
            bail!("process group cancellation is supported on Unix hosts only")
        }
    }
}

impl Drop for ProcessGroup {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn call(tools: &WorkspaceTools, name: &str, args: Value) -> Result<Value> {
        tools
            .execute(
                name,
                args,
                "test-call",
                CancellationToken::new(),
                Arc::new(|_| {}),
            )
            .await
    }

    #[tokio::test]
    async fn files_share_read_tracking_and_access_outside_cwd() {
        let directory = tempfile::tempdir().unwrap();
        let cwd = directory.path().join("workspace");
        std::fs::create_dir(&cwd).unwrap();
        let target = directory.path().join("outside.txt");
        std::fs::write(&target, "first\r\nsecond\r\n").unwrap();
        let tools = WorkspaceTools::new(cwd, directory.path().join("data")).unwrap();
        let edit = json!({"path":"../outside.txt","old_string":"second","new_string":"changed"});
        assert!(call(&tools, "edit", edit.clone()).await.is_err());
        let result = call(&tools, "read", json!({"path":target})).await.unwrap();
        assert!(result["content"].as_str().unwrap().contains("second"));
        call(&tools, "edit", edit).await.unwrap();
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "first\r\nchanged\r\n"
        );
        call(
            &tools,
            "write",
            json!({"path":"nested/new.txt","content":"known"}),
        )
        .await
        .unwrap();
        call(
            &tools,
            "edit",
            json!({"path":"nested/new.txt","old_string":"known","new_string":"updated"}),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn edit_failure_keeps_file_unchanged_and_cancelled_write_never_starts() {
        let directory = tempfile::tempdir().unwrap();
        let tools = WorkspaceTools::new(
            directory.path().to_path_buf(),
            directory.path().join("data"),
        )
        .unwrap();
        call(
            &tools,
            "write",
            json!({"path":"repeat.txt","content":"same same"}),
        )
        .await
        .unwrap();
        assert!(
            call(
                &tools,
                "edit",
                json!({"path":"repeat.txt","old_string":"same","new_string":"new"})
            )
            .await
            .is_err()
        );
        assert_eq!(
            std::fs::read_to_string(directory.path().join("repeat.txt")).unwrap(),
            "same same"
        );
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        assert!(
            tools
                .execute(
                    "write",
                    json!({"path":"never.txt","content":"no"}),
                    "cancelled",
                    cancellation,
                    Arc::new(|_| {})
                )
                .await
                .is_err()
        );
        assert!(!directory.path().join("never.txt").exists());
    }

    #[tokio::test]
    async fn read_slices_and_bounds_large_files() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("large.txt"), "line\n".repeat(3_000)).unwrap();
        let tools = WorkspaceTools::new(
            directory.path().to_path_buf(),
            directory.path().join("data"),
        )
        .unwrap();
        let first = call(&tools, "read", json!({"path":"large.txt"}))
            .await
            .unwrap();
        assert_eq!(first["returned_lines"], 2_000);
        assert_eq!(first["nextOffset"], 2_001);
        assert_eq!(first["truncated"], true);
        let last = call(&tools, "read", json!({"path":"large.txt","offset":2_001}))
            .await
            .unwrap();
        assert_eq!(last["returned_lines"], 1_000);
        assert_eq!(last["truncated"], false);
        assert!(
            call(&tools, "read", json!({"path":"large.txt","offset":4_000}))
                .await
                .is_err()
        );
        std::fs::write(
            directory.path().join("one-line.txt"),
            "x".repeat(MAX_BYTES + 1),
        )
        .unwrap();
        let long = call(&tools, "read", json!({"path":"one-line.txt"}))
            .await
            .unwrap();
        assert_eq!(long["returned_lines"], 0);
        assert_eq!(long["nextOffset"], 1);
        assert!(long["content"].as_str().unwrap().contains("byte-range"));
    }

    #[tokio::test]
    async fn shell_records_full_output_and_exposes_cwd_and_errors() {
        let directory = tempfile::tempdir().unwrap();
        let tools = WorkspaceTools::new(
            directory.path().to_path_buf(),
            directory.path().join("data"),
        )
        .unwrap();
        let result = call(&tools, "exec", json!({"command":"pwd; for i in {1..3000}; do printf 'line %s abcdefghijklmnopqrstuvwxyz\n' \"$i\"; done"})).await.unwrap();
        let preview = result["content"].as_str().unwrap();
        assert!(preview.len() <= MAX_BYTES + 3);
        assert!(preview.lines().count() <= MAX_LINES);
        assert!(preview.contains("line 3000"));
        assert_eq!(result["truncated"], true);
        let full = std::fs::read_to_string(result["fullOutputPath"].as_str().unwrap()).unwrap();
        assert!(full.starts_with(directory.path().to_str().unwrap()));
        assert!(full.contains("line 1 "));
        let error = call(
            &tools,
            "exec",
            json!({"command":"printf 'diagnostic'; exit 7"}),
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("7") && error.contains("diagnostic") && error.contains("Full output:")
        );
        let error = call(
            &tools,
            "exec",
            json!({"command":"printf 'before-timeout'; sleep 30","timeout":0.05}),
        )
        .await
        .unwrap_err()
        .to_string();
        assert!(error.contains("timed out") && error.contains("before-timeout"));
    }

    #[tokio::test]
    async fn shell_bounds_multibyte_output_without_newlines_and_keeps_exact_log() {
        let directory = tempfile::tempdir().unwrap();
        let original = "é漢🙂".repeat(20_000);
        std::fs::write(directory.path().join("unicode.txt"), &original).unwrap();
        let tools = WorkspaceTools::new(
            directory.path().to_path_buf(),
            directory.path().join("data"),
        )
        .unwrap();
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = events.clone();
        let result = tools
            .execute(
                "exec",
                json!({"command":"cat unicode.txt"}),
                "unicode-tail",
                CancellationToken::new(),
                Arc::new(move |event| captured.lock().unwrap().push(event)),
            )
            .await
            .unwrap();
        let preview = result["content"].as_str().unwrap();
        let start = original.ceil_char_boundary(original.len() - MAX_BYTES);
        assert_eq!(preview, &original[start..]);
        assert_eq!(preview.lines().count(), 1);
        assert_eq!(result["truncated"], true);
        assert_eq!(
            std::fs::read(result["fullOutputPath"].as_str().unwrap()).unwrap(),
            original.as_bytes()
        );
        let events = events.lock().unwrap();
        assert!(events.iter().any(|event| {
            event["status"] == "running"
                && event["content"]
                    .as_str()
                    .is_some_and(|text| !text.is_empty())
        }));
        for event in events.iter() {
            if let Some(content) = event["content"].as_str() {
                assert!(content.len() <= MAX_BYTES);
            }
        }
        assert_eq!(events.last().unwrap()["result"], result);
    }

    #[test]
    fn tail_preserves_split_utf8_and_independently_limits_short_lines() {
        let original = "é漢🙂".repeat(10_000);
        let mut tail = Tail::default();
        for chunk in original.as_bytes().chunks(8_191) {
            tail.append(chunk);
            assert!(tail.text().len() <= MAX_BYTES);
        }
        let start = original.ceil_char_boundary(original.len() - MAX_BYTES);
        assert_eq!(tail.text(), &original[start..]);
        let mut tail = Tail::default();
        let original = (0..3_000)
            .map(|line| format!("{line}\n"))
            .collect::<String>();
        tail.append(original.as_bytes());
        let expected = (1_000..3_000)
            .map(|line| format!("{line}\n"))
            .collect::<String>();
        assert_eq!(tail.text(), expected);
        assert!(tail.truncated);
    }

    #[tokio::test]
    async fn timeout_reports_and_persists_partial_stdout_and_stderr() {
        let directory = tempfile::tempdir().unwrap();
        let tools = WorkspaceTools::new(
            directory.path().to_path_buf(),
            directory.path().join("data"),
        )
        .unwrap();
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = events.clone();
        let error = tokio::time::timeout(
            Duration::from_secs(3),
            tools.execute(
                "exec",
                json!({"command":"printf stdout-partial; printf stderr-partial >&2; sleep 30","timeout":0.2}),
                "timeout-partial",
                CancellationToken::new(),
                Arc::new(move |event| captured.lock().unwrap().push(event)),
            ),
        )
        .await
        .unwrap()
        .unwrap_err()
        .to_string();
        assert!(error.contains("timed out"));
        assert!(error.contains("stdout-partial") && error.contains("stderr-partial"));
        let events = events.lock().unwrap();
        let final_event = events.last().unwrap();
        assert_eq!(final_event["status"], "error");
        let result = &final_event["result"];
        let path = result["fullOutputPath"].as_str().unwrap();
        assert!(error.contains(path));
        let log = std::fs::read_to_string(path).unwrap();
        assert_eq!(result["content"], log);
        assert!(log.contains("stdout-partial") && log.contains("stderr-partial"));
        assert_eq!(result["truncated"], false);
    }

    #[test]
    fn cancellation_during_log_preparation_never_starts_the_command() {
        use std::{future::Future, task::Poll};

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        runtime.block_on(async {
            let directory = tempfile::tempdir().unwrap();
            let tools = WorkspaceTools::new(
                directory.path().to_path_buf(),
                directory.path().join("data"),
            )
            .unwrap();
            // Hold the filesystem pool so the command pauses deterministically
            // while preparing its log, after execute's first cancellation check.
            let (ready, started) = tokio::sync::oneshot::channel();
            let (release, blocked) = std::sync::mpsc::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                ready.send(()).unwrap();
                blocked.recv().unwrap();
            });
            started.await.unwrap();
            let cancellation = CancellationToken::new();
            let events = Arc::new(std::sync::Mutex::new(Vec::new()));
            let captured = events.clone();
            let mut execution = Box::pin(tools.execute(
                "exec",
                json!({"command":"touch forbidden"}),
                "cancel-before-spawn",
                cancellation.clone(),
                Arc::new(move |event| captured.lock().unwrap().push(event)),
            ));
            std::future::poll_fn(|context| {
                assert!(execution.as_mut().poll(context).is_pending());
                Poll::Ready(())
            })
            .await;
            cancellation.cancel();
            release.send(()).unwrap();
            blocker.await.unwrap();
            let error = execution.await.unwrap_err().to_string();
            assert!(error.contains("cancelled before execution"));
            assert!(
                events.lock().unwrap().is_empty(),
                "command was never started"
            );
            assert!(!directory.path().join("forbidden").exists());
        });
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn cancellation_stops_the_child_group_after_publishing_live_output() {
        let directory = tempfile::tempdir().unwrap();
        let tools = WorkspaceTools::new(
            directory.path().to_path_buf(),
            directory.path().join("data"),
        )
        .unwrap();
        let cancellation = CancellationToken::new();
        let from_progress = cancellation.clone();
        let descendant_pids = Arc::new(std::sync::Mutex::new(Vec::new()));
        let observed_pids = descendant_pids.clone();
        let progress: ProgressSink = Arc::new(move |event| {
            if let Some(text) = event["content"].as_str()
                && let Ok(pids) = text
                    .split_whitespace()
                    .map(str::parse::<u32>)
                    .collect::<std::result::Result<Vec<_>, _>>()
                && pids.len() == 2
            {
                *observed_pids.lock().unwrap() = pids;
                from_progress.cancel();
            }
        });
        let result = tokio::time::timeout(
            Duration::from_secs(3),
            tools.execute(
                "exec",
                json!({"command":"bash -c 'sleep 30 & grandchild=$!; printf \"%s %s\\n\" \"$$\" \"$grandchild\"; wait' & wait"}),
                "cancellation",
                cancellation,
                progress,
            ),
        )
        .await
        .unwrap();
        assert!(result.unwrap_err().to_string().contains("cancelled"));
        let pids = descendant_pids.lock().unwrap().clone();
        assert_eq!(pids.len(), 2, "child and grandchild pids were streamed");
        for pid in pids {
            // SIGKILL delivery to descendants can trail reaping their parent.
            tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    let process = std::fs::read_to_string(format!("/proc/{pid}/stat"));
                    if process
                        .as_ref()
                        .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
                        || process.as_ref().is_ok_and(|process| {
                            process.rsplit_once(')').is_some_and(|(_, state)| {
                                matches!(state.trim_start().chars().next(), Some('Z' | 'X'))
                            })
                        })
                    {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .expect("descendant still running after cancellation");
        }
    }
}
