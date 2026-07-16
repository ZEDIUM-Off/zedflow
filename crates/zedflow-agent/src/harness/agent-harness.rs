//! Agent harness integration ported from Pi.
//!
//! The harness wires sessions, resources, compaction, branch navigation, provider
//! stream options, and the low-level agent loop without redefining foundation
//! contracts from sibling modules.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use futures::executor::block_on;
use serde_json::Value;
use zedflow_ai::utils::abort_signals::AbortController;
use zedflow_ai::{
    AssistantMessage, ImageContent, Message, Model, Models, ProviderHookError, ProviderResponse,
    SimpleStreamOptions, TextContent, TextContentType, UserContentBlock, UserMessage,
    UserMessageContent, UserMessageRole,
};

use crate::agent_loop::{AgentEventSink, run_agent_loop};
use crate::harness::compaction::branch_summarization::{
    GenerateBranchSummaryOptions as BranchSummaryOptions, collect_entries_for_branch_summary,
    generate_branch_summary,
};
use crate::harness::compaction::compaction::{
    DEFAULT_COMPACTION_SETTINGS, compact as compact_session, prepare_compaction,
};
use crate::harness::messages::convert_to_llm;
use crate::harness::prompt_templates::format_prompt_template_invocation;
use crate::harness::skills::format_skill_invocation;
use crate::harness::types::{
    AbortResult, AfterProviderResponseEvent, AgentHarnessEvent, AgentHarnessOptions,
    AgentHarnessOwnEvent, AgentHarnessPhase, AgentHarnessPromptOptions, AgentHarnessResources,
    AgentHarnessStreamOptions, AgentHarnessStreamOptionsPatch, BeforeAgentStartEvent,
    BeforeAgentStartResult, BeforeProviderPayloadEvent, BeforeProviderPayloadResult,
    BeforeProviderRequestEvent, BeforeProviderRequestResult, BranchSummaryDraft, CompactResult,
    ContextEvent, ContextResult, CustomMessageContent, ExecutionEnv, ModelUpdateEvent,
    NavigateTreeResult, PromptTemplate, QueueUpdateEvent, ResourcesUpdateEvent, RestoreSource,
    SavePointEvent, Session, SessionBeforeCompactEvent, SessionBeforeCompactResult,
    SessionBeforeTreeEvent, SessionBeforeTreeResult, SessionCompactEvent, SessionTreeEvent,
    SettledEvent, Skill, SystemPrompt, SystemPromptContext, ThinkingLevelUpdateEvent,
    ToolCallEvent, ToolCallResult, ToolResultEvent, ToolResultPatch, ToolsUpdateEvent,
};
use crate::types::{
    AgentContext, AgentEvent, AgentLoopConfig, AgentMessage, AgentTool, AgentToolResultContent,
    QueueMode, ThinkingLevel, ToolExecutionMode,
};

/// Harness-level error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentHarnessErrorCode {
    /// Harness is already running an exclusive operation.
    Busy,
    /// Caller passed an invalid argument.
    InvalidArgument,
    /// Harness state does not allow the requested operation.
    InvalidState,
    /// Session operation failed.
    Session,
    /// Hook or subscriber failed.
    Hook,
    /// Compaction failed.
    Compaction,
    /// Branch summary failed.
    BranchSummary,
    /// Unknown failure.
    Unknown,
}

/// Error returned by [`AgentHarness`] operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentHarnessError {
    /// Stable error code.
    pub code: AgentHarnessErrorCode,
    /// Human-readable message.
    pub message: String,
    /// Optional stringified cause.
    pub cause: Option<String>,
}

impl AgentHarnessError {
    /// Create a harness error.
    #[must_use]
    pub fn new(
        code: AgentHarnessErrorCode,
        message: impl Into<String>,
        cause: Option<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            cause,
        }
    }
}

impl fmt::Display for AgentHarnessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for AgentHarnessError {}

/// Result produced by a harness hook.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentHarnessHookResult {
    /// `before_agent_start` result.
    BeforeAgentStart(BeforeAgentStartResult),
    /// `context` result.
    Context(ContextResult),
    /// `before_provider_request` result.
    BeforeProviderRequest(BeforeProviderRequestResult),
    /// `before_provider_payload` result.
    BeforeProviderPayload(BeforeProviderPayloadResult),
    /// `tool_call` result.
    ToolCall(ToolCallResult),
    /// `tool_result` patch.
    ToolResult(ToolResultPatch),
    /// `session_before_compact` result.
    SessionBeforeCompact(SessionBeforeCompactResult),
    /// `session_before_tree` result.
    SessionBeforeTree(SessionBeforeTreeResult),
}

/// Async harness hook callback.
pub type AgentHarnessHook = Arc<
    dyn Fn(
            AgentHarnessOwnEvent,
        ) -> crate::harness::types::HarnessFuture<'static, Option<AgentHarnessHookResult>>
        + Send
        + Sync,
>;

/// Async harness event subscriber.
pub type AgentHarnessSubscriber = Arc<
    dyn Fn(
            AgentHarnessEvent,
        ) -> crate::harness::types::HarnessFuture<'static, Result<(), AgentHarnessError>>
        + Send
        + Sync,
>;

#[derive(Debug, Clone)]
enum PendingSessionWrite {
    Message(AgentMessage),
    ModelChange { provider: String, model_id: String },
    ThinkingLevelChange { thinking_level: String },
    ActiveToolsChange { active_tool_names: Vec<String> },
}

struct AgentHarnessState<TSkill = Skill, TPromptTemplate = crate::harness::types::PromptTemplate> {
    phase: AgentHarnessPhase,
    model: Model,
    thinking_level: ThinkingLevel,
    system_prompt: Option<SystemPrompt<TSkill, TPromptTemplate>>,
    stream_options: AgentHarnessStreamOptions,
    resources: AgentHarnessResources<TSkill, TPromptTemplate>,
    tools: Vec<AgentTool>,
    active_tool_names: Vec<String>,
    steer_queue: Vec<AgentMessage>,
    steering_queue_mode: QueueMode,
    follow_up_queue: Vec<AgentMessage>,
    follow_up_queue_mode: QueueMode,
    next_turn_queue: Vec<AgentMessage>,
    pending_writes: Vec<PendingSessionWrite>,
    hooks: HashMap<String, Vec<AgentHarnessHook>>,
    subscribers: Vec<AgentHarnessSubscriber>,
    abort_controller: Option<AbortController>,
}

/// Integrated Pi-style agent harness.
pub struct AgentHarness<TSkill = Skill, TPromptTemplate = crate::harness::types::PromptTemplate> {
    env: Arc<dyn ExecutionEnv>,
    session: Arc<dyn Session>,
    models: Arc<Models>,
    state: Arc<Mutex<AgentHarnessState<TSkill, TPromptTemplate>>>,
}

impl AgentHarness<Skill, crate::harness::types::PromptTemplate> {
    /// Create a harness from existing foundation modules.
    ///
    /// # Errors
    ///
    /// Returns `invalid_argument` when tool or active-tool names are duplicated or unknown.
    pub fn new(options: AgentHarnessOptions) -> Result<Self, AgentHarnessError> {
        let tools = options.tools.unwrap_or_default();
        validate_unique_names(
            &tools.iter().map(tool_name).collect::<Vec<_>>(),
            "Duplicate tool name(s)",
        )?;
        let active_tool_names = options
            .active_tool_names
            .unwrap_or_else(|| tools.iter().map(tool_name).collect());
        validate_tool_names(&active_tool_names, &tools)?;

        Ok(Self {
            env: options.env,
            session: options.session,
            models: Arc::new(options.models),
            state: Arc::new(Mutex::new(AgentHarnessState {
                phase: AgentHarnessPhase::Idle,
                model: options.model,
                thinking_level: options.thinking_level.unwrap_or_default(),
                system_prompt: options.system_prompt,
                stream_options: options.stream_options.unwrap_or_default(),
                resources: options.resources.unwrap_or(AgentHarnessResources {
                    prompt_templates: None,
                    skills: None,
                }),
                tools,
                active_tool_names,
                steer_queue: Vec::new(),
                steering_queue_mode: options.steering_mode.unwrap_or(QueueMode::OneAtATime),
                follow_up_queue: Vec::new(),
                follow_up_queue_mode: options.follow_up_mode.unwrap_or(QueueMode::OneAtATime),
                next_turn_queue: Vec::new(),
                pending_writes: Vec::new(),
                hooks: HashMap::new(),
                subscribers: Vec::new(),
                abort_controller: None,
            })),
        })
    }

    /// Current execution environment.
    #[must_use]
    pub fn env(&self) -> Arc<dyn ExecutionEnv> {
        Arc::clone(&self.env)
    }

    /// Current session handle.
    #[must_use]
    pub fn session(&self) -> Arc<dyn Session> {
        Arc::clone(&self.session)
    }

    /// Subscribe to every low-level and harness-owned event.
    pub fn subscribe(&self, listener: AgentHarnessSubscriber) -> impl FnOnce() + '_ {
        let mut state = lock_state(&self.state);
        state.subscribers.push(listener.clone());
        move || {
            lock_state(&self.state)
                .subscribers
                .retain(|candidate| !Arc::ptr_eq(candidate, &listener));
        }
    }

    /// Register a hook for one harness-owned event type such as `context` or `tool_call`.
    pub fn on(
        &self,
        event_type: impl Into<String>,
        handler: AgentHarnessHook,
    ) -> impl FnOnce() + '_ {
        let event_type = event_type.into();
        let mut state = lock_state(&self.state);
        state
            .hooks
            .entry(event_type.clone())
            .or_default()
            .push(handler.clone());
        move || {
            if let Some(handlers) = lock_state(&self.state).hooks.get_mut(&event_type) {
                handlers.retain(|candidate| !Arc::ptr_eq(candidate, &handler));
            }
        }
    }

    /// Run a prompt through the low-level agent loop and persist emitted messages.
    ///
    /// # Errors
    ///
    /// Returns harness, hook, session, provider, or loop failures as [`AgentHarnessError`].
    pub async fn prompt(
        &self,
        text: impl Into<String>,
        options: Option<AgentHarnessPromptOptions>,
    ) -> Result<AssistantMessage, AgentHarnessError> {
        self.start_phase(AgentHarnessPhase::Turn)?;
        let result = async {
            let text = text.into();
            let turn_state = self.create_turn_state().await?;
            self.execute_turn(turn_state, text, options.unwrap_or_default())
                .await
        }
        .await;
        self.finish_phase().await;
        result
    }

    /// Invoke a loaded skill by name.
    ///
    /// # Errors
    ///
    /// Returns `invalid_argument` when the skill does not exist.
    pub async fn skill(
        &self,
        name: &str,
        additional_instructions: Option<&str>,
    ) -> Result<AssistantMessage, AgentHarnessError> {
        self.start_phase(AgentHarnessPhase::Turn)?;
        let result = async {
            let turn_state = self.create_turn_state().await?;
            let skill = turn_state
                .resources
                .skills
                .clone()
                .unwrap_or_default()
                .into_iter()
                .find(|candidate| candidate.name == name)
                .ok_or_else(|| invalid_argument(format!("Unknown skill: {name}")))?;
            self.execute_turn(
                turn_state,
                format_skill_invocation(&skill, additional_instructions),
                AgentHarnessPromptOptions::default(),
            )
            .await
        }
        .await;
        self.finish_phase().await;
        result
    }

    /// Invoke a loaded prompt template by name.
    ///
    /// # Errors
    ///
    /// Returns `invalid_argument` when the template does not exist.
    pub async fn prompt_from_template(
        &self,
        name: &str,
        args: &[String],
    ) -> Result<AssistantMessage, AgentHarnessError> {
        self.start_phase(AgentHarnessPhase::Turn)?;
        let result = async {
            let turn_state = self.create_turn_state().await?;
            let template = turn_state
                .resources
                .prompt_templates
                .clone()
                .unwrap_or_default()
                .into_iter()
                .find(|candidate| candidate.name == name)
                .ok_or_else(|| invalid_argument(format!("Unknown prompt template: {name}")))?;
            self.execute_turn(
                turn_state,
                format_prompt_template_invocation(&template, args),
                AgentHarnessPromptOptions::default(),
            )
            .await
        }
        .await;
        self.finish_phase().await;
        result
    }

    /// Queue steering text for the current run.
    ///
    /// # Errors
    ///
    /// Returns `invalid_state` when no run is active.
    pub async fn steer(
        &self,
        text: impl Into<String>,
        options: Option<AgentHarnessPromptOptions>,
    ) -> Result<(), AgentHarnessError> {
        self.queue_running_message(text.into(), options, QueueKind::Steer)
            .await
    }

    /// Queue follow-up text for the current run.
    ///
    /// # Errors
    ///
    /// Returns `invalid_state` when no run is active.
    pub async fn follow_up(
        &self,
        text: impl Into<String>,
        options: Option<AgentHarnessPromptOptions>,
    ) -> Result<(), AgentHarnessError> {
        self.queue_running_message(text.into(), options, QueueKind::FollowUp)
            .await
    }

    /// Queue text for the next prompt.
    ///
    /// # Errors
    ///
    /// Returns hook failures from queue update subscribers.
    pub async fn next_turn(
        &self,
        text: impl Into<String>,
        options: Option<AgentHarnessPromptOptions>,
    ) -> Result<(), AgentHarnessError> {
        {
            let mut state = lock_state(&self.state);
            state.next_turn_queue.push(create_user_message(
                text.into(),
                options.and_then(|value| value.images),
            ));
        }
        emit_queue_update(&self.state).await
    }

    /// Append a message now, or defer until the active turn reaches a save point.
    ///
    /// # Errors
    ///
    /// Returns session failures.
    pub async fn append_message(&self, message: AgentMessage) -> Result<(), AgentHarnessError> {
        if self.phase() == AgentHarnessPhase::Idle {
            self.session
                .append_message(message)
                .await
                .map_err(|error| normalize_session_error(error.to_string()))?;
        } else {
            lock_state(&self.state)
                .pending_writes
                .push(PendingSessionWrite::Message(message));
        }
        Ok(())
    }

    /// Compact current session branch.
    ///
    /// # Errors
    ///
    /// Returns compaction, hook, or session failures.
    pub async fn compact(
        &self,
        custom_instructions: Option<&str>,
    ) -> Result<CompactResult, AgentHarnessError> {
        self.start_phase(AgentHarnessPhase::Compaction)?;
        let result = async {
            let (model, thinking_level) = {
                let state = lock_state(&self.state);
                (state.model.clone(), state.thinking_level)
            };
            let branch_entries = self.session.get_branch(None).await;
            let preparation = prepare_compaction(&branch_entries, DEFAULT_COMPACTION_SETTINGS)
                .map_err(|error| normalize_compaction_error(error.to_string()))?
                .ok_or_else(|| normalize_compaction_error("Nothing to compact"))?;
            let hook = emit_hook(
                &self.state,
                "session_before_compact",
                AgentHarnessOwnEvent::SessionBeforeCompact(SessionBeforeCompactEvent {
                    preparation: preparation.clone(),
                    branch_entries: branch_entries.clone(),
                    custom_instructions: custom_instructions.map(str::to_string),
                }),
            )
            .await?;
            let provided = match hook {
                Some(AgentHarnessHookResult::SessionBeforeCompact(result))
                    if result.cancel.unwrap_or(false) =>
                {
                    return Err(normalize_compaction_error("Compaction cancelled"));
                }
                Some(AgentHarnessHookResult::SessionBeforeCompact(result)) => result.compaction,
                _ => None,
            };
            let result = if let Some(provided) = provided {
                provided
            } else {
                let generated = compact_session(
                    &preparation,
                    &self.models,
                    &model,
                    custom_instructions,
                    Some(thinking_level),
                )
                .map_err(|error| normalize_compaction_error(error.to_string()))?;
                CompactResult {
                    summary: generated.summary,
                    first_kept_entry_id: generated.first_kept_entry_id,
                    tokens_before: generated.tokens_before,
                    details: generated.details,
                }
            };
            let entry_id = self
                .session
                .append_compaction(
                    result.summary.clone(),
                    result.first_kept_entry_id.clone(),
                    result.tokens_before,
                    result.details.clone(),
                    None,
                )
                .await
                .map_err(|error| normalize_session_error(error.to_string()))?;
            if let Some(crate::harness::types::SessionTreeEntry::Compaction(entry)) =
                self.session.get_entry(&entry_id).await
            {
                emit_own(
                    &self.state,
                    AgentHarnessOwnEvent::SessionCompact(SessionCompactEvent {
                        compaction_entry: entry,
                        from_hook: false,
                    }),
                )
                .await?;
            }
            Ok(result)
        }
        .await;
        self.finish_phase().await;
        result
    }

    /// Navigate the session tree, optionally generating a branch summary.
    ///
    /// # Errors
    ///
    /// Returns branch-summary, hook, or session failures.
    pub async fn navigate_tree(
        &self,
        target_id: &str,
        options: NavigateTreeOptions,
    ) -> Result<NavigateTreeResult, AgentHarnessError> {
        self.start_phase(AgentHarnessPhase::BranchSummary)?;
        let result = async {
            let old_leaf_id = self.session.get_leaf_id().await;
            if old_leaf_id.as_deref() == Some(target_id) {
                return Ok(NavigateTreeResult::default());
            }
            let target_entry = self
                .session
                .get_entry(target_id)
                .await
                .ok_or_else(|| invalid_argument(format!("Entry {target_id} not found")))?;
            let collected = collect_entries_for_branch_summary(
                self.session.as_ref(),
                old_leaf_id.as_deref(),
                target_id,
            )
            .await
            .map_err(|error| normalize_branch_error(error.to_string()))?;
            let preparation = crate::harness::types::TreePreparation {
                target_id: target_id.to_string(),
                old_leaf_id: old_leaf_id.clone(),
                common_ancestor_id: collected.common_ancestor_id.clone(),
                entries_to_summarize: collected.entries.clone(),
                user_wants_summary: options.summarize,
                custom_instructions: options.custom_instructions.clone(),
                replace_instructions: Some(options.replace_instructions),
                label: options.label.clone(),
            };
            let hook = emit_hook(
                &self.state,
                "session_before_tree",
                AgentHarnessOwnEvent::SessionBeforeTree(SessionBeforeTreeEvent { preparation }),
            )
            .await?;
            let hook = match hook {
                Some(AgentHarnessHookResult::SessionBeforeTree(result)) => result,
                _ => SessionBeforeTreeResult::default(),
            };
            if hook.cancel.unwrap_or(false) {
                return Ok(NavigateTreeResult {
                    cancelled: true,
                    ..NavigateTreeResult::default()
                });
            }
            let mut summary_text = hook.summary.as_ref().map(|summary| summary.summary.clone());
            let mut summary_details = hook
                .summary
                .as_ref()
                .and_then(|summary| summary.details.clone());
            if summary_text.is_none() && options.summarize && !collected.entries.is_empty() {
                let (model, _) = {
                    let state = lock_state(&self.state);
                    (state.model.clone(), state.thinking_level)
                };
                let summary = generate_branch_summary(
                    &collected.entries,
                    BranchSummaryOptions {
                        models: &self.models,
                        model: &model,
                        signal: None,
                        custom_instructions: hook
                            .custom_instructions
                            .as_deref()
                            .or(options.custom_instructions.as_deref()),
                        replace_instructions: hook
                            .replace_instructions
                            .unwrap_or(options.replace_instructions),
                        reserve_tokens: None,
                    },
                )
                .map_err(|error| normalize_branch_error(error.to_string()))?;
                summary_text = Some(summary.summary);
                summary_details = Some(serde_json::json!({
                    "readFiles": summary.read_files,
                    "modifiedFiles": summary.modified_files,
                }));
            }

            let (new_leaf_id, editor_text) = navigation_target(&target_entry, target_id);
            let summary_id = self
                .session
                .move_to(
                    new_leaf_id,
                    summary_text.map(|summary| BranchSummaryDraft {
                        summary,
                        details: summary_details,
                        from_hook: hook.summary.is_some().then_some(true),
                    }),
                )
                .await
                .map_err(|error| normalize_session_error(error.to_string()))?;
            let summary_entry = summary_id
                .as_deref()
                .and_then(|id| block_on(self.session.get_entry(id)))
                .and_then(|entry| match entry {
                    crate::harness::types::SessionTreeEntry::BranchSummary(entry) => Some(entry),
                    _ => None,
                });
            emit_own(
                &self.state,
                AgentHarnessOwnEvent::SessionTree(SessionTreeEvent {
                    new_leaf_id: self.session.get_leaf_id().await,
                    old_leaf_id,
                    summary_entry: summary_entry.clone(),
                    from_hook: hook.summary.is_some().then_some(true),
                }),
            )
            .await?;
            Ok(NavigateTreeResult {
                cancelled: false,
                editor_text,
                summary_entry,
            })
        }
        .await;
        self.finish_phase().await;
        result
    }

    /// Current model.
    #[must_use]
    pub fn get_model(&self) -> Model {
        lock_state(&self.state).model.clone()
    }

    /// Set active model and persist the change.
    ///
    /// # Errors
    ///
    /// Returns session or hook failures.
    pub async fn set_model(&self, model: Model) -> Result<(), AgentHarnessError> {
        let previous_model = Some(self.get_model());
        if self.phase() == AgentHarnessPhase::Idle {
            self.session
                .append_model_change(model.provider.clone(), model.id.clone())
                .await
                .map_err(|error| normalize_session_error(error.to_string()))?;
        } else {
            lock_state(&self.state)
                .pending_writes
                .push(PendingSessionWrite::ModelChange {
                    provider: model.provider.clone(),
                    model_id: model.id.clone(),
                });
        }
        lock_state(&self.state).model = model.clone();
        emit_own(
            &self.state,
            AgentHarnessOwnEvent::ModelUpdate(ModelUpdateEvent {
                model,
                previous_model,
                source: RestoreSource::Set,
            }),
        )
        .await
    }

    /// Current thinking level.
    #[must_use]
    pub fn get_thinking_level(&self) -> ThinkingLevel {
        lock_state(&self.state).thinking_level
    }

    /// Set active thinking level and persist the change.
    ///
    /// # Errors
    ///
    /// Returns session or hook failures.
    pub async fn set_thinking_level(&self, level: ThinkingLevel) -> Result<(), AgentHarnessError> {
        let previous_level = self.get_thinking_level();
        if self.phase() == AgentHarnessPhase::Idle {
            self.session
                .append_thinking_level_change(thinking_level_string(level))
                .await
                .map_err(|error| normalize_session_error(error.to_string()))?;
        } else {
            lock_state(&self.state)
                .pending_writes
                .push(PendingSessionWrite::ThinkingLevelChange {
                    thinking_level: thinking_level_string(level),
                });
        }
        lock_state(&self.state).thinking_level = level;
        emit_own(
            &self.state,
            AgentHarnessOwnEvent::ThinkingLevelUpdate(ThinkingLevelUpdateEvent {
                level,
                previous_level,
            }),
        )
        .await
    }

    /// Configured tools.
    #[must_use]
    pub fn get_tools(&self) -> Vec<AgentTool> {
        lock_state(&self.state).tools.clone()
    }

    /// Active tool list.
    #[must_use]
    pub fn get_active_tools(&self) -> Vec<AgentTool> {
        let state = lock_state(&self.state);
        active_tools(&state)
    }

    /// Replace tools and active-tool names.
    ///
    /// # Errors
    ///
    /// Returns `invalid_argument` for duplicate or unknown names.
    pub async fn set_tools(
        &self,
        tools: Vec<AgentTool>,
        active_tool_names: Option<Vec<String>>,
    ) -> Result<(), AgentHarnessError> {
        validate_unique_names(
            &tools.iter().map(tool_name).collect::<Vec<_>>(),
            "Duplicate tool name(s)",
        )?;
        let (previous_tool_names, previous_active_tool_names, next_active_tool_names) = {
            let state = lock_state(&self.state);
            let next_active = active_tool_names.unwrap_or_else(|| state.active_tool_names.clone());
            validate_tool_names(&next_active, &tools)?;
            (
                state.tools.iter().map(tool_name).collect::<Vec<_>>(),
                state.active_tool_names.clone(),
                next_active,
            )
        };
        self.persist_active_tools(next_active_tool_names.clone())
            .await?;
        {
            let mut state = lock_state(&self.state);
            state.tools = tools;
            state.active_tool_names = next_active_tool_names.clone();
        }
        emit_own(
            &self.state,
            AgentHarnessOwnEvent::ToolsUpdate(ToolsUpdateEvent {
                tool_names: self.get_tools().iter().map(tool_name).collect(),
                previous_tool_names,
                active_tool_names: next_active_tool_names,
                previous_active_tool_names,
                source: RestoreSource::Set,
            }),
        )
        .await
    }

    /// Set active tool names.
    ///
    /// # Errors
    ///
    /// Returns `invalid_argument` for duplicate or unknown names.
    pub async fn set_active_tools(&self, tool_names: Vec<String>) -> Result<(), AgentHarnessError> {
        let (previous_tool_names, previous_active_tool_names) = {
            let state = lock_state(&self.state);
            validate_tool_names(&tool_names, &state.tools)?;
            (
                state.tools.iter().map(tool_name).collect::<Vec<_>>(),
                state.active_tool_names.clone(),
            )
        };
        self.persist_active_tools(tool_names.clone()).await?;
        lock_state(&self.state).active_tool_names = tool_names.clone();
        emit_own(
            &self.state,
            AgentHarnessOwnEvent::ToolsUpdate(ToolsUpdateEvent {
                tool_names: previous_tool_names.clone(),
                previous_tool_names,
                active_tool_names: tool_names,
                previous_active_tool_names,
                source: RestoreSource::Set,
            }),
        )
        .await
    }

    /// Get steering queue drain mode.
    #[must_use]
    pub fn get_steering_mode(&self) -> QueueMode {
        lock_state(&self.state).steering_queue_mode
    }

    /// Set steering queue drain mode.
    pub fn set_steering_mode(&self, mode: QueueMode) {
        lock_state(&self.state).steering_queue_mode = mode;
    }

    /// Get follow-up queue drain mode.
    #[must_use]
    pub fn get_follow_up_mode(&self) -> QueueMode {
        lock_state(&self.state).follow_up_queue_mode
    }

    /// Set follow-up queue drain mode.
    pub fn set_follow_up_mode(&self, mode: QueueMode) {
        lock_state(&self.state).follow_up_queue_mode = mode;
    }

    /// Get current resources.
    #[must_use]
    pub fn get_resources(&self) -> AgentHarnessResources {
        lock_state(&self.state).resources.clone()
    }

    /// Replace current resources.
    ///
    /// # Errors
    ///
    /// Returns subscriber hook failures.
    pub async fn set_resources(
        &self,
        resources: AgentHarnessResources<Skill, PromptTemplate>,
    ) -> Result<(), AgentHarnessError> {
        let previous_resources = self.get_resources();
        lock_state(&self.state).resources = resources.clone();
        emit_own(
            &self.state,
            AgentHarnessOwnEvent::ResourcesUpdate(ResourcesUpdateEvent {
                resources,
                previous_resources,
            }),
        )
        .await
    }

    /// Get stream options snapshot.
    #[must_use]
    pub fn get_stream_options(&self) -> AgentHarnessStreamOptions {
        lock_state(&self.state).stream_options.clone()
    }

    /// Replace stream options.
    pub fn set_stream_options(&self, stream_options: AgentHarnessStreamOptions) {
        lock_state(&self.state).stream_options = stream_options;
    }

    /// Abort active run and clear steering/follow-up queues.
    ///
    /// # Errors
    ///
    /// Returns hook failures.
    pub async fn abort(&self) -> Result<AbortResult, AgentHarnessError> {
        let (cleared_steer, cleared_follow_up, signal) = {
            let mut state = lock_state(&self.state);
            let cleared_steer = std::mem::take(&mut state.steer_queue);
            let cleared_follow_up = std::mem::take(&mut state.follow_up_queue);
            (
                cleared_steer,
                cleared_follow_up,
                state.abort_controller.clone(),
            )
        };
        if let Some(signal) = signal {
            signal.abort();
        }
        emit_queue_update(&self.state).await?;
        emit_own(
            &self.state,
            AgentHarnessOwnEvent::Abort(crate::harness::types::AbortEvent {
                cleared_steer: cleared_steer.clone(),
                cleared_follow_up: cleared_follow_up.clone(),
            }),
        )
        .await?;
        Ok(AbortResult {
            cleared_steer,
            cleared_follow_up,
        })
    }

    /// This synchronous Rust harness has no background run task; the method is a no-op.
    pub async fn wait_for_idle(&self) {}

    fn phase(&self) -> AgentHarnessPhase {
        lock_state(&self.state).phase
    }

    fn start_phase(&self, phase: AgentHarnessPhase) -> Result<(), AgentHarnessError> {
        let mut state = lock_state(&self.state);
        if state.phase != AgentHarnessPhase::Idle {
            return Err(AgentHarnessError::new(
                AgentHarnessErrorCode::Busy,
                "AgentHarness is busy",
                None,
            ));
        }
        state.phase = phase;
        Ok(())
    }

    async fn finish_phase(&self) {
        let _ = self.flush_pending_session_writes().await;
        let mut state = lock_state(&self.state);
        state.phase = AgentHarnessPhase::Idle;
        state.abort_controller = None;
    }

    async fn create_turn_state(
        &self,
    ) -> Result<AgentHarnessTurnState<Skill, PromptTemplate>, AgentHarnessError> {
        let context = self.session.build_context().await;
        let (resources, tools, active_tool_names, model, thinking_level, stream_options) = {
            let state = lock_state(&self.state);
            (
                state.resources.clone(),
                state.tools.clone(),
                state.active_tool_names.clone(),
                state.model.clone(),
                state.thinking_level,
                state.stream_options.clone(),
            )
        };
        let active_tools = active_tool_names
            .iter()
            .filter_map(|name| tools.iter().find(|tool| tool_name(tool) == *name).cloned())
            .collect::<Vec<_>>();
        let system_prompt = self
            .system_prompt(&model, thinking_level, &active_tools, &resources)
            .await;
        Ok(AgentHarnessTurnState {
            messages: context.messages,
            resources,
            stream_options,
            session_id: self.session.get_metadata().await.id,
            system_prompt,
            model,
            thinking_level,
            active_tools,
        })
    }

    async fn system_prompt(
        &self,
        model: &Model,
        thinking_level: ThinkingLevel,
        active_tools: &[AgentTool],
        resources: &AgentHarnessResources<Skill, PromptTemplate>,
    ) -> String {
        let system_prompt = {
            let mut state = lock_state(&self.state);
            state.system_prompt.take()
        };
        let output = match &system_prompt {
            Some(SystemPrompt::Text(text)) => text.clone(),
            Some(SystemPrompt::Callback(callback)) => {
                callback(SystemPromptContext {
                    env: Arc::clone(&self.env),
                    session: Arc::clone(&self.session),
                    model: model.clone(),
                    thinking_level,
                    active_tools: active_tools.to_vec(),
                    resources: resources.clone(),
                })
                .await
            }
            None => "You are a helpful assistant.".to_string(),
        };
        lock_state(&self.state).system_prompt = system_prompt;
        output
    }

    async fn execute_turn(
        &self,
        turn_state: AgentHarnessTurnState<Skill, PromptTemplate>,
        text: String,
        options: AgentHarnessPromptOptions,
    ) -> Result<AssistantMessage, AgentHarnessError> {
        let mut prompt_messages = vec![create_user_message(text.clone(), options.images.clone())];
        let queued = {
            let mut state = lock_state(&self.state);
            std::mem::take(&mut state.next_turn_queue)
        };
        if !queued.is_empty() {
            emit_queue_update(&self.state).await?;
            prompt_messages.splice(0..0, queued);
        }

        let before = emit_hook(
            &self.state,
            "before_agent_start",
            AgentHarnessOwnEvent::BeforeAgentStart(BeforeAgentStartEvent {
                prompt: text,
                images: options.images,
                system_prompt: turn_state.system_prompt.clone(),
                resources: turn_state.resources.clone(),
            }),
        )
        .await?;
        let before = match before {
            Some(AgentHarnessHookResult::BeforeAgentStart(result)) => result,
            _ => BeforeAgentStartResult::default(),
        };
        if let Some(mut messages) = before.messages {
            prompt_messages.append(&mut messages);
        }

        let controller = AbortController::new();
        let signal = controller.signal();
        lock_state(&self.state).abort_controller = Some(controller);
        let context = AgentContext {
            system_prompt: before
                .system_prompt
                .unwrap_or(turn_state.system_prompt.clone()),
            messages: turn_state.messages.clone(),
            tools: Some(turn_state.active_tools.clone()),
        };
        let config = self.create_loop_config(&turn_state);
        let emit = self.create_event_sink();
        let stream_fn = self.create_stream_fn(&turn_state);
        let result = run_agent_loop(
            prompt_messages,
            context,
            config,
            emit,
            Some(signal.clone()),
            Some(stream_fn),
        )
        .await;
        let assistant = result.iter().rev().find_map(|message| match message {
            AgentMessage::Llm(Message::Assistant(message)) => Some(message.clone()),
            _ => None,
        });
        assistant.ok_or_else(|| {
            AgentHarnessError::new(
                AgentHarnessErrorCode::InvalidState,
                "AgentHarness prompt completed without an assistant message",
                None,
            )
        })
    }

    fn create_loop_config(
        &self,
        turn_state: &AgentHarnessTurnState<Skill, PromptTemplate>,
    ) -> AgentLoopConfig {
        let state = Arc::clone(&self.state);
        let prepare_session = Arc::clone(&self.session);
        let prepare_env = Arc::clone(&self.env);
        let base_model = turn_state.model.clone();
        let thinking_level = turn_state.thinking_level;
        let stream_options =
            stream_options_to_simple(turn_state.stream_options.clone(), thinking_level);
        AgentLoopConfig {
            stream_options,
            model: base_model,
            convert_to_llm: Arc::new(|messages| Box::pin(async move { convert_to_llm(&messages) })),
            transform_context: Some(Arc::new({
                let state = Arc::clone(&state);
                move |messages, _signal| {
                    let state = Arc::clone(&state);
                    Box::pin(async move {
                        match emit_hook(
                            &state,
                            "context",
                            AgentHarnessOwnEvent::Context(ContextEvent {
                                messages: messages.clone(),
                            }),
                        )
                        .await
                        {
                            Ok(Some(AgentHarnessHookResult::Context(result))) => result.messages,
                            _ => messages,
                        }
                    })
                }
            })),
            get_api_key: None,
            should_stop_after_turn: None,
            prepare_next_turn: Some(Arc::new(move |_| {
                let state = Arc::clone(&state);
                let session = Arc::clone(&prepare_session);
                let env = Arc::clone(&prepare_env);
                Box::pin(async move {
                    let context = session.build_context().await;
                    let (model, thinking_level, active_tools, system_prompt) = {
                        let state = lock_state(&state);
                        let active_tools = active_tools(&state);
                        let system_prompt = match &state.system_prompt {
                            Some(SystemPrompt::Text(text)) => text.clone(),
                            _ => "You are a helpful assistant.".to_string(),
                        };
                        (
                            state.model.clone(),
                            state.thinking_level,
                            active_tools,
                            system_prompt,
                        )
                    };
                    let _ = env;
                    Some(crate::types::AgentLoopTurnUpdate {
                        context: Some(AgentContext {
                            system_prompt,
                            messages: context.messages,
                            tools: Some(active_tools),
                        }),
                        model: Some(model),
                        thinking_level: Some(thinking_level),
                    })
                })
            })),
            get_steering_messages: Some(queue_drain_fn(Arc::clone(&self.state), QueueKind::Steer)),
            get_follow_up_messages: Some(queue_drain_fn(
                Arc::clone(&self.state),
                QueueKind::FollowUp,
            )),
            tool_execution: ToolExecutionMode::Parallel,
            before_tool_call: Some(Arc::new({
                let state = Arc::clone(&self.state);
                move |context, _signal| {
                    let state = Arc::clone(&state);
                    Box::pin(async move {
                        let input = value_object_to_hash(context.args.clone());
                        match emit_hook(
                            &state,
                            "tool_call",
                            AgentHarnessOwnEvent::ToolCall(ToolCallEvent {
                                tool_call_id: context.tool_call.id,
                                tool_name: context.tool_call.name,
                                input,
                            }),
                        )
                        .await
                        {
                            Ok(Some(AgentHarnessHookResult::ToolCall(result))) => {
                                Some(crate::types::BeforeToolCallResult {
                                    block: result.block,
                                    reason: result.reason,
                                })
                            }
                            _ => None,
                        }
                    })
                }
            })),
            after_tool_call: Some(Arc::new({
                let state = Arc::clone(&self.state);
                move |context, _signal| {
                    let state = Arc::clone(&state);
                    Box::pin(async move {
                        let input = value_object_to_hash(context.args.clone());
                        match emit_hook(
                            &state,
                            "tool_result",
                            AgentHarnessOwnEvent::ToolResult(ToolResultEvent {
                                tool_call_id: context.tool_call.id,
                                tool_name: context.tool_call.name,
                                input,
                                content: context.result.content,
                                details: context.result.details,
                                is_error: context.is_error,
                            }),
                        )
                        .await
                        {
                            Ok(Some(AgentHarnessHookResult::ToolResult(result))) => {
                                Some(crate::types::AfterToolCallResult {
                                    content: result.content,
                                    details: result.details,
                                    is_error: result.is_error,
                                    terminate: result.terminate,
                                })
                            }
                            _ => None,
                        }
                    })
                }
            })),
        }
    }

    fn create_stream_fn(
        &self,
        turn_state: &AgentHarnessTurnState<Skill, PromptTemplate>,
    ) -> crate::types::StreamFn {
        let models = Arc::clone(&self.models);
        let state = Arc::clone(&self.state);
        let session_id = turn_state.session_id.clone();
        let harness_options = turn_state.stream_options.clone();
        Arc::new(move |model, context, loop_options| {
            let mut options: SimpleStreamOptions = harness_options.clone().into();
            if let Some(loop_options) = loop_options {
                options.reasoning = loop_options.reasoning;
                options.stream.signal = loop_options.stream.signal.clone();
            }
            let patched = block_on(emit_before_provider_request(
                &state,
                model.clone(),
                &session_id,
                harness_options.clone(),
            ))
            .unwrap_or(harness_options.clone());
            apply_stream_options(&mut options, patched);
            options.stream.session_id = Some(session_id.clone());
            options.stream.on_payload = Some(Arc::new({
                let state = Arc::clone(&state);
                move |payload, model| {
                    let state = Arc::clone(&state);
                    Box::pin(async move {
                        emit_before_provider_payload(&state, model, payload)
                            .await
                            .map_err(ProviderHookError::new)
                    })
                }
            }));
            options.stream.on_response = Some(Arc::new({
                let state = Arc::clone(&state);
                move |response: ProviderResponse, _model| {
                    let state = Arc::clone(&state);
                    Box::pin(async move {
                        emit_own(
                            &state,
                            AgentHarnessOwnEvent::AfterProviderResponse(
                                AfterProviderResponseEvent {
                                    status: response.status,
                                    headers: response.headers,
                                },
                            ),
                        )
                        .await
                        .map_err(ProviderHookError::new)
                    })
                }
            }));
            models.stream_simple(model, context, Some(&options))
        })
    }

    fn create_event_sink(&self) -> AgentEventSink {
        let state = Arc::clone(&self.state);
        let session = Arc::clone(&self.session);
        Arc::new(move |event| {
            let state = Arc::clone(&state);
            let session = Arc::clone(&session);
            Box::pin(async move {
                if let Err(error) = handle_agent_event(&state, &session, event).await {
                    let _ = error;
                }
            })
        })
    }

    async fn queue_running_message(
        &self,
        text: String,
        options: Option<AgentHarnessPromptOptions>,
        queue: QueueKind,
    ) -> Result<(), AgentHarnessError> {
        {
            let mut state = lock_state(&self.state);
            if state.phase == AgentHarnessPhase::Idle {
                return Err(AgentHarnessError::new(
                    AgentHarnessErrorCode::InvalidState,
                    match queue {
                        QueueKind::Steer => "Cannot steer while idle",
                        QueueKind::FollowUp => "Cannot follow up while idle",
                    },
                    None,
                ));
            }
            let message = create_user_message(text, options.and_then(|value| value.images));
            match queue {
                QueueKind::Steer => state.steer_queue.push(message),
                QueueKind::FollowUp => state.follow_up_queue.push(message),
            }
        }
        emit_queue_update(&self.state).await
    }

    async fn persist_active_tools(
        &self,
        active_tool_names: Vec<String>,
    ) -> Result<(), AgentHarnessError> {
        if self.phase() == AgentHarnessPhase::Idle {
            self.session
                .append_active_tools_change(active_tool_names)
                .await
                .map_err(|error| normalize_session_error(error.to_string()))?;
        } else {
            lock_state(&self.state)
                .pending_writes
                .push(PendingSessionWrite::ActiveToolsChange { active_tool_names });
        }
        Ok(())
    }

    async fn flush_pending_session_writes(&self) -> Result<(), AgentHarnessError> {
        loop {
            let write = {
                let mut state = lock_state(&self.state);
                if state.pending_writes.is_empty() {
                    None
                } else {
                    Some(state.pending_writes.remove(0))
                }
            };
            let Some(write) = write else { break };
            flush_one_pending_write(self.session.as_ref(), write).await?;
        }
        Ok(())
    }
}

#[derive(Clone)]
struct AgentHarnessTurnState<TSkill, TPromptTemplate> {
    messages: Vec<AgentMessage>,
    resources: AgentHarnessResources<TSkill, TPromptTemplate>,
    stream_options: AgentHarnessStreamOptions,
    session_id: String,
    system_prompt: String,
    model: Model,
    thinking_level: ThinkingLevel,
    active_tools: Vec<AgentTool>,
}

/// Options for navigating the session tree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NavigateTreeOptions {
    /// Generate a summary for the branch being left.
    pub summarize: bool,
    /// Optional custom summarization instructions.
    pub custom_instructions: Option<String>,
    /// Replace default instructions instead of appending to them.
    pub replace_instructions: bool,
    /// Optional label for the branch.
    pub label: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum QueueKind {
    Steer,
    FollowUp,
}

async fn handle_agent_event<TSkill, TPromptTemplate>(
    state: &Arc<Mutex<AgentHarnessState<TSkill, TPromptTemplate>>>,
    session: &Arc<dyn Session>,
    event: AgentEvent,
) -> Result<(), AgentHarnessError> {
    match &event {
        AgentEvent::MessageEnd { message } => {
            session
                .append_message(message.clone())
                .await
                .map_err(|error| normalize_session_error(error.to_string()))?;
            emit_any(state, AgentHarnessEvent::Agent(event)).await?;
        }
        AgentEvent::TurnEnd { .. } => {
            emit_any(state, AgentHarnessEvent::Agent(event)).await?;
            let had_pending_mutations = {
                let state = lock_state(state);
                !state.pending_writes.is_empty()
            };
            emit_own(
                state,
                AgentHarnessOwnEvent::SavePoint(SavePointEvent {
                    had_pending_mutations,
                }),
            )
            .await?;
        }
        AgentEvent::AgentEnd { .. } => {
            {
                let mut state = lock_state(state);
                state.phase = AgentHarnessPhase::Idle;
            }
            emit_any(state, AgentHarnessEvent::Agent(event)).await?;
            let next_turn_count = {
                let state = lock_state(state);
                state.next_turn_queue.len()
            };
            emit_own(
                state,
                AgentHarnessOwnEvent::Settled(SettledEvent { next_turn_count }),
            )
            .await?;
        }
        _ => emit_any(state, AgentHarnessEvent::Agent(event)).await?,
    }
    Ok(())
}

async fn flush_one_pending_write(
    session: &dyn Session,
    write: PendingSessionWrite,
) -> Result<(), AgentHarnessError> {
    match write {
        PendingSessionWrite::Message(message) => session.append_message(message).await.map(|_| ()),
        PendingSessionWrite::ModelChange { provider, model_id } => session
            .append_model_change(provider, model_id)
            .await
            .map(|_| ()),
        PendingSessionWrite::ThinkingLevelChange { thinking_level } => session
            .append_thinking_level_change(thinking_level)
            .await
            .map(|_| ()),
        PendingSessionWrite::ActiveToolsChange { active_tool_names } => session
            .append_active_tools_change(active_tool_names)
            .await
            .map(|_| ()),
    }
    .map_err(|error| normalize_session_error(error.to_string()))
}

fn queue_drain_fn<TSkill, TPromptTemplate>(
    state: Arc<Mutex<AgentHarnessState<TSkill, TPromptTemplate>>>,
    queue: QueueKind,
) -> crate::types::MessageQueueFn
where
    TSkill: Send + 'static,
    TPromptTemplate: Send + 'static,
{
    Arc::new(move || {
        let state = Arc::clone(&state);
        Box::pin(async move {
            let messages = {
                let mut state = lock_state(&state);
                match queue {
                    QueueKind::Steer => {
                        let mode = state.steering_queue_mode;
                        drain_queue(&mut state.steer_queue, mode)
                    }
                    QueueKind::FollowUp => {
                        let mode = state.follow_up_queue_mode;
                        drain_queue(&mut state.follow_up_queue, mode)
                    }
                }
            };
            if !messages.is_empty() {
                let _ = emit_queue_update(&state).await;
            }
            messages
        })
    })
}

fn drain_queue(queue: &mut Vec<AgentMessage>, mode: QueueMode) -> Vec<AgentMessage> {
    match mode {
        QueueMode::All => std::mem::take(queue),
        QueueMode::OneAtATime => {
            if queue.is_empty() {
                Vec::new()
            } else {
                vec![queue.remove(0)]
            }
        }
    }
}

async fn emit_before_provider_request<TSkill, TPromptTemplate>(
    state: &Arc<Mutex<AgentHarnessState<TSkill, TPromptTemplate>>>,
    model: Model,
    session_id: &str,
    stream_options: AgentHarnessStreamOptions,
) -> Result<AgentHarnessStreamOptions, AgentHarnessError> {
    let mut current = stream_options;
    let handlers = hooks_for(state, "before_provider_request");
    for handler in handlers {
        let event = AgentHarnessOwnEvent::BeforeProviderRequest(BeforeProviderRequestEvent {
            model: model.clone(),
            session_id: session_id.to_string(),
            stream_options: current.clone(),
        });
        if let Some(AgentHarnessHookResult::BeforeProviderRequest(result)) = handler(event).await {
            if let Some(patch) = result.stream_options {
                current = apply_stream_options_patch(current, patch);
            }
        }
    }
    Ok(current)
}

async fn emit_before_provider_payload<TSkill, TPromptTemplate>(
    state: &Arc<Mutex<AgentHarnessState<TSkill, TPromptTemplate>>>,
    model: Model,
    payload: Value,
) -> Result<Option<Value>, AgentHarnessError> {
    let mut current = payload;
    let mut changed = false;
    for handler in hooks_for(state, "before_provider_payload") {
        let event = AgentHarnessOwnEvent::BeforeProviderPayload(BeforeProviderPayloadEvent {
            model: model.clone(),
            payload: current.clone(),
        });
        if let Some(AgentHarnessHookResult::BeforeProviderPayload(result)) = handler(event).await {
            current = result.payload;
            changed = true;
        }
    }
    Ok(changed.then_some(current))
}

async fn emit_hook<TSkill, TPromptTemplate>(
    state: &Arc<Mutex<AgentHarnessState<TSkill, TPromptTemplate>>>,
    event_type: &str,
    event: AgentHarnessOwnEvent,
) -> Result<Option<AgentHarnessHookResult>, AgentHarnessError> {
    let mut last = None;
    for handler in hooks_for(state, event_type) {
        if let Some(result) = handler(event.clone()).await {
            last = Some(result);
        }
    }
    Ok(last)
}

async fn emit_queue_update<TSkill, TPromptTemplate>(
    state: &Arc<Mutex<AgentHarnessState<TSkill, TPromptTemplate>>>,
) -> Result<(), AgentHarnessError> {
    let event = {
        let state = lock_state(state);
        AgentHarnessOwnEvent::QueueUpdate(QueueUpdateEvent {
            steer: state.steer_queue.clone(),
            follow_up: state.follow_up_queue.clone(),
            next_turn: state.next_turn_queue.clone(),
        })
    };
    emit_own(state, event).await
}

async fn emit_own<TSkill, TPromptTemplate>(
    state: &Arc<Mutex<AgentHarnessState<TSkill, TPromptTemplate>>>,
    event: AgentHarnessOwnEvent,
) -> Result<(), AgentHarnessError> {
    emit_any(state, AgentHarnessEvent::Harness(event)).await
}

async fn emit_any<TSkill, TPromptTemplate>(
    state: &Arc<Mutex<AgentHarnessState<TSkill, TPromptTemplate>>>,
    event: AgentHarnessEvent,
) -> Result<(), AgentHarnessError> {
    let subscribers = lock_state(state).subscribers.clone();
    for subscriber in subscribers {
        subscriber(event.clone()).await?;
    }
    Ok(())
}

fn hooks_for<TSkill, TPromptTemplate>(
    state: &Arc<Mutex<AgentHarnessState<TSkill, TPromptTemplate>>>,
    event_type: &str,
) -> Vec<AgentHarnessHook> {
    lock_state(state)
        .hooks
        .get(event_type)
        .cloned()
        .unwrap_or_default()
}

fn stream_options_to_simple(
    value: AgentHarnessStreamOptions,
    thinking_level: ThinkingLevel,
) -> SimpleStreamOptions {
    let mut options: SimpleStreamOptions = value.into();
    options.reasoning = (thinking_level != ThinkingLevel::Off).then_some(match thinking_level {
        ThinkingLevel::Off => zedflow_ai::ThinkingLevel::Minimal,
        ThinkingLevel::Minimal => zedflow_ai::ThinkingLevel::Minimal,
        ThinkingLevel::Low => zedflow_ai::ThinkingLevel::Low,
        ThinkingLevel::Medium => zedflow_ai::ThinkingLevel::Medium,
        ThinkingLevel::High => zedflow_ai::ThinkingLevel::High,
        ThinkingLevel::XHigh => zedflow_ai::ThinkingLevel::XHigh,
    });
    options
}

fn apply_stream_options(options: &mut SimpleStreamOptions, value: AgentHarnessStreamOptions) {
    options.stream.transport = value.transport;
    options.stream.timeout_ms = value.timeout_ms;
    options.stream.max_retries = value.max_retries;
    options.stream.max_retry_delay_ms = value.max_retry_delay_ms;
    options.stream.headers = value.headers.map(|headers| {
        headers
            .into_iter()
            .map(|(key, value)| (key, Some(value)))
            .collect()
    });
    options.stream.metadata = value.metadata;
    options.stream.cache_retention = value.cache_retention;
}

fn apply_stream_options_patch(
    mut base: AgentHarnessStreamOptions,
    patch: AgentHarnessStreamOptionsPatch,
) -> AgentHarnessStreamOptions {
    if patch.transport.is_some() {
        base.transport = patch.transport;
    }
    if patch.timeout_ms.is_some() {
        base.timeout_ms = patch.timeout_ms;
    }
    if patch.max_retries.is_some() {
        base.max_retries = patch.max_retries;
    }
    if patch.max_retry_delay_ms.is_some() {
        base.max_retry_delay_ms = patch.max_retry_delay_ms;
    }
    if patch.cache_retention.is_some() {
        base.cache_retention = patch.cache_retention;
    }
    if let Some(headers) = patch.headers {
        let mut next = base.headers.unwrap_or_default();
        for (key, value) in headers {
            if let Some(value) = value {
                next.insert(key, value);
            } else {
                next.remove(&key);
            }
        }
        base.headers = (!next.is_empty()).then_some(next);
    }
    if let Some(metadata) = patch.metadata {
        let mut next = base.metadata.unwrap_or_default();
        for (key, value) in metadata {
            if let Some(value) = value {
                next.insert(key, value);
            } else {
                next.remove(&key);
            }
        }
        base.metadata = (!next.is_empty()).then_some(next);
    }
    base
}

fn create_user_message(text: String, images: Option<Vec<ImageContent>>) -> AgentMessage {
    let mut blocks = vec![UserContentBlock::Text(TextContent {
        content_type: TextContentType::Text,
        text,
        text_signature: None,
    })];
    if let Some(images) = images {
        blocks.extend(images.into_iter().map(UserContentBlock::Image));
    }
    AgentMessage::Llm(Message::User(UserMessage {
        role: UserMessageRole::User,
        content: UserMessageContent::Blocks(blocks),
        timestamp: timestamp_millis(),
    }))
}

fn navigation_target(
    entry: &crate::harness::types::SessionTreeEntry,
    target_id: &str,
) -> (Option<String>, Option<String>) {
    match entry {
        crate::harness::types::SessionTreeEntry::Message(entry)
            if matches!(&entry.message, AgentMessage::Llm(Message::User(_))) =>
        {
            (entry.base.parent_id.clone(), message_text(&entry.message))
        }
        crate::harness::types::SessionTreeEntry::CustomMessage(entry) => (
            entry.base.parent_id.clone(),
            custom_message_text(&entry.content),
        ),
        _ => (Some(target_id.to_string()), None),
    }
}

fn message_text(message: &AgentMessage) -> Option<String> {
    match message {
        AgentMessage::Llm(Message::User(message)) => match &message.content {
            UserMessageContent::Text(text) => Some(text.clone()),
            UserMessageContent::Blocks(blocks) => Some(
                blocks
                    .iter()
                    .filter_map(|block| match block {
                        UserContentBlock::Text(block) => Some(block.text.as_str()),
                        UserContentBlock::Image(_) => None,
                    })
                    .collect::<String>(),
            ),
        },
        _ => None,
    }
}

fn custom_message_text(content: &CustomMessageContent) -> Option<String> {
    match content {
        CustomMessageContent::Text(text) => Some(text.clone()),
        CustomMessageContent::Blocks(blocks) => Some(
            blocks
                .iter()
                .filter_map(|block| match block {
                    AgentToolResultContent::Text(block) => Some(block.text.as_str()),
                    AgentToolResultContent::Image(_) => None,
                })
                .collect::<String>(),
        ),
    }
}

fn active_tools<TSkill, TPromptTemplate>(
    state: &AgentHarnessState<TSkill, TPromptTemplate>,
) -> Vec<AgentTool> {
    state
        .active_tool_names
        .iter()
        .filter_map(|name| {
            state
                .tools
                .iter()
                .find(|tool| tool_name(tool) == *name)
                .cloned()
        })
        .collect()
}

fn validate_tool_names(
    tool_names: &[String],
    tools: &[AgentTool],
) -> Result<(), AgentHarnessError> {
    validate_unique_names(tool_names, "Duplicate active tool name(s)")?;
    let tool_set = tools.iter().map(tool_name).collect::<HashSet<_>>();
    let missing = tool_names
        .iter()
        .filter(|name| !tool_set.contains(*name))
        .cloned()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(invalid_argument(format!(
            "Unknown tool(s): {}",
            missing.join(", ")
        )))
    }
}

fn validate_unique_names(names: &[String], message: &str) -> Result<(), AgentHarnessError> {
    let mut seen = HashSet::new();
    let mut duplicates = HashSet::new();
    for name in names {
        if !seen.insert(name.clone()) {
            duplicates.insert(name.clone());
        }
    }
    if duplicates.is_empty() {
        Ok(())
    } else {
        let mut duplicates = duplicates.into_iter().collect::<Vec<_>>();
        duplicates.sort();
        Err(invalid_argument(format!(
            "{message}: {}",
            duplicates.join(", ")
        )))
    }
}

fn tool_name(tool: &AgentTool) -> String {
    tool.tool.name.clone()
}

fn value_object_to_hash(value: Value) -> HashMap<String, Value> {
    match value {
        Value::Object(map) => map.into_iter().collect(),
        other => HashMap::from([("value".to_string(), other)]),
    }
}

fn thinking_level_string(level: ThinkingLevel) -> String {
    match level {
        ThinkingLevel::Off => "off",
        ThinkingLevel::Minimal => "minimal",
        ThinkingLevel::Low => "low",
        ThinkingLevel::Medium => "medium",
        ThinkingLevel::High => "high",
        ThinkingLevel::XHigh => "xhigh",
    }
    .to_string()
}

fn timestamp_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

fn lock_state<TSkill, TPromptTemplate>(
    state: &Arc<Mutex<AgentHarnessState<TSkill, TPromptTemplate>>>,
) -> std::sync::MutexGuard<'_, AgentHarnessState<TSkill, TPromptTemplate>> {
    state.lock().expect("agent harness state lock poisoned")
}

fn invalid_argument(message: impl Into<String>) -> AgentHarnessError {
    AgentHarnessError::new(AgentHarnessErrorCode::InvalidArgument, message, None)
}

fn normalize_session_error(message: impl Into<String>) -> AgentHarnessError {
    AgentHarnessError::new(AgentHarnessErrorCode::Session, message, None)
}

fn normalize_compaction_error(message: impl Into<String>) -> AgentHarnessError {
    AgentHarnessError::new(AgentHarnessErrorCode::Compaction, message, None)
}

fn normalize_branch_error(message: impl Into<String>) -> AgentHarnessError {
    AgentHarnessError::new(AgentHarnessErrorCode::BranchSummary, message, None)
}
