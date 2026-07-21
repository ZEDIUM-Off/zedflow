use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use zedflow_agent::types::{AgentToolResult, AgentToolResultContent, ToolSchema};
use zedflow_coding_agent::edit::{EditTool, EditToolInput, create_edit_tool};
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
        "keep  \nＡＢＣ１２３\ncafe\u{301}\n你好，世界\nafter   \n",
    )
    .unwrap();
    EditTool::new(&root)
        .execute(EditToolInput {
            path: "fuzzy.txt".into(),
            edits: vec![Edit {
                old_text: "ABC123\ncafé\n你好,世界\n".into(),
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
