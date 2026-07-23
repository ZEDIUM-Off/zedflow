//! `pi` command entry point.

use std::io;
use zedflow_coding_agent::cli::parse_args;
use zedflow_coding_agent::modes::{RpcCommand, run_rpc_loop};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let parsed = parse_args(args);
    if parsed.version {
        println!("{}", zedflow_coding_agent::config::VERSION);
        return;
    }
    if parsed.help {
        println!(
            "pi - AI coding assistant\n\nUse --print for non-interactive mode or --mode rpc for JSONL RPC."
        );
        return;
    }
    if matches!(parsed.mode, Some(zedflow_coding_agent::cli::Mode::Rpc)) {
        if let Err(error) = run_rpc_loop(io::stdin().lock(), io::stdout().lock()) {
            eprintln!("RPC error: {error}");
            std::process::exit(1);
        }
    }
}

#[allow(dead_code)]
fn _command_type(_: RpcCommand) {}
