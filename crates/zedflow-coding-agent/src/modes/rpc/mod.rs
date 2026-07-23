//! RPC mode modules.

#[path = "jsonl.rs"]
pub mod jsonl;
#[path = "rpc-client.rs"]
pub mod rpc_client;
#[path = "rpc-mode.rs"]
pub mod rpc_mode;
#[path = "rpc-types.rs"]
pub mod rpc_types;

pub use jsonl::{decode_json_lines, parse_json_lines, serialize_json_line};
pub use rpc_client::RpcClient;
pub use rpc_mode::{decode_command, run_rpc_mode};
pub use rpc_types::{RpcCommand, RpcExtensionUiResponse, RpcResponse, RpcSessionState};
