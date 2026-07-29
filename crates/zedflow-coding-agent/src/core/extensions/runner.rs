use std::{collections::HashMap, sync::Arc};

use serde_json::Value;

use super::types::{
    CommandHandler, Extension, ExtensionContext, ExtensionError, ExtensionErrorListener,
    ExtensionEvent, ExtensionEventKind, ExtensionMode, ExtensionRuntime, InputEvent,
    InputEventResult, ProjectTrustEvent, ProjectTrustEventDecision, ProjectTrustEventResult,
    SessionActionResult, ToolHandler,
};

pub type NewSessionHandler = Box<dyn Fn() + Send + Sync>;
pub type ForkHandler = Box<dyn Fn() + Send + Sync>;
pub type NavigateTreeHandler = Box<dyn Fn() + Send + Sync>;
pub type SwitchSessionHandler = Box<dyn Fn() + Send + Sync>;
pub type ReloadHandler = Box<dyn Fn() + Send + Sync>;
pub type ShutdownHandler = Box<dyn Fn() + Send + Sync>;

#[must_use]
pub fn emit_project_trust_event(
    event: ProjectTrustEvent,
    extensions: &[Extension],
) -> ProjectTrustEventResult {
    let _ = (event, extensions);
    ProjectTrustEventResult {
        decision: ProjectTrustEventDecision::Undecided,
    }
}

/// Executes native in-process extension contracts. A runner generation invalidates contexts
/// created before reload or shutdown, mirroring Pi's stale extension-instance guard.
pub struct ExtensionRunner {
    pub extensions: Vec<Extension>,
    pub runtime: ExtensionRuntime,
    handlers: HashMap<ExtensionEventKind, Vec<(String, super::types::ExtensionHandler)>>,
    error_listeners: Vec<ExtensionErrorListener>,
    context: ExtensionContext,
    shutdown: bool,
}

impl ExtensionRunner {
    #[must_use]
    pub fn new(extensions: Vec<Extension>) -> Self {
        Self {
            extensions,
            runtime: ExtensionRuntime::default(),
            handlers: HashMap::new(),
            error_listeners: Vec::new(),
            context: ExtensionContext {
                mode: ExtensionMode::Print,
                cwd: String::new(),
                has_ui: false,
                generation: 0,
                stale: false,
                model: None,
                context_usage: None,
            },
            shutdown: false,
        }
    }

    pub fn set_context(&mut self, mode: ExtensionMode, cwd: impl Into<String>, has_ui: bool) {
        self.context.mode = mode;
        self.context.cwd = cwd.into();
        self.context.has_ui = has_ui;
    }
    pub fn set_error_listener(&mut self, listener: ExtensionErrorListener) {
        self.error_listeners.push(listener);
    }
    pub fn report_error(&self, error: ExtensionError) {
        for listener in &self.error_listeners {
            listener(error.clone());
        }
    }
    pub fn on(
        &mut self,
        extension: impl Into<String>,
        kind: ExtensionEventKind,
        handler: super::types::ExtensionHandler,
    ) {
        self.handlers
            .entry(kind)
            .or_default()
            .push((extension.into(), handler));
    }
    #[must_use]
    pub fn has_handlers(&self, kind: &ExtensionEventKind) -> bool {
        self.handlers
            .get(kind)
            .is_some_and(|handlers| !handlers.is_empty())
    }

    /// Every handler runs in registration order; errors are reported and do not stop siblings.
    pub fn emit(&mut self, event: ExtensionEvent) -> Vec<Value> {
        let Some(handlers) = self.handlers.get(&event.kind).cloned() else {
            return Vec::new();
        };
        let mut values = Vec::new();
        for (extension, handler) in handlers {
            match handler(&event, &mut self.context) {
                Ok(Some(value)) => values.push(value),
                Ok(None) => {}
                Err(mut error) => {
                    if error.source.is_none() {
                        error.message = format!("{extension}: {}", error.message);
                    }
                    self.report_error(error);
                }
            }
        }
        values
    }

    /// Input transformations compose in order. `consumed` prevents later handlers from seeing it.
    pub fn emit_input(&mut self, input: InputEvent) -> InputEventResult {
        let event = ExtensionEvent {
            kind: ExtensionEventKind::Input,
            data: match input {
                InputEvent::Key(key) => serde_json::json!({"source":"interactive","key":key}),
                InputEvent::Text(text) => serde_json::json!({"source":"interactive","text":text}),
            },
        };
        let handlers = self
            .handlers
            .get(&ExtensionEventKind::Input)
            .cloned()
            .unwrap_or_default();
        let mut result = InputEventResult::default();
        for (extension, handler) in handlers {
            match handler(&event, &mut self.context) {
                Ok(Some(value)) => {
                    if let Some(replacement) = value.get("replacement").and_then(Value::as_str) {
                        result.replacement = Some(replacement.into());
                    }
                    if value
                        .get("consume")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    {
                        result.consumed = true;
                        break;
                    }
                }
                Ok(None) => {}
                Err(mut error) => {
                    if error.source.is_none() {
                        error.message = format!("{extension}: {}", error.message);
                    }
                    self.report_error(error);
                }
            }
        }
        result
    }

    pub fn invoke_tool(&mut self, name: &str, arguments: Value) -> Result<Value, ExtensionError> {
        let handler: ToolHandler = self
            .runtime
            .registered_tools
            .iter()
            .find(|tool| tool.info.name == name)
            .map(|tool| Arc::clone(&tool.handler))
            .ok_or_else(|| ExtensionError {
                message: format!("unknown extension tool: {name}"),
                source: None,
            })?;
        self.context.assert_active()?;
        handler(arguments, &mut self.context)
    }
    pub fn invoke_command(
        &mut self,
        name: &str,
        args: &[String],
    ) -> Result<SessionActionResult, ExtensionError> {
        let handler: CommandHandler = self
            .runtime
            .registered_commands
            .iter()
            .find(|command| command.info.name == name)
            .map(|command| Arc::clone(&command.handler))
            .ok_or_else(|| ExtensionError {
                message: format!("unknown extension command: {name}"),
                source: None,
            })?;
        self.context.assert_active()?;
        handler(args, &mut self.context)
    }

    /// Shutdown is idempotent and makes all subsequent calls stale.
    pub fn shutdown(&mut self, reason: impl Into<String>) {
        if self.shutdown {
            return;
        }
        self.shutdown = true;
        self.emit(ExtensionEvent {
            kind: ExtensionEventKind::SessionShutdown,
            data: serde_json::json!({"reason": reason.into()}),
        });
        self.context.stale = true;
        self.context.generation += 1;
    }
    pub fn invalidate_context(&mut self) {
        self.context.stale = true;
        self.context.generation += 1;
    }
    #[must_use]
    pub fn context(&self) -> &ExtensionContext {
        &self.context
    }
}
