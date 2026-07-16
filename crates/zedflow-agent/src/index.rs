//! Pi-compatible root facade for `@earendil-works/pi-agent`.
//!
//! This module re-exports the implemented Rust surface that corresponds to
//! `references/pi/packages/agent/src/index.ts`. Missing Pi behavior is left out
//! instead of being hidden behind placeholder APIs.

pub use crate::agent::{Agent, AgentError, AgentEventListener, AgentOptions, AgentPromptInput};
pub use crate::agent_loop::{
    AgentEventSink, AgentEventStream, AgentLoopError, agent_loop, agent_loop_continue,
    error_assistant_stream, run_agent_loop, run_agent_loop_continue,
};
pub use crate::harness::agent_harness::{
    AgentHarness, AgentHarnessError, AgentHarnessErrorCode, AgentHarnessHook,
    AgentHarnessHookResult, AgentHarnessSubscriber, NavigateTreeOptions,
};
pub use crate::harness::compaction::branch_summarization::{
    BranchPreparation, BranchSummaryDetails, CollectEntriesResult,
    collect_entries_for_branch_summary, generate_branch_summary, prepare_branch_entries,
};
pub use crate::harness::compaction::compaction::{
    DEFAULT_COMPACTION_SETTINGS, calculate_context_tokens, compact, estimate_context_tokens,
    estimate_tokens, find_cut_point, find_turn_start_index, generate_summary,
    get_last_assistant_usage, prepare_compaction, should_compact,
};
pub use crate::harness::compaction::utils::serialize_conversation;
pub use crate::harness::messages::{
    BRANCH_SUMMARY_PREFIX, BRANCH_SUMMARY_SUFFIX, BashExecutionMessage, BranchSummaryMessage,
    COMPACTION_SUMMARY_PREFIX, COMPACTION_SUMMARY_SUFFIX, CompactionSummaryMessage, CustomMessage,
    CustomMessageContent, bash_execution_to_text, convert_to_llm, create_branch_summary_message,
    create_compaction_summary_message, create_custom_message,
};
pub use crate::harness::prompt_templates::*;
pub use crate::harness::session::jsonl_repo::JsonlSessionRepo;
pub use crate::harness::session::memory_repo::InMemorySessionRepo;
pub use crate::harness::session::repo_utils::{
    create_session_id, create_timestamp, get_entries_to_fork, get_file_system_result_or_throw,
    to_session,
};
pub use crate::harness::session::session::{Session, build_session_context};
pub use crate::harness::session::uuid::uuidv7;
pub use crate::harness::skills::*;
pub use crate::harness::system_prompt::format_skills_for_system_prompt;
pub use crate::harness::types::*;
pub use crate::harness::utils::shell_output::*;
pub use crate::harness::utils::truncate::*;
pub use crate::proxy::*;
pub use crate::types::*;

/// Index entrypoint name from the source package.
pub const INDEX_ENTRYPOINT: &str = "@earendil-works/pi-agent";
