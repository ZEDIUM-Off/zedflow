use serde_json::{Value, json};
use std::{fs, os::unix::fs::PermissionsExt, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

use zedflow_orchestrator::{
    config,
    handler::{handle_ipc_request, open_rpc_stream},
    ipc_protocol::{InstanceSummary, OrchestratorResponse},
    ipc_server::{self, RpcStream},
    supervisor::OrchestratorSupervisor,
    types::InstanceStatus,
};

fn ready() -> OrchestratorResponse {
    OrchestratorResponse::RpcReady {
        ok: true,
        instance: Some(InstanceSummary {
            id: "pi-1".into(),
            status: InstanceStatus::Online,
            cwd: "/tmp".into(),
            label: None,
            session_id: None,
            session_file: None,
            radius_pi_id: None,
        }),
    }
}

#[tokio::test]
async fn rpc_stream_keeps_socket_open_and_forwards_messages() {
    let _environment = ENV_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("zedflow-rpc-stream-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    unsafe {
        std::env::set_var("PI_ORCHESTRATOR_DIR", &dir);
    }
    let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let stream_requests = requests.clone();
    let server = tokio::spawn(ipc_server::run_ipc_server(
        |_| async { ready() },
        move |_, messages| {
            let requests = stream_requests.clone();
            async move {
                Some(RpcStream::new(
                    move |command| {
                        let command_type = command["type"].as_str().unwrap().to_owned();
                        requests.lock().unwrap().push(command_type.clone());
                        if command_type == "get_state" {
                            messages
                                .send(json!({"type":"response","id":command["id"],"result":"ok"}))
                                .unwrap();
                            messages
                                .send(json!({"type":"agent_end","reason":"done"}))
                                .unwrap();
                            messages
                                .send(json!({"type":"extension_ui_request","id":"ui-1"}))
                                .unwrap();
                        }
                        Ok(())
                    },
                    || {},
                ))
            }
        },
    ));
    let socket = config::socket_path();
    for _ in 0..50 {
        if socket.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let stream = UnixStream::connect(&socket).await.unwrap();
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    write
        .write_all(b"{\"type\":\"rpc_stream\",\"instanceId\":\"pi-1\"}\n")
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&lines.next_line().await.unwrap().unwrap()).unwrap()["type"],
        "rpc_ready"
    );
    write.write_all(b"{\"type\":\"get_state\",\"id\":\"r-1\"}\n{\"type\":\"extension_ui_response\",\"id\":\"ui-1\"}\n").await.unwrap();
    let mut received = Vec::new();
    for _ in 0..3 {
        received.push(
            serde_json::from_str::<Value>(&lines.next_line().await.unwrap().unwrap()).unwrap(),
        );
    }
    assert!(
        received
            .iter()
            .any(|message| message["type"] == "response" && message["id"] == "r-1")
    );
    assert!(
        received
            .iter()
            .any(|message| message["type"] == "agent_end")
    );
    assert!(
        received
            .iter()
            .any(|message| message["type"] == "extension_ui_request")
    );
    for _ in 0..50 {
        if requests.lock().unwrap().len() == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    assert_eq!(
        *requests.lock().unwrap(),
        ["get_state", "extension_ui_response"]
    );
    drop(write);
    server.abort();
    let _ = server.await;
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn rpc_stream_refreshes_live_and_persisted_session_metadata() {
    let _environment = ENV_LOCK.lock().unwrap();
    let dir =
        std::env::temp_dir().join(format!("zedflow-stream-metadata-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    let command = dir.join("rpc.sh");
    fs::write(
        &command,
        r#"#!/bin/sh
state=0
while IFS= read -r line; do
  id=$(printf '%s\n' "$line" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
  case "$line" in
    *'"type":"get_state"'*)
      state=$((state + 1))
      printf '{"type":"response","id":"%s","success":true,"command":"get_state","data":{"sessionId":"session-%s","sessionFile":"/tmp/session-%s.jsonl"}}\n' "$id" "$state" "$state"
      ;;
    *'"type":"new_session"'*)
      printf '{"type":"response","id":"%s","success":true,"command":"new_session"}\n' "$id"
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

    let supervisor = Arc::new(tokio::sync::Mutex::new(OrchestratorSupervisor::new()));
    let instance = supervisor
        .lock()
        .await
        .spawn_instance(dir.to_string_lossy().into_owned(), None)
        .await
        .unwrap();
    assert_eq!(instance.session_id.as_deref(), Some("session-1"));
    let handler_supervisor = supervisor.clone();
    let stream_supervisor = supervisor.clone();
    let server = tokio::spawn(ipc_server::run_ipc_server(
        move |request| {
            let supervisor = handler_supervisor.clone();
            async move { handle_ipc_request(&mut *supervisor.lock().await, request).await }
        },
        move |id, outgoing| {
            let supervisor = stream_supervisor.clone();
            async move { open_rpc_stream(&*supervisor.lock().await, &id, outgoing) }
        },
    ));
    let socket = config::socket_path();
    for _ in 0..50 {
        if socket.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let stream = UnixStream::connect(&socket).await.unwrap();
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    write
        .write_all(
            format!(
                "{{\"type\":\"rpc_stream\",\"instanceId\":\"{}\"}}\n",
                instance.id
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    lines.next_line().await.unwrap().unwrap();
    write
        .write_all(b"{\"type\":\"new_session\"}\n")
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&lines.next_line().await.unwrap().unwrap()).unwrap()["command"],
        "new_session"
    );
    let record = supervisor
        .lock()
        .await
        .get_instance(&instance.id)
        .unwrap()
        .unwrap();
    assert_eq!(record.session_id.as_deref(), Some("session-2"));
    assert_eq!(record.session_file.as_deref(), Some("/tmp/session-2.jsonl"));
    assert_eq!(
        zedflow_orchestrator::storage::get_instance(&instance.id)
            .unwrap()
            .unwrap()
            .session_id
            .as_deref(),
        Some("session-2")
    );
    drop(write);
    server.abort();
    let _ = server.await;
    unsafe {
        std::env::remove_var("PI_ORCHESTRATOR_DIR");
        std::env::remove_var("PI_ORCHESTRATOR_RPC_COMMAND");
    }
    fs::remove_dir_all(dir).unwrap();
}
