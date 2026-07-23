//! Public coding-agent facade.

pub use crate::cli::{
    AppMode, Args, Mode, build_initial_message, help_text, parse_args, resolve_app_mode,
};
pub use crate::modes::{
    PrintModeOptions, PrintOutputMode, RpcClient, RpcCommand, RpcResponse, run_print_mode,
    run_rpc_mode,
};
