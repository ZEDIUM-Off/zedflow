use std::io::Cursor;
use zedflow_coding_agent::{RpcClient, RpcCommand, handle_command_line, run_rpc_loop};

#[test]
fn rpc_protocol_preserves_request_identity_and_errors() {
    let mut client = RpcClient::new();
    let encoded = client.encode(RpcCommand::GetState { id: None });
    assert!(encoded.contains("req_1"));
    let response = handle_command_line(&encoded);
    assert!(response.success && response.id.as_deref() == Some("req_1"));
    assert!(!handle_command_line(r#"{"id":"x","type":"unknown"}"#).success);
}

#[test]
fn rpc_loop_emits_one_json_response_per_input_line() {
    let mut output = Vec::new();
    run_rpc_loop(
        Cursor::new("{\"type\":\"get_state\",\"id\":\"a\"}\n"),
        &mut output,
    )
    .unwrap();
    assert!(String::from_utf8(output).unwrap().contains("\"id\":\"a\""));
}
