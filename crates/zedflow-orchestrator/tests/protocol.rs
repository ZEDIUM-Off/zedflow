use serde_json::json;
use zedflow_orchestrator::{
    ipc_protocol::{
        OrchestratorRequest, OrchestratorResponse, encode_message, parse_request_line,
        parse_response_line,
    },
    types::InstanceStatus,
};

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
