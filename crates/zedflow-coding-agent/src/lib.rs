#![forbid(unsafe_code)]

//! Zedflow coding-agent crate.

/// Deterministic utilities ported from Pi's coding-agent package.
pub mod utils;

#[path = "core/compaction/mod.rs"]
pub mod compaction;
#[path = "core/messages.rs"]
pub mod messages;

#[path = "core/tools/edit.rs"]
pub mod edit;
#[path = "core/tools/edit-diff.rs"]
pub mod edit_diff;
#[path = "core/tools/file-mutation-queue.rs"]
pub mod file_mutation_queue;
#[path = "core/tools/find.rs"]
pub mod find;
#[path = "core/tools/grep.rs"]
pub mod grep;
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
    pub use crate::{compaction, messages};

    pub mod tools {
        pub use crate::{
            edit, edit_diff, file_mutation_queue, find, grep, ls, output_accumulator, path_utils,
            read, truncate, write,
        };
    }
}

/// Crate identity, useful while the clean workspace skeleton is being filled.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
