//! Runtime-neutral JSON RPC loop.

use super::jsonl::{parse_json_lines, serialize_json_line};
use super::rpc_types::{RpcCommand, RpcResponse};
use serde_json::Value;

pub fn decode_command(line: &str) -> Result<RpcCommand, String> {
    serde_json::from_str(line).map_err(|error| error.to_string())
}

/// Handle a JSONL request stream while leaving session semantics to the caller.
pub fn run_rpc_mode<F>(input: &str, mut handler: F) -> String
where
    F: FnMut(RpcCommand) -> Result<Option<Value>, String>,
{
    let mut output = String::new();
    for line in parse_json_lines(input) {
        let command = match decode_command(line) {
            Ok(command) => command,
            Err(error) => {
                output.push_str(
                    &serialize_json_line(&RpcResponse::error(None, "unknown", error))
                        .expect("JSON serialization"),
                );
                continue;
            }
        };
        let id = command.id().map(str::to_owned);
        let name = command.command_type().to_owned();
        let response = match handler(command) {
            Ok(data) => RpcResponse::success(id, name, data),
            Err(error) => RpcResponse::error(id, name, error),
        };
        output.push_str(&serialize_json_line(&response).expect("JSON serialization"));
    }
    output
}
