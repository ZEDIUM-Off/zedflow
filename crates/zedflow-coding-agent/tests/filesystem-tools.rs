use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::process::Command;

use zedflow_agent::types::{AgentTool, AgentToolResult, AgentToolResultContent};
use zedflow_coding_agent::find::{FindOperations, FindTool, FindToolInput, create_find_tool};
use zedflow_coding_agent::ls::{
    LsOperations, LsTool, LsToolInput, LsToolOptions, create_ls_tool, create_ls_tool_definition,
};
use zedflow_coding_agent::read::{ReadOperations, ReadTool, ReadToolInput, create_read_tool};
use zedflow_coding_agent::write::{
    WriteOperations, WriteTool, WriteToolInput, WriteToolOptions, create_write_tool,
    create_write_tool_with_options,
};

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

fn tiny_bmp_1x1_red() -> Vec<u8> {
    let mut bytes = vec![0; 58];
    bytes[0..2].copy_from_slice(b"BM");
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
async fn read_omits_detected_images_that_cannot_be_processed() {
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
    assert_eq!(result.content.len(), 1);
    assert_eq!(
        output(&result),
        "Read image file [image/png]\n[Image omitted: could not be resized below the inline image size limit.]"
    );
}

#[tokio::test]
async fn read_converts_bmp_before_attaching_it() {
    let root = TempDir::new();
    fs::write(root.as_ref().join("image.bmp"), tiny_bmp_1x1_red()).unwrap();

    let result = ReadTool::new(&root)
        .execute(ReadToolInput {
            path: "image.bmp".into(),
            offset: None,
            limit: None,
        })
        .await
        .unwrap();
    assert_eq!(
        output(&result),
        "Read image file [image/png]\n[Image converted from image/bmp to image/png.]"
    );
    match &result.content[1] {
        AgentToolResultContent::Image(image) => {
            assert_eq!(image.mime_type, "image/png");
            assert!(image.data.starts_with("iVBORw0KGgo"));
        }
        AgentToolResultContent::Text(_) => panic!("expected image output"),
    }
}

#[tokio::test]
async fn read_uses_injected_access_mime_and_file_operations() {
    let root = std::env::temp_dir().join("zedflow-virtual-read-root");
    let path = root.join("image.bin");
    let calls = Arc::new(Mutex::new(Vec::new()));
    let access_calls = Arc::clone(&calls);
    let mime_calls = Arc::clone(&calls);
    let read_calls = Arc::clone(&calls);
    let image = tiny_bmp_1x1_red();
    let operations = ReadOperations {
        access: Arc::new(move |path| {
            let calls = Arc::clone(&access_calls);
            Box::pin(async move {
                calls.lock().unwrap().push(("access", path));
                Ok(())
            })
        }),
        detect_image_mime_type: Some(Arc::new(move |path| {
            let calls = Arc::clone(&mime_calls);
            Box::pin(async move {
                calls.lock().unwrap().push(("mime", path));
                Ok(Some("image/bmp".into()))
            })
        })),
        read_file: Arc::new(move |path| {
            let calls = Arc::clone(&read_calls);
            let image = image.clone();
            Box::pin(async move {
                calls.lock().unwrap().push(("read", path));
                Ok(image)
            })
        }),
    };

    let result = ReadTool::with_operations(&root, operations)
        .execute(ReadToolInput {
            path: "image.bin".into(),
            offset: None,
            limit: None,
        })
        .await
        .unwrap();

    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            ("access", path.clone()),
            ("mime", path.clone()),
            ("read", path)
        ]
    );
    assert_eq!(
        output(&result),
        "Read image file [image/png]\n[Image converted from image/bmp to image/png.]"
    );
    assert!(matches!(
        result.content.get(1),
        Some(AgentToolResultContent::Image(_))
    ));
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
            limit: Some(2.0),
        })
        .await
        .unwrap();
    assert_eq!(
        output(&result),
        ".hidden\nA.txt\n\n[2 entries limit reached. Use limit=4 for more]"
    );
    assert_eq!(result.details.unwrap().entry_limit_reached, Some(2.0));
}

#[tokio::test]
async fn ls_preserves_fractional_and_negative_number_limits() {
    let root = TempDir::new();
    for name in ["a", "b", "c"] {
        fs::write(root.as_ref().join(name), name).unwrap();
    }
    let tool = create_ls_tool(&root);

    let result = (tool.execute)(
        "ls-fractional",
        serde_yaml::from_str(r#"{"limit":1.5}"#).unwrap(),
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        output(&result),
        "a\nb\n\n[1.5 entries limit reached. Use limit=3 for more]"
    );

    let result = (tool.execute)(
        "ls-negative",
        serde_yaml::from_str(r#"{"limit":-1}"#).unwrap(),
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(output(&result), "(empty directory)");
}

#[tokio::test]
async fn ls_uses_injected_operations_without_disk_access() {
    let root = PathBuf::from("/virtual-ls-root");
    let directory = root.join("remote");
    let calls = Arc::new(Mutex::new(Vec::new()));
    let operations = LsOperations {
        exists: {
            let calls = Arc::clone(&calls);
            Arc::new(move |path| {
                let calls = Arc::clone(&calls);
                Box::pin(async move {
                    calls.lock().unwrap().push(("exists", path));
                    Ok(true)
                })
            })
        },
        stat: {
            let calls = Arc::clone(&calls);
            Arc::new(move |path| {
                let calls = Arc::clone(&calls);
                Box::pin(async move {
                    let is_directory = path.ends_with("folder") || path.ends_with("remote");
                    calls.lock().unwrap().push(("stat", path));
                    Ok(is_directory)
                })
            })
        },
        read_dir: {
            let calls = Arc::clone(&calls);
            Arc::new(move |path| {
                let calls = Arc::clone(&calls);
                Box::pin(async move {
                    calls.lock().unwrap().push(("read_dir", path));
                    Ok(vec!["z.txt".into(), "folder".into(), "A.txt".into()])
                })
            })
        },
    };

    let tool = create_ls_tool_definition(
        &root,
        LsToolOptions {
            operations: Some(operations),
        },
    );
    let result = tool
        .execute(LsToolInput {
            path: Some("remote".into()),
            limit: None,
        })
        .await
        .unwrap();

    assert_eq!(output(&result), "A.txt\nfolder/\nz.txt");
    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            ("exists", directory.clone()),
            ("stat", directory.clone()),
            ("read_dir", directory.clone()),
            ("stat", directory.join("A.txt")),
            ("stat", directory.join("folder")),
            ("stat", directory.join("z.txt")),
        ]
    );
}

#[tokio::test]
async fn write_uses_injected_operations_without_disk_access() {
    let root = PathBuf::from("/virtual-write-root");
    let path = root.join("nested/file.txt");
    let calls = Arc::new(Mutex::new(Vec::new()));
    let operations = WriteOperations {
        mkdir: {
            let calls = Arc::clone(&calls);
            Arc::new(move |path| {
                let calls = Arc::clone(&calls);
                Box::pin(async move {
                    calls.lock().unwrap().push(("mkdir", path, None));
                    Ok(())
                })
            })
        },
        write_file: {
            let calls = Arc::clone(&calls);
            Arc::new(move |path, content| {
                let calls = Arc::clone(&calls);
                Box::pin(async move {
                    calls
                        .lock()
                        .unwrap()
                        .push(("write_file", path, Some(content)));
                    Ok(())
                })
            })
        },
    };

    let tool = create_write_tool_with_options(
        &root,
        WriteToolOptions {
            operations: Some(operations),
        },
    );
    let result = (tool.execute)(
        "write-1",
        serde_yaml::from_str(r#"{"path":"nested/file.txt","content":"remote"}"#).unwrap(),
        None,
        None,
    )
    .await
    .unwrap();

    assert_eq!(
        output(&result),
        "Successfully wrote 6 bytes to nested/file.txt"
    );
    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            ("mkdir", root.join("nested"), None),
            ("write_file", path, Some("remote".into())),
        ]
    );
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
async fn find_uses_injected_operations_before_provisioning_fd() {
    let root = std::env::temp_dir().join("zedflow-virtual-find-root");
    let search_path = root.join("search");
    let calls = Arc::new(Mutex::new(Vec::new()));
    let exists_calls = Arc::clone(&calls);
    let glob_calls = Arc::clone(&calls);
    let operations = FindOperations {
        exists: Arc::new(move |path| {
            let calls = Arc::clone(&exists_calls);
            Box::pin(async move {
                calls.lock().unwrap().push(("exists", path));
                Ok(true)
            })
        }),
        glob: Arc::new(move |pattern, cwd, options| {
            let calls = Arc::clone(&glob_calls);
            Box::pin(async move {
                assert_eq!(pattern, "**/*.rs");
                assert_eq!(options.ignore, ["**/node_modules/**", "**/.git/**"]);
                assert_eq!(options.limit, 2);
                calls.lock().unwrap().push(("glob", cwd.clone()));
                Ok(vec![cwd.join("a.rs"), cwd.join("nested/b.rs")])
            })
        }),
    };

    let result = FindTool::with_operations(&root, operations)
        .execute(FindToolInput {
            pattern: "**/*.rs".into(),
            path: Some("search".into()),
            limit: Some(2),
        })
        .await
        .unwrap();

    assert_eq!(
        *calls.lock().unwrap(),
        vec![("exists", search_path.clone()), ("glob", search_path)]
    );
    assert_eq!(
        output(&result),
        "a.rs\nnested/b.rs\n\n[2 results limit reached]"
    );
    assert_eq!(result.details.unwrap().result_limit_reached, Some(2));
}

#[cfg(unix)]
#[tokio::test]
async fn find_uses_the_managed_fd_binary_without_path() {
    const CHILD: &str = "ZEDFLOW_MANAGED_FD_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let root = TempDir::new();
        let agent_dir = root.as_ref().join("agent");
        let bin_dir = agent_dir.join("bin");
        let search_dir = root.as_ref().join("search");
        fs::create_dir_all(&bin_dir).unwrap();
        fs::create_dir(&search_dir).unwrap();
        fs::write(search_dir.join("managed.txt"), "").unwrap();
        let fd = bin_dir.join("fd");
        fs::write(
            &fd,
            "#!/bin/sh\nfor last do :; done\nprintf '%s/managed.txt\\n' \"$last\"\n",
        )
        .unwrap();
        fs::set_permissions(&fd, fs::Permissions::from_mode(0o755)).unwrap();

        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "find_uses_the_managed_fd_binary_without_path",
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
    let result = FindTool::new(&root)
        .execute(FindToolInput {
            pattern: "*.txt".into(),
            path: None,
            limit: None,
        })
        .await
        .unwrap();
    assert_eq!(output(&result), "managed.txt");
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
