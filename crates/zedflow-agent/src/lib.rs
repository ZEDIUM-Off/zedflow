#![forbid(unsafe_code)]

//! Zedflow agent crate.
//!
//! The crate root re-exports the Pi-compatible public facade from [`index`].
//! Node-specific exports live in [`node`].

/// Harness module namespace.
pub mod harness {
    /// Integrated agent harness.
    #[path = "agent-harness.rs"]
    pub mod agent_harness;
    /// Conversation compaction and branch summarization.
    pub mod compaction;
    /// Execution environment implementations.
    pub mod env {
        /// Stdlib-backed Node execution environment.
        pub mod nodejs;
    }
    /// Message constructors and LLM conversion.
    pub mod messages;
    /// Markdown prompt template loading and formatting.
    #[path = "prompt-templates.rs"]
    pub mod prompt_templates;
    /// Session tree, storage, and repository implementations.
    pub mod session;
    /// Skill loading and invocation formatting.
    pub mod skills;
    /// System prompt assembly helpers.
    #[path = "system-prompt.rs"]
    pub mod system_prompt;
    /// Shared harness contracts.
    pub mod types;
    /// Harness text/output utilities.
    pub mod utils {
        /// Shell output capture and sanitization.
        #[path = "shell-output.rs"]
        pub mod shell_output;
        /// Output truncation helpers.
        pub mod truncate;
    }
}

/// Stateful agent facade.
pub mod agent;
/// Core agent loop.
#[path = "agent-loop.rs"]
pub mod agent_loop;
/// Pi-compatible root facade.
pub mod index;
/// Node-specific facade.
pub mod node;
/// Proxy assistant-event parsing seam.
pub mod proxy;
/// Agent-loop and public agent contracts.
pub mod types;

pub use index::*;

/// Crate identity.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
