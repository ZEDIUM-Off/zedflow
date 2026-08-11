use zedflow_coding_agent::{RpcClient, RpcCommand};

#[test]
fn clone_command_is_framed_with_a_correlated_request_id() {
    let mut client = RpcClient::new();
    let line = client.encode(RpcCommand::Clone { id: None });
    let command: serde_json::Value = serde_json::from_str(line.trim()).unwrap();

    assert_eq!(command, serde_json::json!({"type":"clone","id":"req_1"}));
}
