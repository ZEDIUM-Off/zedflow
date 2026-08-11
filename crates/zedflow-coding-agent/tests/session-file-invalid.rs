use std::{fs, sync::Arc};

use zedflow_agent::harness::{env::nodejs::NodeExecutionEnv, session::JsonlSessionStorage};
use zedflow_coding_agent::session_manager::SessionErrorCode;

#[tokio::test]
async fn invalid_session_file_is_rejected_without_modifying_it() {
    let path = std::env::temp_dir().join(format!("zedflow-invalid-session-{}", std::process::id()));
    let original = r#"{"type":"event","data":"not a session"}
"#;
    fs::write(&path, original).unwrap();

    let error = match JsonlSessionStorage::open(
        Arc::new(NodeExecutionEnv::with_cwd(
            std::env::temp_dir().to_string_lossy().into_owned(),
        )),
        path.to_string_lossy().into_owned(),
    )
    .await
    {
        Ok(_) => panic!("invalid session file must be rejected"),
        Err(error) => error,
    };

    assert_eq!(error.code, SessionErrorCode::InvalidSession);
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    fs::remove_file(path).unwrap();
}
