use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use zedflow_agent::types::{AgentToolResult, AgentToolResultContent, ToolSchema};
use zedflow_coding_agent::find::{FindToolInput, execute_find};
use zedflow_coding_agent::ls::{LsToolInput, execute_ls};
use zedflow_coding_agent::read::{ReadToolInput, execute_read};
use zedflow_coding_agent::write::{WriteToolInput, execute_write};

fn temp_dir(label: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "zedflow-filesystem-tools-{}-{label}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path
}

fn tiny_bmp() -> Vec<u8> {
    let mut bytes = vec![0; 58];
    bytes[..2].copy_from_slice(b"BM");
    bytes[2..6].copy_from_slice(&58_u32.to_le_bytes());
    bytes[10..14].copy_from_slice(&54_u32.to_le_bytes());
    bytes[14..18].copy_from_slice(&40_u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&1_i32.to_le_bytes());
    bytes[22..26].copy_from_slice(&1_i32.to_le_bytes());
    bytes[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&24_u16.to_le_bytes());
    bytes[34..38].copy_from_slice(&4_u32.to_le_bytes());
    bytes[56] = 0xff;
    bytes
}

fn text(result: &AgentToolResult<ToolSchema>) -> String {
    result
        .content
        .iter()
        .filter_map(|block| match block {
            AgentToolResultContent::Text(content) => Some(content.text.as_str()),
            AgentToolResultContent::Image(_) => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn read_and_write_preserve_relative_paths_offsets_and_truncation() {
    let dir = temp_dir("read-write");
    let lines = (1..=2_001)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    execute_write(
        &dir,
        WriteToolInput {
            path: "nested/file.txt".into(),
            content: lines,
        },
        None,
    )
    .await
    .unwrap();

    let limited = execute_read(
        &dir,
        ReadToolInput {
            path: "nested/file.txt".into(),
            offset: Some(10),
            limit: Some(2),
        },
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        text(&limited),
        "line 10\nline 11\n\n[1990 more lines in file. Use offset=12 to continue.]"
    );

    let truncated = execute_read(
        &dir,
        ReadToolInput {
            path: "nested/file.txt".into(),
            offset: None,
            limit: None,
        },
        None,
    )
    .await
    .unwrap();
    assert!(
        text(&truncated).ends_with("[Showing lines 1-2000 of 2001. Use offset=2001 to continue.]")
    );
    assert_eq!(truncated.details["truncation"]["truncatedBy"], "lines");

    fs::write(dir.join("pixel.bmp"), tiny_bmp()).unwrap();
    let image = execute_read(
        &dir,
        ReadToolInput {
            path: "pixel.bmp".into(),
            offset: None,
            limit: None,
        },
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        text(&image),
        "Read image file [image/png]\n[Image converted from image/bmp to image/png.]"
    );
    assert!(matches!(
        image.content.get(1),
        Some(AgentToolResultContent::Image(content)) if content.mime_type == "image/png"
    ));
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn find_respects_gitignore_path_globs_and_result_limits() {
    let dir = temp_dir("find");
    fs::create_dir_all(dir.join("src/nested")).unwrap();
    fs::write(dir.join(".gitignore"), "ignored.rs\n").unwrap();
    fs::write(dir.join("src/ignored.rs"), "").unwrap();
    fs::write(dir.join("src/root.rs"), "").unwrap();
    fs::write(dir.join("src/nested/deep.rs"), "").unwrap();

    let all = execute_find(
        &dir,
        FindToolInput {
            pattern: "src/**/*.rs".into(),
            path: None,
            limit: None,
        },
        None,
    )
    .await
    .unwrap();
    assert_eq!(text(&all), "src/nested/deep.rs\nsrc/root.rs");

    let limited = execute_find(
        &dir,
        FindToolInput {
            pattern: "src/**/*.rs".into(),
            path: None,
            limit: Some(1),
        },
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        text(&limited),
        "src/root.rs\n\n[1 results limit reached. Use limit=2 for more, or refine pattern]"
    );
    assert_eq!(limited.details["resultLimitReached"], 1);
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn ls_is_sorted_includes_dotfiles_and_marks_directories() {
    let dir = temp_dir("ls");
    fs::write(dir.join("b.txt"), "").unwrap();
    fs::write(dir.join(".hidden"), "").unwrap();
    fs::create_dir(dir.join("A-dir")).unwrap();

    let result = execute_ls(
        &dir,
        LsToolInput {
            path: None,
            limit: None,
        },
        None,
    )
    .await
    .unwrap();
    assert_eq!(text(&result), ".hidden\nA-dir/\nb.txt");
    assert!(result.details.is_null());
    fs::remove_dir_all(dir).unwrap();
}
