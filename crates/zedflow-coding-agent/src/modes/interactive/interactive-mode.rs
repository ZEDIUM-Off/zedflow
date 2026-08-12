//! Interactive-mode contracts that do not require the TUI package.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs, io,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use serde_json::Value;
use zedflow_agent::{
    harness::types::{AgentHarnessEvent, AgentHarnessOwnEvent},
    types::{AgentEvent, AgentMessage},
};
use zedflow_tui::{Component, ProcessTerminal, Terminal, Text, Tui};

use crate::core::{http_dispatcher, settings_manager::DefaultProjectTrust};

use crate::{
    agent_session_runtime::AgentSessionRuntime,
    auth_storage::AuthStorage,
    config::{get_agent_dir, get_auth_path, get_sessions_dir},
    extensions::{
        ExtensionError, ExtensionEvent, ExtensionEventKind, ExtensionMode, ExtensionRunner,
        InputEvent, ProviderConfig, SessionActionResult,
    },
    keybindings::KeybindingsManager,
    modes_interactive_components_index::{
        assistant_message::{AssistantContent, StopReason, StreamingAssistantMessage},
        compaction_summary_message::CompactionSummaryMessageComponent,
        custom_editor::CustomEditor,
        footer::{FooterSnapshot, format_cwd_for_footer},
        status_indicator::{IdleStatus, WorkingStatusIndicator},
        tool_execution::ToolExecutionComponent,
        user_message::UserMessageComponent,
    },
    modes_interactive_components_index::{
        model_selector::{ModelItem, ModelSelector},
        oauth_selector::{
            AuthSelectorMode, AuthSelectorProvider, AuthSelectorProviderType, OAuthSelector,
        },
        scoped_models_selector::{ScopedModel, ScopedModelsSelector},
        settings_selector::{SettingChoice, SettingsAction, SettingsSelector},
        trust_selector::TrustSelectorState,
    },
    session_manager,
    session_selector::SessionSelectorState,
    settings_manager::SettingsManager,
    slash_commands::{BuiltinSlashCommandId, parse_builtin_slash_command},
    tree_selector::TreeSelectorState,
    trust_manager::ProjectTrustStore,
    user_message_selector::{UserMessageItem, UserMessageSelectorState},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuiltinCommandOutcome {
    Success(String),
    Cancelled(String),
    Action(BuiltinCommandAction),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuiltinCommandAction {
    Settings,
    Model(Option<String>),
    ScopedModels,
    Export(Option<PathBuf>),
    Import(PathBuf),
    Share,
    Copy,
    Name(Option<String>),
    Session,
    Changelog,
    Hotkeys,
    Fork,
    Clone,
    Tree,
    Trust,
    Login,
    Logout,
    New,
    Resume,
}

pub trait BuiltinCommandService: Send {
    fn execute(
        &mut self,
        command: BuiltinSlashCommandId,
        arguments: &str,
    ) -> Result<BuiltinCommandOutcome, String>;
}

/// Live command parser boundary. Effects are typed so `InteractiveMode` can
/// mount the matching selector or operate on its active runtime; reload is the
/// only effect that must remain owned by `main`'s resource loader.
#[derive(Default)]
pub struct LiveBuiltinCommandService {
    reload: Option<Box<dyn FnMut() -> Result<(), String> + Send>>,
}

impl LiveBuiltinCommandService {
    #[must_use]
    pub fn with_reload(reload: impl FnMut() -> Result<(), String> + Send + 'static) -> Self {
        Self {
            reload: Some(Box::new(reload)),
        }
    }
}

impl BuiltinCommandService for LiveBuiltinCommandService {
    fn execute(
        &mut self,
        command: BuiltinSlashCommandId,
        arguments: &str,
    ) -> Result<BuiltinCommandOutcome, String> {
        use BuiltinCommandAction as Action;
        use BuiltinSlashCommandId as Command;
        let action = match command {
            Command::Settings => Action::Settings,
            Command::Model => Action::Model((!arguments.is_empty()).then(|| arguments.to_owned())),
            Command::ScopedModels => Action::ScopedModels,
            Command::Export => {
                Action::Export((!arguments.is_empty()).then(|| PathBuf::from(arguments)))
            }
            Command::Import if arguments.is_empty() => {
                return Err("Usage: /import <path.jsonl>".into());
            }
            Command::Import => Action::Import(PathBuf::from(arguments)),
            Command::Share => Action::Share,
            Command::Copy => Action::Copy,
            Command::Name => Action::Name((!arguments.is_empty()).then(|| arguments.to_owned())),
            Command::Session => Action::Session,
            Command::Changelog => Action::Changelog,
            Command::Hotkeys => Action::Hotkeys,
            Command::Fork => Action::Fork,
            Command::Clone => Action::Clone,
            Command::Tree => Action::Tree,
            Command::Trust => Action::Trust,
            Command::Login => Action::Login,
            Command::Logout => Action::Logout,
            Command::New => Action::New,
            Command::Resume => Action::Resume,
            Command::Reload => {
                if let Some(reload) = &mut self.reload {
                    reload()?;
                }
                return Ok(BuiltinCommandOutcome::Success(
                    "Reloaded keybindings, extensions, skills, prompts, themes".into(),
                ));
            }
            Command::Compact | Command::Quit => unreachable!("handled by InteractiveMode"),
        };
        Ok(BuiltinCommandOutcome::Action(action))
    }
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
    editor_text: Arc<Mutex<Option<String>>>,
    editor_history: Arc<Mutex<VecDeque<String>>>,
    builtin_commands: Box<dyn BuiltinCommandService>,
    selector_actions: Arc<Mutex<VecDeque<SelectorAction>>>,
    prompt_task: Option<JoinHandle<Result<(), String>>>,
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
                cwd: format_cwd_for_footer(
                    std::path::Path::new(&current_dir()),
                    std::env::var_os("HOME")
                        .as_deref()
                        .map(std::path::Path::new),
                ),
                stats: vec!["0.0%/0 (auto)".into()],
                ..FooterSnapshot::default()
            })),
            compacting: false,
            compaction_queue: VecDeque::new(),
            submitted_terminal_inputs: Arc::new(Mutex::new(VecDeque::new())),
            editor_text: Arc::new(Mutex::new(None)),
            editor_history: Arc::new(Mutex::new(VecDeque::new())),
            builtin_commands: Box::<LiveBuiltinCommandService>::default(),
            selector_actions: Arc::new(Mutex::new(VecDeque::new())),
            prompt_task: None,
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
                if message_role(&message) != "assistant" {
                    return;
                }
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

    /// Render the mounted interactive tree and native overlay stack without writing a terminal.
    #[must_use]
    pub fn render_current_frame(&self, width: usize, height: usize) -> Vec<String> {
        self.tui.render_frame(width, height)
    }

    fn mount_root(&mut self) {
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
                Arc::clone(&self.editor_text),
                Arc::clone(&self.editor_history),
            ));
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        self.mount_root();
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
        self.drain_selector_actions()?;
        Ok(count)
    }

    pub fn stop(&mut self) -> io::Result<()> {
        let terminal_result = self.tui.stop();
        let abort_result = if let Some(runtime) = &self.runtime
            && self.prompt_task.is_some()
        {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(io::Error::other)?
                .block_on(runtime.session().abort())
                .map(|_| ())
                .map_err(|error| io::Error::other(error.to_string()))
        } else {
            Ok(())
        };
        let join_result = self.prompt_task.take().map_or(Ok(()), |task| {
            task.join()
                .map_err(|_| io::Error::other("prompt task panicked"))?
                .map_err(io::Error::other)
        });
        abort_result?;
        join_result?;
        if let Some(runner) = &mut self.extension_runner {
            runner.shutdown("interactive mode stopped");
        }
        self.state = InteractiveState::Stopped;
        terminal_result
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
                    Ok(BuiltinCommandOutcome::Action(action)) => {
                        if let Err(error) = self.apply_builtin_action(action) {
                            self.show_status(error);
                        }
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
        self.editor_history.lock().unwrap().push_back(text.clone());
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
        if self
            .prompt_task
            .as_ref()
            .is_some_and(JoinHandle::is_finished)
        {
            let result = self
                .prompt_task
                .take()
                .expect("finished prompt task")
                .join()
                .map_err(|_| io::Error::other("prompt task panicked"))?;
            if let Err(error) = result {
                self.show_status(error);
            }
            while !self.compacting {
                let Some(input) = self.compaction_queue.pop_front() else {
                    break;
                };
                self.pending_user_inputs.push_back(input);
            }
        }
        if self.prompt_task.is_some() {
            return Ok(false);
        }
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
        self.prompt_task = Some(std::thread::spawn(move || {
            let executor = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| error.to_string())?;
            executor
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
                })
                .map_err(|error| error.to_string())
        }));
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

impl InteractiveMode {
    fn apply_builtin_action(&mut self, action: BuiltinCommandAction) -> Result<(), String> {
        use BuiltinCommandAction as Action;
        match action {
            Action::Settings => {
                let settings = SettingsManager::create(current_dir(), get_agent_dir());
                self.mount_selector(LiveSelector::Settings(SettingsSelector::new(
                    settings_choices(&settings),
                )))
            }
            Action::Model(search) => {
                let current =
                    SettingsManager::create(current_dir(), get_agent_dir()).get_default_model();
                let models = zedflow_ai::providers::all::builtin_models()
                    .get_models(None)
                    .into_iter()
                    .map(|model| ModelItem {
                        provider: model.provider.to_string(),
                        id: model.id,
                        name: model.name,
                    })
                    .collect();
                let mut selector = ModelSelector::new(models, &[], current.as_deref());
                if let Some(search) = search {
                    selector.filter(&search);
                }
                self.mount_selector(LiveSelector::Model(selector));
            }
            Action::ScopedModels => {
                let settings = SettingsManager::create(current_dir(), get_agent_dir());
                let all = zedflow_ai::providers::all::builtin_models()
                    .get_models(None)
                    .into_iter()
                    .map(|model| ScopedModel {
                        full_id: format!("{}/{}", model.provider, model.id),
                        name: model.name,
                    })
                    .collect();
                self.mount_selector(LiveSelector::Scoped(ScopedModelsSelector::new(
                    all,
                    settings.get_enabled_models(),
                )));
            }
            Action::Export(path) => self.export_active_session(path)?,
            Action::Import(path) => self.mount_selector(LiveSelector::ConfirmImport(path)),
            Action::Share => {
                self.show_status("Sharing requires the configured GitHub CLI service");
            }
            Action::Copy => {
                self.copy_last_assistant()?;
            }
            Action::Name(name) => self.name_session(name)?,
            Action::Session => self.show_session_info()?,
            Action::Changelog => {
                self.show_status("Changelog opened");
            }
            Action::Hotkeys => {
                self.show_status("Hotkeys opened");
            }
            Action::Fork => {
                self.mount_selector(LiveSelector::Message(self.user_message_selector()?))
            }
            Action::Clone => self.clone_active_session()?,
            Action::Tree => self.mount_selector(LiveSelector::Tree(self.tree_selector()?)),
            Action::Trust => {
                let store = ProjectTrustStore::new(get_agent_dir());
                let saved = store
                    .get_entry(current_dir())
                    .map_err(|error| error.to_string())?;
                self.mount_selector(LiveSelector::Trust(
                    TrustSelectorState::new(current_dir(), saved)
                        .map_err(|error| error.to_string())?,
                ));
            }
            Action::Login => self.mount_selector(LiveSelector::Auth {
                selector: OAuthSelector::with_mode(AuthSelectorMode::Login, login_providers()),
                auth: AuthStorage::create(get_auth_path()),
            }),
            Action::Logout => {
                let mut auth = AuthStorage::create(get_auth_path());
                let providers = auth
                    .list()
                    .into_iter()
                    .map(|id| AuthSelectorProvider {
                        name: id.clone(),
                        id,
                        auth_type: AuthSelectorProviderType::ApiKey,
                    })
                    .collect();
                // Reload before mounting: the selector is backed by the actual auth file.
                auth.reload();
                self.mount_selector(LiveSelector::Auth {
                    selector: OAuthSelector::with_mode(AuthSelectorMode::Logout, providers),
                    auth,
                });
            }
            Action::New => self.replace_runtime(&[])?,
            Action::Resume => {
                let settings = SettingsManager::create(current_dir(), get_agent_dir());
                let dir = settings.get_session_dir().unwrap_or_else(get_sessions_dir);
                let sessions = session_manager::list_session_infos(
                    dir,
                    Some(PathBuf::from(current_dir()).as_path()),
                    |_, _| {},
                )
                .map_err(|error| error.to_string())?;
                self.mount_selector(LiveSelector::Session(SessionSelectorState::new(
                    sessions, None,
                )));
            }
        }
        Ok(())
    }

    fn user_message_selector(&self) -> Result<UserMessageSelectorState, String> {
        let runtime = self.runtime.clone().ok_or("No active session")?;
        let entries = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?
            .block_on(runtime.session().session().get_entries());
        let messages = entries
            .into_iter()
            .filter_map(|entry| {
                let entry = serde_json::to_value(entry).ok()?;
                let message = entry.get("message")?;
                (message.get("role").and_then(Value::as_str) == Some("user")).then_some(())?;
                Some(UserMessageItem {
                    id: entry.get("id")?.as_str()?.to_owned(),
                    text: message_json_text(message),
                    timestamp: entry
                        .get("timestamp")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                })
            })
            .collect();
        Ok(UserMessageSelectorState::new(messages, None))
    }

    fn tree_selector(&self) -> Result<TreeSelectorState, String> {
        let runtime = self.runtime.clone().ok_or("No active session")?;
        let (entries, leaf) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?
            .block_on(async {
                let session = runtime.session().session();
                (session.get_entries().await, session.get_leaf_id().await)
            });
        Ok(TreeSelectorState::from_session_tree(
            &session_manager::build_session_tree(&entries),
            leaf,
            None,
        ))
    }

    fn mount_selector(&mut self, selector: LiveSelector) {
        self.tui.show_overlay(SelectorOverlay {
            selector,
            actions: Arc::clone(&self.selector_actions),
        });
        let _ = self.tui.request_render(false);
    }

    fn drain_selector_actions(&mut self) -> io::Result<()> {
        let actions = std::mem::take(&mut *self.selector_actions.lock().unwrap());
        for action in actions {
            let mut dismiss = true;
            match action {
                SelectorAction::Cancelled => {
                    self.show_status("Selection cancelled");
                }
                SelectorAction::Import(path) => self
                    .replace_runtime(&["--session".into(), path.display().to_string()])
                    .map_err(io::Error::other)?,
                SelectorAction::Resume(path) => self
                    .replace_runtime(&["--session".into(), path.display().to_string()])
                    .map_err(io::Error::other)?,
                SelectorAction::Logout(provider) => {
                    self.show_status(format!("Logged out of {provider}"));
                }
                SelectorAction::Trust { updates, trusted } => {
                    ProjectTrustStore::new(get_agent_dir()).set_many(&updates)?;
                    self.show_status(format!(
                        "Saved trust decision: {}",
                        if trusted { "trusted" } else { "untrusted" }
                    ));
                }
                SelectorAction::Model(model) => {
                    SettingsManager::create(current_dir(), get_agent_dir())
                        .set_default_model_and_provider(model.provider, model.id)?;
                    self.show_status("Model selected");
                }
                SelectorAction::Scoped(ids) => {
                    SettingsManager::create(current_dir(), get_agent_dir())
                        .set_enabled_models(ids)?;
                    self.show_status("Model selection saved to settings");
                }
                SelectorAction::Settings(SettingsAction::Change { id, value }) => {
                    dismiss = false;
                    let settings = SettingsManager::create(current_dir(), get_agent_dir());
                    persist_setting(&settings, &id, &value)?;
                    self.show_status(format!("{id} saved to settings"));
                }
                SelectorAction::Settings(SettingsAction::Cancel) => {}
                SelectorAction::NavigateTree(entry_id) => {
                    let runtime = self
                        .runtime
                        .clone()
                        .ok_or_else(|| io::Error::other("No active session"))?;
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(io::Error::other)?
                        .block_on(runtime.session().session().move_to(Some(entry_id), None))
                        .map_err(io::Error::other)?;
                    self.show_status("Navigated to selected point");
                }
                SelectorAction::Fork(entry_id) => {
                    let runtime = self
                        .runtime
                        .clone()
                        .ok_or_else(|| io::Error::other("No active session"))?;
                    let selected_text = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(io::Error::other)?
                        .block_on(async {
                            runtime
                                .session()
                                .session()
                                .get_entry(&entry_id)
                                .await
                                .and_then(|entry| serde_json::to_value(entry).ok())
                                .and_then(|entry| entry.get("message").cloned())
                                .map(|message| message_json_text(&message))
                                .ok_or_else(|| io::Error::other("Invalid entry ID for forking"))
                        })?;
                    self.set_runtime(
                        runtime
                            .fork_at_entry(
                                entry_id,
                                zedflow_agent::harness::types::SessionForkPosition::Before,
                            )
                            .map_err(io::Error::other)?,
                    );
                    *self.editor_text.lock().unwrap() = Some(selected_text);
                    self.show_status("Forked to new session");
                }
            }
            if dismiss {
                let count = self.tui.overlay_count();
                let _ = count
                    .checked_sub(1)
                    .and_then(|index| self.tui.hide_overlay(index));
                self.tui.request_render(false)?;
            }
        }
        Ok(())
    }

    fn clone_active_session(&mut self) -> Result<(), String> {
        let runtime = self.runtime.clone().ok_or("No active session")?;
        let leaf_id = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?
            .block_on(runtime.session().session().get_leaf_id());
        let Some(leaf_id) = leaf_id else {
            self.show_status("Nothing to clone yet");
            return Ok(());
        };
        self.set_runtime(
            runtime
                .fork_at_entry(
                    leaf_id,
                    zedflow_agent::harness::types::SessionForkPosition::At,
                )
                .map_err(|error| error.to_string())?,
        );
        *self.editor_text.lock().unwrap() = Some(String::new());
        self.show_status("Cloned to new session");
        Ok(())
    }

    fn replace_runtime(&mut self, args: &[String]) -> Result<(), String> {
        let runtime =
            crate::rpc_entry::create_runtime_for_args(args).map_err(|error| error.to_string())?;
        self.transcript.lock().unwrap().entries.clear();
        self.set_runtime(runtime);
        self.show_status("Session loaded");
        Ok(())
    }

    fn export_active_session(&mut self, path: Option<PathBuf>) -> Result<(), String> {
        let runtime = self.runtime.clone().ok_or("No active session to export")?;
        let (metadata, entries) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?
            .block_on(async {
                let session = runtime.session().session();
                (session.get_metadata().await, session.get_entries().await)
            });
        let mut jsonl = serde_json::to_string(&serde_json::json!({ "type": "session", "id": metadata.id, "timestamp": metadata.created_at, "cwd": runtime.cwd() })).map_err(|error| error.to_string())?;
        for entry in entries {
            jsonl.push('\n');
            jsonl.push_str(&serde_json::to_string(&entry).map_err(|error| error.to_string())?);
        }
        jsonl.push('\n');
        let output =
            path.unwrap_or_else(|| PathBuf::from(format!("pi-session-{}.html", metadata.id)));
        if output
            .extension()
            .is_some_and(|extension| extension == "jsonl")
        {
            fs::write(&output, jsonl).map_err(|error| error.to_string())?;
        } else {
            fs::write(&output, crate::export_html::export_session_to_html(&jsonl))
                .map_err(|error| error.to_string())?;
        }
        self.show_status(format!("Session exported to: {}", output.display()));
        Ok(())
    }

    fn name_session(&mut self, name: Option<String>) -> Result<(), String> {
        let runtime = self.runtime.clone().ok_or("No active session")?;
        let session = runtime.session().session();
        let has_name = name.is_some();
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?
            .block_on(async {
                match name {
                    Some(name) => session.append_session_name(name).await.map(|_| ()),
                    None => Ok(()),
                }
            });
        result.map_err(|error| error.to_string())?;
        self.show_status(if has_name {
            "Session name set"
        } else {
            "Session name requested"
        });
        Ok(())
    }

    fn show_session_info(&mut self) -> Result<(), String> {
        let runtime = self.runtime.clone().ok_or("No active session")?;
        let metadata = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?
            .block_on(runtime.session().session().get_metadata());
        self.show_status(format!("Session {}", metadata.id));
        Ok(())
    }

    fn copy_last_assistant(&mut self) -> Result<(), String> {
        let runtime = self.runtime.clone().ok_or("No active session")?;
        let entries = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?
            .block_on(runtime.session().session().get_entries());
        let text = entries
            .into_iter()
            .rev()
            .find_map(|entry| {
                serde_json::to_value(entry).ok().and_then(|entry| {
                    (entry
                        .get("message")
                        .and_then(|message| message.get("role"))
                        .and_then(Value::as_str)
                        == Some("assistant"))
                    .then_some(message_json_text(&entry))
                })
            })
            .filter(|text| !text.is_empty())
            .ok_or("No agent messages to copy yet")?;
        crate::utils::clipboard::copy_to_clipboard(&text).map_err(|error| error.to_string())?;
        self.show_status("Copied last agent message to clipboard");
        Ok(())
    }
}

fn settings_choices(settings: &SettingsManager) -> Vec<SettingChoice> {
    let bool_choice = |id: &str, label: &str, description: &str, value: bool| SettingChoice {
        id: id.into(),
        label: label.into(),
        description: description.into(),
        value: value.to_string(),
        values: vec!["true".into(), "false".into()],
    };
    let default_trust = match settings.get_default_project_trust() {
        DefaultProjectTrust::Ask => "Ask",
        DefaultProjectTrust::Always => "Always trust",
        DefaultProjectTrust::Never => "Never trust",
    };
    vec![
        bool_choice("autocompact", "Auto-compact", "Automatically compact context when it gets too large", settings.get_compaction_settings().0),
        bool_choice("auto-resize-images", "Auto-resize images", "Resize large images to 2000x2000 max for better model compatibility", settings.get_image_auto_resize()),
        bool_choice("block-images", "Block images", "Prevent images from being sent to LLM providers", settings.get_block_images()),
        bool_choice("skill-commands", "Skill commands", "Register skills as /skill:name commands", settings.get_enable_skill_commands()),
        bool_choice("show-hardware-cursor", "Show hardware cursor", "Show the terminal cursor while still positioning it for IME support", settings.get_show_hardware_cursor()),
        SettingChoice { id: "editor-padding".into(), label: "Editor padding".into(), description: "Horizontal padding for input editor (0-3)".into(), value: settings.get_editor_padding_x().to_string(), values: vec!["0".into(), "1".into(), "2".into(), "3".into()] },
        SettingChoice { id: "output-padding".into(), label: "Output padding".into(), description: "Horizontal padding for user messages, assistant messages, and thinking".into(), value: settings.get_output_pad().to_string(), values: vec!["0".into(), "1".into()] },
        SettingChoice { id: "autocomplete-max-visible".into(), label: "Autocomplete max items".into(), description: "Max visible items in autocomplete dropdown (3-20)".into(), value: settings.get_autocomplete_max_visible().to_string(), values: vec!["3".into(), "5".into(), "7".into(), "10".into(), "15".into(), "20".into()] },
        bool_choice("clear-on-shrink", "Clear on shrink", "Clear empty rows when content shrinks (may cause flicker)", settings.get_clear_on_shrink()),
        bool_choice("terminal-progress", "Terminal progress", "Show OSC 9;4 progress indicators in the terminal tab bar", settings.get_show_terminal_progress()),
        SettingChoice { id: "steering-mode".into(), label: "Steering mode".into(), description: "Enter while streaming queues steering messages. 'one-at-a-time': deliver one, wait for response. 'all': deliver all at once.".into(), value: settings.get_steering_mode(), values: vec!["one-at-a-time".into(), "all".into()] },
        SettingChoice { id: "follow-up-mode".into(), label: "Follow-up mode".into(), description: "Follow-up queues messages until agent stops. 'one-at-a-time': deliver one, wait for response. 'all': deliver all at once.".into(), value: settings.get_follow_up_mode(), values: vec!["one-at-a-time".into(), "all".into()] },
        SettingChoice { id: "transport".into(), label: "Transport".into(), description: "Preferred transport for providers that support multiple transports".into(), value: match settings.get_transport() { zedflow_ai::Transport::Sse => "sse", zedflow_ai::Transport::Websocket => "websocket", zedflow_ai::Transport::WebsocketCached => "websocket-cached", zedflow_ai::Transport::Auto => "auto" }.into(), values: vec!["sse".into(), "websocket".into(), "websocket-cached".into(), "auto".into()] },
        SettingChoice { id: "http-idle-timeout".into(), label: "HTTP idle timeout".into(), description: "Maximum idle gap while waiting for HTTP headers or body chunks. Disable for local models that pause longer than five minutes.".into(), value: http_dispatcher::format_http_idle_timeout_ms(settings.get_http_idle_timeout_ms()), values: http_dispatcher::HTTP_IDLE_TIMEOUT_CHOICES.iter().map(|choice| choice.label.into()).collect() },
        bool_choice("hide-thinking", "Hide thinking", "Hide thinking blocks in assistant responses", settings.get_hide_thinking_block()),
        bool_choice("collapse-changelog", "Collapse changelog", "Show condensed changelog after updates", settings.get_collapse_changelog()),
        bool_choice("quiet-startup", "Quiet startup", "Disable verbose printing at startup", settings.get_quiet_startup()),
        bool_choice("install-telemetry", "Install telemetry", "Send an anonymous version/update ping after changelog-detected updates", settings.get_enable_install_telemetry()),
        SettingChoice { id: "default-project-trust".into(), label: "Default project trust".into(), description: "Fallback behavior when no extension or saved trust decision decides project trust".into(), value: default_trust.into(), values: vec!["Ask".into(), "Always trust".into(), "Never trust".into()] },
        SettingChoice { id: "double-escape-action".into(), label: "Double-escape action".into(), description: "Action when pressing Escape twice with empty editor".into(), value: settings.get_double_escape_action(), values: vec!["tree".into(), "fork".into(), "none".into()] },
        SettingChoice { id: "tree-filter-mode".into(), label: "Tree filter mode".into(), description: "Default filter when opening /tree".into(), value: settings.get_tree_filter_mode(), values: vec!["default".into(), "no-tools".into(), "user-only".into(), "labeled-only".into(), "all".into()] },
        SettingChoice { id: "warnings".into(), label: "Warnings".into(), description: "Enable or disable individual warnings".into(), value: "configure".into(), values: vec![] },
        SettingChoice { id: "thinking".into(), label: "Thinking level".into(), description: "Reasoning depth for thinking-capable models".into(), value: settings.get_default_thinking_level(), values: vec!["off".into()] },
        SettingChoice { id: "theme".into(), label: "Theme".into(), description: "Color theme for the interface".into(), value: settings.get_theme_setting().unwrap_or_else(|| "dark".into()), values: vec!["dark".into(), "light".into()] },
    ]
}

fn persist_setting(settings: &SettingsManager, id: &str, value: &str) -> io::Result<()> {
    let enabled = || value == "true";
    match id {
        "autocompact" => settings.set_compaction_enabled(enabled()),
        "auto-resize-images" => settings.set_image_auto_resize(enabled()),
        "block-images" => settings.set_block_images(enabled()),
        "skill-commands" => settings.set_enable_skill_commands(enabled()),
        "show-hardware-cursor" => settings.set_show_hardware_cursor(enabled()),
        "editor-padding" => settings.set_editor_padding_x(value.parse().map_err(io::Error::other)?),
        "output-padding" => settings.set_output_pad(value.parse().map_err(io::Error::other)?),
        "autocomplete-max-visible" => {
            settings.set_autocomplete_max_visible(value.parse().map_err(io::Error::other)?)
        }
        "clear-on-shrink" => settings.set_clear_on_shrink(enabled()),
        "terminal-progress" => settings.set_show_terminal_progress(enabled()),
        "steering-mode" => settings.set_steering_mode(value),
        "follow-up-mode" => settings.set_follow_up_mode(value),
        "thinking" => settings.set_default_thinking_level(value),
        "transport" => settings.set_transport(match value {
            "sse" => zedflow_ai::Transport::Sse,
            "websocket" => zedflow_ai::Transport::Websocket,
            "websocket-cached" => zedflow_ai::Transport::WebsocketCached,
            _ => zedflow_ai::Transport::Auto,
        }),
        "http-idle-timeout" => http_dispatcher::HTTP_IDLE_TIMEOUT_CHOICES
            .iter()
            .find(|choice| choice.label == value)
            .map_or(Ok(()), |choice| {
                settings.set_http_idle_timeout_ms(choice.timeout_ms)
            }),
        "hide-thinking" => settings.set_hide_thinking_block(enabled()),
        "collapse-changelog" => settings.set_collapse_changelog(enabled()),
        "quiet-startup" => settings.set_quiet_startup(enabled()),
        "install-telemetry" => settings.set_enable_install_telemetry(enabled()),
        "default-project-trust" => settings.set_default_project_trust(match value {
            "Always trust" => DefaultProjectTrust::Always,
            "Never trust" => DefaultProjectTrust::Never,
            _ => DefaultProjectTrust::Ask,
        }),
        "double-escape-action" => settings.set_double_escape_action(value),
        "tree-filter-mode" => settings.set_tree_filter_mode(value),
        "theme" => settings.set_theme(value),
        _ => Ok(()),
    }
}

fn login_providers() -> Vec<AuthSelectorProvider> {
    let mut providers = zedflow_ai::providers::all::builtin_models()
        .get_models(None)
        .into_iter()
        .map(|model| model.provider.to_string())
        .collect::<Vec<_>>();
    providers.sort();
    providers.dedup();
    providers
        .into_iter()
        .map(|id| AuthSelectorProvider {
            name: id.clone(),
            id,
            auth_type: AuthSelectorProviderType::ApiKey,
        })
        .collect()
}

fn message_json_text(message: &Value) -> String {
    message
        .get("content")
        .map_or_else(String::new, |content| match content {
            Value::String(text) => text.clone(),
            Value::Array(parts) => parts
                .iter()
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n"),
            _ => String::new(),
        })
}

#[derive(Debug)]
enum SelectorAction {
    Cancelled,
    Import(PathBuf),
    Resume(PathBuf),
    Logout(String),
    Trust {
        updates: Vec<crate::trust_manager::ProjectTrustUpdate>,
        trusted: bool,
    },
    Model(ModelItem),
    Scoped(Option<Vec<String>>),
    Settings(SettingsAction),
    NavigateTree(String),
    Fork(String),
}

enum LiveSelector {
    Settings(SettingsSelector),
    Model(ModelSelector),
    Scoped(ScopedModelsSelector),
    Session(SessionSelectorState),
    Trust(TrustSelectorState),
    Auth {
        selector: OAuthSelector,
        auth: AuthStorage,
    },
    ConfirmImport(PathBuf),
    Message(UserMessageSelectorState),
    Tree(TreeSelectorState),
}

struct SelectorOverlay {
    selector: LiveSelector,
    actions: Arc<Mutex<VecDeque<SelectorAction>>>,
}
impl SelectorOverlay {
    fn title(&self) -> &str {
        match &self.selector {
            LiveSelector::Settings(_) => "Settings",
            LiveSelector::Model(_) => "Select model",
            LiveSelector::Scoped(_) => "Scoped models",
            LiveSelector::Session(_) => "Resume session",
            LiveSelector::Trust(_) => "Project trust",
            LiveSelector::Auth { selector, .. } => {
                if selector.mode == AuthSelectorMode::Login {
                    "Login"
                } else {
                    "Logout"
                }
            }
            LiveSelector::ConfirmImport(_) => "Import session",
            LiveSelector::Message(_) => "Fork from message",
            LiveSelector::Tree(_) => "Session tree",
        }
    }
    fn move_selection(&mut self, down: bool) {
        match &mut self.selector {
            LiveSelector::Model(value) => value.move_selection(if down { 1 } else { -1 }),
            LiveSelector::Scoped(value) => {
                if down {
                    value.selected += 1
                } else {
                    value.selected = value.selected.saturating_sub(1)
                }
            }
            LiveSelector::Session(value) => {
                if down {
                    value.move_down()
                } else {
                    value.move_up()
                }
            }
            LiveSelector::Trust(value) => {
                if down {
                    value.move_down()
                } else {
                    value.move_up()
                }
            }
            LiveSelector::Auth { selector, .. } => {
                selector.move_selection(if down { 1 } else { -1 })
            }
            LiveSelector::Settings(value) => value.move_selection(if down { 1 } else { -1 }),
            LiveSelector::Message(value) => {
                if down {
                    value.move_down()
                } else {
                    value.move_up()
                }
            }
            LiveSelector::Tree(value) => {
                if down {
                    value.move_down()
                } else {
                    value.move_up()
                }
            }
            _ => {}
        }
    }
    fn select(&mut self) {
        let action = match &mut self.selector {
            LiveSelector::Model(selector) => selector
                .selected_model()
                .cloned()
                .map(SelectorAction::Model),
            LiveSelector::Scoped(selector) => Some(SelectorAction::Scoped(
                selector.enabled_ids().map(ToOwned::to_owned),
            )),
            LiveSelector::Session(selector) => match selector.select() {
                crate::session_selector::SessionSelectorAction::Select(path) => {
                    Some(SelectorAction::Resume(path))
                }
                _ => None,
            },
            LiveSelector::Trust(selector) => {
                selector.select().map(|selection| SelectorAction::Trust {
                    updates: selection.updates,
                    trusted: selection.trusted,
                })
            }
            LiveSelector::Auth { selector, auth } if selector.mode == AuthSelectorMode::Logout => {
                selector
                    .selected_provider()
                    .map(|provider| provider.id.clone())
                    .and_then(|provider| {
                        auth.logout(&provider)
                            .ok()
                            .map(|()| SelectorAction::Logout(provider))
                    })
            }
            LiveSelector::ConfirmImport(path) => Some(SelectorAction::Import(path.clone())),
            LiveSelector::Settings(selector) => selector.activate().map(SelectorAction::Settings),
            LiveSelector::Message(selector) => match selector.select() {
                crate::user_message_selector::UserMessageSelectorAction::Select(id) => {
                    Some(SelectorAction::Fork(id))
                }
                _ => None,
            },
            LiveSelector::Tree(selector) => match selector.select() {
                crate::tree_selector::TreeSelectorAction::Select(id) => {
                    Some(SelectorAction::NavigateTree(id))
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(action) = action {
            self.actions.lock().unwrap().push_back(action);
        }
        if let LiveSelector::Settings(selector) = &self.selector {
            self.actions.lock().unwrap().extend(
                selector
                    .drain_actions()
                    .into_iter()
                    .map(SelectorAction::Settings),
            );
        }
    }
}
impl Component for SelectorOverlay {
    fn render(&self, width: usize) -> Vec<String> {
        if let LiveSelector::Settings(selector) = &self.selector {
            return selector.render(width);
        }
        let mut lines = vec![format!("{}", self.title())];
        match &self.selector {
            LiveSelector::Session(selector) => {
                lines.extend(selector.visible_sessions().take(8).map(|session| {
                    format!(
                        "  {}",
                        session.name.as_deref().unwrap_or(&session.session_id)
                    )
                }))
            }
            LiveSelector::Trust(selector) => lines.extend(
                selector
                    .options()
                    .iter()
                    .enumerate()
                    .map(|(index, option)| {
                        format!(
                            "{} {}",
                            if index == selector.selected_index() {
                                "→"
                            } else {
                                " "
                            },
                            option.label
                        )
                    }),
            ),
            LiveSelector::Auth { selector, .. } => {
                lines.push(selector.selected_provider().map_or_else(
                    || selector.empty_message().into(),
                    |provider| format!("→ {}", provider.name),
                ))
            }
            LiveSelector::Model(selector) => lines.push(selector.selected_model().map_or_else(
                || "No models available".into(),
                |model| format!("→ {}", model.full_id()),
            )),
            LiveSelector::Scoped(selector) => lines.push(selector.selected_model().map_or_else(
                || "No models available".into(),
                |model| format!("→ {}", model.full_id),
            )),
            LiveSelector::ConfirmImport(path) => {
                lines.push(format!("Replace current session with {}?", path.display()))
            }
            LiveSelector::Message(selector) => lines.push(
                selector
                    .messages()
                    .get(selector.selected_index())
                    .map_or_else(
                        || "No messages to fork from".into(),
                        |message| {
                            format!("→ {}", UserMessageSelectorState::normalized_text(message))
                        },
                    ),
            ),
            LiveSelector::Tree(selector) => lines.push(selector.selected_item().map_or_else(
                || "No entries in session".into(),
                |item| format!("→ {}", item.text),
            )),
            _ => lines.push("Use arrows and Enter; Escape cancels".into()),
        }
        lines
            .into_iter()
            .map(|line| line.chars().take(width).collect())
            .collect()
    }
    fn handle_input(&mut self, data: &str) {
        if let LiveSelector::Settings(selector) = &mut self.selector {
            selector.handle_input(data);
            self.actions.lock().unwrap().extend(
                selector
                    .drain_actions()
                    .into_iter()
                    .map(SelectorAction::Settings),
            );
            return;
        }
        match data {
            "\x1b" => self
                .actions
                .lock()
                .unwrap()
                .push_back(SelectorAction::Cancelled),
            "\x1b[A" => self.move_selection(false),
            "\x1b[B" => self.move_selection(true),
            "\r" | "\n" => self.select(),
            _ => {}
        }
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
    editor: Arc<Mutex<CustomEditor>>,
    footer: Arc<Mutex<FooterSnapshot>>,
    editor_text: Arc<Mutex<Option<String>>>,
    editor_history: Arc<Mutex<VecDeque<String>>>,
    submitted: Arc<Mutex<VecDeque<String>>>,
    clear_requested: Arc<AtomicBool>,
    last_sigint: Option<Instant>,
}
impl EditorFooterView {
    fn new(
        submitted: Arc<Mutex<VecDeque<String>>>,
        footer: Arc<Mutex<FooterSnapshot>>,
        editor_text: Arc<Mutex<Option<String>>>,
        editor_history: Arc<Mutex<VecDeque<String>>>,
    ) -> Self {
        let mut editor = CustomEditor::new(KeybindingsManager::create(get_agent_dir()));
        let on_submit = Arc::clone(&submitted);
        editor.editor_mut().on_submit = Some(Box::new(move |text| {
            on_submit.lock().unwrap().push_back(text.to_owned());
        }));
        let on_exit = Arc::clone(&submitted);
        editor.on_ctrl_d = Some(Box::new(move || {
            on_exit.lock().unwrap().push_back("/exit".into());
        }));
        let clear_requested = Arc::new(AtomicBool::new(false));
        let request_clear = Arc::clone(&clear_requested);
        editor.on_action("app.clear", move || {
            request_clear.store(true, Ordering::Release);
        });
        editor.set_focused(true);
        Self {
            editor: Arc::new(Mutex::new(editor)),
            footer,
            editor_text,
            editor_history,
            submitted,
            clear_requested,
            last_sigint: None,
        }
    }
}
impl Component for EditorFooterView {
    fn render(&self, width: usize) -> Vec<String> {
        let mut editor = self.editor.lock().unwrap();
        if let Some(text) = self.editor_text.lock().unwrap().take() {
            editor.editor_mut().set_text(&text);
        }
        let mut lines = editor.render(width);
        lines.extend(self.footer.lock().unwrap().render(width));
        lines
    }
    fn handle_input(&mut self, data: &str) {
        for text in std::mem::take(&mut *self.editor_history.lock().unwrap()) {
            self.editor
                .lock()
                .unwrap()
                .editor_mut()
                .add_to_history(&text);
        }
        if let Some(text) = self.editor_text.lock().unwrap().take() {
            self.editor.lock().unwrap().editor_mut().set_text(&text);
        }
        self.editor.lock().unwrap().handle_input(data);
        if self.clear_requested.swap(false, Ordering::AcqRel) {
            let now = Instant::now();
            if self
                .last_sigint
                .is_some_and(|previous| now.duration_since(previous) < Duration::from_millis(500))
            {
                self.submitted.lock().unwrap().push_back("/exit".into());
                self.last_sigint = None;
            } else {
                self.editor.lock().unwrap().editor_mut().set_text("");
                self.last_sigint = Some(now);
            }
        }
    }
    fn set_focused(&mut self, focused: bool) {
        self.editor.lock().unwrap().set_focused(focused);
    }
    fn is_focused(&self) -> bool {
        self.editor.lock().unwrap().is_focused()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_footer_restores_forked_user_text_before_rendering() {
        let text = Arc::new(Mutex::new(Some("restore this".to_owned())));
        let footer = EditorFooterView::new(
            Arc::new(Mutex::new(VecDeque::new())),
            Arc::new(Mutex::new(FooterSnapshot::default())),
            Arc::clone(&text),
            Arc::new(Mutex::new(VecDeque::new())),
        );
        let _ = footer.render(80);
        assert_eq!(
            footer.editor.lock().unwrap().editor().get_text(),
            "restore this"
        );
    }

    #[test]
    fn settings_selector_has_frozen_main_list_order_and_persists_changes() {
        let settings =
            SettingsManager::in_memory(crate::core::settings_manager::Settings::default());
        assert_eq!(
            settings_choices(&settings)
                .into_iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            [
                "autocompact",
                "auto-resize-images",
                "block-images",
                "skill-commands",
                "show-hardware-cursor",
                "editor-padding",
                "output-padding",
                "autocomplete-max-visible",
                "clear-on-shrink",
                "terminal-progress",
                "steering-mode",
                "follow-up-mode",
                "transport",
                "http-idle-timeout",
                "hide-thinking",
                "collapse-changelog",
                "quiet-startup",
                "install-telemetry",
                "default-project-trust",
                "double-escape-action",
                "tree-filter-mode",
                "warnings",
                "thinking",
                "theme",
            ]
        );
        persist_setting(&settings, "steering-mode", "all").unwrap();
        persist_setting(&settings, "follow-up-mode", "all").unwrap();
        persist_setting(&settings, "thinking", "off").unwrap();
        assert_eq!(settings.get_steering_mode(), "all");
        assert_eq!(settings.get_follow_up_mode(), "all");
        assert_eq!(settings.get_default_thinking_level(), "off");
    }
}
