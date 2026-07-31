use std::{fs, sync::Arc};

use zedflow_agent::harness::{
    env::nodejs::NodeExecutionEnv, session::JsonlSessionStorage, types::SessionErrorCode,
};

#[tokio::test]
async fn rejects_unmigrated_session_file_versions() {
    let file = std::env::temp_dir().join(format!(
        "zedflow-legacy-session-{}.jsonl",
        std::process::id()
    ));
    fs::write(
        &file,
        r#"{"type":"session","version":2,"id":"old","timestamp":"2025-01-01T00:00:00.000Z","cwd":"/tmp"}
"#,
    )
    .unwrap();

    let error = match JsonlSessionStorage::open(
        Arc::new(NodeExecutionEnv::with_cwd("/tmp")),
        file.to_string_lossy().into_owned(),
    )
    .await
    {
        Ok(_) => panic!("legacy session must be rejected"),
        Err(error) => error,
    };
    assert_eq!(error.code, SessionErrorCode::InvalidSession);
    fs::remove_file(file).unwrap();
}
