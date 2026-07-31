use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};
use zedflow_orchestrator::{
    config,
    ipc_protocol::{InstanceSummary, OrchestratorResponse},
    ipc_server::{self, RpcStream},
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
