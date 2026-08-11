//! Interactive-mode contracts that do not require the TUI package.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    io,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::{Arc, Mutex},
    time::Duration,
};

use serde_json::Value;
use zedflow_agent::{
    harness::types::{AgentHarnessEvent, AgentHarnessOwnEvent},
    types::{AgentEvent, AgentMessage},
};
use zedflow_tui::{Component, ProcessTerminal, Terminal, Text, Tui};

use crate::{
    agent_session_runtime::AgentSessionRuntime,
    extensions::{
        ExtensionError, ExtensionEvent, ExtensionEventKind, ExtensionMode, ExtensionRunner,
        InputEvent, ProviderConfig, SessionActionResult,
    },
    modes_interactive_components_index::{
        assistant_message::{AssistantContent, StopReason, StreamingAssistantMessage},
        compaction_summary_message::CompactionSummaryMessageComponent,
        footer::FooterSnapshot,
        status_indicator::{IdleStatus, WorkingStatusIndicator},
        tool_execution::ToolExecutionComponent,
        user_message::UserMessageComponent,
    },
    slash_commands::{BuiltinSlashCommandId, parse_builtin_slash_command},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuiltinCommandOutcome {
    Success(String),
    Cancelled(String),
}

pub trait BuiltinCommandService: Send {
    fn execute(
        &mut self,
        command: BuiltinSlashCommandId,
        arguments: &str,
    ) -> Result<BuiltinCommandOutcome, String>;
}

/// Local, deterministic command boundary used by the live TUI. Commands that
/// need a choice report the selector they opened; filesystem/provider effects
/// remain behind the corresponding accepted service boundaries.
#[derive(Default)]
pub struct LiveBuiltinCommandService;

impl BuiltinCommandService for LiveBuiltinCommandService {
    fn execute(
        &mut self,
        command: BuiltinSlashCommandId,
        arguments: &str,
    ) -> Result<BuiltinCommandOutcome, String> {
        use BuiltinSlashCommandId as Command;
        let status = match command {
            Command::Settings => "Settings selector opened".into(),
            Command::Model => format!("Model selector opened{}", suffix(arguments)),
            Command::ScopedModels => "Scoped models selector opened".into(),
            Command::Export => format!("Export requested{}", suffix(arguments)),
            Command::Import if arguments.is_empty() => {
                return Err("Usage: /import <path.jsonl>".into());
            }
            Command::Import => format!("Import confirmation opened for {arguments}"),
            Command::Share => "Share requested".into(),
            Command::Copy => "Copy requested".into(),
            Command::Name if arguments.is_empty() => "Session name requested".into(),
            Command::Name => format!("Session name set: {arguments}"),
            Command::Session => "Session info opened".into(),
            Command::Changelog => "Changelog opened".into(),
            Command::Hotkeys => "Hotkeys opened".into(),
            Command::Fork => "Fork selector opened".into(),
            Command::Clone => "Clone requested".into(),
            Command::Tree => "Session tree opened".into(),
            Command::Trust => "Trust selector opened".into(),
            Command::Login => "Login selector opened".into(),
            Command::Logout => "Logout selector opened".into(),
            Command::New => "New session requested".into(),
            Command::Resume => "Session selector opened".into(),
            Command::Reload => "Resources reloaded".into(),
            Command::Compact | Command::Quit => unreachable!("handled by InteractiveMode"),
        };
        Ok(BuiltinCommandOutcome::Success(status))
    }
}

fn suffix(arguments: &str) -> String {
    (!arguments.is_empty())
        .then(|| format!(": {arguments}"))
        .unwrap_or_default()
}

/// Parse the first argument of an interactive path command (`/import` or
/// `/export`). The command must be a complete token; quoted arguments have
/// their matching outer quotes removed.
#[must_use]
pub fn get_path_command_argument(text: &str, command: &str) -> Option<String> {
    let prefix = format!("{command} ");
    if text == command || !text.starts_with(&prefix) {
        return None;
    }

    let argument = text[prefix.len()..].trim_start();
    if argument.is_empty() {
        return None;
    }

    match argument.as_bytes()[0] {
        b'\'' | b'"' => {
            let quote = argument.as_bytes()[0] as char;
            argument[1..]
                .find(quote)
                .map(|end| argument[1..end + 1].to_owned())
        }
        _ => argument.split_whitespace().next().map(str::to_owned),
    }
}

/// Quote a path using the same conservative shell-safe form as Pi.
#[must_use]
pub fn quote_if_needed(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "_-./~:@".contains(character))
    {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

/// Whether selecting a provider should use an API-key login rather than an
/// OAuth flow. Built-in display-name entries are API-key providers in Pi;
/// other built-ins and registered OAuth providers are not.
#[must_use]
pub fn is_api_key_login_provider(
    provider_id: &str,
    oauth_provider_ids: &HashSet<String>,
    built_in_provider_ids: &HashSet<String>,
    built_in_display_name_ids: &HashSet<String>,
) -> bool {
    if built_in_display_name_ids.contains(provider_id) {
        return true;
    }
    if built_in_provider_ids.contains(provider_id) {
        return false;
    }
    !oauth_provider_ids.contains(provider_id)
}

/// Pi's Anthropic subscription credentials use this key prefix.
#[must_use]
pub fn is_anthropic_subscription_auth_key(api_key: Option<&str>) -> bool {
    api_key.is_some_and(|key| key.starts_with("sk-ant-oat"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractiveState {
    Created,
    Running,
    Stopped,
}

pub struct InteractiveMode {
    state: InteractiveState,
    tui: Tui,
    pending_user_inputs: VecDeque<String>,
    last_status: Option<String>,
    extension_runner: Option<ExtensionRunner>,
    runtime: Option<AgentSessionRuntime>,
    session_events: Arc<Mutex<VecDeque<AgentHarnessEvent>>>,
    rendered_events: Arc<Mutex<Vec<String>>>,
    transcript: Arc<Mutex<TranscriptState>>,
    footer: Arc<Mutex<FooterSnapshot>>,
    compacting: bool,
    compaction_queue: VecDeque<String>,
    submitted_terminal_inputs: Arc<Mutex<VecDeque<String>>>,
    builtin_commands: Box<dyn BuiltinCommandService>,
    exit_requested: bool,
}

impl Default for InteractiveState {
    fn default() -> Self {
        Self::Created
    }
}

impl Default for InteractiveMode {
    fn default() -> Self {
        Self::with_terminal(ProcessTerminal::new())
    }
}

impl InteractiveMode {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_terminal(terminal: impl Terminal + 'static) -> Self {
        Self {
            state: InteractiveState::Created,
            tui: Tui::with_terminal(terminal),
            pending_user_inputs: VecDeque::new(),
            last_status: None,
            extension_runner: None,
            runtime: None,
            session_events: Arc::new(Mutex::new(VecDeque::new())),
            rendered_events: Arc::new(Mutex::new(Vec::new())),
            transcript: Arc::new(Mutex::new(TranscriptState::default())),
            footer: Arc::new(Mutex::new(FooterSnapshot {
                cwd: current_dir(),
                ..FooterSnapshot::default()
            })),
            compacting: false,
            compaction_queue: VecDeque::new(),
            submitted_terminal_inputs: Arc::new(Mutex::new(VecDeque::new())),
            builtin_commands: Box::<LiveBuiltinCommandService>::default(),
            exit_requested: false,
        }
    }

    #[must_use]
    pub fn with_extension_runner(
        terminal: impl Terminal + 'static,
        extension_runner: ExtensionRunner,
    ) -> Self {
        let mut mode = Self::with_terminal(terminal);
        mode.extension_runner = Some(extension_runner);
        mode
    }

    #[must_use]
    pub fn with_runtime(terminal: impl Terminal + 'static, runtime: AgentSessionRuntime) -> Self {
        let mut mode = Self::with_terminal(terminal);
        mode.set_runtime(runtime);
        mode
    }

    #[must_use]
    pub fn with_runtime_and_extension_runner(
        terminal: impl Terminal + 'static,
        runtime: AgentSessionRuntime,
        extension_runner: ExtensionRunner,
    ) -> Self {
        let mut mode = Self::with_extension_runner(terminal, extension_runner);
        mode.set_runtime(runtime);
        mode
    }

    pub fn set_builtin_command_service(&mut self, service: impl BuiltinCommandService + 'static) {
        self.builtin_commands = Box::new(service);
    }

    fn set_runtime(&mut self, runtime: AgentSessionRuntime) {
        let events = Arc::clone(&self.session_events);
        // The harness retains the listener; dropping its returned teardown closure
        // deliberately keeps this mode subscribed for the runtime's lifetime.
        let _ = runtime.session().subscribe(Arc::new(move |event| {
            events.lock().unwrap().push_back(event);
            Box::pin(async { Ok(()) })
        }));
        self.runtime = Some(runtime);
    }

    fn drain_session_events(&mut self) -> io::Result<()> {
        let events = std::mem::take(&mut *self.session_events.lock().unwrap());
        for event in events {
            self.apply_session_event(event);
        }
        self.tui.request_render(false)
    }

    /// Apply one session event to the transcript in arrival order.
    pub fn apply_session_event(&mut self, event: AgentHarnessEvent) {
        let mut transcript = self.transcript.lock().unwrap();
        let text = match event {
            AgentHarnessEvent::Agent(AgentEvent::MessageStart { message }) => {
                let role = message_role(&message);
                if role == "assistant" {
                    transcript.start_assistant(&message);
                } else if role == "user" {
                    transcript
                        .entries
                        .push(TranscriptEntry::User(message_content(&message)));
                } else {
                    transcript
                        .entries
                        .push(TranscriptEntry::Status(message_content(&message)));
                }
                format!("{role}: {}", message_content(&message))
            }
            AgentHarnessEvent::Agent(AgentEvent::MessageUpdate { message, .. }) => {
                transcript.update_assistant(&message);
                "assistant: streaming".into()
            }
            AgentHarnessEvent::Agent(AgentEvent::MessageEnd { message }) => {
                transcript.finish_assistant(&message);
                "assistant: complete".into()
            }
            AgentHarnessEvent::Agent(AgentEvent::ToolExecutionStart {
                tool_call_id,
                tool_name,
                args,
            }) => {
                transcript.start_tool(tool_call_id, tool_name.clone(), args);
                format!("tool: {tool_name}")
            }
            AgentHarnessEvent::Agent(AgentEvent::ToolExecutionUpdate {
                tool_call_id,
                partial_result,
                ..
            }) => {
                transcript.update_tool(&tool_call_id, partial_result, false, true);
                "tool: streaming".into()
            }
            AgentHarnessEvent::Agent(AgentEvent::ToolExecutionEnd {
                tool_call_id,
                tool_name,
                result,
                is_error,
            }) => {
                transcript.update_tool(&tool_call_id, result, is_error, false);
                format!(
                    "tool: {tool_name} {}",
                    if is_error { "failed" } else { "done" }
                )
            }
            AgentHarnessEvent::Agent(AgentEvent::AgentStart) => {
                transcript.working = true;
                "assistant: working".into()
            }
            AgentHarnessEvent::Agent(AgentEvent::AgentEnd { .. }) => {
                transcript.working = false;
                transcript.streaming_assistant = None;
                transcript.pending_tools.clear();
                "assistant: idle".into()
            }
            AgentHarnessEvent::Harness(AgentHarnessOwnEvent::QueueUpdate(queue)) => {
                let count = queue.steer.len() + queue.follow_up.len() + queue.next_turn.len();
                transcript.queued = count;
                format!("queue: {count}")
            }
            AgentHarnessEvent::Harness(AgentHarnessOwnEvent::SessionBeforeCompact(_)) => {
                self.compacting = true;
                transcript.compacting = true;
                "compaction: working".into()
            }
            AgentHarnessEvent::Harness(AgentHarnessOwnEvent::SessionCompact(event)) => {
                self.compacting = false;
                transcript.compacting = false;
                transcript.entries.clear();
                transcript.entries.push(TranscriptEntry::Compaction {
                    summary: event.compaction_entry.summary,
                    tokens_before: event.compaction_entry.tokens_before,
                });
                transcript.streaming_assistant = None;
                transcript.pending_tools.clear();
                "compaction: complete".into()
            }
            // Harness tool events are extension lifecycle notifications. Low-level
            // execution events own transcript components, avoiding duplicate rows.
            AgentHarnessEvent::Harness(AgentHarnessOwnEvent::ToolCall(tool)) => {
                format!("tool: {}", tool.tool_name)
            }
            AgentHarnessEvent::Harness(AgentHarnessOwnEvent::ToolResult(tool)) => {
                format!(
                    "tool: {} {}",
                    tool.tool_name,
                    if tool.is_error { "failed" } else { "done" }
                )
            }
            _ => return,
        };
        drop(transcript);
        self.rendered_events.lock().unwrap().push(text);
    }

    pub fn tui_mut(&mut self) -> &mut Tui {
        &mut self.tui
    }

    #[must_use]
    pub fn rendered_events(&self) -> Vec<String> {
        self.rendered_events.lock().unwrap().clone()
    }

    /// Render the current stateful transcript for deterministic end-user tests.
    #[must_use]
    pub fn rendered_transcript(&self, width: usize) -> Vec<String> {
        TranscriptView::new(Arc::clone(&self.transcript)).render(width)
    }

    pub fn run(&mut self) -> io::Result<()> {
        if self.tui.root.is_empty() {
            // Pi order: transcript, pending/status row, editor, footer. Overlays are
            // managed by Tui's native overlay stack above this root tree.
            self.tui
                .root
                .add_child(TranscriptView::new(Arc::clone(&self.transcript)));
            self.tui
                .root
                .add_child(ActivityView::new(Arc::clone(&self.transcript)));
            self.tui.root.add_child(EditorFooterView::new(
                Arc::clone(&self.submitted_terminal_inputs),
                Arc::clone(&self.footer),
            ));
        }
        self.tui.start()?;
        self.state = InteractiveState::Running;
        if let Some(runner) = &mut self.extension_runner {
            runner.set_context(ExtensionMode::Tui, current_dir(), true);
            runner.emit(ExtensionEvent {
                kind: ExtensionEventKind::SessionStart,
                data: serde_json::json!({}),
            });
        }
        Ok(())
    }

    /// Pump terminal input and resize events on the thread that owns this mode.
    pub fn pump_events(&mut self, timeout: Duration) -> io::Result<usize> {
        let count = self.tui.pump_events(timeout)?;
        let submitted = std::mem::take(&mut *self.submitted_terminal_inputs.lock().unwrap());
        for input in submitted {
            self.queue_user_input(input);
        }
        self.drain_session_events()?;
        Ok(count)
    }

    pub fn stop(&mut self) -> io::Result<()> {
        if let Some(runner) = &mut self.extension_runner {
            runner.shutdown("interactive mode stopped");
        }
        let result = self.tui.stop();
        self.state = InteractiveState::Stopped;
        result
    }

    /// Restore the terminal while an external editor owns it, then resume the TUI.
    pub fn suspend_for_external_editor<T>(
        &mut self,
        edit: impl FnOnce() -> io::Result<T>,
    ) -> io::Result<T> {
        self.stop()?;
        let edit = catch_unwind(AssertUnwindSafe(edit));
        let resume = self.tui.start();
        if resume.is_ok() {
            self.state = InteractiveState::Running;
        }
        let resume = resume.and_then(|()| self.tui.request_render(true));

        match edit {
            Ok(edit) => {
                resume?;
                edit
            }
            Err(payload) => {
                let _ = resume;
                resume_unwind(payload)
            }
        }
    }

    #[must_use]
    pub fn state(&self) -> InteractiveState {
        self.state
    }

    #[must_use]
    pub fn exit_requested(&self) -> bool {
        self.exit_requested
    }

    /// Queue startup input until the interactive consumer is ready.
    pub fn queue_user_input(&mut self, text: impl Into<String>) {
        let text = text.into().trim().to_owned();
        if text.is_empty() {
            return;
        }
        let input = self
            .extension_runner
            .as_mut()
            .map(|runner| runner.emit_input(InputEvent::Text(text.clone())));
        if input.as_ref().is_some_and(|result| result.consumed) {
            return;
        }
        let text = input.and_then(|result| result.replacement).unwrap_or(text);
        if let Some((command, arguments)) = parse_builtin_slash_command(&text) {
            match command {
                BuiltinSlashCommandId::Compact => self.pending_user_inputs.push_back(text),
                BuiltinSlashCommandId::Quit => self.exit_requested = true,
                _ => match self.builtin_commands.execute(command, arguments) {
                    Ok(BuiltinCommandOutcome::Success(status))
                    | Ok(BuiltinCommandOutcome::Cancelled(status)) => {
                        self.show_status(status);
                    }
                    Err(error) => {
                        self.show_status(error);
                    }
                },
            }
            return;
        }
        if let Some((command, args)) = text.strip_prefix('/').and_then(|text| {
            let mut words = text.split_whitespace();
            words
                .next()
                .map(|command| (command, words.map(str::to_owned).collect::<Vec<_>>()))
        }) && let Some(runner) = &mut self.extension_runner
            && runner
                .runtime
                .commands
                .iter()
                .any(|registered| registered.name == command)
        {
            if let Err(error) = runner.invoke_command(command, &args) {
                self.show_status(error.message);
            }
            return;
        }
        self.pending_user_inputs.push_back(text);
    }

    /// Return queued input in submission order, matching `getUserInput()`.
    pub fn get_user_input(&mut self) -> Option<String> {
        self.pending_user_inputs.pop_front()
    }

    /// Submit one queued input to the active session runtime.
    ///
    /// Returns `true` when an input was consumed. Errors are shown in the TUI
    /// status area so the interactive loop remains usable after a failed turn.
    pub fn process_next_user_input(&mut self) -> io::Result<bool> {
        self.drain_session_events()?;
        let Some(input) = self.get_user_input() else {
            return Ok(false);
        };
        if self.compacting {
            self.compaction_queue.push_back(input);
            self.show_status("Queued message for after compaction");
            return Ok(true);
        }
        let Some(runtime) = self.runtime.clone() else {
            self.pending_user_inputs.push_front(input);
            return Ok(false);
        };
        let compact_instructions = input.strip_prefix("/compact").and_then(|rest| {
            (rest.is_empty() || rest.starts_with(char::is_whitespace))
                .then(|| rest.trim().to_owned())
        });
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| io::Error::other(error.to_string()))?
            .block_on(async move {
                if let Some(instructions) = compact_instructions {
                    runtime
                        .session()
                        .compact((!instructions.is_empty()).then_some(instructions.as_str()))
                        .await
                        .map(|_| ())
                } else {
                    runtime.session().prompt(input, None).await.map(|_| ())
                }
            });
        self.drain_session_events()?;
        if let Err(error) = result {
            self.show_status(error.to_string());
        }
        while !self.compacting {
            let Some(input) = self.compaction_queue.pop_front() else {
                break;
            };
            self.pending_user_inputs.push_back(input);
        }
        Ok(true)
    }

    #[must_use]
    pub fn pending_user_input_count(&self) -> usize {
        self.pending_user_inputs.len()
    }

    /// Coalesce adjacent status updates and return the text currently shown.
    pub fn show_status(&mut self, message: impl Into<String>) -> &str {
        self.last_status = Some(message.into());
        self.last_status.as_deref().unwrap_or_default()
    }

    #[must_use]
    pub fn last_status(&self) -> Option<&str> {
        self.last_status.as_deref()
    }

    /// The single extension dispatch point for interactive lifecycle events.
    pub fn emit_extension_event(&mut self, kind: ExtensionEventKind, data: Value) -> Vec<Value> {
        self.extension_runner
            .as_mut()
            .map_or_else(Vec::new, |runner| {
                runner.emit(ExtensionEvent { kind, data })
            })
    }

    pub fn invoke_extension_tool(
        &mut self,
        name: &str,
        arguments: Value,
    ) -> Result<Value, ExtensionError> {
        self.extension_runner
            .as_mut()
            .ok_or_else(|| ExtensionError {
                message: "no interactive extension runner".into(),
                source: None,
            })?
            .invoke_tool(name, arguments)
    }

    pub fn invoke_extension_command(
        &mut self,
        name: &str,
        args: &[String],
    ) -> Result<SessionActionResult, ExtensionError> {
        self.extension_runner
            .as_mut()
            .ok_or_else(|| ExtensionError {
                message: "no interactive extension runner".into(),
                source: None,
            })?
            .invoke_command(name, args)
    }

    #[must_use]
    pub fn extension_providers(&self) -> &[ProviderConfig] {
        self.extension_runner
            .as_ref()
            .map_or(&[], |runner| &runner.runtime.providers)
    }
}

#[derive(Default)]
struct TranscriptState {
    entries: Vec<TranscriptEntry>,
    streaming_assistant: Option<usize>,
    pending_tools: HashMap<String, usize>,
    queued: usize,
    working: bool,
    compacting: bool,
}

struct AssistantSnapshot {
    content: Vec<AssistantContent>,
    stop_reason: StopReason,
    error_message: Option<String>,
}

struct ToolSnapshot {
    name: String,
    arguments: String,
    result: Option<(String, bool, bool)>,
    started: bool,
    args_complete: bool,
}

enum TranscriptEntry {
    User(String),
    Assistant(AssistantSnapshot),
    Tool(ToolSnapshot),
    Compaction { summary: String, tokens_before: u64 },
    Status(String),
}

impl TranscriptState {
    fn start_assistant(&mut self, message: &AgentMessage) {
        self.entries
            .push(TranscriptEntry::Assistant(assistant_snapshot(message)));
        self.streaming_assistant = Some(self.entries.len() - 1);
        self.sync_tool_calls(message);
    }

    fn update_assistant(&mut self, message: &AgentMessage) {
        if self.streaming_assistant.is_none() {
            self.start_assistant(message);
            return;
        }
        if let Some(TranscriptEntry::Assistant(snapshot)) = self
            .streaming_assistant
            .and_then(|index| self.entries.get_mut(index))
        {
            *snapshot = assistant_snapshot(message);
        }
        self.sync_tool_calls(message);
    }

    fn finish_assistant(&mut self, message: &AgentMessage) {
        self.update_assistant(message);
        for index in self.pending_tools.values().copied() {
            if let Some(TranscriptEntry::Tool(tool)) = self.entries.get_mut(index) {
                tool.args_complete = true;
            }
        }
        self.streaming_assistant = None;
    }

    fn sync_tool_calls(&mut self, message: &AgentMessage) {
        let json = serde_json::to_value(message).unwrap_or_default();
        for call in json
            .get("content")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(id) = call.get("id").and_then(Value::as_str) else {
                continue;
            };
            if call.get("type").and_then(Value::as_str) != Some("toolCall") {
                continue;
            }
            let name = call
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("tool")
                .to_owned();
            let arguments = json_text(call.get("arguments").cloned().unwrap_or(Value::Null));
            if let Some(index) = self.pending_tools.get(id).copied() {
                if let Some(TranscriptEntry::Tool(tool)) = self.entries.get_mut(index) {
                    tool.arguments = arguments;
                }
            } else {
                self.entries.push(TranscriptEntry::Tool(ToolSnapshot {
                    name,
                    arguments,
                    result: None,
                    started: false,
                    args_complete: false,
                }));
                self.pending_tools
                    .insert(id.to_owned(), self.entries.len() - 1);
            }
        }
    }

    fn start_tool(&mut self, id: String, name: String, args: Value) {
        let index = if let Some(index) = self.pending_tools.get(&id).copied() {
            index
        } else {
            self.entries.push(TranscriptEntry::Tool(ToolSnapshot {
                name: name.clone(),
                arguments: json_text(args.clone()),
                result: None,
                started: false,
                args_complete: true,
            }));
            let index = self.entries.len() - 1;
            self.pending_tools.insert(id.clone(), index);
            index
        };
        if let Some(TranscriptEntry::Tool(tool)) = self.entries.get_mut(index) {
            tool.name = name;
            tool.arguments = json_text(args);
            tool.started = true;
        }
    }

    fn update_tool(&mut self, id: &str, result: Value, is_error: bool, partial: bool) {
        if let Some(index) = self.pending_tools.get(id).copied()
            && let Some(TranscriptEntry::Tool(tool)) = self.entries.get_mut(index)
        {
            tool.result = Some((tool_result_text(result), is_error, partial));
            if !partial {
                self.pending_tools.remove(id);
            }
        }
    }
}

struct TranscriptView(Arc<Mutex<TranscriptState>>);
impl TranscriptView {
    fn new(state: Arc<Mutex<TranscriptState>>) -> Self {
        Self(state)
    }
}
impl Component for TranscriptView {
    fn render(&self, width: usize) -> Vec<String> {
        self.0.lock().unwrap().entries.iter().flat_map(|entry| match entry {
            TranscriptEntry::User(text) => UserMessageComponent::new(text, 1).render(width),
            TranscriptEntry::Assistant(snapshot) => {
                let mut component = StreamingAssistantMessage::default();
                component.update_snapshot(snapshot.content.clone());
                component.set_stop(snapshot.stop_reason, snapshot.error_message.clone());
                component.render(width)
            }
            TranscriptEntry::Tool(tool) => {
                let mut component = ToolExecutionComponent::new(&tool.name, &tool.arguments);
                if tool.started { component.mark_execution_started(); }
                if tool.args_complete { component.set_args_complete(); }
                if let Some((result, error, partial)) = &tool.result {
                    component.update_result_content(
                        vec![crate::modes_interactive_components_index::tool_execution::ToolResultContent::Text(result.clone())],
                        *error,
                        *partial,
                    );
                }
                component.render(width)
            }
            TranscriptEntry::Compaction { summary, tokens_before } => {
                CompactionSummaryMessageComponent::new(
                    zedflow_agent::harness::messages::CompactionSummaryMessage {
                        role: "compactionSummary".into(),
                        summary: summary.clone(),
                        tokens_before: *tokens_before,
                        timestamp: 0,
                    },
                ).render(width)
            }
            TranscriptEntry::Status(text) => Text::new(text, 1, 0).render(width),
        }).collect()
    }
}

struct ActivityView(Arc<Mutex<TranscriptState>>);
impl ActivityView {
    fn new(state: Arc<Mutex<TranscriptState>>) -> Self {
        Self(state)
    }
}
impl Component for ActivityView {
    fn render(&self, width: usize) -> Vec<String> {
        let state = self.0.lock().unwrap();
        let mut lines = if state.compacting {
            WorkingStatusIndicator::new("Compacting... (escape to cancel)", None).render(width)
        } else if state.working {
            WorkingStatusIndicator::new("Working...", None).render(width)
        } else {
            IdleStatus.render(width)
        };
        if state.queued > 0 {
            lines.extend(
                Text::new(format!("{} queued message(s)", state.queued), 1, 0).render(width),
            );
        }
        lines
    }
}

struct EditorFooterView {
    editor: TranscriptEditor,
    footer: Arc<Mutex<FooterSnapshot>>,
}
impl EditorFooterView {
    fn new(submitted: Arc<Mutex<VecDeque<String>>>, footer: Arc<Mutex<FooterSnapshot>>) -> Self {
        Self {
            editor: TranscriptEditor::new(submitted),
            footer,
        }
    }
}
impl Component for EditorFooterView {
    fn render(&self, width: usize) -> Vec<String> {
        let mut lines = self.editor.render(width);
        lines.extend(self.footer.lock().unwrap().render(width));
        lines
    }
    fn handle_input(&mut self, data: &str) {
        self.editor.handle_input(data);
    }
}

struct TranscriptEditor {
    value: String,
    submitted: Arc<Mutex<VecDeque<String>>>,
}

impl TranscriptEditor {
    fn new(submitted: Arc<Mutex<VecDeque<String>>>) -> Self {
        Self {
            value: String::new(),
            submitted,
        }
    }
}

impl Component for TranscriptEditor {
    fn render(&self, width: usize) -> Vec<String> {
        vec![self.value.chars().take(width).collect()]
    }

    fn handle_input(&mut self, data: &str) {
        match data {
            "\x03" => self.submitted.lock().unwrap().push_back("/exit".into()),
            "\r" | "\n" => {
                let input = std::mem::take(&mut self.value);
                self.submitted.lock().unwrap().push_back(input);
            }
            "\x7f" => {
                self.value.pop();
            }
            _ if data.chars().all(|character| !character.is_control()) => self.value.push_str(data),
            _ => {}
        }
    }
}

fn message_role(message: &AgentMessage) -> &str {
    match serde_json::to_value(message)
        .unwrap_or_default()
        .get("role")
        .and_then(Value::as_str)
    {
        Some("user") => "user",
        Some("assistant") => "assistant",
        _ => "message",
    }
}

fn assistant_snapshot(message: &AgentMessage) -> AssistantSnapshot {
    let json = serde_json::to_value(message).unwrap_or_default();
    let content = json
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|part| match part.get("type").and_then(Value::as_str) {
            Some("text") => Some(AssistantContent::Text(
                part.get("text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .into(),
            )),
            Some("thinking") => Some(AssistantContent::Thinking(
                part.get("thinking")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .into(),
            )),
            Some("toolCall") => Some(AssistantContent::ToolCall),
            _ => None,
        })
        .collect();
    let stop_reason = match json.get("stopReason").and_then(Value::as_str) {
        Some("length") => StopReason::Length,
        Some("aborted") => StopReason::Aborted,
        Some("error") => StopReason::Error,
        _ => StopReason::Complete,
    };
    AssistantSnapshot {
        content,
        stop_reason,
        error_message: json
            .get("errorMessage")
            .and_then(Value::as_str)
            .map(str::to_owned),
    }
}

fn json_text(value: Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text,
        value => serde_json::to_string(&value).unwrap_or_default(),
    }
}

fn tool_result_text(value: Value) -> String {
    let content = value.get("content").and_then(Value::as_array);
    if let Some(content) = content {
        let text = content
            .iter()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n");
        if !text.is_empty() {
            return text;
        }
    }
    json_text(value)
}

fn message_content(message: &AgentMessage) -> String {
    let json = serde_json::to_value(message).unwrap_or_default();
    match json.get("content") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(|part| {
                part.get("text")
                    .or_else(|| part.get("thinking"))
                    .and_then(Value::as_str)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn current_dir() -> String {
    std::env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_default()
}
