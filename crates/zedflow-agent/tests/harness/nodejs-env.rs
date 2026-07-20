use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use futures::executor::block_on;
use futures::task::noop_waker;
use zedflow_agent::harness::env::nodejs::{NodeExecutionEnv, NodeExecutionEnvOptions};
use zedflow_agent::harness::types::{
    CreateDirOptions, CreateTempFileOptions, FileContent, FileErrorCode, FileKind, FileSystem,
    ReadTextLinesOptions, RemoveOptions, Shell, ShellExecOptions,
};
use zedflow_agent::harness::utils::shell_output::execute_shell_with_capture;
use zedflow_agent::proxy::{
    ProxyAssistantMessageEvent, ProxyEventState, initial_proxy_assistant_message,
    process_proxy_event, process_proxy_event_json,
};
use zedflow_ai::utils::abort_signals::AbortController;
use zedflow_ai::{
    AssistantContentBlock, AssistantMessageEvent, DoneStopReason, ErrorStopReason, Model,
    StopReason, Usage,
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "zedflow-agent-nodejs-env-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp test directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn env(&self) -> NodeExecutionEnv {
        NodeExecutionEnv::with_cwd(self.path.to_string_lossy())
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn path_string(path: impl AsRef<Path>) -> String {
    path.as_ref().to_string_lossy().into_owned()
}

#[test]
fn reads_writes_lists_and_removes_files_and_directories() {
    let temp = TempDir::new();
    let env = temp.env();
    let root = temp.path();

    assert_eq!(
        block_on(env.absolute_path("nested/child", None)).unwrap(),
        path_string(root.join("nested/child"))
    );
    assert_eq!(
        block_on(env.join_path(&[path_string(root), "nested".into(), "child".into()], None))
            .unwrap(),
        path_string(root.join("nested/child"))
    );

    block_on(env.create_dir("nested/child", CreateDirOptions::default())).unwrap();
    block_on(env.write_file(
        "nested/child/file.txt",
        FileContent::Text("hel".into()),
        None,
    ))
    .unwrap();
    block_on(env.append_file(
        "nested/child/file.txt",
        FileContent::Text("lo".into()),
        None,
    ))
    .unwrap();

    assert_eq!(
        block_on(env.read_text_file("nested/child/file.txt", None)).unwrap(),
        "hello"
    );
    assert_eq!(
        block_on(env.read_text_lines(
            "nested/child/file.txt",
            ReadTextLinesOptions {
                max_lines: Some(1),
                abort_signal: None,
            },
        ))
        .unwrap(),
        vec!["hello"]
    );
    assert_eq!(
        block_on(env.read_binary_file("nested/child/file.txt", None)).unwrap(),
        b"hello"
    );

    let entries = block_on(env.list_dir("nested/child", None)).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "file.txt");
    assert_eq!(
        entries[0].path,
        path_string(root.join("nested/child/file.txt"))
    );
    assert_eq!(entries[0].kind, FileKind::File);
    assert_eq!(entries[0].size, 5);

    assert!(block_on(env.exists("nested/child/file.txt", None)).unwrap());
    block_on(env.remove("nested/child/file.txt", RemoveOptions::default())).unwrap();
    assert!(!block_on(env.exists("nested/child/file.txt", None)).unwrap());
}

#[cfg(unix)]
#[test]
fn file_info_and_list_dir_report_symlinks_without_following_them() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new();
    let env = temp.env();
    let root = temp.path();

    block_on(env.write_file("dir/file.txt", FileContent::Text("hello".into()), None)).unwrap();
    symlink(root.join("dir/file.txt"), root.join("file-link")).unwrap();
    symlink(root.join("dir"), root.join("dir-link")).unwrap();

    let dir = block_on(env.file_info("dir", None)).unwrap();
    assert_eq!(dir.kind, FileKind::Directory);
    let file = block_on(env.file_info("dir/file.txt", None)).unwrap();
    assert_eq!(file.kind, FileKind::File);
    assert_eq!(file.size, 5);
    assert_eq!(
        block_on(env.file_info("file-link", None)).unwrap().kind,
        FileKind::Symlink
    );
    assert_eq!(
        block_on(env.file_info("dir-link", None)).unwrap().kind,
        FileKind::Symlink
    );
    assert_eq!(
        block_on(env.canonical_path("file-link", None)).unwrap(),
        path_string(fs::canonicalize(root.join("dir/file.txt")).unwrap())
    );

    let mut entries: Vec<_> = block_on(env.list_dir(".", None))
        .unwrap()
        .into_iter()
        .map(|entry| (entry.name, entry.kind))
        .collect();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    assert_eq!(
        entries,
        vec![
            ("dir".to_string(), FileKind::Directory),
            ("dir-link".to_string(), FileKind::Symlink),
            ("file-link".to_string(), FileKind::Symlink),
        ]
    );
}

#[test]
fn reports_file_errors_and_honors_create_remove_options() {
    let temp = TempDir::new();
    let env = temp.env();
    let root = temp.path();

    let missing = block_on(env.file_info("missing.txt", None)).unwrap_err();
    assert_eq!(missing.code, FileErrorCode::NotFound);
    assert_eq!(missing.path, Some(path_string(root.join("missing.txt"))));
    assert!(!block_on(env.exists("missing.txt", None)).unwrap());

    block_on(env.write_file("file.txt", FileContent::Text("hello".into()), None)).unwrap();
    assert_eq!(
        block_on(env.list_dir("file.txt", None)).unwrap_err().code,
        FileErrorCode::NotDirectory
    );

    let create = block_on(env.create_dir(
        "missing/child",
        CreateDirOptions {
            recursive: false,
            abort_signal: None,
        },
    ));
    assert_eq!(create.unwrap_err().code, FileErrorCode::NotFound);

    block_on(env.write_file(
        "dir/child/file.txt",
        FileContent::Text("hello".into()),
        None,
    ))
    .unwrap();
    assert!(block_on(env.remove("dir", RemoveOptions::default())).is_err());
    block_on(env.remove(
        "dir",
        RemoveOptions {
            recursive: true,
            force: false,
            abort_signal: None,
        },
    ))
    .unwrap();
    assert!(!block_on(env.exists("dir", None)).unwrap());

    assert!(block_on(env.remove("missing", RemoveOptions::default())).is_err());
    block_on(env.remove(
        "missing",
        RemoveOptions {
            recursive: false,
            force: true,
            abort_signal: None,
        },
    ))
    .unwrap();
}

#[test]
fn creates_temp_paths_and_returns_aborted_file_results() {
    let temp = TempDir::new();
    let env = temp.env();
    block_on(env.write_file("file.txt", FileContent::Text("hello".into()), None)).unwrap();

    let temp_dir = block_on(env.create_temp_dir(Some("node-env-test-"), None)).unwrap();
    assert!(Path::new(&temp_dir).is_dir());
    let temp_file = block_on(env.create_temp_file(CreateTempFileOptions {
        prefix: Some("prefix-".into()),
        suffix: Some(".txt".into()),
        abort_signal: None,
    }))
    .unwrap();
    assert!(Path::new(&temp_file).is_file());
    assert!(temp_file.ends_with(".txt"));

    let controller = AbortController::new();
    controller.abort();
    let signal = controller.signal();
    assert_eq!(
        block_on(env.read_text_file("file.txt", Some(signal.clone())))
            .unwrap_err()
            .code,
        FileErrorCode::Aborted
    );
    assert_eq!(
        block_on(env.read_binary_file("file.txt", Some(signal.clone())))
            .unwrap_err()
            .code,
        FileErrorCode::Aborted
    );
    assert_eq!(
        block_on(env.list_dir(".", Some(signal))).unwrap_err().code,
        FileErrorCode::Aborted
    );

    let _ = fs::remove_dir_all(temp_dir);
    let _ = fs::remove_file(temp_file);
}

#[test]
fn executes_commands_in_cwd_with_env_and_stream_callbacks() {
    let temp = TempDir::new();
    let env = temp.env();

    let output = block_on(env.exec(
        "printf '%s:%s' \"$PWD\" \"$NODE_ENV_TEST\"",
        Some(ShellExecOptions {
            env: Some(HashMap::from([(
                "NODE_ENV_TEST".to_string(),
                "ok".to_string(),
            )])),
            ..ShellExecOptions::default()
        }),
    ))
    .unwrap();
    assert_eq!(output.stdout, format!("{}:ok", path_string(temp.path())));
    assert_eq!(output.stderr, "");
    assert_eq!(output.exit_code, 0);

    let stdout = Arc::new(Mutex::new(String::new()));
    let stderr = Arc::new(Mutex::new(String::new()));
    let stdout_sink = Arc::clone(&stdout);
    let stderr_sink = Arc::clone(&stderr);
    let output = block_on(env.exec(
        "printf out; printf err >&2",
        Some(ShellExecOptions {
            on_stdout: Some(Arc::new(move |chunk| {
                stdout_sink.lock().unwrap().push_str(&chunk)
            })),
            on_stderr: Some(Arc::new(move |chunk| {
                stderr_sink.lock().unwrap().push_str(&chunk)
            })),
            ..ShellExecOptions::default()
        }),
    ))
    .unwrap();
    assert_eq!(output.stdout, "out");
    assert_eq!(output.stderr, "err");
    assert_eq!(*stdout.lock().unwrap(), "out");
    assert_eq!(*stderr.lock().unwrap(), "err");
}

#[test]
fn reports_nonzero_timeout_callback_and_shell_errors() {
    let temp = TempDir::new();
    let env = temp.env();

    let output = block_on(env.exec("exit 7", None)).unwrap();
    assert_eq!(output.exit_code, 7);

    let timeout = block_on(env.exec(
        "sleep 2",
        Some(ShellExecOptions {
            timeout: Some(1),
            ..ShellExecOptions::default()
        }),
    ))
    .unwrap_err();
    assert_eq!(
        timeout.code,
        zedflow_agent::harness::types::ExecutionErrorCode::Timeout
    );

    let callback_error = block_on(env.exec(
        "printf out",
        Some(ShellExecOptions {
            on_stdout: Some(Arc::new(|_| panic!("callback failed"))),
            ..ShellExecOptions::default()
        }),
    ))
    .unwrap_err();
    assert_eq!(
        callback_error.code,
        zedflow_agent::harness::types::ExecutionErrorCode::CallbackError
    );

    let missing_shell = NodeExecutionEnv::new(NodeExecutionEnvOptions {
        cwd: path_string(temp.path()),
        shell_path: Some(path_string(temp.path().join("missing-shell"))),
        shell_env: None,
    });
    let error = block_on(missing_shell.exec("printf ok", None)).unwrap_err();
    assert_eq!(
        error.code,
        zedflow_agent::harness::types::ExecutionErrorCode::ShellUnavailable
    );

    let controller = AbortController::new();
    controller.abort();
    let aborted = block_on(env.exec(
        "printf ok",
        Some(ShellExecOptions {
            abort_signal: Some(controller.signal()),
            ..ShellExecOptions::default()
        }),
    ))
    .unwrap_err();
    assert_eq!(
        aborted.code,
        zedflow_agent::harness::types::ExecutionErrorCode::Aborted
    );
}

#[cfg(target_os = "linux")]
fn assert_process_gone(pid_file: &Path) {
    let pid = fs::read_to_string(pid_file).unwrap();
    let process = Path::new("/proc").join(pid.trim());
    for _ in 0..20 {
        if !process.exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("process {} survived", pid.trim());
}

#[cfg(target_os = "linux")]
fn process_tree_command() -> &'static str {
    "sh -c 'sleep 30 & echo $! > grandchild.pid; wait' & echo $! > child.pid; wait"
}

#[cfg(target_os = "linux")]
#[test]
fn timeout_kills_the_shell_process_tree() {
    let temp = TempDir::new();
    let error = block_on(temp.env().exec(
        process_tree_command(),
        Some(ShellExecOptions {
            timeout: Some(1),
            ..ShellExecOptions::default()
        }),
    ))
    .unwrap_err();
    assert_eq!(
        error.code,
        zedflow_agent::harness::types::ExecutionErrorCode::Timeout
    );
    assert_process_gone(&temp.path().join("child.pid"));
    assert_process_gone(&temp.path().join("grandchild.pid"));
}

#[cfg(target_os = "linux")]
#[test]
fn dropping_exec_future_kills_the_shell_process_tree() {
    let temp = TempDir::new();
    let env = temp.env();
    let mut future = env.exec(process_tree_command(), None);
    let waker = noop_waker();
    assert!(matches!(
        future.as_mut().poll(&mut Context::from_waker(&waker)),
        Poll::Pending
    ));

    for _ in 0..20 {
        if temp.path().join("grandchild.pid").exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(temp.path().join("grandchild.pid").exists());
    drop(future);

    assert_process_gone(&temp.path().join("child.pid"));
    assert_process_gone(&temp.path().join("grandchild.pid"));
}

#[cfg(target_os = "linux")]
#[test]
fn abort_kills_the_shell_process_tree() {
    let temp = TempDir::new();
    let controller = AbortController::new();
    let aborter = controller.clone();
    let abort_thread = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(250));
        aborter.abort();
    });
    let error = block_on(temp.env().exec(
        process_tree_command(),
        Some(ShellExecOptions {
            abort_signal: Some(controller.signal()),
            ..ShellExecOptions::default()
        }),
    ))
    .unwrap_err();
    abort_thread.join().unwrap();
    assert_eq!(
        error.code,
        zedflow_agent::harness::types::ExecutionErrorCode::Aborted
    );
    assert_process_gone(&temp.path().join("child.pid"));
    assert_process_gone(&temp.path().join("grandchild.pid"));
}

#[cfg(unix)]
#[test]
fn reports_spawn_error_for_non_executable_custom_shell() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempDir::new();
    let shell = temp.path().join("not-executable-shell");
    fs::write(&shell, "not executable").unwrap();
    fs::set_permissions(&shell, fs::Permissions::from_mode(0o644)).unwrap();

    let env = NodeExecutionEnv::new(NodeExecutionEnvOptions {
        cwd: path_string(temp.path()),
        shell_path: Some(path_string(shell)),
        shell_env: None,
    });
    let error = block_on(env.exec("printf ok", None)).unwrap_err();
    assert_eq!(
        error.code,
        zedflow_agent::harness::types::ExecutionErrorCode::SpawnError
    );
}

#[test]
fn captures_large_shell_output_to_full_output_file() {
    let temp = TempDir::new();
    let env = temp.env();
    let result = block_on(execute_shell_with_capture(
        &env,
        "i=0; while [ \"$i\" -lt 15000 ]; do printf 'line\\n'; i=$((i+1)); done",
        None,
    ))
    .unwrap();

    assert!(result.truncated);
    let full_output_path = result.full_output_path.expect("full output path");
    let full_output = block_on(env.read_text_file(&full_output_path, None)).unwrap();
    assert!(full_output.lines().count() >= 15000);
    assert!(result.output.len() < full_output.len());
    let _ = fs::remove_file(full_output_path);
}

#[test]
fn reconstructs_proxy_text_tool_and_terminal_events() {
    let model = Model {
        id: "model-id".into(),
        provider: "provider-id".into(),
        ..Model::default()
    };
    let partial = initial_proxy_assistant_message(&model);
    let mut state = ProxyEventState::default();

    assert!(matches!(
        process_proxy_event_json(r#"{"type":"start"}"#, &partial, &mut state)
            .unwrap()
            .unwrap(),
        AssistantMessageEvent::Start { .. }
    ));

    process_proxy_event_json(
        r#"{"type":"text_start","contentIndex":0}"#,
        &partial,
        &mut state,
    )
    .unwrap();
    let text_delta = process_proxy_event_json(
        r#"{"type":"text_delta","contentIndex":0,"delta":"hello"}"#,
        &partial,
        &mut state,
    )
    .unwrap()
    .unwrap();
    assert!(matches!(
        text_delta,
        AssistantMessageEvent::TextDelta { delta, .. } if delta == "hello"
    ));
    process_proxy_event_json(
        r#"{"type":"text_end","contentIndex":0,"contentSignature":"sig"}"#,
        &partial,
        &mut state,
    )
    .unwrap();
    let snapshot = partial.snapshot();
    match &snapshot.content[0] {
        AssistantContentBlock::Text(content) => {
            assert_eq!(content.text, "hello");
            assert_eq!(content.text_signature.as_deref(), Some("sig"));
        }
        other => panic!("expected text content, got {other:?}"),
    }

    process_proxy_event(
        ProxyAssistantMessageEvent::ToolcallStart {
            content_index: 1,
            id: "call-1".into(),
            tool_name: "calculate".into(),
        },
        &partial,
        &mut state,
    )
    .unwrap();
    process_proxy_event(
        ProxyAssistantMessageEvent::ToolcallDelta {
            content_index: 1,
            delta: r#"{"expression":"#.into(),
        },
        &partial,
        &mut state,
    )
    .unwrap();
    process_proxy_event(
        ProxyAssistantMessageEvent::ToolcallDelta {
            content_index: 1,
            delta: r#""2 + 2"}"#.into(),
        },
        &partial,
        &mut state,
    )
    .unwrap();
    let tool_end = process_proxy_event(
        ProxyAssistantMessageEvent::ToolcallEnd { content_index: 1 },
        &partial,
        &mut state,
    )
    .unwrap()
    .unwrap();
    match tool_end {
        AssistantMessageEvent::ToolcallEnd { tool_call, .. } => {
            assert_eq!(tool_call.id, "call-1");
            assert_eq!(tool_call.name, "calculate");
            assert_eq!(tool_call.arguments["expression"], "2 + 2");
        }
        other => panic!("expected toolcall_end, got {other:?}"),
    }

    let usage = Usage {
        input: 1,
        output: 2,
        total_tokens: 3,
        ..Usage::default()
    };
    let done = process_proxy_event(
        ProxyAssistantMessageEvent::Done {
            reason: DoneStopReason::ToolUse,
            usage: usage.clone(),
        },
        &partial,
        &mut state,
    )
    .unwrap()
    .unwrap();
    match done {
        AssistantMessageEvent::Done { reason, message } => {
            assert_eq!(reason, DoneStopReason::ToolUse);
            assert_eq!(message.stop_reason, StopReason::ToolUse);
            assert_eq!(message.usage, usage);
        }
        other => panic!("expected done, got {other:?}"),
    }
}

#[test]
fn rejects_proxy_deltas_for_the_wrong_content_kind_and_maps_error_events() {
    let model = Model::default();
    let partial = initial_proxy_assistant_message(&model);
    let mut state = ProxyEventState::default();

    process_proxy_event_json(
        r#"{"type":"text_start","contentIndex":0}"#,
        &partial,
        &mut state,
    )
    .unwrap();
    let error = process_proxy_event_json(
        r#"{"type":"toolcall_delta","contentIndex":0,"delta":"{}"}"#,
        &partial,
        &mut state,
    )
    .unwrap_err();
    assert!(error.to_string().contains("non-toolCall"));

    let event = process_proxy_event(
        ProxyAssistantMessageEvent::Error {
            reason: ErrorStopReason::Error,
            error_message: Some("boom".into()),
            usage: Usage::default(),
        },
        &partial,
        &mut state,
    )
    .unwrap()
    .unwrap();
    match event {
        AssistantMessageEvent::Error { reason, error } => {
            assert_eq!(reason, ErrorStopReason::Error);
            assert_eq!(error.stop_reason, StopReason::Error);
            assert_eq!(error.error_message.as_deref(), Some("boom"));
        }
        other => panic!("expected error event, got {other:?}"),
    }
}
