//! Compaction utilities that do not invoke a model or session runtime.

pub mod utils;

pub use utils::*;

// Model-backed compaction and branch summarization live in the agent harness;
// re-export them here so coding-agent callers use the Pi package namespace.
pub use zedflow_agent::harness::compaction::branch_summarization;
pub use zedflow_agent::harness::compaction::compaction::{
    CompactionDetails, CompactionResult, ContextUsageEstimate, DEFAULT_COMPACTION_SETTINGS,
    calculate_context_tokens, compact, estimate_context_tokens, generate_summary,
    get_last_assistant_usage, prepare_compaction, should_compact,
};
