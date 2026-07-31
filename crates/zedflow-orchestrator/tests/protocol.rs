use zedflow_orchestrator::{
    ipc_protocol::{
        OrchestratorRequest, OrchestratorResponse, encode_message, parse_request_line,
        parse_response_line,
    },
    types::InstanceStatus,
};

#[test]
fn protocol_uses_jsonl_and_pi_wire_tags() {
    let request = OrchestratorRequest::Stop {
        instance_id: "pi-1".into(),
    };
    let line = encode_message(&request).unwrap();
    assert_eq!(line, "{\"type\":\"stop\",\"instance_id\":\"pi-1\"}\n");
    assert_eq!(parse_request_line(line.trim()).unwrap(), request);
    let response = parse_response_line("{\"type\":\"status_result\",\"ok\":true,\"instance\":{\"id\":\"pi-1\",\"status\":\"online\",\"cwd\":\"/tmp\"}}").unwrap();
    assert!(
        matches!(response, OrchestratorResponse::StatusResult { instance: Some(instance), .. } if instance.status == InstanceStatus::Online)
    );
}
