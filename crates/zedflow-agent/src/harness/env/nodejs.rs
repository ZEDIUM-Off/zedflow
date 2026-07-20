//! Node-backed execution environment ported to Rust stdlib.
//!
//! This mirrors Pi's `harness/env/nodejs.ts` filesystem and shell seam using
//! `std::fs`, `std::process`, `uuid`, and `wait-timeout`.

use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use command_group::{CommandGroup, GroupChild};
use futures::channel::oneshot;
use uuid::Uuid;

use crate::harness::types::{
    CreateDirOptions, CreateTempFileOptions, ExecutionError, ExecutionErrorCode, FileContent,
    FileError, FileErrorCode, FileInfo, FileKind, FileSystem, HarnessFuture, ReadTextLinesOptions,
    RemoveOptions, Result, Shell, ShellExecOptions, ShellExecOutput,
};

const MAX_TIMEOUT_MS: u64 = 2_147_483_647;
const MAX_TIMEOUT_SECONDS: u64 = MAX_TIMEOUT_MS / 1000;
const PROCESS_POLL_MS: u64 = 50;

/// Local stdlib implementation of Pi's Node `ExecutionEnv`.
#[derive(Debug, Clone)]
pub struct NodeExecutionEnv {
    cwd: String,
    shell_path: Option<String>,
    shell_env: Option<HashMap<String, String>>,
}

/// Construction options for [`NodeExecutionEnv`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeExecutionEnvOptions {
    /// Working directory used to resolve relative paths.
    pub cwd: String,
    /// Optional bash-compatible shell path.
    pub shell_path: Option<String>,
    /// Environment overrides applied to every shell command.
    pub shell_env: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ShellConfig {
    shell: String,
    args: Vec<String>,
    command_transport: CommandTransport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandTransport {
    Argv,
    Stdin,
}

impl NodeExecutionEnv {
    /// Create a new local execution environment.
    #[must_use]
    pub fn new(options: NodeExecutionEnvOptions) -> Self {
        Self {
            cwd: absolute_lexical(Path::new(&options.cwd))
                .to_string_lossy()
                .into_owned(),
            shell_path: options.shell_path,
            shell_env: options.shell_env,
        }
    }

    /// Create an environment rooted at `cwd` with default shell selection.
    #[must_use]
    pub fn with_cwd(cwd: impl Into<String>) -> Self {
        Self::new(NodeExecutionEnvOptions {
            cwd: cwd.into(),
            shell_path: None,
            shell_env: None,
        })
    }

    fn resolve_path(&self, path: impl AsRef<Path>) -> PathBuf {
        let path = path.as_ref();
        if path.is_absolute() {
            normalize_lexical(path)
        } else {
            normalize_lexical(Path::new(&self.cwd).join(path))
        }
    }
}

impl FileSystem for NodeExecutionEnv {
    fn cwd(&self) -> &str {
        &self.cwd
    }

    fn absolute_path<'a>(
        &'a self,
        path: &'a str,
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<String, FileError>> {
        Box::pin(async move {
            if let Some(error) = aborted_file(&abort_signal, Some(path.to_string())) {
                return Err(error);
            }
            Ok(self.resolve_path(path).to_string_lossy().into_owned())
        })
    }

    fn join_path<'a>(
        &'a self,
        parts: &'a [String],
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<String, FileError>> {
        Box::pin(async move {
            if let Some(error) = aborted_file(&abort_signal, None) {
                return Err(error);
            }
            if parts.is_empty() {
                return Ok(".".to_string());
            }
            let joined = parts.iter().fold(PathBuf::new(), |mut path, part| {
                path.push(part);
                path
            });
            Ok(joined.to_string_lossy().into_owned())
        })
    }

    fn read_text_file<'a>(
        &'a self,
        path: &'a str,
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<String, FileError>> {
        Box::pin(async move {
            let resolved = self.resolve_path(path);
            if let Some(error) = aborted_file(&abort_signal, Some(path_string(&resolved))) {
                return Err(error);
            }
            fs::read_to_string(&resolved).map_err(|error| to_file_error(error, Some(&resolved)))
        })
    }

    fn read_text_lines<'a>(
        &'a self,
        path: &'a str,
        options: ReadTextLinesOptions,
    ) -> HarnessFuture<'a, Result<Vec<String>, FileError>> {
        Box::pin(async move {
            let resolved = self.resolve_path(path);
            if let Some(error) = aborted_file(&options.abort_signal, Some(path_string(&resolved))) {
                return Err(error);
            }
            if options.max_lines == Some(0) {
                return Ok(Vec::new());
            }

            let content = fs::read_to_string(&resolved)
                .map_err(|error| to_file_error(error, Some(&resolved)))?;
            let mut lines = Vec::new();
            for line in content.lines() {
                if let Some(error) =
                    aborted_file(&options.abort_signal, Some(path_string(&resolved)))
                {
                    return Err(error);
                }
                lines.push(line.to_string());
                if options.max_lines.is_some_and(|max| lines.len() >= max) {
                    break;
                }
            }
            Ok(lines)
        })
    }

    fn read_binary_file<'a>(
        &'a self,
        path: &'a str,
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<Vec<u8>, FileError>> {
        Box::pin(async move {
            let resolved = self.resolve_path(path);
            if let Some(error) = aborted_file(&abort_signal, Some(path_string(&resolved))) {
                return Err(error);
            }
            fs::read(&resolved).map_err(|error| to_file_error(error, Some(&resolved)))
        })
    }

    fn write_file<'a>(
        &'a self,
        path: &'a str,
        content: FileContent,
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<(), FileError>> {
        Box::pin(async move {
            let resolved = self.resolve_path(path);
            if let Some(error) = aborted_file(&abort_signal, Some(path_string(&resolved))) {
                return Err(error);
            }
            if let Some(parent) = resolved.parent() {
                fs::create_dir_all(parent).map_err(|error| to_file_error(error, Some(parent)))?;
            }
            if let Some(error) = aborted_file(&abort_signal, Some(path_string(&resolved))) {
                return Err(error);
            }
            write_content(&resolved, content, false)
        })
    }

    fn append_file<'a>(
        &'a self,
        path: &'a str,
        content: FileContent,
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<(), FileError>> {
        Box::pin(async move {
            let resolved = self.resolve_path(path);
            if let Some(error) = aborted_file(&abort_signal, Some(path_string(&resolved))) {
                return Err(error);
            }
            if let Some(parent) = resolved.parent() {
                fs::create_dir_all(parent).map_err(|error| to_file_error(error, Some(parent)))?;
            }
            write_content(&resolved, content, true)
        })
    }

    fn file_info<'a>(
        &'a self,
        path: &'a str,
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<FileInfo, FileError>> {
        Box::pin(async move {
            let resolved = self.resolve_path(path);
            if let Some(error) = aborted_file(&abort_signal, Some(path_string(&resolved))) {
                return Err(error);
            }
            let metadata = fs::symlink_metadata(&resolved)
                .map_err(|error| to_file_error(error, Some(&resolved)))?;
            file_info_from_metadata(&resolved, metadata)
        })
    }

    fn list_dir<'a>(
        &'a self,
        path: &'a str,
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<Vec<FileInfo>, FileError>> {
        Box::pin(async move {
            let resolved = self.resolve_path(path);
            if let Some(error) = aborted_file(&abort_signal, Some(path_string(&resolved))) {
                return Err(error);
            }
            let entries =
                fs::read_dir(&resolved).map_err(|error| to_file_error(error, Some(&resolved)))?;
            let mut infos = Vec::new();
            for entry in entries {
                if let Some(error) = aborted_file(&abort_signal, Some(path_string(&resolved))) {
                    return Err(error);
                }
                let entry = entry.map_err(|error| to_file_error(error, Some(&resolved)))?;
                let entry_path = entry.path();
                let metadata = fs::symlink_metadata(&entry_path)
                    .map_err(|error| to_file_error(error, Some(&entry_path)))?;
                infos.push(file_info_from_metadata(&entry_path, metadata)?);
            }
            Ok(infos)
        })
    }

    fn canonical_path<'a>(
        &'a self,
        path: &'a str,
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<String, FileError>> {
        Box::pin(async move {
            let resolved = self.resolve_path(path);
            if let Some(error) = aborted_file(&abort_signal, Some(path_string(&resolved))) {
                return Err(error);
            }
            fs::canonicalize(&resolved)
                .map(|path| path.to_string_lossy().into_owned())
                .map_err(|error| to_file_error(error, Some(&resolved)))
        })
    }

    fn exists<'a>(
        &'a self,
        path: &'a str,
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<bool, FileError>> {
        Box::pin(async move {
            let resolved = self.resolve_path(path);
            if let Some(error) = aborted_file(&abort_signal, Some(path_string(&resolved))) {
                return Err(error);
            }
            match fs::symlink_metadata(&resolved) {
                Ok(_) => Ok(true),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
                Err(error) => Err(to_file_error(error, Some(&resolved))),
            }
        })
    }

    fn create_dir<'a>(
        &'a self,
        path: &'a str,
        options: CreateDirOptions,
    ) -> HarnessFuture<'a, Result<(), FileError>> {
        Box::pin(async move {
            let resolved = self.resolve_path(path);
            if let Some(error) = aborted_file(&options.abort_signal, Some(path_string(&resolved))) {
                return Err(error);
            }
            let result = if options.recursive {
                fs::create_dir_all(&resolved)
            } else {
                fs::create_dir(&resolved)
            };
            result.map_err(|error| to_file_error(error, Some(&resolved)))
        })
    }

    fn remove<'a>(
        &'a self,
        path: &'a str,
        options: RemoveOptions,
    ) -> HarnessFuture<'a, Result<(), FileError>> {
        Box::pin(async move {
            let resolved = self.resolve_path(path);
            if let Some(error) = aborted_file(&options.abort_signal, Some(path_string(&resolved))) {
                return Err(error);
            }
            let metadata = match fs::symlink_metadata(&resolved) {
                Ok(metadata) => metadata,
                Err(error) if options.force && error.kind() == io::ErrorKind::NotFound => {
                    return Ok(());
                }
                Err(error) => return Err(to_file_error(error, Some(&resolved))),
            };
            let result = if metadata.is_dir() && !metadata.file_type().is_symlink() {
                if options.recursive {
                    fs::remove_dir_all(&resolved)
                } else {
                    fs::remove_dir(&resolved)
                }
            } else {
                fs::remove_file(&resolved)
            };
            match result {
                Ok(()) => Ok(()),
                Err(error) if options.force && error.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(to_file_error(error, Some(&resolved))),
            }
        })
    }

    fn create_temp_dir<'a>(
        &'a self,
        prefix: Option<&'a str>,
        abort_signal: Option<zedflow_ai::AbortSignal>,
    ) -> HarnessFuture<'a, Result<String, FileError>> {
        Box::pin(async move {
            if let Some(error) = aborted_file(&abort_signal, None) {
                return Err(error);
            }
            let prefix = prefix.unwrap_or("tmp-");
            for _ in 0..16 {
                let path = env::temp_dir().join(format!("{prefix}{}", Uuid::new_v4()));
                match fs::create_dir(&path) {
                    Ok(()) => return Ok(path_string(&path)),
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(error) => return Err(to_file_error(error, Some(&path))),
                }
            }
            Err(FileError::new(
                FileErrorCode::Unknown,
                "failed to create unique temporary directory",
                None,
                None,
            ))
        })
    }

    fn create_temp_file<'a>(
        &'a self,
        options: CreateTempFileOptions,
    ) -> HarnessFuture<'a, Result<String, FileError>> {
        Box::pin(async move {
            if let Some(error) = aborted_file(&options.abort_signal, None) {
                return Err(error);
            }
            let dir = env::temp_dir().join(format!("tmp-{}", Uuid::new_v4()));
            fs::create_dir(&dir).map_err(|error| to_file_error(error, Some(&dir)))?;
            let prefix = options.prefix.as_deref().unwrap_or("");
            let suffix = options.suffix.as_deref().unwrap_or("");
            let path = dir.join(format!("{prefix}{}{suffix}", Uuid::new_v4()));
            File::create(&path).map_err(|error| to_file_error(error, Some(&path)))?;
            Ok(path_string(&path))
        })
    }

    fn cleanup<'a>(&'a self) -> HarnessFuture<'a, ()> {
        Box::pin(async {})
    }
}

impl Shell for NodeExecutionEnv {
    fn exec<'a>(
        &'a self,
        command: &'a str,
        options: Option<ShellExecOptions>,
    ) -> HarnessFuture<'a, Result<ShellExecOutput, ExecutionError>> {
        Box::pin(async move {
            let options = options.unwrap_or_default();
            if options
                .abort_signal
                .as_ref()
                .is_some_and(|signal| signal.aborted())
            {
                return Err(ExecutionError::new(
                    ExecutionErrorCode::Aborted,
                    "aborted",
                    None,
                ));
            }
            let timeout = resolve_timeout(options.timeout)?;
            let cwd = options
                .cwd
                .as_deref()
                .map_or_else(|| PathBuf::from(&self.cwd), |cwd| self.resolve_path(cwd));
            let shell_config = get_shell_config(self.shell_path.as_deref())?;
            let command = command.to_string();
            let shell_env = self.shell_env.clone();
            let (sender, receiver) = oneshot::channel();
            thread::spawn(move || {
                let _ = sender.send(exec_blocking(
                    &command,
                    &cwd,
                    &shell_config,
                    shell_env.as_ref(),
                    options,
                    timeout,
                ));
            });
            receiver.await.map_err(|_| {
                ExecutionError::new(ExecutionErrorCode::Unknown, "command worker stopped", None)
            })?
        })
    }

    fn cleanup<'a>(&'a self) -> HarnessFuture<'a, ()> {
        Box::pin(async {})
    }
}

fn exec_blocking(
    command: &str,
    cwd: &Path,
    shell_config: &ShellConfig,
    shell_env: Option<&HashMap<String, String>>,
    options: ShellExecOptions,
    timeout: Option<Duration>,
) -> Result<ShellExecOutput, ExecutionError> {
    let mut process = Command::new(&shell_config.shell);
    process.current_dir(cwd);
    process.envs(env::vars());
    if let Some(shell_env) = shell_env {
        process.envs(shell_env);
    }
    if let Some(extra_env) = &options.env {
        process.envs(extra_env);
    }
    process.stdout(Stdio::piped()).stderr(Stdio::piped());

    match shell_config.command_transport {
        CommandTransport::Argv => {
            process.stdin(Stdio::null());
            process.args(&shell_config.args).arg(command);
        }
        CommandTransport::Stdin => {
            process.stdin(Stdio::piped());
            process.args(&shell_config.args);
        }
    }

    let mut child = process.group_spawn().map_err(|error| {
        ExecutionError::new(
            ExecutionErrorCode::SpawnError,
            error.to_string(),
            Some(error.to_string()),
        )
    })?;

    if shell_config.command_transport == CommandTransport::Stdin {
        if let Some(mut stdin) = child.inner().stdin.take() {
            let _ = stdin.write_all(command.as_bytes());
        }
    }

    let stdout = Arc::new(Mutex::new(String::new()));
    let stderr = Arc::new(Mutex::new(String::new()));
    let callback_failed = Arc::new(AtomicBool::new(false));

    let stdout_thread = child.inner().stdout.take().map(|stream| {
        read_stream_thread(
            stream,
            Arc::clone(&stdout),
            options.on_stdout.clone(),
            Arc::clone(&callback_failed),
        )
    });
    let stderr_thread = child.inner().stderr.take().map(|stream| {
        read_stream_thread(
            stream,
            Arc::clone(&stderr),
            options.on_stderr.clone(),
            Arc::clone(&callback_failed),
        )
    });

    let status = wait_child(
        &mut child,
        timeout,
        options.abort_signal.as_ref(),
        &callback_failed,
    )?;

    if let Some(thread) = stdout_thread {
        let _ = thread.join();
    }
    if let Some(thread) = stderr_thread {
        let _ = thread.join();
    }

    if callback_failed.load(Ordering::SeqCst) {
        return Err(ExecutionError::new(
            ExecutionErrorCode::CallbackError,
            "output callback failed",
            None,
        ));
    }
    if options
        .abort_signal
        .as_ref()
        .is_some_and(|signal| signal.aborted())
    {
        return Err(ExecutionError::new(
            ExecutionErrorCode::Aborted,
            "aborted",
            None,
        ));
    }

    Ok(ShellExecOutput {
        stdout: take_string(&stdout),
        stderr: take_string(&stderr),
        exit_code: status.code().unwrap_or(0),
    })
}

fn wait_child(
    child: &mut GroupChild,
    timeout: Option<Duration>,
    abort_signal: Option<&zedflow_ai::AbortSignal>,
    callback_failed: &AtomicBool,
) -> Result<std::process::ExitStatus, ExecutionError> {
    let mut remaining = timeout;
    loop {
        if abort_signal.is_some_and(zedflow_ai::AbortSignal::aborted) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ExecutionError::new(
                ExecutionErrorCode::Aborted,
                "aborted",
                None,
            ));
        }
        if callback_failed.load(Ordering::SeqCst) {
            let _ = child.kill();
            return child.wait().map_err(to_execution_unknown);
        }

        if let Some(status) = child.try_wait().map_err(to_execution_unknown)? {
            return Ok(status);
        }

        let poll_for = remaining.map_or(Duration::from_millis(PROCESS_POLL_MS), |left| {
            left.min(Duration::from_millis(PROCESS_POLL_MS))
        });
        thread::sleep(poll_for);
        if let Some(left) = remaining.as_mut() {
            if *left <= poll_for {
                if child.try_wait().map_err(to_execution_unknown)?.is_none() {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(ExecutionError::new(
                        ExecutionErrorCode::Timeout,
                        "timeout",
                        None,
                    ));
                }
            } else {
                *left -= poll_for;
            }
        }
    }
}

fn read_stream_thread(
    stream: impl Read + Send + 'static,
    output: Arc<Mutex<String>>,
    callback: Option<Arc<dyn Fn(String) + Send + Sync>>,
    callback_failed: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut buffer = [0_u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    let chunk = String::from_utf8_lossy(&buffer[..count]).into_owned();
                    if let Ok(mut output) = output.lock() {
                        output.push_str(&chunk);
                    }
                    if let Some(callback) = &callback {
                        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            callback(chunk.clone());
                        }))
                        .is_err()
                        {
                            callback_failed.store(true, Ordering::SeqCst);
                            break;
                        }
                    }
                }
                Err(_) => break,
            }
        }
    })
}

fn resolve_timeout(timeout: Option<u64>) -> Result<Option<Duration>, ExecutionError> {
    let Some(timeout) = timeout else {
        return Ok(None);
    };
    if timeout == 0 {
        return Err(ExecutionError::new(
            ExecutionErrorCode::Timeout,
            "Invalid timeout: must be a finite number of seconds",
            None,
        ));
    }
    let timeout_ms = timeout.checked_mul(1000).ok_or_else(|| {
        ExecutionError::new(
            ExecutionErrorCode::Timeout,
            format!("Invalid timeout: maximum is {MAX_TIMEOUT_SECONDS} seconds"),
            None,
        )
    })?;
    if timeout_ms > MAX_TIMEOUT_MS {
        return Err(ExecutionError::new(
            ExecutionErrorCode::Timeout,
            format!("Invalid timeout: maximum is {MAX_TIMEOUT_SECONDS} seconds"),
            None,
        ));
    }
    Ok(Some(Duration::from_millis(timeout_ms)))
}

fn get_shell_config(custom_shell_path: Option<&str>) -> Result<ShellConfig, ExecutionError> {
    if let Some(custom_shell_path) = custom_shell_path {
        if Path::new(custom_shell_path).exists() {
            return Ok(get_bash_shell_config(custom_shell_path));
        }
        return Err(ExecutionError::new(
            ExecutionErrorCode::ShellUnavailable,
            format!("Custom shell path not found: {custom_shell_path}"),
            None,
        ));
    }

    #[cfg(windows)]
    {
        let mut candidates = Vec::new();
        if let Some(program_files) = env::var_os("ProgramFiles") {
            candidates.push(PathBuf::from(program_files).join("Git\\bin\\bash.exe"));
        }
        if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
            candidates.push(PathBuf::from(program_files_x86).join("Git\\bin\\bash.exe"));
        }
        for candidate in candidates {
            if candidate.exists() {
                return Ok(get_bash_shell_config(&candidate.to_string_lossy()));
            }
        }
        if let Some(bash) = find_bash_on_path() {
            return Ok(get_bash_shell_config(&bash));
        }
        return Err(ExecutionError::new(
            ExecutionErrorCode::ShellUnavailable,
            "No bash shell found",
            None,
        ));
    }

    #[cfg(not(windows))]
    {
        if Path::new("/bin/bash").exists() {
            return Ok(get_bash_shell_config("/bin/bash"));
        }
        if let Some(bash) = find_bash_on_path() {
            return Ok(get_bash_shell_config(&bash));
        }
        Ok(ShellConfig {
            shell: "sh".to_string(),
            args: vec!["-c".to_string()],
            command_transport: CommandTransport::Argv,
        })
    }
}

fn get_bash_shell_config(shell: &str) -> ShellConfig {
    if is_legacy_wsl_bash_path(shell) {
        ShellConfig {
            shell: shell.to_string(),
            args: vec!["-s".to_string()],
            command_transport: CommandTransport::Stdin,
        }
    } else {
        ShellConfig {
            shell: shell.to_string(),
            args: vec!["-c".to_string()],
            command_transport: CommandTransport::Argv,
        }
    }
}

fn find_bash_on_path() -> Option<String> {
    let (program, arg) = if cfg!(windows) {
        ("where", "bash.exe")
    } else {
        ("which", "bash")
    };
    let output = Command::new(program)
        .arg(arg)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .map(str::trim)
        .find(|line| Path::new(line).exists())
        .map(str::to_string)
}

fn is_legacy_wsl_bash_path(path: &str) -> bool {
    let normalized = path.replace('/', "\\").to_lowercase();
    let Some(rest) = normalized.strip_suffix("\\bash.exe") else {
        return false;
    };
    rest.as_bytes().get(1) == Some(&b':')
        && (rest.ends_with("\\windows\\system32") || rest.ends_with("\\windows\\sysnative"))
}

fn write_content(path: &Path, content: FileContent, append: bool) -> Result<(), FileError> {
    let mut options = OpenOptions::new();
    options.create(true).write(true);
    if append {
        options.append(true);
    } else {
        options.truncate(true);
    }
    let mut file = options
        .open(path)
        .map_err(|error| to_file_error(error, Some(path)))?;
    match content {
        FileContent::Text(content) => file.write_all(content.as_bytes()),
        FileContent::Binary(content) => file.write_all(&content),
    }
    .map_err(|error| to_file_error(error, Some(path)))
}

fn file_info_from_metadata(path: &Path, metadata: fs::Metadata) -> Result<FileInfo, FileError> {
    let kind = if metadata.is_file() {
        FileKind::File
    } else if metadata.is_dir() {
        FileKind::Directory
    } else if metadata.file_type().is_symlink() {
        FileKind::Symlink
    } else {
        return Err(FileError::new(
            FileErrorCode::Invalid,
            "Unsupported file type",
            Some(path_string(path)),
            None,
        ));
    };
    Ok(FileInfo {
        name: path
            .file_name()
            .and_then(OsStr::to_str)
            .map_or_else(|| path_string(path), str::to_string),
        path: path_string(path),
        kind,
        size: metadata.len(),
        mtime_ms: metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |duration| {
                duration.as_millis().try_into().unwrap_or(u64::MAX)
            }),
    })
}

fn to_file_error(error: io::Error, path: Option<&Path>) -> FileError {
    let code = match error.kind() {
        io::ErrorKind::NotFound => FileErrorCode::NotFound,
        io::ErrorKind::PermissionDenied => FileErrorCode::PermissionDenied,
        io::ErrorKind::NotADirectory => FileErrorCode::NotDirectory,
        io::ErrorKind::IsADirectory => FileErrorCode::IsDirectory,
        io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData => FileErrorCode::Invalid,
        _ => FileErrorCode::Unknown,
    };
    FileError::new(
        code,
        error.to_string(),
        path.map(path_string),
        Some(error.to_string()),
    )
}

fn to_execution_unknown(error: io::Error) -> ExecutionError {
    ExecutionError::new(
        ExecutionErrorCode::Unknown,
        error.to_string(),
        Some(error.to_string()),
    )
}

fn aborted_file(
    signal: &Option<zedflow_ai::AbortSignal>,
    path: Option<String>,
) -> Option<FileError> {
    signal
        .as_ref()
        .filter(|signal| signal.aborted())
        .map(|_| FileError::new(FileErrorCode::Aborted, "aborted", path, None))
}

fn take_string(value: &Arc<Mutex<String>>) -> String {
    value.lock().map(|value| value.clone()).unwrap_or_default()
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn absolute_lexical(path: &Path) -> PathBuf {
    if path.is_absolute() {
        normalize_lexical(path)
    } else {
        normalize_lexical(
            env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(path),
        )
    }
}

fn normalize_lexical(path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

#[allow(dead_code)]
fn _system_time_millis(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |duration| {
        duration.as_millis().try_into().unwrap_or(u64::MAX)
    })
}
