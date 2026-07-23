//! Non-interactive coding-agent modes.

#[path = "print-mode.rs"]
pub mod print_mode;
#[path = "rpc/mod.rs"]
pub mod rpc;

pub use print_mode::{
    PrintModeOptions, PrintOutputMode, render_text_response, run_print_mode, serialize_event,
};
pub use rpc::{
    RpcClient, RpcCommand, RpcResponse, decode_command, decode_json_lines, parse_json_lines,
    run_rpc_mode, serialize_json_line,
};
