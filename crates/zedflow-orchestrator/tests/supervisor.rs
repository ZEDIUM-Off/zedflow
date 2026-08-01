use serde_json::json;
use std::{fs, os::unix::fs::PermissionsExt, time::Duration};
use zedflow_orchestrator::{supervisor::OrchestratorSupervisor, types::InstanceStatus};

#[tokio::test]
async fn syncs_session_metadata_and_marks_exited_process_error() {
    let dir = std::env::temp_dir().join(format!("zedflow-supervisor-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let command = dir.join("rpc.sh");
    fs::write(
        &command,
        r#"#!/bin/sh
prompt=0
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
  case "$line" in
    *'"type":"get_state"'*)
      printf '{"type":"response","id":"%s","success":true,"command":"get_state","data":{"sessionId":"session-1","sessionFile":"/tmp/session.jsonl"}}\n' "$id"
      [ "$prompt" -eq 1 ] && exit 0
      ;;
    *'"type":"prompt"'*)
      prompt=1
      printf '{"type":"response","id":"%s","success":true,"command":"prompt"}\n' "$id"
      ;;
  esac
done
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&command).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&command, permissions).unwrap();
    unsafe {
        std::env::set_var("PI_ORCHESTRATOR_DIR", &dir);
        std::env::set_var("PI_ORCHESTRATOR_RPC_COMMAND", &command);
    }

    let mut supervisor = OrchestratorSupervisor::new();
    let instance = supervisor
        .spawn_instance(dir.to_string_lossy().into_owned(), None)
        .await
        .unwrap();
    assert_eq!(instance.session_id.as_deref(), Some("session-1"));
    assert_eq!(instance.session_file.as_deref(), Some("/tmp/session.jsonl"));
    supervisor
        .handle_rpc(&instance.id, json!({"type":"prompt"}))
        .unwrap();

    for _ in 0..50 {
        if matches!(
            supervisor
                .get_instance(&instance.id)
                .unwrap()
                .unwrap()
                .status,
            InstanceStatus::Error
        ) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        supervisor
            .get_instance(&instance.id)
            .unwrap()
            .unwrap()
            .status,
        InstanceStatus::Error
    );
    assert!(
        supervisor
            .handle_rpc(&instance.id, json!({"type":"get_state"}))
            .unwrap()
            .is_none()
    );
    unsafe {
        std::env::remove_var("PI_ORCHESTRATOR_DIR");
        std::env::remove_var("PI_ORCHESTRATOR_RPC_COMMAND");
    }
    fs::remove_dir_all(dir).unwrap();
}
