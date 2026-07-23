//! RPC entry-point argument normalization.

use crate::cli::{Args, Mode, parse_args};

pub fn rpc_args(args: &[String]) -> Args {
    let mut normalized = Vec::with_capacity(args.len() + 2);
    normalized.extend(["--mode".to_owned(), "rpc".to_owned()]);
    normalized.extend_from_slice(args);
    let mut parsed = parse_args(&normalized);
    parsed.mode = Some(Mode::Rpc);
    parsed
}
