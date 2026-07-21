use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::process::Command;

use zedflow_agent::types::{AgentToolResult, AgentToolResultContent, ToolSchema};
use zedflow_coding_agent::edit::{EditOperations, EditTool, EditToolInput, create_edit_tool};
use zedflow_coding_agent::edit_diff::{Edit, compute_edits_diff};
use zedflow_coding_agent::grep::{GrepTool, GrepToolInput};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "zedflow-search-edit-tools-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl AsRef<Path> for TempDir {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn json(value: &str) -> ToolSchema {
    serde_yaml::from_str(value).unwrap()
}

fn output<T>(result: &AgentToolResult<T>) -> &str {
    match result.content.first().unwrap() {
        AgentToolResultContent::Text(content) => &content.text,
        AgentToolResultContent::Image(_) => panic!("expected text output"),
    }
}

#[cfg(unix)]
#[tokio::test]
async fn grep_streams_and_stops_the_managed_rg_at_the_match_limit() {
    const CHILD: &str = "ZEDFLOW_MANAGED_RG_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let root = TempDir::new();
        let agent_dir = root.as_ref().join("agent");
        let bin_dir = agent_dir.join("bin");
        let search_dir = root.as_ref().join("search");
        fs::create_dir_all(&bin_dir).unwrap();
        fs::create_dir(&search_dir).unwrap();
        fs::write(search_dir.join("managed.txt"), "managed hit\n").unwrap();
        let rg = bin_dir.join("rg");
        fs::write(
            &rg,
            "#!/bin/sh\nfor last do :; done\nprintf '{\"type\":\"match\",\"data\":{\"path\":{\"text\":\"%s/managed.txt\"},\"lines\":{\"text\":\"managed hit\\\\n\"},\"line_number\":1}}\\n' \"$last\"\nwhile :; do :; done\n",
        )
        .unwrap();
        fs::set_permissions(&rg, fs::Permissions::from_mode(0o755)).unwrap();

        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "grep_streams_and_stops_the_managed_rg_at_the_match_limit",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env("PI_CODING_AGENT_DIR", agent_dir)
            .env("PI_OFFLINE", "1")
            .env("PATH", root.as_ref().join("empty-path"))
            .env("ZEDFLOW_MANAGED_SEARCH_ROOT", search_dir)
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }

    let root = PathBuf::from(std::env::var_os("ZEDFLOW_MANAGED_SEARCH_ROOT").unwrap());
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        GrepTool::new(&root).execute(GrepToolInput {
            pattern: "managed".into(),
            path: None,
            glob: None,
            ignore_case: None,
            literal: None,
            context: None,
            limit: Some(1),
        }),
    )
    .await
    .expect("grep should terminate rg as soon as the match limit is reached")
    .unwrap();
    assert!(output(&result).starts_with("managed.txt:1: managed hit"));
    assert_eq!(result.details.unwrap().match_limit_reached, Some(1));
}

#[tokio::test]
async fn grep_formats_file_paths_context_limits_and_long_lines() {
    let root = TempDir::new();
    fs::write(
        root.as_ref().join("context.txt"),
        format!("before\nmatch {}\nafter\nmatch two\n", "x".repeat(600)),
    )
    .unwrap();

    let result = GrepTool::new(&root)
        .execute(GrepToolInput {
            pattern: "match".into(),
            path: Some("context.txt".into()),
            glob: None,
            ignore_case: None,
            literal: None,
            context: Some(1),
            limit: Some(1),
        })
        .await
        .unwrap();

    let text = output(&result);
    assert!(text.contains("context.txt-1- before"));
    assert!(text.contains("context.txt:2: match "));
    assert!(text.contains("... [truncated]"));
    assert!(text.contains("[1 matches limit reached. Use limit=2 for more, or refine pattern"));
    assert!(!text.contains("match two"));
    let details = result.details.unwrap();
    assert_eq!(details.match_limit_reached, Some(1));
    assert!(details.lines_truncated);
}

#[tokio::test]
async fn edit_uses_injected_operations_without_local_files() {
    let root = std::env::current_dir().unwrap().join("virtual-edit-root");
    let expected_path = root.join("remote.txt");
    let calls = Arc::new(Mutex::new(Vec::new()));
    let written = Arc::new(Mutex::new(None));

    let access_calls = Arc::clone(&calls);
    let read_calls = Arc::clone(&calls);
    let write_calls = Arc::clone(&calls);
    let captured_write = Arc::clone(&written);
    let operations = EditOperations {
        access: Arc::new(move |path| {
            let calls = Arc::clone(&access_calls);
            Box::pin(async move {
                calls.lock().unwrap().push(("access", path));
                Ok(())
            })
        }),
        read_file: Arc::new(move |path| {
            let calls = Arc::clone(&read_calls);
            Box::pin(async move {
                calls.lock().unwrap().push(("read", path));
                Ok(b"before\r\nafter\r\n".to_vec())
            })
        }),
        write_file: Arc::new(move |path, content| {
            let calls = Arc::clone(&write_calls);
            let written = Arc::clone(&captured_write);
            Box::pin(async move {
                calls.lock().unwrap().push(("write", path.clone()));
                *written.lock().unwrap() = Some((path, content));
                Ok(())
            })
        }),
    };

    let result = EditTool::with_operations(&root, operations)
        .execute(EditToolInput {
            path: "remote.txt".into(),
            edits: vec![Edit {
                old_text: "before\nafter".into(),
                new_text: "changed".into(),
            }],
        })
        .await
        .unwrap();

    assert_eq!(
        output(&result),
        "Successfully replaced 1 block(s) in remote.txt."
    );
    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            ("access", expected_path.clone()),
            ("read", expected_path.clone()),
            ("write", expected_path.clone()),
        ]
    );
    assert_eq!(
        *written.lock().unwrap(),
        Some((expected_path, "changed\r\n".into()))
    );
}

#[tokio::test]
async fn edit_preserves_injected_access_errors() {
    let operations = EditOperations {
        access: Arc::new(|_| {
            Box::pin(async {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "remote backend denied access",
                ))
            })
        }),
        read_file: Arc::new(|_| Box::pin(async { panic!("read must not run") })),
        write_file: Arc::new(|_, _| Box::pin(async { panic!("write must not run") })),
    };

    let error = EditTool::with_operations("virtual-edit-root", operations)
        .execute(EditToolInput {
            path: "remote.txt".into(),
            edits: vec![Edit {
                old_text: "before".into(),
                new_text: "after".into(),
            }],
        })
        .await
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "Could not edit file: remote.txt. remote backend denied access."
    );
}

#[tokio::test]
async fn edit_prepares_legacy_input_and_preserves_bom_and_crlf() {
    let root = TempDir::new();
    fs::write(
        root.as_ref().join("legacy.txt"),
        "\u{feff}before\r\nafter\r\n",
    )
    .unwrap();
    let tool = create_edit_tool(&root);
    assert!(tool.tool.parameters["properties"].get("oldText").is_none());

    let prepared = tool.prepare_arguments.as_ref().unwrap()(json(
        r#"{"path":"legacy.txt","oldText":"before\n","newText":"changed\n"}"#,
    ))
    .unwrap();
    assert_eq!(
        prepared,
        json(r#"{"path":"legacy.txt","edits":[{"oldText":"before\n","newText":"changed\n"}]}"#)
    );
    let stringified = tool.prepare_arguments.as_ref().unwrap()(json(
        r#"{"path":"legacy.txt","edits":"[{\"oldText\":\"before\",\"newText\":\"changed\"}]"}"#,
    ))
    .unwrap();
    assert!(stringified["edits"].is_array());

    let result = (tool.execute)("edit-1", prepared, None, None)
        .await
        .unwrap();
    assert_eq!(
        output(&result),
        "Successfully replaced 1 block(s) in legacy.txt."
    );
    assert_eq!(
        fs::read_to_string(root.as_ref().join("legacy.txt")).unwrap(),
        "\u{feff}changed\r\nafter\r\n"
    );
}

#[tokio::test]
async fn edit_result_reuses_the_large_preview_diff() {
    let root = TempDir::new();
    let content = (1..=1_000)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(root.as_ref().join("preview.txt"), content).unwrap();
    let edits = vec![
        Edit {
            old_text: "line 50\n".into(),
            new_text: "line 50 changed\n".into(),
        },
        Edit {
            old_text: "line 950\n".into(),
            new_text: "line 950 changed\n".into(),
        },
    ];
    let preview = compute_edits_diff("preview.txt", &edits, &root)
        .await
        .unwrap();
    let result = EditTool::new(&root)
        .execute(EditToolInput {
            path: "preview.txt".into(),
            edits,
        })
        .await
        .unwrap();

    assert_eq!(result.details.diff, preview.diff);
    assert_eq!(result.details.first_changed_line, Some(50));
    assert!(result.details.diff.contains("line 50 changed"));
    assert!(result.details.diff.contains("line 950 changed"));
    assert!(result.details.diff.contains("..."));
    assert!(result.details.diff.lines().count() < 30);
    assert!(result.details.patch.contains("--- preview.txt"));
    assert!(result.details.patch.contains("+++ preview.txt"));
}

#[tokio::test]
async fn edit_matches_all_entries_against_the_original_and_rejects_overlaps() {
    let root = TempDir::new();
    fs::write(root.as_ref().join("multi.txt"), "foo\nbar\nbaz\n").unwrap();
    EditTool::new(&root)
        .execute(EditToolInput {
            path: "multi.txt".into(),
            edits: vec![
                Edit {
                    old_text: "foo\n".into(),
                    new_text: "foo bar\n".into(),
                },
                Edit {
                    old_text: "bar\n".into(),
                    new_text: "BAR\n".into(),
                },
            ],
        })
        .await
        .unwrap();
    assert_eq!(
        fs::read_to_string(root.as_ref().join("multi.txt")).unwrap(),
        "foo bar\nBAR\nbaz\n"
    );

    fs::write(root.as_ref().join("overlap.txt"), "one\ntwo\nthree\n").unwrap();
    let error = EditTool::new(&root)
        .execute(EditToolInput {
            path: "overlap.txt".into(),
            edits: vec![
                Edit {
                    old_text: "one\ntwo\n".into(),
                    new_text: "ONE\nTWO\n".into(),
                },
                Edit {
                    old_text: "two\nthree\n".into(),
                    new_text: "TWO\nTHREE\n".into(),
                },
            ],
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("overlap"));
}

#[tokio::test]
async fn fuzzy_edit_preserves_untouched_line_bytes() {
    let root = TempDir::new();
    fs::write(
        root.as_ref().join("fuzzy.txt"),
        "keep  \nＡＢＣ１２３\ncafe\u{301}\n가\n㍑\n你好，世界\nafter   \n",
    )
    .unwrap();
    EditTool::new(&root)
        .execute(EditToolInput {
            path: "fuzzy.txt".into(),
            edits: vec![Edit {
                old_text: "ABC123\ncafé\n가\nリットル\n你好,世界\n".into(),
                new_text: "changed\n".into(),
            }],
        })
        .await
        .unwrap();
    assert_eq!(
        fs::read_to_string(root.as_ref().join("fuzzy.txt")).unwrap(),
        "keep  \nchanged\nafter   \n"
    );
}
