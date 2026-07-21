#![forbid(unsafe_code)]

//! Zedflow coding-agent crate.

/// Deterministic utilities ported from Pi's coding-agent package.
pub mod utils;

#[path = "core/tools/file-mutation-queue.rs"]
pub mod file_mutation_queue;
#[path = "core/tools/find.rs"]
pub mod find;
#[path = "core/tools/ls.rs"]
pub mod ls;
#[path = "core/tools/output-accumulator.rs"]
pub mod output_accumulator;
#[path = "core/tools/path-utils.rs"]
pub mod path_utils;
#[path = "core/tools/read.rs"]
pub mod read;
#[path = "core/tools/truncate.rs"]
pub mod truncate;
#[path = "core/tools/write.rs"]
pub mod write;

/// Pi-compatible tool namespaces.
pub mod core {
    pub mod tools {
        pub use crate::{
            file_mutation_queue, find, ls, output_accumulator, path_utils, read, truncate, write,
        };
    }
}

/// Crate identity, useful while the clean workspace skeleton is being filled.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
