#![forbid(unsafe_code)]

//! Zedflow coding-agent crate.

/// Deterministic utilities ported from Pi's coding-agent package.
pub mod utils;

#[path = "core/tools/file-mutation-queue.rs"]
pub mod file_mutation_queue;
#[path = "core/tools/output-accumulator.rs"]
pub mod output_accumulator;
#[path = "core/tools/path-utils.rs"]
pub mod path_utils;
#[path = "core/tools/truncate.rs"]
pub mod truncate;

/// Pi-compatible tool namespaces.
pub mod core {
    pub mod tools {
        pub use crate::{file_mutation_queue, output_accumulator, path_utils, truncate};
    }
}

/// Crate identity, useful while the clean workspace skeleton is being filled.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
