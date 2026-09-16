//! Observation around an ADK node. It delegates execution exactly once and preserves its output.
use crate::event_sink::{EventSink, TerminalPermit, TerminalSlot};
use adk_graph::{Node, NodeContext, NodeOutput, StateSchema, error::Result};
use async_trait::async_trait;
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

struct ObservedNode {
    inner: Arc<dyn Node>,
    sender: EventSink,
    label: String,
    kind: String,
    config: Value,
    path: String,
    store: Option<zf_storage::content_store::ContentStore>,
    terminal: TerminalSlot,
    preparation: bool,
}
pub fn wrap(
    inner: Arc<dyn Node>,
    sender: EventSink,
    label: String,
    kind: String,
    config: Value,
    path: String,
    store: Option<zf_storage::content_store::ContentStore>,
) -> Arc<dyn Node> {
    let terminal = sender.terminal_slot();
    Arc::new(ObservedNode {
        terminal,
        inner,
        sender,
        label,
        kind,
        config,
        path,
        store,
        preparation: false,
    })
}
/// Cover revision selection before the selected node's observer starts. The
/// guard is handed to that observer, so preparation and execution are one
/// attempt. A structural boundary that executes no node emits no activity.
pub fn wrap_preparation(
    inner: Arc<dyn Node>,
    sender: EventSink,
    label: String,
    kind: String,
    path: String,
) -> Arc<dyn Node> {
    Arc::new(ObservedNode {
        terminal: sender.terminal_slot(),
        inner,
        sender,
        label,
        kind,
        config: Value::Null,
        path,
        store: None,
        preparation: true,
    })
}
tokio::task_local! {
    static PREPARATION: Arc<Mutex<Option<CompletionGuard>>>;
}
// A timeout or cancellation drops the node future before it returns a result.
struct CompletionGuard {
    terminal: Option<TerminalPermit>,
    event: Option<Value>,
    start: Instant,
}
impl CompletionGuard {
    fn fail(&mut self, error: &str) {
        if let Some(mut event) = self.event.take() {
            event["status"] = json!("error");
            event["error"] = json!(error);
            event["endedAt"] = json!(now());
            event["durationMs"] = json!(self.start.elapsed().as_millis() as u64);
            if let Some(terminal) = self.terminal.take() {
                terminal.send(event);
            }
        }
    }
}
impl Drop for CompletionGuard {
    fn drop(&mut self) {
        if let Some(mut event) = self.event.take() {
            event["status"] = json!("interrupted");
            event["error"] =
                json!("Invocation interrompue avant son retour (annulation ou délai dépassé)");
            event["endedAt"] = json!(now());
            event["durationMs"] = json!(self.start.elapsed().as_millis() as u64);
            if let Some(terminal) = self.terminal.take() {
                terminal.send(event);
            }
        }
    }
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}
impl ObservedNode {
    fn begin(&self, ctx: &NodeContext) -> Result<CompletionGuard> {
        let start = Instant::now();
        let mut event = json!({"type":"node_activity","occurrenceId":Uuid::new_v4().to_string(),"node":self.name(),"path":self.path,"nodePath":self.path,"label":self.label,"kind":self.kind,"step":ctx.step,"thread":ctx.config.thread_id,"status":"running","phase":"preparing","startedAt":now()});
        let terminal = match self.terminal.try_reserve() {
            Ok(terminal) => Some(terminal),
            Err(tokio::sync::TryAcquireError::Closed) => None,
            Err(tokio::sync::TryAcquireError::NoPermits) => {
                let message = "Observation admission saturated: previous terminal is still pending; invocation rejected before execution";
                event["status"] = json!("error");
                event["phase"] = json!("admission");
                event["error"] = json!(message);
                event["endedAt"] = json!(now());
                event["durationMs"] = json!(start.elapsed().as_millis() as u64);
                self.terminal.reject(event);
                return Err(adk_graph::error::GraphError::NodeExecutionFailed {
                    node: self.name().into(),
                    message: message.into(),
                });
            }
        };
        Ok(CompletionGuard {
            terminal,
            event: Some(event),
            start,
        })
    }
    async fn prepare(&self, ctx: &NodeContext) -> Result<NodeOutput> {
        let guard = Arc::new(Mutex::new(Some(self.begin(ctx)?)));
        let result = PREPARATION
            .scope(guard.clone(), self.inner.execute(ctx))
            .await;
        let guard = guard.lock().unwrap_or_else(|p| p.into_inner()).take();
        if let Some(mut guard) = guard {
            // The selected observer takes ownership before doing any I/O. If
            // none was entered, this is a preparation error or an internal
            // graph boundary, not execution of the baseline node.
            if let Err(error) = &result {
                guard.fail(&error.to_string());
            } else {
                guard.event = None;
            }
        }
        result
    }
}
#[async_trait]
impl Node for ObservedNode {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn validate(&self) -> Result<()> {
        self.inner.validate()
    }
    fn validate_against(&self, parent: &StateSchema) -> Result<()> {
        self.inner.validate_against(parent)
    }
    async fn execute(&self, ctx: &NodeContext) -> Result<NodeOutput> {
        if self.preparation {
            return self.prepare(ctx).await;
        }
        let prepared = PREPARATION
            .try_with(|guard| {
                let mut guard = guard.lock().unwrap_or_else(|p| p.into_inner());
                if guard.as_ref().is_some_and(|guard| {
                    guard
                        .event
                        .as_ref()
                        .is_some_and(|event| event["path"] == self.path)
                }) {
                    guard.take()
                } else {
                    None
                }
            })
            .ok()
            .flatten();
        let mut completion = match prepared {
            Some(guard) => guard,
            None => self.begin(ctx)?,
        };
        let start = completion.start;
        let mut event = completion
            .event
            .as_ref()
            .expect("new attempt event")
            .clone();
        let id = event["occurrenceId"]
            .as_str()
            .expect("attempt identity")
            .to_owned();
        let input = if self.kind == "tool"
            && ["execute_calls", "execute_next_call"]
                .contains(&self.config["tool"].as_str().unwrap_or(""))
        {
            let calls = ctx
                .state
                .get(
                    self.config["toolCallsField"]
                        .as_str()
                        .unwrap_or("toolCalls"),
                )
                .cloned()
                .unwrap_or(json!([]));
            if self.config["tool"] == "execute_next_call" {
                calls
                    .as_array()
                    .and_then(|calls| calls.first())
                    .cloned()
                    .unwrap_or(Value::Null)
            } else {
                calls
            }
        } else if self.kind == "tool" {
            self.config
                .get("inputField")
                .and_then(Value::as_str)
                .and_then(|field| ctx.state.get(field))
                .cloned()
                .unwrap_or_else(|| self.config.get("arguments").cloned().unwrap_or(json!({})))
        } else if matches!(self.kind.as_str(), "agent" | "model") {
            ctx.state
                .get(self.config["inputField"].as_str().unwrap_or("input"))
                .cloned()
                .unwrap_or(Value::Null)
        } else {
            json!(ctx.state)
        };
        event["label"] = json!(self.label);
        event["kind"] = json!(self.kind);
        event["input"] = input;
        event["ui"] = json!(self.config.get("ui"));
        event["tool"] = json!(self.config.get("tool"));
        if let Some(revision) = crate::revisions::current_revision() {
            event["flowRevision"] = revision;
        }
        // A timeout can occur during snapshot storage as well as during the
        // node itself. Keep a complete fallback before the first CAS await.
        completion.event = Some(event.clone());
        if let Some(store) = &self.store {
            let exact_state = json!(ctx.state);
            let state_ref = store.intern(&exact_state).await.map_err(|e| {
                let error = adk_graph::error::GraphError::NodeExecutionFailed {
                    node: self.name().into(),
                    message: e.to_string(),
                };
                completion.fail(&error.to_string());
                error
            })?;
            event["stateRef"] = json!(state_ref);
            let input_ref = if event["input"] == exact_state {
                state_ref
            } else {
                store.intern(&event["input"]).await.map_err(|e| {
                    let error = adk_graph::error::GraphError::NodeExecutionFailed {
                        node: self.name().into(),
                        message: e.to_string(),
                    };
                    completion.fail(&error.to_string());
                    error
                })?
            };
            event.as_object_mut().expect("event object").remove("input");
            event["inputRef"] = json!(input_ref);
        }
        completion.event = Some(event.clone());
        // Observation must never replace a node's result with a disconnected-client error.
        let _ = self.sender.send(event.clone()).await;
        event["phase"] = json!("executing");
        completion.event = Some(event.clone());
        let result = crate::runtime::CURRENT_OCCURRENCE
            .scope((self.path.clone(), id), self.inner.execute(ctx))
            .await;
        event["durationMs"] = json!(start.elapsed().as_millis() as u64);
        event["endedAt"] = json!(now());
        match &result {
            Ok(output) => {
                event["status"] = json!(if output.interrupt.is_some() {
                    "waiting"
                } else {
                    "completed"
                });
                event["output"] = json!(output.updates);
                // Text accompanying tool requests is conversational progress.
                // Its identity is this model occurrence, independent of the
                // later final-output occurrence. Historical agent nodes retain
                // their explicit output-node publication semantics.
                let field = self.config["field"].as_str().unwrap_or("output");
                if self.kind == "model"
                    && output.interrupt.is_none()
                    && output.updates.get("hasToolCalls") == Some(&Value::Bool(true))
                    && output
                        .updates
                        .get(field)
                        .and_then(Value::as_str)
                        .is_some_and(|text| !text.is_empty())
                {
                    event["messageField"] = json!(field);
                }
                if let Some(interrupt) = &output.interrupt {
                    event["interrupt"] = serde_json::to_value(interrupt).unwrap_or(Value::Null);
                }
            }
            Err(error) => {
                event["status"] = json!("error");
                event["error"] = json!(error.to_string());
            }
        }
        if let Some(store) = &self.store
            && let Some(output) = event.get("output").cloned()
        {
            completion.event = Some(event.clone());
            match store.intern(&output).await {
                Ok(reference) => {
                    event["outputRef"] = json!(reference);
                    let mut visible = json!({});
                    for key in ["response", "output", "__zedflow:consumedMessages"] {
                        if let Some(value) = output.get(key) {
                            visible[key] = value.clone();
                        }
                    }
                    if let Some(field) = event["messageField"].as_str()
                        && let Some(value) = output.get(field)
                    {
                        visible[field] = value.clone();
                    }
                    event["output"] = visible;
                }
                Err(error) => {
                    let error = adk_graph::error::GraphError::NodeExecutionFailed {
                        node: self.name().into(),
                        message: error.to_string(),
                    };
                    completion.fail(&error.to_string());
                    return Err(error);
                }
            }
        }
        // Keep the cancellation guard until the terminal event has entered the bounded queue.
        let _ = self.sender.send(event).await;
        completion.event = None;
        result
    }
}
