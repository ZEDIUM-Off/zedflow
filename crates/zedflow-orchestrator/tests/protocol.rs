use serde_json::json;
use std::{fs, sync::Mutex};
use zedflow_orchestrator::{
    handler::handle_ipc_request,
    ipc_protocol::{
        OrchestratorRequest, OrchestratorResponse, encode_message, parse_request_line,
        parse_response_line,
    },
    storage,
    supervisor::OrchestratorSupervisor,
    types::{InstanceRecord, InstanceStatus},
};

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn requests_use_pi_instance_id_wire_field() {
    let requests = [
        OrchestratorRequest::Stop {
            instance_id: "pi-1".into(),
        },
        OrchestratorRequest::Status {
            instance_id: "pi-1".into(),
        },
        OrchestratorRequest::Rpc {
            instance_id: "pi-1".into(),
            command: json!({"type": "get_state"}),
        },
        OrchestratorRequest::RpcStream {
            instance_id: "pi-1".into(),
        },
    ];

    for request in requests {
        let line = encode_message(&request).unwrap();
        assert!(line.contains("\"instanceId\":\"pi-1\""));
        assert_eq!(parse_request_line(line.trim()).unwrap(), request);
    }
}

#[test]
fn protocol_uses_jsonl_and_pi_wire_tags() {
    let response = parse_response_line("{\"type\":\"status_result\",\"ok\":true,\"instance\":{\"id\":\"pi-1\",\"status\":\"online\",\"cwd\":\"/tmp\"}}").unwrap();
    assert!(
        matches!(response, OrchestratorResponse::StatusResult { instance: Some(instance), .. } if instance.status == InstanceStatus::Online)
    );
}

#[test]
fn stop_response_uses_pi_instance_id_wire_field() {
    let response = OrchestratorResponse::StopResult {
        ok: true,
        instance_id: Some("pi-1".into()),
    };

    let line = encode_message(&response).unwrap();
    assert!(line.contains("\"instanceId\":\"pi-1\""));
    assert_eq!(parse_response_line(line.trim()).unwrap(), response);
}

#[tokio::test]
async fn stop_of_persisted_non_live_instance_reports_unknown_and_preserves_record() {
    let _env = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let dir = std::env::temp_dir().join(format!("zedflow-protocol-{}", uuid::Uuid::new_v4()));
    unsafe { std::env::set_var("PI_ORCHESTRATOR_DIR", &dir) };
    let record = InstanceRecord {
        id: "persisted-only".into(),
        status: InstanceStatus::Stopped,
        cwd: "/tmp".into(),
        created_at: "0".into(),
        last_seen_at: None,
        label: None,
        session_id: None,
        session_file: None,
        radius_pi_id: None,
    };
    storage::upsert_instance(&record).unwrap();

    let response = handle_ipc_request(
        &mut OrchestratorSupervisor::new(),
        OrchestratorRequest::Stop {
            instance_id: record.id.clone(),
        },
    )
    .await;

    assert_eq!(
        response,
        OrchestratorResponse::Error {
            ok: false,
            error: "Unknown instance: persisted-only".into(),
        }
    );
    assert_eq!(storage::get_instance(&record.id).unwrap(), Some(record));
    unsafe { std::env::remove_var("PI_ORCHESTRATOR_DIR") };
    fs::remove_dir_all(dir).unwrap();
}
