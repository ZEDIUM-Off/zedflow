use std::{fs, sync::Arc};

use zedflow_agent::harness::{
    env::nodejs::NodeExecutionEnv,
    session::{JsonlSessionStorage, JsonlSessionStorageCreateOptions, Session},
};

#[tokio::test]
async fn jsonl_session_appends_entries_and_reopens_them() {
    let file = std::env::temp_dir().join(format!("zedflow-session-{}.jsonl", std::process::id()));
    let cwd = std::env::temp_dir().to_string_lossy().into_owned();
    let storage = JsonlSessionStorage::create(
        Arc::new(NodeExecutionEnv::with_cwd(cwd.clone())),
        file.to_string_lossy().into_owned(),
        JsonlSessionStorageCreateOptions {
            cwd: cwd.clone(),
            session_id: "file-session".into(),
            parent_session_path: None,
        },
    )
    .await
    .unwrap();
    Session::new(storage)
        .append_custom_entry("data", None)
        .await
        .unwrap();

    let reopened = JsonlSessionStorage::open(
        Arc::new(NodeExecutionEnv::with_cwd(cwd)),
        file.to_string_lossy().into_owned(),
    )
    .await
    .unwrap();
    assert_eq!(Session::new(reopened).get_entries().await.len(), 1);
    fs::remove_file(file).unwrap();
}
