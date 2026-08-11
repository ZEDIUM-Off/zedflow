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

/// Deterministic stand-in for the SDK's Codex cache probe.
///
/// It deliberately does not open a live Codex transport: this crate has no configured
/// credentialed transport, so callers get repeatable tool-loop records instead.
#[derive(Debug, Default)]
pub struct CodexCacheProbeSession {
    messages: Vec<CodexCacheProbeMessage>,
    assistant_requests: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodexCacheProbeMessage {
    User {
        turn: u32,
        marker: String,
    },
    Assistant(CodexCacheProbeAssistant),
    ToolResult {
        turn: u32,
        marker: String,
        text: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexCacheProbeAssistant {
    pub turn: u32,
    pub subrequest: u8,
    pub text: String,
    pub usage: CodexCacheProbeUsage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodexCacheProbeUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub total_tokens: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexCacheProbeTurn {
    pub turn: u32,
    pub tool_result: String,
    pub assistants: [CodexCacheProbeAssistant; 2],
}

impl CodexCacheProbeSession {
    /// The explicit reason live Codex probing is unavailable in this deterministic SDK surface.
    pub const LIVE_TRANSPORT_UNAVAILABLE: &'static str = "live Codex transport requires configured credentials and is not available in the deterministic SDK probe";

    #[must_use]
    pub fn messages(&self) -> &[CodexCacheProbeMessage] {
        &self.messages
    }

    /// Append one user turn, its mandatory custom-tool loop, and its final assistant reply.
    ///
    /// # Errors
    ///
    /// Returns an error for zero turns or empty markers, neither of which is a valid probe turn.
    pub fn prompt_probe(
        &mut self,
        turn: u32,
        marker: impl Into<String>,
    ) -> Result<CodexCacheProbeTurn, &'static str> {
        if turn == 0 {
            return Err("probe turn must be positive");
        }
        let marker = marker.into();
        if marker.is_empty() {
            return Err("probe marker must not be empty");
        }

        self.messages.push(CodexCacheProbeMessage::User {
            turn,
            marker: marker.clone(),
        });
        let tool_call = self.assistant(turn, 1, format!("deterministic_probe({turn}, {marker})"));
        self.messages
            .push(CodexCacheProbeMessage::Assistant(tool_call.clone()));
        let tool_result =
            format!("deterministic_probe_result turn={turn} marker={marker} fixed=OK");
        self.messages.push(CodexCacheProbeMessage::ToolResult {
            turn,
            marker: marker.clone(),
            text: tool_result.clone(),
        });
        let final_reply = self.assistant(turn, 2, format!("turn={turn} marker={marker} fixed=OK"));
        self.messages
            .push(CodexCacheProbeMessage::Assistant(final_reply.clone()));

        Ok(CodexCacheProbeTurn {
            turn,
            tool_result,
            assistants: [tool_call, final_reply],
        })
    }

    fn assistant(&mut self, turn: u32, subrequest: u8, text: String) -> CodexCacheProbeAssistant {
        let cache_read = self.assistant_requests;
        self.assistant_requests += 1;
        CodexCacheProbeAssistant {
            turn,
            subrequest,
            text,
            usage: CodexCacheProbeUsage {
                input: 1,
                output: 1,
                cache_read,
                cache_write: 0,
                total_tokens: 2,
            },
        }
    }
}
