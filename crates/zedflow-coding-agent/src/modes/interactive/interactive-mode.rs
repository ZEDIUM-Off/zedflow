//! Interactive-mode contracts that do not require the TUI package.

use std::{
    collections::{HashSet, VecDeque},
    io,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::{Arc, Mutex},
    time::Duration,
};

use serde_json::Value;
use zedflow_tui::{Component, ProcessTerminal, Terminal, Tui};

use crate::{
    agent_session_runtime::AgentSessionRuntime,
    extensions::{
        ExtensionError, ExtensionEvent, ExtensionEventKind, ExtensionMode, ExtensionRunner,
        InputEvent, ProviderConfig, SessionActionResult,
    },
};

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
    submitted_terminal_inputs: Arc<Mutex<VecDeque<String>>>,
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
            submitted_terminal_inputs: Arc::new(Mutex::new(VecDeque::new())),
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
        mode.runtime = Some(runtime);
        mode
    }

    #[must_use]
    pub fn with_runtime_and_extension_runner(
        terminal: impl Terminal + 'static,
        runtime: AgentSessionRuntime,
        extension_runner: ExtensionRunner,
    ) -> Self {
        let mut mode = Self::with_extension_runner(terminal, extension_runner);
        mode.runtime = Some(runtime);
        mode
    }

    pub fn tui_mut(&mut self) -> &mut Tui {
        &mut self.tui
    }

    pub fn run(&mut self) -> io::Result<()> {
        self.tui.root.add_child(InteractiveInput::new(Arc::clone(
            &self.submitted_terminal_inputs,
        )));
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
        let Some(input) = self.get_user_input() else {
            return Ok(false);
        };
        let Some(runtime) = self.runtime.clone() else {
            self.pending_user_inputs.push_front(input);
            return Ok(false);
        };
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| io::Error::other(error.to_string()))?
            .block_on(async move { runtime.session().prompt(input, None).await });
        if let Err(error) = result {
            self.show_status(error.to_string());
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

struct InteractiveInput {
    value: String,
    submitted: Arc<Mutex<VecDeque<String>>>,
}

impl InteractiveInput {
    fn new(submitted: Arc<Mutex<VecDeque<String>>>) -> Self {
        Self {
            value: String::new(),
            submitted,
        }
    }
}

impl Component for InteractiveInput {
    fn render(&self, width: usize) -> Vec<String> {
        vec![self.value.chars().take(width).collect()]
    }

    fn handle_input(&mut self, data: &str) {
        match data {
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

fn current_dir() -> String {
    std::env::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_default()
}
