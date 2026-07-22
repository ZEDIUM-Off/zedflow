//! Coding-agent custom messages and conversion to model-compatible messages.
//!
//! The shared agent crate owns the `AgentMessage` union, so its Pi-compatible
//! conversion implementation is the single source of truth.

pub use zedflow_agent::{
    BRANCH_SUMMARY_PREFIX, BRANCH_SUMMARY_SUFFIX, BashExecutionMessage, BranchSummaryMessage,
    COMPACTION_SUMMARY_PREFIX, COMPACTION_SUMMARY_SUFFIX, CompactionSummaryMessage, CustomMessage,
    CustomMessageContent, bash_execution_to_text, convert_to_llm, create_branch_summary_message,
    create_compaction_summary_message, create_custom_message,
};
