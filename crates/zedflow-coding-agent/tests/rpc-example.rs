use std::io::Cursor;
use zedflow_coding_agent::{RpcClient, RpcCommand, RpcResponse, run_rpc_loop};

#[test]
fn rpc_example_flow_encodes_a_request_and_reads_its_response() {
    let mut client = RpcClient::new();
    let request = client.encode(RpcCommand::GetState { id: None });
    let mut output = Vec::new();

    run_rpc_loop(Cursor::new(request), &mut output).unwrap();
    let response = RpcClient::decode(std::str::from_utf8(&output).unwrap().trim()).unwrap();

    assert_eq!(
        response,
        RpcResponse::success(Some("req_1".into()), "get_state", None)
    );
}
