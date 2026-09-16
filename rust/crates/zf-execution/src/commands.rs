//! Origin-independent command admission and durable execution transitions.
use adk_graph::State;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;
use zf_flows::schema::Composition;
use zf_runtime::workspace_context::ContextSnapshot;
use zf_storage::workspaces::Workspace;

use crate::service::{ExecutionContext, ExecutionService, launch};
use crate::sessions::record_loaded_skill;

/// Identity supplied by an authenticated adapter, never decoded from a command body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Actor {
    pub id: String,
    pub workspace_id: String,
}

/// Operation checked by the service's explicitly configured authorizer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandKind {
    Read,
    Start,
    Answer,
    SelectModel,
    ActivateCapability,
    Queue,
    RemoveMessage,
    Cancel,
    Resume,
    Rename,
    Authoring,
    PatchContextWindow,
    SelectContextWindow,
    ExportSessions,
    ImportSessions,
    OpenWorkspace,
    UpdateWorkspace,
}

/// Required policy boundary shared by every command origin.
#[async_trait]
pub trait CommandAuthorizer: Send + Sync {
    async fn authorize(
        &self,
        actor: &Actor,
        command: CommandKind,
        workspace: &Workspace,
        run: Option<&Value>,
    ) -> anyhow::Result<()>;
}

/// Domain failures adapters can classify without importing a transport protocol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionError {
    Busy(String),
    Forbidden(String),
    Conflict(String),
    Invalid(String),
    NotFound(String),
}
impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy(message)
            | Self::Forbidden(message)
            | Self::Conflict(message)
            | Self::Invalid(message)
            | Self::NotFound(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for ExecutionError {}

/// Attach the admitted caller to the command event before its atomic persistence.
pub(crate) async fn persist_actor_command(
    b: &ExecutionContext,
    id: &str,
    run: &Value,
    event: &Value,
) -> anyhow::Result<()> {
    let mut event = event.clone();
    event["actor"] = json!({"id": b.actor.id, "workspaceId": b.actor.workspace_id});
    crate::sessions::persist_command(b, id, run, &event).await?;
    if let Some(service) = b.services.lock().await.get(id) {
        service.replace_queue(run["queue"].as_array().cloned().unwrap_or_default());
        if let Some(bindings) = run["modelBindings"].as_object() {
            for (path, selection) in bindings {
                service.set_binding(path.clone(), selection.clone());
            }
        }
    }
    Ok(())
}

/// A session name change; whitespace and length are normalized by the service.
#[derive(Deserialize)]
pub struct RenameRun {
    pub name: String,
}
pub(crate) async fn command_load(b: &ExecutionContext, id: &str) -> anyhow::Result<Value> {
    let (run, _) = b.sync.latest(id).await?;
    zf_storage::session_store::hydrate_command(&b.content, &run).await
}
pub(crate) async fn definition_run(b: &ExecutionContext, id: &str) -> anyhow::Result<Value> {
    let (mut run, _) = b.sync.latest(id).await?;
    for name in [
        "composition",
        "flowSource",
        "flowPackage",
        "runtimeGraph",
        "context",
    ] {
        if let Some(reference) = run[format!("{name}Ref")].as_str() {
            run[name] = b.content.resolve(reference).await?;
        }
    }
    Ok(run)
}
pub(crate) async fn command_ack(b: &ExecutionContext, id: &str) -> anyhow::Result<Value> {
    let (workspace, revision) = b.sync.head(id).await?;
    Ok(json!({"id":id,"workspaceId":workspace,"revision":revision}))
}

impl ExecutionService {
    /// Rename a session after admission and commit its command event.
    pub async fn rename(
        &self,
        actor: &Actor,
        id: &str,
        request: RenameRun,
    ) -> anyhow::Result<Value> {
        let b = self.admit(actor, CommandKind::Rename, Some(id)).await?;
        let _guard = b.writer.lock().await;
        if request.name.trim().is_empty() {
            return Err(ExecutionError::Invalid("Le nom de session est requis".into()).into());
        }
        let mut run = command_load(&b, id).await?;
        run["name"] = json!(request.name.trim().chars().take(160).collect::<String>());
        persist_actor_command(&b, id, &run, &json!({"type":"session_renamed"})).await?;
        command_ack(&b, id).await
    }
}

pub(crate) fn expand_run_skill(
    run: &mut Value,
    text: &str,
    node_path: Option<&str>,
) -> anyhow::Result<String> {
    let context: ContextSnapshot = serde_json::from_value(run["context"].clone())?;
    if run["composition"]["formatVersion"].as_u64().unwrap_or(1) >= 2 {
        if let Some(invocation) = text.strip_prefix("/skill:") {
            let name = invocation
                .split_whitespace()
                .next()
                .ok_or_else(|| anyhow::anyhow!("Nom de skill absent"))?;
            let path = node_path
                .ok_or_else(|| anyhow::anyhow!("Choisissez l’agent destinataire de ce skill"))?;
            anyhow::ensure!(
                matches!(run_node_kind(run, path), Some("agent" | "model")),
                "Agent cible introuvable"
            );
            let config = run_context_config(run, path)
                .ok_or_else(|| anyhow::anyhow!("Configuration d’agent absente"))?;
            let key = zf_runtime::agent_capabilities::skill_activation_key(config, &context, name)?;
            update_activation(run, path, &key, true)?;
        }
        return Ok(text.into());
    }
    let (expanded, loaded) = context.expand_skill_with_metadata(text)?;
    if let Some(loaded) = loaded {
        record_loaded_skill(run, loaded);
    }
    Ok(expanded)
}

pub(crate) fn node_config<'a>(composition: &'a Value, path: &str) -> Option<&'a Value> {
    let (first, rest) = path
        .split_once('/')
        .map_or((path, None), |(a, b)| (a, Some(b)));
    let node = composition["nodes"]
        .as_array()?
        .iter()
        .find(|n| n["id"] == first)?;
    match rest {
        Some(rest) => node_config(&node["data"]["config"]["composition"], rest),
        None => Some(&node["data"]["config"]),
    }
}
pub(crate) fn node_kind<'a>(composition: &'a Value, path: &str) -> Option<&'a str> {
    let (first, rest) = path
        .split_once('/')
        .map_or((path, None), |(a, b)| (a, Some(b)));
    let node = composition["nodes"]
        .as_array()?
        .iter()
        .find(|n| n["id"] == first)?;
    match rest {
        Some(rest) => node_kind(&node["data"]["config"]["composition"], rest),
        None => node["data"]["kind"].as_str(),
    }
}
pub(crate) fn run_node_config<'a>(run: &'a Value, path: &str) -> Option<&'a Value> {
    if let Some(flows) = run["runtimeGraph"]["flows"].as_object()
        && let Some((instance, flow)) = flows
            .iter()
            .filter(|(instance, _)| path.starts_with(&format!("{instance}/")))
            .max_by_key(|(instance, _)| instance.len())
    {
        return node_config(
            &flow["composition"],
            path.strip_prefix(&format!("{instance}/"))?,
        );
    }
    node_config(&run["composition"], path)
}
pub(crate) fn run_context_config<'a>(run: &'a Value, path: &str) -> Option<&'a Value> {
    let config = run_node_config(run, path)?;
    if let Some(context_node) = config["contextNode"].as_str() {
        let prefix = path
            .rsplit_once('/')
            .map(|(prefix, _)| format!("{prefix}/"))
            .unwrap_or_default();
        run_node_config(run, &format!("{prefix}{context_node}"))
    } else {
        Some(config)
    }
}
pub(crate) fn run_node_kind<'a>(run: &'a Value, path: &str) -> Option<&'a str> {
    if let Some(flows) = run["runtimeGraph"]["flows"].as_object()
        && let Some((instance, flow)) = flows
            .iter()
            .filter(|(instance, _)| path.starts_with(&format!("{instance}/")))
            .max_by_key(|(instance, _)| instance.len())
    {
        return node_kind(
            &flow["composition"],
            path.strip_prefix(&format!("{instance}/"))?,
        );
    }
    node_kind(&run["composition"], path)
}
pub(crate) fn validate_run_bindings(run: &Value, bindings: &Value) -> anyhow::Result<()> {
    let values = bindings
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Les modèles doivent être indexés par nœud"))?;
    for (path, selection) in values {
        anyhow::ensure!(
            matches!(run_node_kind(run, path), Some("agent" | "model"))
                && run_node_config(run, path).is_some_and(|c| c["modelBinding"] == "runtime"),
            "Ce nœud ne propose pas de modèle configurable : {path}"
        );
        zf_runtime::model_catalog::validate_selection(selection)?;
    }
    Ok(())
}
pub(crate) fn validate_bindings(doc: &Composition, bindings: &Value) -> anyhow::Result<()> {
    let document = serde_json::to_value(doc)?;
    let values = bindings
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Les modèles doivent être indexés par nœud"))?;
    for (path, selection) in values {
        anyhow::ensure!(
            matches!(node_kind(&document, path), Some("agent" | "model"))
                && node_config(&document, path).is_some_and(|c| c["modelBinding"] == "runtime"),
            "Ce nœud ne propose pas de modèle configurable : {path}"
        );
        zf_runtime::model_catalog::validate_selection(selection)?;
    }
    Ok(())
}
/// A response addressed to one currently open wait.
#[derive(Deserialize)]
pub struct Answer {
    #[serde(rename = "waitId")]
    pub wait_id: String,
    pub value: Value,
    #[serde(default, rename = "nodePath")]
    pub node_path: Option<String>,
}
impl ExecutionService {
    /// Resolve the exact open wait and durably claim its continuation.
    pub async fn answer(&self, actor: &Actor, id: &str, answer: Answer) -> anyhow::Result<Value> {
        let b = self.admit(actor, CommandKind::Answer, Some(id)).await?;
        let _guard = b.writer.lock().await;
        let mut run = command_load(&b, id).await?;
        if run["status"] != "waiting" || run["wait"]["id"] != answer.wait_id {
            return Err(ExecutionError::Conflict("Cette attente n'est plus ouverte".into()).into());
        }
        let path = run["wait"]["nodePath"]
            .as_str()
            .or_else(|| run["wait"]["node"].as_str())
            .ok_or_else(|| anyhow::anyhow!("Origine de l’attente absente"))?
            .to_owned();
        let mut input = State::new();
        if run["wait"]["kind"] == "model_selection" {
            validate_run_bindings(&run, &json!({path.clone():answer.value.clone()}))?;
            set_run_binding(&mut run, &path, answer.value);
        } else {
            if !matches!(run["wait"]["kind"].as_str(), Some("input" | "inbox")) {
                return Err(ExecutionError::Conflict("Cette attente requiert une ressource ou une route, pas une réponse utilisateur"
                    .into(),
            ).into());
            }
            let response_type = run["wait"]["config"]["responseType"]
                .as_str()
                .unwrap_or("text");
            let valid = match response_type {
                "confirmation" => answer.value.is_boolean(),
                "text" => answer.value.as_str().is_some_and(|s| !s.trim().is_empty()),
                _ => false,
            };
            if !valid {
                return Err(ExecutionError::Invalid(
                    "Réponse incompatible avec cette attente".into(),
                )
                .into());
            }
            let visible_text = answer
                .value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| answer.value.to_string());
            let value = if let Some(text) = answer.value.as_str() {
                json!(expand_run_skill(
                    &mut run,
                    text,
                    answer.node_path.as_deref()
                )?)
            } else {
                answer.value
            };
            // The root graph's ADK channels are local even when observation paths
            // are qualified. Routed child paths remain qualified for their host.
            let answer_path = if run
                .get("runtimeGraph")
                .is_some_and(|value| !value.is_null())
            {
                path.strip_prefix("root/").unwrap_or(&path)
            } else {
                &path
            };
            input.insert(
                format!("answer:{answer_path}"),
                json!({"__zedflowAnswerId":answer.wait_id,"value":value}),
            );
            run["messages"]
                .as_array_mut()
                .ok_or_else(|| anyhow::anyhow!("Messages invalides"))?
                .push(json!({"id":answer.wait_id,"role":"user","text":visible_text}));
        }
        claim_resume(&b, id, &mut run, input).await?;
        command_ack(&b, id).await
    }
}

fn set_run_binding(run: &mut Value, path: &str, selection: Value) {
    if !run["modelBindings"].is_object() {
        run["modelBindings"] = json!({});
    }
    run["modelBindings"][path] = selection.clone();
    run["modelRevision"] = json!(run["modelRevision"].as_u64().unwrap_or(0) + 1);
}

pub(crate) async fn claim_resume(
    b: &ExecutionContext,
    id: &str,
    run: &mut Value,
    input: State,
) -> anyhow::Result<()> {
    claim_resume_with_event(
        b,
        id,
        run,
        input,
        &json!({"type":"run_status","status":"running"}),
    )
    .await
}

async fn claim_resume_with_event(
    b: &ExecutionContext,
    id: &str,
    run: &mut Value,
    input: State,
    event: &Value,
) -> anyhow::Result<()> {
    zf_storage::session_archive::ensure_resume_allowed(run)?;
    let recreate_services = matches!(
        run["status"].as_str(),
        Some("stopped" | "interrupted" | "paused")
    );
    let checkpoint = run["checkpoint"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Checkpoint absent"))?
        .to_owned();
    if let Some(activities) = run["activities"].as_array_mut() {
        for activity in activities.iter_mut().filter(|a| a["status"] == "waiting") {
            activity["status"] = json!("resumed");
        }
    }
    run["resumeInput"] = json!(input);
    run["resumeCheckpoint"] = json!(checkpoint);
    run["status"] = json!("running");
    run["wait"] = Value::Null;
    run["error"] = Value::Null;
    run["activeNode"] = Value::Null;
    run["abortRequested"] = json!(false);
    run["resumeClaimId"] = json!(Uuid::new_v4().to_string());
    persist_actor_command(b, id, run, event).await?;
    if recreate_services {
        // A failed claim must not discard the previous runtime's token or claims.
        b.services.lock().await.remove(id);
    }
    launch(
        b.clone(),
        id.into(),
        input,
        Some(checkpoint),
        run["resumeClaimId"].as_str().map(str::to_owned),
    );
    Ok(())
}

/// An optimistic update to a node model binding.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectModel {
    pub node_path: String,
    pub selection: Value,
    pub revision: u64,
}
impl ExecutionService {
    /// Update a binding at the expected revision and resolve a matching model wait.
    pub async fn select_model(
        &self,
        actor: &Actor,
        id: &str,
        request: SelectModel,
    ) -> anyhow::Result<Value> {
        let b = self
            .admit(actor, CommandKind::SelectModel, Some(id))
            .await?;
        let _guard = b.writer.lock().await;
        let mut run = command_load(&b, id).await?;
        if run["modelRevision"].as_u64().unwrap_or(0) != request.revision {
            return Err(ExecutionError::Conflict(
                "Les modèles ont été modifiés dans une autre interface".into(),
            )
            .into());
        }
        validate_run_bindings(
            &run,
            &json!({request.node_path.clone():request.selection.clone()}),
        )?;
        set_run_binding(&mut run, &request.node_path, request.selection);
        let event = json!({"type":"model_binding_changed","nodePath":request.node_path,"revision":run["modelRevision"]});
        // Resolving the currently open selection through the panel is equivalent to
        // submitting its card. Other bindings never resume an unrelated wait.
        if run["status"] == "waiting"
            && run["wait"]["kind"] == "model_selection"
            && run["wait"]["nodePath"] == request.node_path
        {
            claim_resume_with_event(&b, id, &mut run, State::new(), &event).await?;
        } else {
            persist_actor_command(&b, id, &run, &event).await?;
        }
        command_ack(&b, id).await
    }
}

/// An idempotent steering or follow-up message for a flow inbox.
#[derive(Deserialize)]
pub struct MessageRequest {
    pub id: String,
    pub kind: String,
    pub text: String,
    #[serde(default, rename = "nodePath")]
    pub node_path: Option<String>,
}

/// An explicit capability activation on an agent node.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivateCapability {
    pub node_path: String,
    pub item_id: String,
    #[serde(default)]
    pub skill_name: Option<String>,
    #[serde(default = "enabled")]
    pub active: bool,
}
fn enabled() -> bool {
    true
}

fn update_activation(run: &mut Value, path: &str, key: &str, active: bool) -> anyhow::Result<()> {
    if !run["capabilityActivations"].is_object() {
        run["capabilityActivations"] = json!({});
    }
    let mut ids: Vec<String> = run["capabilityActivations"]
        .get(path)
        .cloned()
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default();
    ids.retain(|id| id != key);
    if active {
        ids.push(key.into());
    }
    run["capabilityActivations"][path] = json!(ids);
    Ok(())
}

impl ExecutionService {
    /// Change the active capability set of the addressed agent.
    pub async fn activate_capability(
        &self,
        actor: &Actor,
        id: &str,
        request: ActivateCapability,
    ) -> anyhow::Result<Value> {
        let b = self
            .admit(actor, CommandKind::ActivateCapability, Some(id))
            .await?;
        let _writer = b.writer.lock().await;
        let mut run = command_load(&b, id).await?;
        if run["composition"]["formatVersion"].as_u64().unwrap_or(1) < 2
            || !matches!(
                run_node_kind(&run, &request.node_path),
                Some("agent" | "model")
            )
        {
            return Err(ExecutionError::Invalid(
                "Les pièces explicites nécessitent un agent de flow v2".into(),
            )
            .into());
        }
        let context: ContextSnapshot = serde_json::from_value(run["context"].clone())?;
        let config = run_context_config(&run, &request.node_path)
            .ok_or_else(|| anyhow::anyhow!("Configuration du contexte absente"))?;
        let key = zf_runtime::agent_capabilities::activation_key(
            config,
            &context,
            &request.item_id,
            request.skill_name.as_deref(),
        )?;
        update_activation(&mut run, &request.node_path, &key, request.active)?;
        persist_actor_command(&b,id,&run,&json!({"type":"capability_activation","nodePath":request.node_path,"itemId":request.item_id,"key":key,"active":request.active})).await?;
        command_ack(&b, id).await
    }
}
impl ExecutionService {
    /// Queue one idempotent message and wake a matching inbox wait.
    pub async fn queue(
        &self,
        actor: &Actor,
        id: &str,
        request: MessageRequest,
    ) -> anyhow::Result<Value> {
        let b = self.admit(actor, CommandKind::Queue, Some(id)).await?;
        if request.id.trim().is_empty() || request.id.len() > 200 {
            return Err(ExecutionError::Invalid("Identifiant de message invalide".into()).into());
        }
        if !["steering", "followup"].contains(&request.kind.as_str())
            || request.text.trim().is_empty()
        {
            return Err(ExecutionError::Invalid("Message invalide".into()).into());
        }
        let _guard = b.writer.lock().await;
        let mut run = command_load(&b, id).await?;
        if ["completed", "error"].contains(&run["status"].as_str().unwrap_or_default()) {
            return Err(ExecutionError::Conflict(
                "Cette exécution ne peut pas recevoir de message en file".into(),
            )
            .into());
        }
        if !has_control(&run["composition"], "inbox") {
            return Err(ExecutionError::Invalid(
                "Ce flow ne possède pas de nœud de réception des messages".into(),
            )
            .into());
        }
        if !run["queue"].is_array() {
            run["queue"] = json!([]);
        }
        if let Some(existing) = run["queue"]
            .as_array()
            .and_then(|q| q.iter().find(|m| m["id"] == request.id))
        {
            if existing["kind"] != request.kind || existing["originalText"] != request.text {
                return Err(ExecutionError::Conflict(
                    "Cet identifiant désigne un autre message".into(),
                )
                .into());
            }
            return command_ack(&b, id).await;
        }
        let text = expand_run_skill(&mut run, &request.text, request.node_path.as_deref())?;
        run["queue"].as_array_mut().ok_or_else(||anyhow::anyhow!("File invalide"))?
        .push(json!({"id":request.id,"kind":request.kind,"text":text,"originalText":request.text,"status":"pending"}));
        let event = json!({"type":"message_queued","id":request.id,"kind":request.kind});
        if run["status"] == "waiting"
            && run["wait"]["kind"] != "model_selection"
            && run_node_kind(&run, run["wait"]["nodePath"].as_str().unwrap_or("")) == Some("inbox")
        {
            claim_resume_with_event(&b, id, &mut run, State::new(), &event).await?;
        } else {
            persist_actor_command(&b, id, &run, &event).await?;
        }
        command_ack(&b, id).await
    }
}
pub(crate) fn has_control(doc: &Value, kind: &str) -> bool {
    doc["nodes"].as_array().into_iter().flatten().any(|n| {
        n["data"]["kind"] == kind
            || (n["data"]["kind"] == "subgraph"
                && has_control(&n["data"]["config"]["composition"], kind))
    })
}
impl ExecutionService {
    /// Cancel a queued message while it remains unconsumed.
    pub async fn remove_message(
        &self,
        actor: &Actor,
        id: &str,
        message: &str,
    ) -> anyhow::Result<Value> {
        let b = self
            .admit(actor, CommandKind::RemoveMessage, Some(id))
            .await?;
        let _guard = b.writer.lock().await;
        let mut run = command_load(&b, id).await?;
        let queued = run["queue"]
            .as_array_mut()
            .and_then(|q| q.iter_mut().find(|m| m["id"] == message))
            .ok_or_else(|| ExecutionError::NotFound("Message introuvable".into()))?;
        if queued["status"] != "pending" {
            return Err(ExecutionError::Conflict("Ce message a déjà été traité".into()).into());
        }
        let service = b.services.lock().await.get(id).cloned();
        let cancellation = if let Some(service) = service {
            Some(
                service
                    .reserve_message_cancellation(message)
                    .await
                    .map_err(|error| ExecutionError::Conflict(error.to_string()))?,
            )
        } else {
            None
        };
        queued["status"] = json!("cancelled");
        persist_actor_command(
            &b,
            id,
            &run,
            &json!({"type":"message_cancelled","id":message}),
        )
        .await?;
        if let Some(cancellation) = cancellation {
            cancellation.commit();
        }
        command_ack(&b, id).await
    }
}
impl ExecutionService {
    /// Request cancellation and stop a suspended execution.
    pub async fn cancel(&self, actor: &Actor, id: &str) -> anyhow::Result<Value> {
        let b = self.admit(actor, CommandKind::Cancel, Some(id)).await?;
        let _guard = b.writer.lock().await;
        let mut run = command_load(&b, id).await?;
        if !["running", "waiting"].contains(&run["status"].as_str().unwrap_or_default()) {
            return command_ack(&b, id).await;
        }
        run["abortRequested"] = json!(true);
        if run["status"] == "waiting" {
            run["status"] = json!("stopped");
        }
        persist_actor_command(&b, id, &run, &json!({"type":"abort_requested"})).await?;
        if let Some(service) = b.services.lock().await.get(id) {
            service.cancel.cancel();
        }
        command_ack(&b, id).await
    }
}
/// Optional steering supplied when resuming a stopped execution.
#[derive(Deserialize, Default)]
pub struct ResumeRequest {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default, rename = "nodePath")]
    pub node_path: Option<String>,
}
impl ExecutionService {
    /// Resume an interrupted execution from its recorded checkpoint.
    pub async fn resume(
        &self,
        actor: &Actor,
        id: &str,
        request: ResumeRequest,
    ) -> anyhow::Result<Value> {
        let b = self.admit(actor, CommandKind::Resume, Some(id)).await?;
        let _guard = b.writer.lock().await;
        let mut run = command_load(&b, id).await?;
        if !["stopped", "interrupted", "paused"]
            .contains(&run["status"].as_str().unwrap_or_default())
        {
            return Err(
                ExecutionError::Conflict("Cette exécution n’est pas interrompue".into()).into(),
            );
        }
        if let Some(text) = request.text.filter(|t| !t.trim().is_empty()) {
            let original_text = text.clone();
            let text = expand_run_skill(&mut run, &text, request.node_path.as_deref())?;
            if !run["queue"].is_array() {
                run["queue"] = json!([]);
            }
            run["queue"].as_array_mut().ok_or_else(||anyhow::anyhow!("File invalide"))?
            .push(json!({"id":Uuid::new_v4().to_string(),"kind":"steering","text":text,"originalText":original_text,"status":"pending"}));
        }
        let input = run
            .get("resumeInput")
            .filter(|v| v.is_object())
            .cloned()
            .map(serde_json::from_value)
            .transpose()?
            .unwrap_or_default();
        claim_resume(&b, id, &mut run, input).await?;
        command_ack(&b, id).await
    }
}
