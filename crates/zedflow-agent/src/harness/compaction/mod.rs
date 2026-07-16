//! Compaction and branch summarization modules.

/// Branch summary collection and generation.
#[path = "branch-summarization.rs"]
pub mod branch_summarization;
/// Conversation compaction preparation and execution.
pub mod compaction;
/// Shared serialization and file-operation helpers.
pub mod utils;
