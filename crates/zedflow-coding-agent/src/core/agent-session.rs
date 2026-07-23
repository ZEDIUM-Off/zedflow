//! Coding-agent session facade.
//!
//! The integrated harness already owns prompt queuing, persistence, retries,
//! compaction, and tree navigation.  Keep one implementation and expose it
//! through the package-level name used by Pi's coding-agent API.

pub use zedflow_agent::harness::agent_harness::{
    AgentHarness as AgentSession, AgentHarnessError, AgentHarnessErrorCode, AgentHarnessHook,
    AgentHarnessHookResult, AgentHarnessSubscriber,
};
pub use zedflow_agent::harness::types::{
    AgentHarnessEvent as AgentSessionEvent, AgentHarnessOptions as AgentSessionOptions,
    AgentHarnessPromptOptions as PromptOptions, AgentHarnessResources as SessionResources,
};
