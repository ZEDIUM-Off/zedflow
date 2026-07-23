//! Client-side helpers for the JSONL RPC protocol.

use super::jsonl::serialize_json_line;
use super::rpc_types::RpcResponse;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Default)]
pub struct RpcClient {
    next_id: u64,
}

impl RpcClient {
    pub fn new() -> Self {
        Self { next_id: 1 }
    }
    pub fn next_id(&mut self) -> String {
        let id = self.next_id.to_string();
        self.next_id += 1;
        id
    }
    pub fn encode<T: Serialize>(&self, command: &T) -> serde_json::Result<String> {
        serialize_json_line(command)
    }
    pub fn decode_response(line: &str) -> serde_json::Result<RpcResponse> {
        serde_json::from_str(line)
    }
    pub fn response_data(response: &RpcResponse) -> Result<Option<&Value>, String> {
        if response.success {
            Ok(response.data.as_ref())
        } else {
            Err(response
                .error
                .clone()
                .unwrap_or_else(|| "RPC request failed".into()))
        }
    }
}
