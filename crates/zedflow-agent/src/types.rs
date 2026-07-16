//! Canonical agent-loop contracts ported from Pi `packages/agent/src/types.ts`.
//!
//! This module owns type foundations only. Runtime behavior is filled by later
//! port units; AI model, message, stream, and tool primitives are reused from
//! `zedflow-ai` instead of duplicated here.

use std::collections::HashSet;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use zedflow_ai::{
    Api as AiApi, AssistantMessage, AssistantMessageEvent, AssistantMessageEventStream,
    Context as AiContext, ImageContent, Message, Model, SimpleStreamOptions, TextContent, Tool,
    ToolCall, ToolResultMessage, Transport, UserMessageContent,
};

/// JSON Schema value used in place of Pi's TypeBox `TSchema`.
pub type ToolSchema = serde_json::Value;

/// Stream function used by the agent loop.
pub type StreamFn = zedflow_ai::StreamFunction<AiApi, SimpleStreamOptions>;

/// Boxed future used by async callback contracts.
pub type AgentFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Configuration for how tool calls from a single assistant message are executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ToolExecutionMode {
    /// Execute tool calls one by one.
    Sequential,
    /// Prepare sequentially, then execute allowed tools concurrently.
    #[default]
    Parallel,
}

/// Controls how many queued user messages are injected at a queue drain point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum QueueMode {
    /// Drain and inject every queued message.
    #[default]
    All,
    /// Drain and inject only the oldest queued message.
    OneAtATime,
}

/// Thinking/reasoning level accepted by the agent package, including `off`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingLevel {
    /// Disable model thinking.
    #[default]
    Off,
    /// Minimal reasoning effort.
    Minimal,
    /// Low reasoning effort.
    Low,
    /// Medium reasoning effort.
    Medium,
    /// High reasoning effort.
    High,
    /// Extra-high reasoning effort.
    #[serde(rename = "xhigh")]
    XHigh,
}

/// Agent message union: canonical LLM messages plus app-defined custom payloads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AgentMessage {
    /// Pi AI message reused from `zedflow-ai`.
    Llm(Message),
    /// Application-defined message payload.
    Custom(Value),
}

impl From<Message> for AgentMessage {
    fn from(value: Message) -> Self {
        Self::Llm(value)
    }
}

/// Final or partial result produced by a tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentToolResult<TDetails = Value> {
    /// Text or image content returned to the model.
    pub content: Vec<AgentToolResultContent>,
    /// Arbitrary structured details for logs or UI rendering.
    pub details: TDetails,
    /// Hint that the agent should stop after the current tool batch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminate: Option<bool>,
}

/// Content block returned by agent tools.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AgentToolResultContent {
    /// Text content block.
    Text(TextContent),
    /// Image content block.
    Image(ImageContent),
}

/// Callback used by tools to stream partial execution updates.
pub type AgentToolUpdateCallback<TDetails = Value> =
    Arc<dyn Fn(AgentToolResult<TDetails>) + Send + Sync>;

/// Optional compatibility shim for raw tool-call arguments before schema validation.
pub type PrepareArgumentsFn = Arc<dyn Fn(Value) -> Value + Send + Sync>;

/// Tool execution function contract.
pub type AgentToolExecuteFn<TDetails = Value> = Arc<
    dyn for<'a> Fn(
            &'a str,
            Value,
            Option<zedflow_ai::AbortSignal>,
            Option<AgentToolUpdateCallback<TDetails>>,
        ) -> AgentFuture<'a, AgentToolResult<TDetails>>
        + Send
        + Sync,
>;

/// Tool definition used by the agent runtime.
#[derive(Clone)]
pub struct AgentTool<TDetails = Value> {
    /// Pi AI tool schema and model-visible metadata.
    pub tool: Tool<ToolSchema>,
    /// Human-readable label for UI display.
    pub label: String,
    /// Optional argument preparation hook.
    pub prepare_arguments: Option<PrepareArgumentsFn>,
    /// Tool execution callback.
    pub execute: Option<AgentToolExecuteFn<TDetails>>,
    /// Per-tool execution mode override.
    pub execution_mode: Option<ToolExecutionMode>,
}

impl<TDetails> fmt::Debug for AgentTool<TDetails> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentTool")
            .field("tool", &self.tool)
            .field("label", &self.label)
            .field("has_prepare_arguments", &self.prepare_arguments.is_some())
            .field("has_execute", &self.execute.is_some())
            .field("execution_mode", &self.execution_mode)
            .finish()
    }
}

/// Context snapshot passed into the low-level agent loop.
#[derive(Debug, Clone)]
pub struct AgentContext<TDetails = Value> {
    /// System prompt included with the request.
    pub system_prompt: String,
    /// Transcript visible to the model after conversion.
    pub messages: Vec<AgentMessage>,
    /// Tools available for this run.
    pub tools: Option<Vec<AgentTool<TDetails>>>,
}

/// Public agent state snapshot.
#[derive(Debug, Clone)]
pub struct AgentState<TDetails = Value> {
    /// System prompt sent with each model request.
    pub system_prompt: String,
    /// Active model used for future turns.
    pub model: Model,
    /// Requested reasoning level for future turns.
    pub thinking_level: ThinkingLevel,
    /// Available tools.
    pub tools: Vec<AgentTool<TDetails>>,
    /// Conversation transcript.
    pub messages: Vec<AgentMessage>,
    /// True while the agent is processing a prompt or continuation.
    pub is_streaming: bool,
    /// Partial assistant message for the current streamed response, if any.
    pub streaming_message: Option<AgentMessage>,
    /// Tool call ids currently executing.
    pub pending_tool_calls: HashSet<String>,
    /// Error message from the most recent failed or aborted assistant turn, if any.
    pub error_message: Option<String>,
}

/// Result returned from `before_tool_call`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BeforeToolCallResult {
    /// Prevents the tool from executing when true.
    pub block: Option<bool>,
    /// Error text used when a call is blocked.
    pub reason: Option<String>,
}

/// Partial override returned from `after_tool_call`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AfterToolCallResult<TDetails = Value> {
    /// Replacement tool result content.
    pub content: Option<Vec<AgentToolResultContent>>,
    /// Replacement tool result details.
    pub details: Option<TDetails>,
    /// Replacement error flag.
    pub is_error: Option<bool>,
    /// Replacement early-termination hint.
    pub terminate: Option<bool>,
}

/// Context passed to `before_tool_call`.
#[derive(Debug, Clone)]
pub struct BeforeToolCallContext<TDetails = Value> {
    /// The assistant message that requested the tool call.
    pub assistant_message: AssistantMessage,
    /// The raw tool call block.
    pub tool_call: ToolCall,
    /// Validated tool arguments for the target tool schema.
    pub args: Value,
    /// Current agent context at preparation time.
    pub context: AgentContext<TDetails>,
}

/// Context passed to `after_tool_call`.
#[derive(Debug, Clone)]
pub struct AfterToolCallContext<TDetails = Value> {
    /// The assistant message that requested the tool call.
    pub assistant_message: AssistantMessage,
    /// The raw tool call block.
    pub tool_call: ToolCall,
    /// Validated tool arguments for the target tool schema.
    pub args: Value,
    /// Executed tool result before overrides.
    pub result: AgentToolResult<TDetails>,
    /// Whether the result is currently treated as an error.
    pub is_error: bool,
    /// Current agent context at finalization time.
    pub context: AgentContext<TDetails>,
}

/// Context passed to `should_stop_after_turn`.
#[derive(Debug, Clone)]
pub struct ShouldStopAfterTurnContext<TDetails = Value> {
    /// The assistant message that completed the turn.
    pub message: AssistantMessage,
    /// Tool result messages emitted by the turn.
    pub tool_results: Vec<ToolResultMessage>,
    /// Current agent context after appending turn output.
    pub context: AgentContext<TDetails>,
    /// Messages this loop invocation will return if it exits here.
    pub new_messages: Vec<AgentMessage>,
}

/// Replacement runtime state used before starting another provider request.
#[derive(Debug, Clone)]
pub struct AgentLoopTurnUpdate<TDetails = Value> {
    /// Context for the next provider request.
    pub context: Option<AgentContext<TDetails>>,
    /// Model for the next provider request.
    pub model: Option<Model>,
    /// Thinking level for the next provider request.
    pub thinking_level: Option<ThinkingLevel>,
}

/// Context passed to `prepare_next_turn`.
pub type PrepareNextTurnContext<TDetails = Value> = ShouldStopAfterTurnContext<TDetails>;

/// Converts agent messages to LLM-compatible Pi AI messages.
pub type ConvertToLlmFn =
    Arc<dyn Fn(Vec<AgentMessage>) -> AgentFuture<'static, Vec<Message>> + Send + Sync>;

/// Optional transform applied before converting to LLM messages.
pub type TransformContextFn = Arc<
    dyn Fn(
            Vec<AgentMessage>,
            Option<zedflow_ai::AbortSignal>,
        ) -> AgentFuture<'static, Vec<AgentMessage>>
        + Send
        + Sync,
>;

/// Dynamic API-key resolver.
pub type GetApiKeyFn = Arc<dyn Fn(String) -> AgentFuture<'static, Option<String>> + Send + Sync>;

/// Graceful stop hook after a turn completes.
pub type ShouldStopAfterTurnFn<TDetails = Value> =
    Arc<dyn Fn(ShouldStopAfterTurnContext<TDetails>) -> AgentFuture<'static, bool> + Send + Sync>;

/// Hook that may replace context/model/thinking before the next turn.
pub type PrepareNextTurnFn<TDetails = Value> = Arc<
    dyn Fn(
            PrepareNextTurnContext<TDetails>,
        ) -> AgentFuture<'static, Option<AgentLoopTurnUpdate<TDetails>>>
        + Send
        + Sync,
>;

/// Queued-message provider hook.
pub type MessageQueueFn = Arc<dyn Fn() -> AgentFuture<'static, Vec<AgentMessage>> + Send + Sync>;

/// Hook called before a tool executes.
pub type BeforeToolCallFn<TDetails = Value> = Arc<
    dyn Fn(
            BeforeToolCallContext<TDetails>,
            Option<zedflow_ai::AbortSignal>,
        ) -> AgentFuture<'static, Option<BeforeToolCallResult>>
        + Send
        + Sync,
>;

/// Hook called after a tool executes.
pub type AfterToolCallFn<TDetails = Value> = Arc<
    dyn Fn(
            AfterToolCallContext<TDetails>,
            Option<zedflow_ai::AbortSignal>,
        ) -> AgentFuture<'static, Option<AfterToolCallResult<TDetails>>>
        + Send
        + Sync,
>;

/// Low-level agent-loop configuration.
#[derive(Clone)]
pub struct AgentLoopConfig<TDetails = Value> {
    /// Base provider stream options.
    pub stream_options: SimpleStreamOptions,
    /// Model used for the first provider request.
    pub model: Model,
    /// Converts agent messages to LLM-compatible messages.
    pub convert_to_llm: ConvertToLlmFn,
    /// Optional transform applied before LLM conversion.
    pub transform_context: Option<TransformContextFn>,
    /// Optional dynamic API-key resolver.
    pub get_api_key: Option<GetApiKeyFn>,
    /// Optional graceful stop hook.
    pub should_stop_after_turn: Option<ShouldStopAfterTurnFn<TDetails>>,
    /// Optional next-turn preparation hook.
    pub prepare_next_turn: Option<PrepareNextTurnFn<TDetails>>,
    /// Optional steering message provider.
    pub get_steering_messages: Option<MessageQueueFn>,
    /// Optional follow-up message provider.
    pub get_follow_up_messages: Option<MessageQueueFn>,
    /// Tool execution mode.
    pub tool_execution: ToolExecutionMode,
    /// Optional pre-execution tool hook.
    pub before_tool_call: Option<BeforeToolCallFn<TDetails>>,
    /// Optional post-execution tool hook.
    pub after_tool_call: Option<AfterToolCallFn<TDetails>>,
}

impl<TDetails> fmt::Debug for AgentLoopConfig<TDetails> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentLoopConfig")
            .field("model", &self.model)
            .field("tool_execution", &self.tool_execution)
            .finish_non_exhaustive()
    }
}

/// Events emitted by the agent for UI updates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum AgentEvent {
    /// Agent run started.
    AgentStart,
    /// Agent run ended.
    AgentEnd {
        /// New messages produced by the run.
        messages: Vec<AgentMessage>,
    },
    /// Assistant turn started.
    TurnStart,
    /// Assistant turn ended.
    TurnEnd {
        /// Completed assistant-visible message.
        message: AgentMessage,
        /// Tool results emitted by the turn.
        tool_results: Vec<ToolResultMessage>,
    },
    /// Message started.
    MessageStart {
        /// Message being emitted.
        message: AgentMessage,
    },
    /// Assistant message stream update.
    MessageUpdate {
        /// Partial assistant message.
        message: AgentMessage,
        /// Raw assistant stream event.
        assistant_message_event: AssistantMessageEvent,
    },
    /// Message finished.
    MessageEnd {
        /// Finished message.
        message: AgentMessage,
    },
    /// Tool execution started.
    ToolExecutionStart {
        /// Tool call id.
        tool_call_id: String,
        /// Tool name.
        tool_name: String,
        /// Validated arguments.
        args: Value,
    },
    /// Tool execution partial update.
    ToolExecutionUpdate {
        /// Tool call id.
        tool_call_id: String,
        /// Tool name.
        tool_name: String,
        /// Validated arguments.
        args: Value,
        /// Partial result payload.
        partial_result: Value,
    },
    /// Tool execution finished.
    ToolExecutionEnd {
        /// Tool call id.
        tool_call_id: String,
        /// Tool name.
        tool_name: String,
        /// Final result payload.
        result: Value,
        /// Whether final result is an error.
        is_error: bool,
    },
}
