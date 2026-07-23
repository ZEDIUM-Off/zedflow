//! `pi` command entry point.

use std::io;
use zedflow_coding_agent::cli::{Mode, parse_args};
use zedflow_coding_agent::rpc_entry;

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
    if let Err(error) = dispatch_runtime_mode(parsed.mode) {
        eprintln!("Runtime error: {error}");
        std::process::exit(1);
    }
}

fn dispatch_runtime_mode(mode: Option<Mode>) -> io::Result<()> {
    match mode {
        Some(Mode::Rpc) => rpc_entry::run(io::stdin().lock(), io::stdout()),
        Some(Mode::Text) | Some(Mode::Json) | None => Ok(()),
    }
}
