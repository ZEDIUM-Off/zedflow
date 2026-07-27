//! `pi` command entry point.

use std::io;
use zedflow_coding_agent::cli::{Mode, parse_args};
use zedflow_coding_agent::rpc_entry;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let parsed = parse_args(args.iter().cloned());
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
    if let Err(error) = dispatch_runtime_mode(&args, parsed.mode, parsed.print) {
        eprintln!("Runtime error: {error}");
        std::process::exit(1);
    }
}

fn dispatch_runtime_mode(args: &[String], mode: Option<Mode>, print: bool) -> io::Result<()> {
    if print {
        return Err(io::Error::other(
            "print mode is not wired yet; use --mode rpc or the default terminal mode",
        ));
    }
    match mode {
        Some(Mode::Rpc) => rpc_entry::run(io::stdin().lock(), io::stdout()),
        Some(Mode::Text) | Some(Mode::Json) => Err(io::Error::other(
            "text/json modes are not wired yet; use --mode rpc or the default terminal mode",
        )),
        None => zedflow_coding_agent::modes::interactive::interactive_mode::run(args),
    }
}
