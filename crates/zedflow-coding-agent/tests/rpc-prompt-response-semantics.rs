use std::io::Cursor;
use zedflow_coding_agent::{RpcResponse, run_rpc_loop};

#[test]
fn prompt_emits_one_correlated_success_response() {
    let mut output = Vec::new();
    run_rpc_loop(
        Cursor::new("{\"id\":\"prompt-1\",\"type\":\"prompt\",\"message\":\"Hello\"}\n"),
        &mut output,
    )
    .unwrap();

    let responses: Vec<RpcResponse> = std::str::from_utf8(&output)
        .unwrap()
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        responses,
        vec![RpcResponse::success(
            Some("prompt-1".into()),
            "prompt",
            None
        )]
    );
}

#[test]
fn malformed_prompt_emits_one_correlated_failure_response() {
    let mut output = Vec::new();
    run_rpc_loop(
        Cursor::new("{\"id\":\"prompt-2\",\"type\":\"prompt\"}\n"),
        &mut output,
    )
    .unwrap();

    let response: RpcResponse = serde_json::from_slice(&output).unwrap();
    assert_eq!(response.id.as_deref(), Some("prompt-2"));
    assert_eq!(response.command, "prompt");
    assert!(!response.success);
}
