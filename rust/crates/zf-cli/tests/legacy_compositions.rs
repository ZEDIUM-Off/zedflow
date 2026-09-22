use serde_json::json;
use std::{
    path::Path,
    process::{Command, Output},
};

fn import(root: &Path, dry: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_zf"));
    command
        .arg("--workspace")
        .arg(root.join("workspace"))
        .arg("migrate-compositions")
        .arg("--data")
        .arg(root.join("data"))
        .arg("--flow-home")
        .arg(root.join("home"));
    if dry {
        command.arg("--dry-run");
    }
    command.output().unwrap()
}
#[tokio::test]
async fn explicit_command_previews_imports_and_repeats_with_isolated_roots() {
    let root = tempfile::tempdir().unwrap();
    for dir in ["data", "workspace", "home"] {
        std::fs::create_dir(root.path().join(dir)).unwrap();
    }
    let db = sqlx::SqlitePool::connect(&format!(
        "sqlite://{}?mode=rwc",
        root.path().join("data/zedflow.db").display()
    ))
    .await
    .unwrap();
    sqlx::query("CREATE TABLE compositions(id TEXT PRIMARY KEY, document TEXT NOT NULL)")
        .execute(&db)
        .await
        .unwrap();
    let doc = json!({"formatVersion":3,"id":"old","name":"Old","nodes":[
        {"id":"start","position":{"x":0,"y":0},"data":{"kind":"start","label":"Start"}},
        {"id":"end","position":{"x":100,"y":0},"data":{"kind":"end","label":"End"}}],
        "edges":[{"id":"done","source":"start","target":"end"}]});
    sqlx::query("INSERT INTO compositions VALUES('old',?)")
        .bind(doc.to_string())
        .execute(&db)
        .await
        .unwrap();
    db.close().await;
    let dry = import(root.path(), true);
    assert!(
        dry.status.success(),
        "{}",
        String::from_utf8_lossy(&dry.stderr)
    );
    let preview: serde_json::Value = serde_json::from_slice(&dry.stdout).unwrap();
    assert_eq!(preview["complete"], false);
    assert!(preview["backup"].is_null());
    assert!(!root.path().join("workspace/.zedflow").exists());
    let completed = import(root.path(), false);
    assert!(
        completed.status.success(),
        "{}",
        String::from_utf8_lossy(&completed.stderr)
    );
    let receipt: serde_json::Value = serde_json::from_slice(&completed.stdout).unwrap();
    assert_eq!(receipt["complete"], true);
    assert!(Path::new(receipt["backup"].as_str().unwrap()).is_file());
    assert!(
        root.path()
            .join("workspace/.zedflow/flow/old/flow.rs")
            .is_file()
    );
    let repeated = import(root.path(), false);
    assert!(
        repeated.status.success(),
        "{}",
        String::from_utf8_lossy(&repeated.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&repeated.stdout).unwrap(),
        receipt
    );
    assert!(!root.path().join("home/.zedflow/flow").exists());
}
