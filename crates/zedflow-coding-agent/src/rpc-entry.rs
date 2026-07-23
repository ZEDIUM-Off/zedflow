//! RPC-only entry-point helpers.

use crate::{cli::parse_args, modes::run_rpc_loop};
use std::io::{self, BufRead, Write};

#[must_use]
pub fn rpc_args(args: &[String]) -> crate::cli::Args {
    let mut combined = vec!["--mode".to_owned(), "rpc".to_owned()];
    combined.extend_from_slice(args);
    parse_args(combined)
}

pub fn run<R: BufRead, W: Write>(reader: R, writer: W) -> io::Result<()> {
    run_rpc_loop(reader, writer)
}
