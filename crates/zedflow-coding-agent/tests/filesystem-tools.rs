use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use zedflow_agent::types::{AgentTool, AgentToolResult, AgentToolResultContent};
use zedflow_coding_agent::find::{FindTool, FindToolInput, create_find_tool};
use zedflow_coding_agent::ls::{LsTool, LsToolInput, create_ls_tool};
use zedflow_coding_agent::read::{ReadTool, ReadToolInput, create_read_tool};
use zedflow_coding_agent::write::{WriteTool, WriteToolInput, create_write_tool};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "zedflow-filesystem-tools-{}-{}",
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

fn output<T>(result: &AgentToolResult<T>) -> &str {
    match result.content.first().unwrap() {
        AgentToolResultContent::Text(content) => &content.text,
        AgentToolResultContent::Image(_) => panic!("expected text output"),
    }
}

#[tokio::test]
async fn write_creates_parents_and_read_honors_relative_offset_and_limit() {
    let root = TempDir::new();
    let write = WriteTool::new(&root);
    let result = write
        .execute(WriteToolInput {
            path: "nested/file.txt".into(),
            content: "one\ntwo\nthree\nfour".into(),
        })
        .await
        .unwrap();
    assert_eq!(
        output(&result),
        "Successfully wrote 18 bytes to nested/file.txt"
    );

    let result = ReadTool::new(&root)
        .execute(ReadToolInput {
            path: "nested/file.txt".into(),
            offset: Some(2),
            limit: Some(2),
        })
        .await
        .unwrap();
    assert_eq!(
        output(&result),
        "two\nthree\n\n[1 more lines in file. Use offset=4 to continue.]"
    );
    assert!(result.details.is_none());
}

#[tokio::test]
async fn read_detects_images_by_content_and_base64_encodes_them() {
    let root = TempDir::new();
    let png_header = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
    fs::write(root.as_ref().join("image.txt"), png_header).unwrap();

    let result = ReadTool::new(&root)
        .execute(ReadToolInput {
            path: "image.txt".into(),
            offset: None,
            limit: None,
        })
        .await
        .unwrap();
    assert_eq!(output(&result), "Read image file [image/png]");
    match &result.content[1] {
        AgentToolResultContent::Image(image) => {
            assert_eq!(image.mime_type, "image/png");
            assert_eq!(image.data, "iVBORw0KGgoAAAANSUhEUg==");
        }
        AgentToolResultContent::Text(_) => panic!("expected image output"),
    }
}

#[tokio::test]
async fn read_truncation_is_deterministic_and_reports_continuation() {
    let root = TempDir::new();
    let content = (1..=2_001)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(root.as_ref().join("large.txt"), content).unwrap();

    let result = ReadTool::new(&root)
        .execute(ReadToolInput {
            path: "large.txt".into(),
            offset: None,
            limit: None,
        })
        .await
        .unwrap();
    let text = output(&result);
    assert!(text.starts_with("line 1\nline 2\n"));
    assert!(
        text.contains("line 2000\n\n[Showing lines 1-2000 of 2001. Use offset=2001 to continue.]")
    );
    assert!(!text.contains("line 2001"));
    assert_eq!(result.details.unwrap().truncation.output_lines, 2_000);
}

#[tokio::test]
async fn ls_sorts_entries_marks_directories_and_reports_limits() {
    let root = TempDir::new();
    fs::write(root.as_ref().join("b.txt"), "b").unwrap();
    fs::write(root.as_ref().join("A.txt"), "a").unwrap();
    fs::write(root.as_ref().join(".hidden"), "hidden").unwrap();
    fs::create_dir(root.as_ref().join("folder")).unwrap();

    let tool = LsTool::new(&root);
    let result = tool
        .execute(LsToolInput {
            path: None,
            limit: None,
        })
        .await
        .unwrap();
    assert_eq!(output(&result), ".hidden\nA.txt\nb.txt\nfolder/");

    let result = tool
        .execute(LsToolInput {
            path: Some(".".into()),
            limit: Some(2),
        })
        .await
        .unwrap();
    assert_eq!(
        output(&result),
        ".hidden\nA.txt\n\n[2 entries limit reached. Use limit=4 for more]"
    );
    assert_eq!(result.details.unwrap().entry_limit_reached, Some(2));
}

#[test]
fn factories_share_the_agent_runtime_details_type() {
    let tools: Vec<AgentTool> = vec![
        create_read_tool("."),
        create_write_tool("."),
        create_find_tool("."),
        create_ls_tool("."),
    ];
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.tool.name.as_str())
            .collect::<Vec<_>>(),
        ["read", "write", "find", "ls"]
    );
}

#[tokio::test]
async fn find_matches_path_globs_and_respects_gitignore() {
    let root = TempDir::new();
    fs::create_dir_all(root.as_ref().join("src/nested")).unwrap();
    fs::create_dir(root.as_ref().join(".secret")).unwrap();
    fs::write(root.as_ref().join("src/nested/kept.spec.ts"), "").unwrap();
    fs::write(root.as_ref().join("src/nested/ignored.spec.ts"), "").unwrap();
    fs::write(root.as_ref().join(".secret/hidden.spec.ts"), "").unwrap();
    fs::write(root.as_ref().join(".gitignore"), "ignored.spec.ts\n").unwrap();

    let tool = FindTool::new(&root);
    let result = tool
        .execute(FindToolInput {
            pattern: "src/**/*.spec.ts".into(),
            path: None,
            limit: None,
        })
        .await
        .unwrap();
    assert_eq!(output(&result), "src/nested/kept.spec.ts");

    let result = tool
        .execute(FindToolInput {
            pattern: "**/*.spec.ts".into(),
            path: Some(".".into()),
            limit: Some(1),
        })
        .await
        .unwrap();
    let text = output(&result);
    assert!(!text.contains("ignored.spec.ts"));
    assert!(text.contains("1 results limit reached. Use limit=2 for more, or refine pattern"));
    assert_eq!(result.details.unwrap().result_limit_reached, Some(1));
}
