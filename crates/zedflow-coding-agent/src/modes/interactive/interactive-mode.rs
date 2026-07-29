//! Interactive-mode contracts that do not require the TUI package.

use std::{
    collections::{HashSet, VecDeque},
    io,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    time::Duration,
};

use zedflow_tui::{ProcessTerminal, Terminal, Tui};

use crate::extensions::{
    ExtensionEvent, ExtensionEventKind, ExtensionMode, ExtensionRunner, InputEvent,
    SessionActionResult,
};
use serde_json::Value;

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
    extensions: ExtensionRunner,
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
        Self::with_terminal_and_extensions(terminal, ExtensionRunner::new(Vec::new()))
    }

    #[must_use]
    pub fn with_extension_runner(extensions: ExtensionRunner) -> Self {
        Self::with_terminal_and_extensions(ProcessTerminal::new(), extensions)
    }

    #[must_use]
    pub fn with_terminal_and_extensions(
        terminal: impl Terminal + 'static,
        mut extensions: ExtensionRunner,
    ) -> Self {
        extensions.set_context(
            ExtensionMode::Tui,
            std::env::current_dir()
                .unwrap_or_default()
                .display()
                .to_string(),
            true,
        );
        Self {
            state: InteractiveState::Created,
            tui: Tui::with_terminal(terminal),
            pending_user_inputs: VecDeque::new(),
            last_status: None,
            extensions,
        }
    }

    pub fn tui_mut(&mut self) -> &mut Tui {
        &mut self.tui
    }

    pub fn run(&mut self) -> io::Result<()> {
        self.tui.start()?;
        self.state = InteractiveState::Running;
        self.extensions.emit(ExtensionEvent {
            kind: ExtensionEventKind::SessionStart,
            data: serde_json::json!({"mode":"interactive"}),
        });
        Ok(())
    }

    /// Pump terminal input and resize events on the thread that owns this mode.
    pub fn pump_events(&mut self, timeout: Duration) -> io::Result<usize> {
        self.tui.pump_events(timeout)
    }

    pub fn stop(&mut self) -> io::Result<()> {
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
        let result = self.extensions.emit_input(InputEvent::Text(text.clone()));
        if !result.consumed {
            self.pending_user_inputs
                .push_back(result.replacement.unwrap_or(text));
        }
    }

    pub fn emit_session_event(&mut self, kind: ExtensionEventKind, data: Value) -> Vec<Value> {
        self.extensions.emit(ExtensionEvent { kind, data })
    }

    pub fn invoke_extension_tool(
        &mut self,
        name: &str,
        arguments: Value,
    ) -> Result<Value, crate::extensions::ExtensionError> {
        self.extensions.invoke_tool(name, arguments)
    }

    pub fn invoke_extension_command(
        &mut self,
        name: &str,
        args: &[String],
    ) -> Result<SessionActionResult, crate::extensions::ExtensionError> {
        self.extensions.invoke_command(name, args)
    }

    pub fn emit_provider_event(&mut self, kind: ExtensionEventKind, data: Value) -> Vec<Value> {
        self.extensions.emit(ExtensionEvent { kind, data })
    }

    #[must_use]
    pub fn extension_runner(&self) -> &ExtensionRunner {
        &self.extensions
    }

    /// Return queued input in submission order, matching `getUserInput()`.
    pub fn get_user_input(&mut self) -> Option<String> {
        self.pending_user_inputs.pop_front()
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
}

impl Drop for InteractiveMode {
    fn drop(&mut self) {
        self.extensions.shutdown("interactive mode ended");
    }
}
