use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn zf(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_zf"))
        .current_dir(root)
        .args(args)
        .env("HOME", root.join("home"))
        .output()
        .expect("CLI process")
}
fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("JSON output")
}
fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("home")).unwrap();
    root
}

#[test]
fn init_all_templates_then_validate_offline_without_database_or_activation() {
    let root = fixture();
    for (kind, path) in [
        ("flow", ".zedflow/flow/demo"),
        ("bridge", ".zedflow/bridges/demo.rs"),
        ("context", ".zedflow/context/demo.rs"),
    ] {
        let created = success(zf(root.path(), &["init", kind, "demo"]));
        assert_eq!(created["activated"], false);
        assert_eq!(created["executed"], false);
        assert_eq!(success(zf(root.path(), &["validate", path]))["valid"], true);
    }
    let plan = success(zf(root.path(), &["compile", ".zedflow/flow/demo"]));
    assert!(plan["revision"].as_str().is_some());
    assert!(!root.path().join(".zedflow/zedflow.db").exists());
    assert!(!root.path().join(".zedflow/execution.lock").exists());
}

#[test]
fn repeated_init_preserves_authored_files_and_rejects_path_traversal() {
    let root = fixture();
    for (kind, path) in [
        ("flow", ".zedflow/flow/demo/flow.rs"),
        ("bridge", ".zedflow/bridges/demo.rs"),
        ("context", ".zedflow/context/demo.rs"),
    ] {
        success(zf(root.path(), &["init", kind, "demo"]));
        fs::write(root.path().join(path), "authored invalid source").unwrap();
        assert!(!zf(root.path(), &["init", kind, "demo"]).status.success());
        assert_eq!(
            fs::read_to_string(root.path().join(path)).unwrap(),
            "authored invalid source"
        );
    }
    assert!(
        !zf(root.path(), &["init", "flow", "../escaped"])
            .status
            .success()
    );
    assert!(!root.path().join("escaped").exists());
}

#[cfg(unix)]
#[test]
fn init_rejects_symlink_catalogue_and_existing_symlink_destination() {
    use std::os::unix::fs::symlink;
    let root = fixture();
    let outside = fixture();
    symlink(outside.path(), root.path().join(".zedflow")).unwrap();
    assert!(!zf(root.path(), &["init", "flow", "demo"]).status.success());
    assert!(!outside.path().join("flow").exists());
    fs::remove_file(root.path().join(".zedflow")).unwrap();
    fs::create_dir_all(root.path().join(".zedflow/context")).unwrap();
    let target = outside.path().join("authored.rs");
    fs::write(&target, "keep").unwrap();
    symlink(&target, root.path().join(".zedflow/context/demo.rs")).unwrap();
    assert!(
        !zf(root.path(), &["init", "context", "demo"])
            .status
            .success()
    );
    assert_eq!(fs::read_to_string(target).unwrap(), "keep");
}

#[test]
fn package_validation_diagnoses_missing_inventory_and_compile_pins_secondary_bytes() {
    let root = fixture();
    success(zf(root.path(), &["init", "flow", "demo"]));
    let first = success(zf(root.path(), &["compile", ".zedflow/flow/demo"]));
    fs::write(
        root.path().join(".zedflow/flow/demo/README.md"),
        "new documentation",
    )
    .unwrap();
    let second = success(zf(root.path(), &["compile", ".zedflow/flow/demo"]));
    assert_ne!(first["revision"], second["revision"]);
    fs::remove_file(root.path().join(".zedflow/flow/demo/README.md")).unwrap();
    let failed = zf(root.path(), &["validate", ".zedflow/flow/demo"]);
    assert!(!failed.status.success());
    let diagnostic = String::from_utf8_lossy(&failed.stderr);
    assert!(diagnostic.contains(".zedflow/flow/demo"));
    assert!(diagnostic.contains("inventory"));
}

#[test]
fn standalone_requires_data_and_rejects_a_second_owner() {
    let root = fixture();
    let failed = zf(root.path(), &["sessions", "--standalone"]);
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("--data"));
    fs::create_dir(root.path().join("data")).unwrap();
    let owner = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .read(true)
        .open(root.path().join("data/execution.lock"))
        .unwrap();
    owner.try_lock().unwrap();
    let failed = zf(root.path(), &["sessions", "--standalone", "--data", "data"]);
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("already owned"));
    assert!(!root.path().join("data/zedflow.db").exists());
}

#[test]
fn offline_explicit_run_uses_service_and_persists_actor() {
    let root = fixture();
    success(zf(root.path(), &["init", "flow", "demo"]));
    fs::copy(
        root.path().join(".zedflow/flow/demo/flow.rs"),
        root.path().join("standalone.rs"),
    )
    .unwrap();
    let run = success(zf(
        root.path(),
        &[
            "run",
            "--source",
            "standalone.rs",
            "--input",
            "{\"input\":\"bonjour\"}",
            "--standalone",
            "--data",
            "data",
            "--flow-home",
            "home",
            "--context-home",
            "home",
        ],
    ));
    assert!(run["id"].as_str().is_some());
    assert_eq!(run["startedBy"]["id"], "zf-cli-local");
    assert_eq!(run["status"], "completed");
    assert!(root.path().join("data/zedflow.db").is_file());
    assert_eq!(
        success(zf(
            root.path(),
            &[
                "sessions",
                "--standalone",
                "--data",
                "data",
                "--flow-home",
                "home",
                "--context-home",
                "home"
            ]
        )),
        json!([])
    );
}

#[test]
fn unavailable_daemon_does_not_fall_back_to_local_storage() {
    let root = fixture();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let failed = zf(
        root.path(),
        &["sessions", "--daemon", &format!("http://{addr}")],
    );
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("daemon inaccessible"));
    assert!(!root.path().join(".zedflow").exists());
}

#[test]
fn daemon_run_transmits_selection_and_open_json_without_local_data() {
    use std::io::{Read, Write};
    let root = fixture();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        let body = loop {
            let mut chunk = [0; 4096];
            let count = socket.read(&mut chunk).unwrap();
            assert!(count > 0);
            bytes.extend_from_slice(&chunk[..count]);
            if let Some(index) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..index]);
                assert!(headers.starts_with("POST /api/runs?workspaceId=space HTTP/1.1"));
                let length: usize = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(str::trim)
                            .map(str::parse)
                    })
                    .unwrap()
                    .unwrap();
                if bytes.len() >= index + 4 + length {
                    break serde_json::from_slice::<Value>(&bytes[index + 4..index + 4 + length])
                        .unwrap();
                }
            }
        };
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{\"id\":\"remote\"}").unwrap();
        body
    });
    let result = success(zf(
        root.path(),
        &[
            "run",
            "--daemon",
            &format!("http://{addr}"),
            "--workspace-id",
            "space",
            "--flow-key",
            "catalog-key",
            "--flow-hash",
            "revision",
            "--input",
            "{\"nested\":{\"unknown\":[1,null]}}",
        ],
    ));
    assert_eq!(result["id"], "remote");
    let body = server.join().unwrap();
    assert_eq!(body["flowKey"], "catalog-key");
    assert_eq!(body["flowHash"], "revision");
    assert_eq!(body["input"]["nested"]["unknown"], json!([1, null]));
    assert_eq!(body["workspaceId"], "space");
    assert!(body.get("actor").is_none());
    assert!(!root.path().join(".zedflow").exists());
}

#[test]
fn cargo_export_is_explicitly_unavailable_until_packager_is_delivered() {
    let root = fixture();
    let result = zf(root.path(), &["export", "missing", "--output", "out"]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("export_unavailable"));
    assert!(!root.path().join("out").exists());
}

#[test]
fn compilation_snapshot_resolves_selected_bridge_and_keeps_precise_diagnostics() {
    let root = fixture();
    success(zf(root.path(), &["init", "flow", "demo"]));
    success(zf(root.path(), &["init", "bridge", "connection"]));
    let mut snapshot = zf_compiler::prepared::CompilationSnapshot::default();
    snapshot.flows.insert(
        "selected".into(),
        zf_compiler::programs::SourceSnapshot::capture(
            fs::read_to_string(root.path().join(".zedflow/flow/demo/flow.rs")).unwrap(),
        ),
    );
    snapshot.bridges.insert(
        "connection".into(),
        zf_compiler::programs::SourceSnapshot::capture(
            fs::read_to_string(root.path().join(".zedflow/bridges/connection.rs")).unwrap(),
        ),
    );
    fs::write(
        root.path().join("snapshot.json"),
        serde_json::to_vec(&snapshot).unwrap(),
    )
    .unwrap();
    let result = success(zf(
        root.path(),
        &[
            "compile",
            "snapshot.json",
            "--snapshot",
            "--flow",
            "selected",
            "--bridge",
            "connection",
        ],
    ));
    assert!(result["revision"].is_string());
    let failed = zf(
        root.path(),
        &[
            "compile",
            "snapshot.json",
            "--snapshot",
            "--flow",
            "selected",
            "--bridge",
            "missing",
        ],
    );
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("missing"));
    assert!(!root.path().join(".zedflow/zedflow.db").exists());
}

#[test]
fn package_entry_run_refuses_to_drop_package_identity_and_dependencies() {
    let root = fixture();
    success(zf(root.path(), &["init", "flow", "demo"]));
    let failed = zf(
        root.path(),
        &[
            "run",
            "--source",
            ".zedflow/flow/demo/flow.rs",
            "--standalone",
            "--data",
            "data",
        ],
    );
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("--flow-key"));
    assert!(!root.path().join("data").exists());
}
