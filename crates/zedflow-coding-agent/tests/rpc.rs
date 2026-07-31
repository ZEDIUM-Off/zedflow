use std::io::Cursor;
use zedflow_coding_agent::{RpcResponse, handle_command_line, run_rpc_loop};

#[test]
fn rpc_preserves_request_identity_and_returns_command_error_envelopes() {
    let response = handle_command_line(r#"{"id":"request-7","type":"unknown"}"#);

    assert_eq!(response.id.as_deref(), Some("request-7"));
    assert_eq!(response.command, "unknown");
    assert!(!response.success);
    assert_eq!(response.response_type, "response");
}

#[test]
fn rpc_writes_one_json_response_per_input_record() {
    let mut output = Vec::new();
    run_rpc_loop(
        Cursor::new("{\"id\":\"a\",\"type\":\"get_state\"}\nnot json\n"),
        &mut output,
    )
    .unwrap();

    let responses: Vec<RpcResponse> = std::str::from_utf8(&output)
        .unwrap()
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0].id.as_deref(), Some("a"));
    assert!(responses[0].success);
    assert_eq!(responses[1].command, "parse");
    assert!(!responses[1].success);
}
