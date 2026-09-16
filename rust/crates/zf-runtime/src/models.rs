//! One model invocation per graph node. Graph edges own tool dispatch and loops.
use adk_core::{
    Content, GenerateContentConfig, Llm, LlmRequest, LlmResponse, LlmResponseStream, Part,
};
use adk_graph::prelude::*;
use anyhow::{Result, ensure};
use async_trait::async_trait;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use zf_compiler::graph_compiler::combined_config;

#[derive(Clone, Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Parameters {
    #[serde(skip)]
    capability_config: Option<Value>,
    context_node: Option<String>,
    #[serde(skip)]
    context_program: Option<zf_context::resources::ContextProgram>,
    #[serde(skip)]
    selection: Value,
    provider: String,
    model: String,
    instructions: String,
    global_instructions: String,
    description: String,
    input_field: String,
    field: String,
    history_field: String,
    tool_calls_field: String,
    tools: Vec<String>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    top_k: Option<i32>,
    max_output_tokens: Option<i32>,
    stop_sequences: Vec<String>,
    response_format: String,
    response_schema: Option<Value>,
    fixture_steps: Vec<Value>,
}
impl Default for Parameters {
    fn default() -> Self {
        Self {
            capability_config: None,
            context_node: None,
            context_program: None,
            selection: Value::Null,
            provider: "fixture".into(),
            model: String::new(),
            instructions: "Réponds de manière concise en français.".into(),
            global_instructions: String::new(),
            description: String::new(),
            input_field: "input".into(),
            field: "output".into(),
            history_field: "messages".into(),
            tool_calls_field: "toolCalls".into(),
            tools: vec![],
            temperature: None,
            top_p: None,
            top_k: None,
            max_output_tokens: None,
            stop_sequences: vec![],
            response_format: "text".into(),
            response_schema: None,
            fixture_steps: vec![],
        }
    }
}
fn parameters(config: &Value) -> Result<Parameters> {
    let mut p: Parameters = serde_json::from_value(config.clone())?;
    p.selection = json!({"provider":p.provider,"model":p.model,"reasoningEffort":config["reasoningEffort"],"reasoningSummary":config["reasoningSummary"],"textVerbosity":config["textVerbosity"]});
    if crate::agent_capabilities::is_v2(config) {
        p.tools = crate::agent_capabilities::tools(config)?;
        p.instructions.clear();
        p.global_instructions.clear();
        p.capability_config = Some(config.clone());
    }
    if let Some(program) = config
        .get("contextProgram")
        .filter(|value| !value.is_null())
    {
        let program = crate::inference::program(program)?;
        crate::inference::validate_grants(config, &program)?;
        p.tools = program
            .strategy
            .capabilities
            .iter()
            .map(|capability| capability.id.clone())
            .collect();
        p.context_program = Some(program);
        p.capability_config = Some(config.clone());
        p.instructions.clear();
        p.global_instructions.clear();
    }
    ensure!(
        config["thinkingBudget"].is_null(),
        "thinkingBudget n'est pas pris en charge"
    );
    if p.provider != "codex" {
        for field in ["reasoningEffort", "reasoningSummary", "textVerbosity"] {
            ensure!(
                config[field].is_null(),
                "{field} est réservé au fournisseur codex"
            );
        }
    }
    ensure!(
        ["fixture", "gemini", "codex"].contains(&p.provider.as_str()),
        "Fournisseur non disponible : {}",
        p.provider
    );
    ensure!(
        p.provider == "fixture" || !p.model.trim().is_empty(),
        "Un identifiant de modèle est requis"
    );
    ensure!(
        p.temperature
            .is_none_or(|v| v.is_finite() && (0.0..=2.0).contains(&v)),
        "temperature doit être comprise entre 0 et 2"
    );
    ensure!(
        p.top_p
            .is_none_or(|v| v.is_finite() && (0.0..=1.0).contains(&v)),
        "topP doit être compris entre 0 et 1"
    );
    ensure!(p.top_k.is_none_or(|v| v > 0), "topK doit être positif");
    ensure!(
        p.max_output_tokens.is_none_or(|v| v > 0),
        "maxOutputTokens doit être positif"
    );
    ensure!(
        ["text", "json"].contains(&p.response_format.as_str()),
        "Format attendu : text ou json"
    );
    ensure!(
        p.response_schema.as_ref().is_none_or(Value::is_object),
        "responseSchema doit être un objet JSON Schema"
    );
    ensure!(
        p.stop_sequences.len() <= 16 && p.stop_sequences.iter().all(|s| !s.is_empty()),
        "16 séquences d'arrêt non vides maximum"
    );
    for field in [
        &p.input_field,
        &p.field,
        &p.history_field,
        &p.tool_calls_field,
    ] {
        ensure!(
            !field.trim().is_empty(),
            "Les canaux de modèle doivent avoir un nom"
        );
    }
    ensure!(
        p.field != p.history_field
            && p.field != p.tool_calls_field
            && p.history_field != p.tool_calls_field,
        "Les canaux de sortie, historique et appels doivent être distincts"
    );
    let declarations = crate::operations::tool_declarations();
    for tool in &p.tools {
        ensure!(
            p.context_program.is_some() || declarations.contains_key(tool),
            "Outil non disponible : {tool}"
        );
    }
    ensure!(
        p.fixture_steps.is_empty() || p.provider == "fixture",
        "fixtureSteps est réservé au fournisseur fixture"
    );
    for step in &p.fixture_steps {
        ensure!(
            step["echoRequest"] == true
                || step["text"].is_string()
                || (step["tool"].is_string()
                    && (step["args"].is_object() || step["argsFromInput"] == true))
                || step["calls"]
                    .as_array()
                    .is_some_and(|calls| !calls.is_empty()),
            "Étape fixture attendue : text, tool et args, ou calls non vide"
        );
        let calls = step["calls"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_else(|| {
                if step["tool"].is_string() {
                    std::slice::from_ref(step)
                } else {
                    &[]
                }
            });
        for call in calls.iter().filter(|_| step["echoRequest"] != true) {
            let name = call["tool"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Nom d'outil fixture requis"))?;
            ensure!(
                call["args"].is_object() || call["argsFromInput"] == true,
                "Arguments fixture requis sous forme d'objet"
            );
            ensure!(
                p.tools.iter().any(|tool| tool == name),
                "Outil fixture non déclaré : {name}"
            );
        }
    }
    if p.provider == "codex" {
        crate::codex::validate(config)?;
        ensure!(
            p.temperature.is_none()
                && p.top_p.is_none()
                && p.top_k.is_none()
                && p.max_output_tokens.is_none()
                && p.stop_sequences.is_empty(),
            "Codex n'accepte pas ces paramètres d'échantillonnage ; retirer temperature, topP, topK, maxOutputTokens et stopSequences"
        );
    }
    Ok(p)
}
pub fn validate(config: &Value) -> Result<()> {
    ensure!(
        config["modelBinding"].is_null() || config["modelBinding"].is_string(),
        "modelBinding doit être fixed ou runtime"
    );
    let binding = config["modelBinding"].as_str().unwrap_or("fixed");
    ensure!(
        ["fixed", "runtime"].contains(&binding),
        "modelBinding doit être fixed ou runtime"
    );
    if binding == "runtime" {
        let mut draft = config.clone();
        draft["provider"] = json!("fixture");
        draft["model"] = json!("");
        for field in ["reasoningEffort", "reasoningSummary", "textVerbosity"] {
            draft
                .as_object_mut()
                .expect("runtime config is an object")
                .remove(field);
        }
        parameters(&draft).map(|_| ())
    } else {
        parameters(config).map(|_| ())
    }
}

/// Resolve only model preferences. Runtime choices cannot change a node's tools,
/// instructions, channels or graph routing.
pub fn resolve_binding(config: &Value, binding: &Value) -> Result<Value> {
    ensure!(
        binding.is_object(),
        "La sélection du modèle doit être un objet"
    );
    ensure!(
        binding["provider"].is_string(),
        "Le fournisseur du modèle est requis"
    );
    ensure!(
        binding["thinkingBudget"].is_null(),
        "thinkingBudget n'est pas pris en charge"
    );
    let mut resolved = config.clone();
    let fields = [
        "provider",
        "model",
        "reasoningEffort",
        "reasoningSummary",
        "textVerbosity",
    ];
    for field in fields {
        resolved
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("Configuration modèle invalide"))?
            .remove(field);
        if let Some(value) = binding.get(field) {
            resolved[field] = value.clone();
        }
    }
    parameters(&resolved)?;
    Ok(resolved)
}

/// A graph node whose provider is chosen at its invocation boundary. In particular,
/// a missing runtime binding pauses here rather than requiring every graph branch
/// to have a provider before any work can start.
pub struct ConfiguredModelNode {
    id: String,
    path: String,
    config: Value,
    services: Arc<crate::runtime::RunServices>,
    #[cfg(test)]
    model_factory: Option<Arc<TestModelFactory>>,
}

#[cfg(test)]
type TestModelFactory = dyn Fn(&Value) -> Arc<dyn Llm> + Send + Sync;

impl ConfiguredModelNode {
    fn invocation_model(&self, config: &Value) -> Result<ModelNode> {
        #[cfg(test)]
        if let Some(factory) = &self.model_factory {
            return Ok(ModelNode {
                id: self.id.clone(),
                model: factory(config),
                params: parameters(config)?,
            });
        }
        node(&self.id, config)
    }
}

pub fn node_with_services(
    id: &str,
    config: &Value,
    path: &str,
    services: Arc<crate::runtime::RunServices>,
) -> Result<ConfiguredModelNode> {
    validate(config)?;
    Ok(ConfiguredModelNode {
        id: id.into(),
        path: path.into(),
        config: config.clone(),
        services,
        #[cfg(test)]
        model_factory: None,
    })
}

pub fn inference_node_with_services(
    id: &str,
    config: &Value,
    context_config: &Value,
    path: &str,
    services: Arc<crate::runtime::RunServices>,
) -> Result<ConfiguredModelNode> {
    node_with_services(id, &combined_config(config, context_config), path, services)
}

pub struct ContextNode {
    id: String,
    path: String,
    model_path: String,
    config: Value,
    services: Arc<crate::runtime::RunServices>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreparedRequest {
    context_path: String,
    model_path: String,
    config_hash: String,
    selection: Value,
    snapshot: crate::agent_capabilities::EffectiveContext,
    contents: Vec<Content>,
    /// Canonical conversation before this inference, independent of projections.
    history: Vec<Content>,
    tools: HashMap<String, Value>,
}

const SELECTION_FIELDS: &[&str] = &[
    "provider",
    "model",
    "reasoningEffort",
    "reasoningSummary",
    "textVerbosity",
];
fn captured_selection(config: &Value) -> Value {
    Value::Object(
        SELECTION_FIELDS
            .iter()
            .filter_map(|key| {
                config
                    .get(key)
                    .map(|value| ((*key).to_owned(), value.clone()))
            })
            .collect(),
    )
}
fn restore_selection(config: &Value, selection: &Value) -> Value {
    let mut config = config.clone();
    if let Some(values) = config.as_object_mut() {
        for key in SELECTION_FIELDS {
            values.remove(*key);
            if let Some(value) = selection.get(key) {
                values.insert((*key).to_owned(), value.clone());
            }
        }
    }
    config
}

fn configuration_hash(config: &Value) -> Result<String> {
    use sha2::Digest;
    Ok(format!(
        "{:x}",
        sha2::Sha256::digest(serde_json::to_vec(config)?)
    ))
}

pub fn context_node_with_services(
    id: &str,
    config: &Value,
    model_config: &Value,
    path: &str,
    services: Arc<crate::runtime::RunServices>,
) -> Result<ContextNode> {
    let model_id = config["modelNode"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Nœud Modèle associé absent"))?;
    let prefix = path.rsplit_once('/').map_or("", |(prefix, _)| prefix);
    let model_path = if prefix.is_empty() {
        model_id.to_owned()
    } else {
        format!("{prefix}/{model_id}")
    };
    let config = combined_config(model_config, config);
    validate(&config)?;
    ensure!(
        config.get("contextProgram").is_some_and(|v| !v.is_null()),
        "Le programme de contexte doit être résolu avant l’exécution"
    );
    Ok(ContextNode {
        id: id.into(),
        path: path.into(),
        model_path,
        config,
        services,
    })
}

#[async_trait]
impl adk_graph::Node for ContextNode {
    fn name(&self) -> &str {
        &self.id
    }
    async fn execute(&self, ctx: &NodeContext) -> adk_graph::error::Result<NodeOutput> {
        let error = |failure: anyhow::Error| GraphError::NodeExecutionFailed {
            node: self.id.clone(),
            message: failure.to_string(),
        };
        if self.services.cancel.is_cancelled() {
            return Ok(crate::operations::stopped(&self.path));
        }
        let config = if self.config["modelBinding"] == "runtime" {
            let Some(binding) = self.services.binding(&self.model_path) else {
                return Ok(NodeOutput::interrupt_with_data(
                    "Choisissez le modèle avant de préparer son contexte",
                    json!({"kind":"model_selection","nodePath":self.model_path,"prompt":"Choisissez le modèle avant de préparer son contexte"}),
                ));
            };
            resolve_binding(&self.config, &binding).map_err(error)?
        } else {
            self.config.clone()
        };
        let program = crate::inference::program(&config["contextProgram"]).map_err(error)?;
        let prepared = tokio::select! {
            biased;
            _ = self.services.cancel.cancelled() => return Ok(crate::operations::stopped(&self.path)),
            result = crate::inference::prepare_at(&config, &program, &self.services, &self.model_path, ctx, config["provider"].as_str().unwrap_or("fixture")) => result.map_err(error)?,
        };
        if let Some(wait) = prepared.wait {
            return Ok(NodeOutput::interrupt_with_data(
                "Le contexte attend une ressource",
                wait.clone(),
            )
            .with_update("contextNeeds", wait));
        }
        if !prepared.needs.is_empty() {
            let need = json!({"kind":"context_resources","nodePath":self.path,"modelNodePath":self.model_path,"invocationId":prepared.snapshot.invocation_id,"contextSnapshotId":prepared.snapshot.invocation_id,"programHash":program.hash,"needs":prepared.needs});
            self.services
                .emit(json!({"type":"context_resources","nodePath":self.path,"request":need}))
                .await;
            return Ok(NodeOutput::interrupt_with_data(
                "Des ressources de contexte doivent être produites",
                need.clone(),
            )
            .with_update("contextNeeds", need));
        }
        let id = prepared.snapshot.invocation_id.clone();
        let history_field = config["historyField"].as_str().unwrap_or("messages");
        let canonical = prepared
            .snapshot
            .resources
            .iter()
            .find(|resource| {
                let Some(name) = resource["name"].as_str() else {
                    return false;
                };
                let binding = &config["contextProgram"]["bindings"][name];
                binding["kind"] == "conversation"
                    && binding["historyField"].as_str().unwrap_or("messages") == history_field
            })
            .map(|resource| resource["value"].clone())
            .or_else(|| ctx.state.get(history_field).cloned())
            .unwrap_or_else(|| json!([]));
        let history: Vec<Content> =
            serde_json::from_value(canonical).map_err(|e| error(e.into()))?;
        let record = PreparedRequest {
            context_path: self.path.clone(),
            model_path: self.model_path.clone(),
            config_hash: configuration_hash(&config).map_err(error)?,
            selection: captured_selection(&config),
            snapshot: prepared.snapshot,
            contents: prepared.contents,
            history,
            tools: prepared.tools,
        };
        // ADK checkpoints carry an identity only. Contents, tool schemas and
        // provenance are immutable records interned by the canonical store.
        let value = serde_json::to_value(record).map_err(|e| error(e.into()))?;
        self.services
            .persist_record("prepared-requests", &id, &value)
            .await
            .map_err(error)?;
        self.services.emit(json!({"type":"context_prepared","nodePath":self.path,"modelNodePath":self.model_path,"preparationId":id})).await;
        Ok(NodeOutput::new()
            .with_update(&format!("__zedflow:prepared:{}", self.id), json!(id))
            .with_update("contextNeeds", Value::Null))
    }
}

#[async_trait]
impl adk_graph::Node for ConfiguredModelNode {
    fn name(&self) -> &str {
        &self.id
    }

    async fn execute(&self, ctx: &NodeContext) -> adk_graph::error::Result<NodeOutput> {
        if self.services.cancel.is_cancelled() {
            return Ok(crate::operations::stopped(&self.path));
        }
        let prepared = if let Some(context_id) = self.config["contextNode"].as_str() {
            let error = |message: String| GraphError::NodeExecutionFailed {
                node: self.id.clone(),
                message,
            };
            let id = ctx
                .state
                .get(&format!("__zedflow:prepared:{context_id}"))
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    error("Le Modèle doit être précédé de son Contexte à chaque passage".into())
                })?;
            let raw = self
                .services
                .read_record("prepared-requests", id)
                .await
                .map_err(|e| error(e.to_string()))?
                .ok_or_else(|| error("Préparation du contexte introuvable".into()))?;
            Some(serde_json::from_value::<PreparedRequest>(raw).map_err(|e| error(e.to_string()))?)
        } else {
            None
        };
        let config = if let Some(prepared) = &prepared && self.config["modelBinding"] == "runtime" {
            // A selection changed during preparation applies on the next Context
            // passage. This call uses the preferences which shaped its window.
            Ok(restore_selection(&self.config,&prepared.selection))
        } else if self.config["modelBinding"] == "runtime" {
            let Some(binding) = self.services.binding(&self.path) else {
                return Ok(NodeOutput::interrupt_with_data(
                    "Choisissez un modèle pour poursuivre ce nœud",
                    json!({"kind":"model_selection","nodePath":self.path,"prompt":"Choisissez un modèle pour poursuivre ce nœud"}),
                ));
            };
            resolve_binding(&self.config, &binding)
        } else {
            Ok(self.config.clone())
        }.map_err(|error| GraphError::NodeExecutionFailed { node:self.id.clone(), message:error.to_string() })?;
        let model =
            self.invocation_model(&config)
                .map_err(|error| GraphError::NodeExecutionFailed {
                    node: self.id.clone(),
                    message: error.to_string(),
                })?;
        let selection = json!({"provider":model.params.provider,"model":model.model.name(),"reasoningEffort":config["reasoningEffort"],"reasoningSummary":config["reasoningSummary"],"textVerbosity":config["textVerbosity"]});
        self.services.emit(json!({"type":"model_selection","nodePath":self.path,"selection":selection,"step":ctx.step})).await;
        tokio::select! {
            biased;
            _ = self.services.cancel.cancelled() => Ok(crate::operations::stopped(&self.path)),
            result = model.execute_request_with_prepared(ctx, Some((&self.services, &self.path)), prepared) => {
                let mut output = result?;
                if let Some(Value::Object(metadata)) = output.updates.get_mut("modelResponse") {
                    metadata.insert("selection".into(), selection.clone());
                    if self.config["modelBinding"] == "runtime" {
                        metadata.insert("runtimeSelection".into(), selection);
                    }
                }
                Ok(output)
            },
        }
    }
}

struct Fixture {
    steps: Vec<Value>,
}
#[async_trait]
impl Llm for Fixture {
    fn name(&self) -> &str {
        "zedflow-fixture"
    }
    async fn generate_content(
        &self,
        req: LlmRequest,
        _stream: bool,
    ) -> adk_core::Result<LlmResponseStream> {
        let last = req.contents.last();
        let response = last.and_then(|c| {
            c.parts.iter().find_map(|p| {
                if let Part::FunctionResponse {
                    function_response, ..
                } = p
                {
                    Some(function_response)
                } else {
                    None
                }
            })
        });
        let input = req
            .contents
            .iter()
            .rev()
            .flat_map(|c| &c.parts)
            .find_map(|p| {
                if let Part::Text { text } = p {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .unwrap_or("");
        let completed_tools = req
            .contents
            .iter()
            .flat_map(|content| &content.parts)
            .filter(|part| matches!(part, Part::FunctionResponse { .. }))
            .count();
        let mut preceding_tools = 0;
        let scripted = self.steps.iter().find(|step| {
            if step["echoRequest"] == true || completed_tools == preceding_tools {
                return true;
            }
            preceding_tools += step["calls"]
                .as_array()
                .map_or_else(|| usize::from(step["tool"].is_string()), Vec::len);
            false
        });
        let content = if let Some(step) = scripted {
            if step["echoRequest"] == true {
                Content::new("model")
                    .with_text(json!({"contents":req.contents,"tools":req.tools}).to_string())
            } else {
                let mut content = Content::new("model");
                if let Some(text) = step["text"].as_str() {
                    content = content.with_text(text);
                }
                let calls = step["calls"]
                    .as_array()
                    .map(Vec::as_slice)
                    .unwrap_or_else(|| {
                        if step["tool"].is_string() {
                            std::slice::from_ref(step)
                        } else {
                            &[]
                        }
                    });
                for (index, call) in calls.iter().enumerate() {
                    let args = if call["argsFromInput"] == true {
                        let value: Value = serde_json::from_str(input).map_err(|e| {
                            adk_core::AdkError::model(format!(
                                "Fixture input arguments must be JSON: {e}"
                            ))
                        })?;
                        if !value.is_object() {
                            return Err(adk_core::AdkError::model(
                                "Fixture input arguments must be an object",
                            ));
                        }
                        value
                    } else {
                        call["args"].clone()
                    };
                    content.parts.push(Part::FunctionCall {
                        name: call["tool"].as_str().unwrap_or_default().into(),
                        args,
                        id: Some(if calls.len() == 1 {
                            format!("fixture-step-{completed_tools}")
                        } else {
                            format!("fixture-step-{completed_tools}-{index}")
                        }),
                        thought_signature: None,
                    });
                }
                content
            }
        } else if let Some(response) = response {
            Content::new("model").with_text(format!(
                "Outil {} exécuté par le graphe ADK.\n\n{}",
                response.name, response.response
            ))
        } else if let Some(name) = req
            .tools
            .get_key_value("read")
            .map(|(name, _)| name)
            .or_else(|| req.tools.keys().min())
        {
            let args = match name.as_str() {
                "delay" => json!({"milliseconds":1200}),
                "format_text" => json!({"text":input,"mode":"uppercase"}),
                "read" => json!({"path":input}),
                _ => json!({"value":{"input":input,"source":"ADK fixture"}}),
            };
            let mut content = Content::new("model");
            content.parts.push(Part::FunctionCall {
                name: name.clone(),
                args,
                id: Some(format!("fixture-call-{}", req.contents.len())),
                thought_signature: None,
            });
            content
        } else if req
            .config
            .as_ref()
            .is_some_and(|cfg| cfg.response_schema.is_some())
        {
            Content::new("model").with_text(json!({"input":input,"mode":"fixture"}).to_string())
        } else {
            Content::new("model").with_text(format!("Mode démonstration · modèle local déterministe\n\nEntrée reçue : {input}\n\nLe graphe ADK a exécuté ce nœud. Vous pouvez modifier ses instructions, ses connexions et les étapes suivantes dans Concevoir."))
        };
        Ok(Box::pin(futures::stream::iter([Ok(LlmResponse {
            content: Some(content),
            turn_complete: true,
            ..Default::default()
        })])))
    }
}

pub struct ModelNode {
    id: String,
    model: Arc<dyn Llm>,
    params: Parameters,
}
pub fn node(id: &str, config: &Value) -> Result<ModelNode> {
    let params = parameters(config)?;
    let model: Arc<dyn Llm> = match params.provider.as_str() {
        "fixture" => Arc::new(Fixture {
            steps: params.fixture_steps.clone(),
        }),
        "gemini" => Arc::new(adk_model::gemini::GeminiModel::new(
            std::env::var("GOOGLE_API_KEY")?,
            &params.model,
        )?),
        "codex" => crate::codex::model(config)?,
        other => anyhow::bail!("Fournisseur non disponible : {other}"),
    };
    Ok(ModelNode {
        id: id.into(),
        model,
        params,
    })
}
#[async_trait]
impl adk_graph::Node for ModelNode {
    fn name(&self) -> &str {
        &self.id
    }
    fn description(&self) -> &str {
        &self.params.description
    }
    async fn execute(&self, ctx: &NodeContext) -> adk_graph::error::Result<NodeOutput> {
        self.execute_request(ctx, None).await
    }
}

impl ModelNode {
    async fn execute_request(
        &self,
        ctx: &NodeContext,
        runtime: Option<(&Arc<crate::runtime::RunServices>, &str)>,
    ) -> adk_graph::error::Result<NodeOutput> {
        self.execute_request_with_prepared(ctx, runtime, None).await
    }
    async fn execute_request_with_prepared(
        &self,
        ctx: &NodeContext,
        runtime: Option<(&Arc<crate::runtime::RunServices>, &str)>,
        prepared_request: Option<PreparedRequest>,
    ) -> adk_graph::error::Result<NodeOutput> {
        let error = |message: String| GraphError::NodeExecutionFailed {
            node: self.id.clone(),
            message,
        };
        let p = &self.params;
        let input_marker = format!("__zedflow:input:{}", p.input_field);
        let model_marker = format!("__zedflow:model-input:{}", self.id);
        let current_turn = ctx.state.get(&input_marker).cloned().unwrap_or(Value::Null);
        let consumed_marker = format!("__zedflow:prepared-consumed:{}", self.id);
        let mut preparation_id = None;
        let (mut history, snapshot, contents, tools) = if let Some(context_id) = &p.context_node {
            let (_, path) = runtime
                .ok_or_else(|| error("Un nœud Modèle requiert les services d’exécution".into()))?;
            let id = ctx
                .state
                .get(&format!("__zedflow:prepared:{context_id}"))
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    error("Le Modèle doit être précédé de son Contexte à chaque passage".into())
                })?;
            if ctx.state.get(&consumed_marker).and_then(Value::as_str) == Some(id) {
                return Err(error(
                    "Cette préparation a déjà été consommée ; repassez par le nœud Contexte".into(),
                ));
            }
            let mut prepared =
                prepared_request.ok_or_else(|| error("Préparation du contexte absente".into()))?;
            if prepared.snapshot.invocation_id != id {
                return Err(error(
                    "L’identité de préparation ne correspond pas au checkpoint".into(),
                ));
            }
            if prepared.model_path != path {
                return Err(error(
                    "Cette préparation appartient à un autre Modèle".into(),
                ));
            }
            let config = p
                .capability_config
                .as_ref()
                .ok_or_else(|| error("Contrat de contexte absent".into()))?;
            if prepared.config_hash
                != configuration_hash(config).map_err(|e| error(e.to_string()))?
            {
                return Err(error(
                    "La configuration a changé après préparation ; repassez par le nœud Contexte"
                        .into(),
                ));
            }
            // Preparation has its own passage; tool requests originate from
            // the consuming model passage, not from the context node.
            if let Some(origin) = crate::runtime::current_origin() {
                prepared.snapshot.origin = origin;
            }
            preparation_id = Some(id.to_owned());
            (
                prepared.history,
                Some(prepared.snapshot),
                prepared.contents,
                prepared.tools,
            )
        } else if let Some(program) = &p.context_program {
            let (services, path) = runtime
                .ok_or_else(|| error("A context strategy requires runtime services".into()))?;
            let config = p
                .capability_config
                .as_ref()
                .ok_or_else(|| error("Context grants are missing".into()))?;
            let prepared =
                crate::inference::prepare_at(config, program, services, path, ctx, &p.provider)
                    .await
                    .map_err(|failure| error(failure.to_string()))?;
            if let Some(wait) = prepared.wait {
                return Ok(NodeOutput::interrupt_with_data(
                    "Un flow de préparation du contexte attend une reprise",
                    wait.clone(),
                )
                .with_update("contextNeeds", wait));
            }
            if !prepared.needs.is_empty() {
                let need = json!({"kind":"context_resources","nodePath":path,"invocationId":prepared.snapshot.invocation_id,"contextSnapshotId":prepared.snapshot.invocation_id,"programHash":program.hash,"needs":prepared.needs});
                services
                    .emit(json!({"type":"context_resources","nodePath":path,"request":need}))
                    .await;
                return Ok(NodeOutput::interrupt_with_data(
                    "Des ressources de contexte doivent être produites",
                    need.clone(),
                )
                .with_update("contextNeeds", need));
            }
            let history = prepared
                .contents
                .iter()
                .filter(|content| !matches!(content.role.as_str(), "system" | "developer"))
                .cloned()
                .collect();
            (
                history,
                Some(prepared.snapshot),
                prepared.contents,
                prepared.tools,
            )
        } else {
            let mut history: Vec<Content> = ctx
                .state
                .get(&p.history_field)
                .cloned()
                .map(serde_json::from_value)
                .transpose()
                .map_err(|e| error(format!("Historique invalide : {e}")))?
                .unwrap_or_default();
            // Tool responses already provide the next model input. Other entries start a user turn.
            let after_tool = history.last().is_some_and(|c| {
                c.parts
                    .iter()
                    .any(|part| matches!(part, Part::FunctionResponse { .. }))
            });
            let new_input =
                ctx.state.get(&model_marker) != Some(&current_turn) && !current_turn.is_null();
            if !after_tool || new_input {
                let input = ctx
                    .state
                    .get(&p.input_field)
                    .map(|v| {
                        v.as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| v.to_string())
                    })
                    .unwrap_or_default();
                history.push(Content::new("user").with_text(input));
            }
            let snapshot = if let Some(config) = &p.capability_config {
                let (services, path) = runtime.ok_or_else(|| {
                    error("Les agents v2 nécessitent les services d’exécution".into())
                })?;
                Some(
                    crate::agent_capabilities::capture(config, services, path, &ctx.state)
                        .await
                        .map_err(|e| error(e.to_string()))?,
                )
            } else {
                None
            };
            let mut contents = history.clone();
            if let Some(snapshot) = &snapshot
                && !snapshot.files.is_empty()
            {
                contents.insert(0, Content::new("user").with_text(&snapshot.files));
            }
            let authored = crate::operations::render(
                &format!("{}\n{}", p.global_instructions, p.instructions),
                &ctx.state,
            );
            // Workspace instructions and skill catalogs are literal source text, not
            // templates in the flow's state substitution language.
            let instruction = if let Some(snapshot) = &snapshot {
                snapshot.system.clone()
            } else {
                match runtime {
                    Some((services, _)) => {
                        format!("{}\n{authored}", services.system_prompt_for_tools(&p.tools))
                    }
                    None => authored,
                }
            };
            if !instruction.trim().is_empty() {
                contents.insert(0, Content::new("system").with_text(instruction));
            }
            let declarations = crate::operations::tool_declarations();
            let tools: HashMap<_, _> = p
                .tools
                .iter()
                .filter_map(|name| {
                    declarations
                        .get(name)
                        .map(|value| (name.clone(), value.clone()))
                })
                .collect();
            (history, snapshot, contents, tools)
        };
        let config = GenerateContentConfig {
            temperature: p.temperature,
            top_p: p.top_p,
            top_k: p.top_k,
            max_output_tokens: p.max_output_tokens,
            stop_sequences: p.stop_sequences.clone(),
            response_schema: p
                .response_schema
                .clone()
                .or_else(|| (p.response_format == "json").then(|| json!({"type":"object"}))),
            ..Default::default()
        };
        let req = LlmRequest {
            model: self.model.name().into(),
            contents,
            config: Some(config),
            tools,
            previous_response_id: None,
        };
        let request_manifest = if let Some((services, path)) = runtime {
            let invocation_id = snapshot
                .as_ref()
                .map(|snapshot| snapshot.invocation_id.clone())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            // LlmRequest deliberately skips tools in serde; explicitly preserve
            // the actual provider-neutral declarations without credentials.
            let mut request = serde_json::to_value(&req).map_err(|e| error(e.to_string()))?;
            request["tools"] = json!(req.tools);
            if p.provider != "codex" {
                let parts =
                    crate::inference_raw::segments(&request).map_err(|e| error(e.to_string()))?;
                let boundary = if p.provider == "fixture" {
                    "fixtureInput"
                } else {
                    "adkRequest"
                };
                crate::inference_raw::record_capture(services, &invocation_id, boundary, &parts)
                    .await
                    .map_err(|e| error(e.to_string()))?;
            }
            let mut selection = p.selection.clone();
            selection["model"] = json!(self.model.name());
            let manifest = json!({"invocationId":invocation_id,"nodePath":path,"origin":crate::runtime::current_origin(),"request":request,"selection":selection});
            let reference = services
                .persist_record("model-requests", &invocation_id, &manifest)
                .await
                .map_err(|e| error(e.to_string()))?;
            services.emit(json!({"type":"model_request","nodePath":path,"invocationId":invocation_id,"requestRef":reference})).await;
            Some((invocation_id, reference))
        } else {
            None
        };
        // Providers may stream internally, but the graph commits one completed model result.
        let generate = self.model.generate_content(req, runtime.is_some());
        let mut responses =
            if let (Some((services, _)), Some((invocation, _))) = (runtime, &request_manifest) {
                crate::inference_raw::CAPTURE
                    .scope((Arc::clone(services), invocation.clone()), generate)
                    .await
            } else {
                generate.await
            }
            .map_err(|e| error(e.to_string()))?;
        let mut completed = Vec::new();
        let mut confirmed_input = false;
        while let Some(response) = responses.next().await {
            let response = response.map_err(|e| error(e.to_string()))?;
            if !confirmed_input
                && p.provider != "codex"
                && let (Some((services, _)), Some((invocation, _))) = (runtime, &request_manifest)
            {
                crate::inference_raw::record_dispatch(services, invocation)
                    .await
                    .map_err(|e| error(e.to_string()))?;
                confirmed_input = true;
            }
            if let Some(message) = &response.error_message {
                return Err(error(message.clone()));
            }
            if response.interrupted || response.error_code.is_some() {
                return Err(error("La réponse du modèle a été interrompue".into()));
            }
            if response.finish_reason == Some(adk_core::FinishReason::MaxTokens)
                && response
                    .content
                    .iter()
                    .flat_map(|content| &content.parts)
                    .any(|part| matches!(part, Part::FunctionCall { .. }))
            {
                return Err(error(
                    "Appel d'outil tronqué par la limite de tokens".into(),
                ));
            }
            ctx.report_progress();
            if response.partial
                && let Some((services, path)) = runtime
            {
                for part in response.content.iter().flat_map(|content| &content.parts) {
                    if let Part::Text { text } = part {
                        services.emit(json!({"type":"model_delta","nodePath":path,"text":text,"step":ctx.step})).await;
                    }
                }
            }
            if !response.partial {
                completed.push(response);
            }
        }
        if completed.is_empty() {
            return Err(error(
                "Le modèle n'a produit aucune réponse complète".into(),
            ));
        }
        let mut text = String::new();
        let mut calls = Vec::new();
        let mut output = NodeOutput::new();
        let mut metadata = json!({"provider":p.provider,"model":self.model.name()});
        if let Some(id) = preparation_id {
            metadata["preparationId"] = json!(id);
            metadata["contextNode"] = json!(p.context_node);
            output = output.with_update(&consumed_marker, json!(id));
        }
        if let Some(program) = &p.context_program {
            metadata["contextProgramHash"] = json!(program.hash);
            metadata["contextProgramVersion"] = json!(program.strategy.version);
            output = output.with_update("contextNeeds", Value::Null);
        }
        if let Some((id, reference)) = request_manifest {
            metadata["invocationId"] = json!(id);
            if let Some(reference) = reference {
                metadata["requestRef"] = json!(reference);
            }
        }
        if let Some((_, path)) = runtime {
            metadata["nodePath"] = json!(path);
        }
        for result in completed {
            if let Some(usage) = result.usage_metadata {
                metadata["usage"] = json!(usage);
            }
            if let Some(extra) = result.provider_metadata {
                metadata["providerMetadata"] = extra;
            }
            if let Some(finish) = result.finish_reason {
                metadata["finishReason"] = json!(finish);
            }
            if let Some(mut content) = result.content {
                if let Some(snapshot) = snapshot.as_ref().filter(|_| p.context_program.is_some()) {
                    for (index, part) in content.parts.iter_mut().enumerate() {
                        if let Part::FunctionCall { id, .. } = part
                            && id.is_none()
                        {
                            *id = Some(format!(
                                "{}:{}:{index}",
                                snapshot.invocation_id,
                                calls.len()
                            ));
                        }
                    }
                }
                for part in &content.parts {
                    match part {
                        Part::Text { text: chunk } => text.push_str(chunk),
                        Part::Thinking { thinking, .. } => {
                            let previous = metadata["reasoningSummary"].as_str().unwrap_or("");
                            metadata["reasoningSummary"] = json!(format!("{previous}{thinking}"));
                            output = output.with_event(StreamEvent::custom(
                                &self.id,
                                "reasoning_summary",
                                json!({"summary":thinking}),
                            ));
                        }
                        Part::FunctionCall { name, args, id, .. } => {
                            ensure_tool_allowed(&p.tools, name).map_err(error)?;
                            if !args.is_object() {
                                return Err(error(format!(
                                    "Arguments incomplets ou invalides pour {name}"
                                )));
                            }
                            let call = json!({"name":name,"args":args,"id":id});
                            output = output.with_event(StreamEvent::custom(
                                &self.id,
                                "tool_requested",
                                call.clone(),
                            ));
                            calls.push(call);
                        }
                        _ if p.context_program.is_some() => {
                            return Err(error(
                                "Unsupported response modality for this model adapter".into(),
                            ));
                        }
                        _ => {}
                    }
                }
                history.push(content);
            }
        }
        if let Some(snapshot) = &snapshot
            && let Some((services, _)) = runtime
        {
            crate::agent_capabilities::seal_calls(services, snapshot, &mut calls)
                .await
                .map_err(|e| error(e.to_string()))?;
            metadata["invocationId"] = json!(snapshot.invocation_id);
            // Full context is stored once per invocation outside graph state.
            // Carrying it here duplicates it in every downstream checkpoint/activity.
            metadata["contextSnapshotId"] = json!(snapshot.invocation_id);
        }
        let value = if p.response_format == "json" && calls.is_empty() {
            serde_json::from_str(&text)
                .map_err(|e| error(format!("Réponse JSON invalide : {e}")))?
        } else {
            json!(text)
        };
        if !calls.is_empty() {
            output = output.with_update("toolResults", json!([]));
        }
        Ok(output
            .with_update(&p.field, value)
            .with_update(&p.tool_calls_field, json!(calls))
            .with_update("hasToolCalls", json!(!calls.is_empty()))
            .with_update(&p.history_field, json!(history))
            .with_update("modelResponse", metadata)
            .with_update(&model_marker, current_turn))
    }
}
fn ensure_tool_allowed(tools: &[String], name: &str) -> std::result::Result<(), String> {
    if tools.iter().any(|tool| tool == name) {
        Ok(())
    } else {
        Err(format!("Le modèle a demandé un outil non déclaré : {name}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adk_graph::Node;
    use std::sync::Mutex;
    struct RecordingModel(Arc<Mutex<Option<LlmRequest>>>);
    struct ScriptedModel(Vec<LlmResponse>);
    struct GatedModel {
        name: String,
        first: Arc<std::sync::atomic::AtomicBool>,
        started: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Semaphore>,
    }
    #[async_trait]
    impl Llm for GatedModel {
        fn name(&self) -> &str {
            &self.name
        }
        async fn generate_content(
            &self,
            _request: LlmRequest,
            _stream: bool,
        ) -> adk_core::Result<LlmResponseStream> {
            if self.first.swap(false, std::sync::atomic::Ordering::SeqCst) {
                self.started.notify_one();
                self.release.acquire().await.unwrap().forget();
            }
            Ok(Box::pin(futures::stream::iter([Ok(LlmResponse {
                content: Some(Content::new("model").with_text("finished")),
                turn_complete: true,
                ..Default::default()
            })])))
        }
    }
    #[tokio::test]
    async fn model_selection_is_snapshotted_during_an_invocation_and_reloaded_on_the_next_one() {
        let directory = tempfile::tempdir().unwrap();
        let services = crate::runtime::RunServices::new(
            "model-snapshots".into(),
            directory.path().into(),
            directory.path().join("data"),
            crate::workspace_context::ContextSnapshot::default(),
            json!({}),
            vec![],
        )
        .unwrap();
        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Semaphore::new(0));
        let first = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let selections = Arc::new(Mutex::new(Vec::new()));
        let factory: Arc<TestModelFactory> = {
            let started = started.clone();
            let release = release.clone();
            let selections = selections.clone();
            Arc::new(move |config| {
                selections.lock().unwrap().push(config.clone());
                Arc::new(GatedModel {
                    name: config["model"].as_str().unwrap().into(),
                    first: first.clone(),
                    started: started.clone(),
                    release: release.clone(),
                })
            })
        };
        let mut runtime = node_with_services(
            "model",
            &json!({"modelBinding":"runtime","provider":"fixture"}),
            "child/model",
            services.clone(),
        )
        .unwrap();
        runtime.model_factory = Some(factory.clone());
        let runtime = Arc::new(runtime);
        let context = || NodeContext::new(State::new(), ExecutionConfig::new("selection"), 0);
        assert!(
            runtime
                .execute(&context())
                .await
                .unwrap()
                .interrupt
                .is_some(),
            "embedded defaults cannot satisfy a runtime binding"
        );
        assert!(
            selections.lock().unwrap().is_empty(),
            "unreached model selection creates no provider invocation"
        );
        services.set_binding(
            "child/model".into(),
            json!({"provider":"codex","model":"before","reasoningEffort":"low"}),
        );
        let running = {
            let runtime = runtime.clone();
            tokio::spawn(async move {
                runtime
                    .execute(&NodeContext::new(
                        State::new(),
                        ExecutionConfig::new("selection"),
                        1,
                    ))
                    .await
                    .unwrap()
            })
        };
        tokio::time::timeout(std::time::Duration::from_secs(2), started.notified())
            .await
            .unwrap();
        services.set_binding(
            "child/model".into(),
            json!({"provider":"codex","model":"after","reasoningEffort":"high"}),
        );
        release.add_permits(1);
        let output = running.await.unwrap();
        assert_eq!(
            output.updates["modelResponse"]["selection"]["model"],
            "before"
        );
        assert_eq!(
            output.updates["modelResponse"]["runtimeSelection"]["reasoningEffort"],
            "low"
        );
        let next = runtime.execute(&context()).await.unwrap();
        assert_eq!(next.updates["modelResponse"]["selection"]["model"], "after");
        assert_eq!(
            next.updates["modelResponse"]["runtimeSelection"]["reasoningEffort"],
            "high"
        );
        let mut fixed=node_with_services("model",&json!({"modelBinding":"fixed","provider":"codex","model":"fixed","reasoningEffort":"medium"}),"child/model",services).unwrap();
        fixed.model_factory = Some(factory);
        let fixed = fixed.execute(&context()).await.unwrap();
        assert_eq!(
            fixed.updates["modelResponse"]["selection"]["model"],
            "fixed"
        );
        assert_eq!(
            fixed.updates["modelResponse"]["selection"]["reasoningEffort"],
            "medium"
        );
        assert!(
            fixed.updates["modelResponse"]
                .get("runtimeSelection")
                .is_none()
        );
        let selections = selections.lock().unwrap();
        assert_eq!(
            selections
                .iter()
                .map(|selection| selection["model"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["before", "after", "fixed"]
        );
    }
    #[tokio::test]
    async fn fixture_batches_advance_by_completed_calls_and_keep_call_ids_distinct() {
        let fixture = Fixture {
            steps: vec![
                json!({"text":"First I format.","tool":"format_text","args":{"text":"one"}}),
                json!({"text":"Then I read.","calls":[{"tool":"read","args":{"path":"a"}},{"tool":"read","args":{"path":"b"}}]}),
                json!({"text":"finished"}),
            ],
        };
        for (completed, expected_calls, expected_text) in [
            (0, 1, "First I format."),
            (1, 2, "Then I read."),
            (3, 0, "finished"),
        ] {
            let contents = (0..completed)
                .map(|id| {
                    let mut content = Content::new("user");
                    content.parts.push(Part::FunctionResponse {
                        function_response: adk_core::FunctionResponseData::new(
                            "read",
                            json!({"ok":true}),
                        ),
                        id: Some(id.to_string()),
                        annotations: None,
                    });
                    content
                })
                .collect();
            let request = LlmRequest {
                model: "fixture".into(),
                contents,
                config: None,
                tools: HashMap::new(),
                previous_response_id: None,
            };
            let mut response = fixture.generate_content(request, false).await.unwrap();
            let content = response.next().await.unwrap().unwrap().content.unwrap();
            let calls: Vec<_> = content
                .parts
                .iter()
                .filter_map(|part| {
                    if let Part::FunctionCall { id, .. } = part {
                        Some(id)
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(calls.len(), expected_calls);
            assert_eq!(
                calls.iter().collect::<std::collections::HashSet<_>>().len(),
                expected_calls
            );
            assert_eq!(
                content.parts[0],
                Part::Text {
                    text: expected_text.into()
                }
            );
        }
    }

    #[test]
    fn fixture_commentary_does_not_bypass_tool_argument_or_capability_validation() {
        for step in [
            json!({"text":"I will read.","tool":"read","args":{"path":"note.txt"}}),
            json!({"text":"I will read.","calls":[{"tool":"read","args":null}]}),
        ] {
            assert!(parameters(&json!({"fixtureSteps":[step]})).is_err());
        }
        assert!(parameters(&json!({"tools":["read"],"fixtureSteps":[{"text":"I will read.","tool":"read","args":{"path":"note.txt"}}]})).is_ok());
    }
    #[async_trait]
    impl Llm for ScriptedModel {
        fn name(&self) -> &str {
            "scripted"
        }
        async fn generate_content(
            &self,
            _request: LlmRequest,
            _stream: bool,
        ) -> adk_core::Result<LlmResponseStream> {
            Ok(Box::pin(futures::stream::iter(
                self.0.iter().cloned().map(Ok).collect::<Vec<_>>(),
            )))
        }
    }
    #[tokio::test]
    async fn incomplete_model_calls_never_become_dispatchable_tools() {
        let mut call = Content::new("model");
        call.parts.push(Part::FunctionCall {
            name: "inspect_json".into(),
            args: json!({"value":"anything"}),
            id: Some("incomplete".into()),
            thought_signature: None,
        });
        for response in [
            LlmResponse {
                content: Some(call.clone()),
                partial: true,
                ..Default::default()
            },
            LlmResponse {
                content: Some(call.clone()),
                interrupted: true,
                ..Default::default()
            },
            LlmResponse {
                content: Some(call),
                finish_reason: Some(adk_core::FinishReason::MaxTokens),
                ..Default::default()
            },
        ] {
            let model = ModelNode {
                id: "model".into(),
                model: Arc::new(ScriptedModel(vec![response])),
                params: parameters(&json!({"tools":["inspect_json"]})).unwrap(),
            };
            assert!(
                model
                    .execute(&NodeContext::new(
                        State::new(),
                        ExecutionConfig::new("incomplete"),
                        0
                    ))
                    .await
                    .is_err()
            );
        }
    }
    #[async_trait]
    impl Llm for RecordingModel {
        fn name(&self) -> &str {
            "recording"
        }
        async fn generate_content(
            &self,
            request: LlmRequest,
            _stream: bool,
        ) -> adk_core::Result<LlmResponseStream> {
            *self.0.lock().unwrap() = Some(request);
            Ok(Box::pin(futures::stream::iter([Ok(LlmResponse {
                content: Some(Content::new("model").with_text(r#"{"ok":true}"#)),
                turn_complete: true,
                ..Default::default()
            })])))
        }
    }
    #[tokio::test]
    async fn cas_request_manifest_preserves_exact_messages_and_tool_declarations() {
        let root = tempfile::tempdir().unwrap();
        let services = crate::runtime::RunServices::new(
            "request-test".into(),
            root.path().into(),
            root.path().join("data"),
            crate::workspace_context::ContextSnapshot::default(),
            json!({}),
            vec![],
        )
        .unwrap();
        let store = zf_storage::content_store::ContentStore::new(
            sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect("sqlite::memory:")
                .await
                .unwrap(),
        )
        .await
        .unwrap();
        services.set_content_store(store.clone());
        let captured = Arc::new(Mutex::new(None));
        let model=ModelNode { id:"a".into(),model:Arc::new(RecordingModel(captured.clone())),params:parameters(&json!({"__zedflowVersion":2,"attachments":{"instructions":{"items":[{"id":"text","source":{"kind":"text","text":"Literal {{value}}"}}]},"tools":{"items":[{"id":"read","name":"read"}]}}})).unwrap() };
        let ctx = NodeContext::new(
            State::from([("input".into(), json!("hello"))]),
            ExecutionConfig::new("request-test"),
            0,
        );
        let output = model
            .execute_request(&ctx, Some((&services, "nested/a")))
            .await
            .unwrap();
        let request = captured.lock().unwrap().take().unwrap();
        let manifest = store
            .resolve(
                output.updates["modelResponse"]["requestRef"]
                    .as_str()
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(manifest["request"]["contents"], json!(request.contents));
        assert_eq!(manifest["request"]["tools"], json!(request.tools));
        assert_eq!(manifest["nodePath"], "nested/a");
        assert!(!services.data.join("model-requests").exists());
        let records = store.records("request-test").await.unwrap();
        assert!(records.iter().any(|r| r.kind == "capability-snapshots"));
        assert!(records.iter().any(|r| r.kind == "model-calls"));
        assert!(records.iter().any(|r| r.kind == "model-requests"));
    }
    #[tokio::test]
    async fn v2_bare_model_has_no_implicit_workspace_prompt_or_legacy_tools() {
        let root = tempfile::tempdir().unwrap();
        let services = crate::runtime::RunServices::new(
            "bare".into(),
            root.path().into(),
            root.path().join("data"),
            crate::workspace_context::ContextSnapshot {
                cwd: root.path().into(),
                instructions: vec![crate::workspace_context::Instruction {
                    path: root.path().join("AGENTS.md"),
                    content: "DO NOT INJECT".into(),
                    hash: "snapshot".into(),
                }],
                ..Default::default()
            },
            json!({}),
            vec![],
        )
        .unwrap();
        let captured = Arc::new(Mutex::new(None));
        let model = ModelNode {
            id: "bare".into(),
            model: Arc::new(RecordingModel(captured.clone())),
            params: parameters(
                &json!({"__zedflowVersion":2,"tools":["exec"],"instructions":"legacy ignored"}),
            )
            .unwrap(),
        };
        let ctx = NodeContext::new(
            State::from([("input".into(), json!("hello"))]),
            ExecutionConfig::new("bare"),
            0,
        );
        let output = model
            .execute_request(&ctx, Some((&services, "nested/bare")))
            .await
            .unwrap();
        let request = captured.lock().unwrap().take().unwrap();
        assert!(request.tools.is_empty());
        assert_eq!(request.contents.len(), 1);
        assert_eq!(
            json!(request.contents[0]),
            json!(Content::new("user").with_text("hello"))
        );
        assert!(
            output.updates["modelResponse"]
                .get("effectiveContext")
                .is_none()
        );
        let snapshot_id = output.updates["modelResponse"]["contextSnapshotId"]
            .as_str()
            .unwrap();
        let snapshot: Value = serde_json::from_slice(
            &std::fs::read(
                services
                    .data
                    .join("capability-snapshots")
                    .join(format!("{snapshot_id}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(snapshot["system"], "");
        assert_eq!(snapshot["agentPath"], "nested/bare");
        let legacy = ModelNode {
            id: "legacy".into(),
            model: Arc::new(RecordingModel(captured.clone())),
            params: parameters(&json!({"tools":["exec"],"instructions":"legacy kept"})).unwrap(),
        };
        legacy
            .execute_request(&ctx, Some((&services, "legacy")))
            .await
            .unwrap();
        let request = captured.lock().unwrap().take().unwrap();
        assert!(request.tools.contains_key("exec"));
        assert!(
            serde_json::to_string(&request.contents)
                .unwrap()
                .contains("DO NOT INJECT")
        );
    }

    #[tokio::test]
    async fn large_effective_context_is_durable_but_not_copied_into_graph_state() {
        let root = tempfile::tempdir().unwrap();
        let services = crate::runtime::RunServices::new(
            "compact".into(),
            root.path().into(),
            root.path().join("data"),
            crate::workspace_context::ContextSnapshot::default(),
            json!({}),
            vec![],
        )
        .unwrap();
        let body = "Context preserved once.\n".repeat(10_000);
        let captured = Arc::new(Mutex::new(None));
        let model=ModelNode { id:"agent".into(),model:Arc::new(RecordingModel(captured.clone())),params:parameters(&json!({"__zedflowVersion":2,"attachments":{"instructions":{"items":[{"id":"large","source":{"kind":"text","text":body}}]}}})).unwrap() };
        let ctx = NodeContext::new(
            State::from([("input".into(), json!("hello"))]),
            ExecutionConfig::new("compact"),
            0,
        );
        let output = model
            .execute_request(&ctx, Some((&services, "agent")))
            .await
            .unwrap();
        assert!(
            serde_json::to_vec(&output.updates).unwrap().len() < 2048,
            "business output and metadata must not carry source contents"
        );
        assert_eq!(output.updates["output"], r#"{"ok":true}"#);
        let snapshot_id = output.updates["modelResponse"]["contextSnapshotId"]
            .as_str()
            .unwrap();
        let snapshot: Value = serde_json::from_slice(
            &std::fs::read(
                services
                    .data
                    .join("capability-snapshots")
                    .join(format!("{snapshot_id}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(snapshot["system"], body);
        assert_eq!(snapshot["resources"][0]["content"], body);
        assert!(
            serde_json::to_vec(&captured.lock().unwrap().as_ref().unwrap().contents)
                .unwrap()
                .len()
                > 200_000
        );
    }

    #[tokio::test]
    async fn model_parameters_reach_the_adk_request() {
        let captured = Arc::new(Mutex::new(None));
        let params = parameters(&json!({"provider":"fixture","temperature":0.3,"topP":0.8,"topK":5,"maxOutputTokens":512,"instructions":"Pour {{input}}","globalInstructions":"Global","responseFormat":"json","responseSchema":{"type":"object"},"stopSequences":["STOP"],"tools":["inspect_json"],"field":"answer"})).unwrap();
        let node = ModelNode {
            id: "model".into(),
            model: Arc::new(RecordingModel(captured.clone())),
            params,
        };
        let context = NodeContext::new(
            State::from([("input".into(), json!("Ada"))]),
            ExecutionConfig::new("test"),
            0,
        );
        let output = node.execute(&context).await.unwrap();
        assert_eq!(output.updates["answer"], json!({"ok":true}));
        let request = captured.lock().unwrap().take().unwrap();
        let config = request.config.unwrap();
        assert_eq!(config.max_output_tokens, Some(512));
        assert!((config.temperature.unwrap() - 0.3).abs() < f32::EPSILON);
        assert!((config.top_p.unwrap() - 0.8).abs() < f32::EPSILON);
        assert_eq!(config.top_k, Some(5));
        assert_eq!(config.stop_sequences, vec!["STOP"]);
        assert_eq!(config.response_schema, Some(json!({"type":"object"})));
        assert_eq!(
            request.contents[0].parts[0],
            Part::Text {
                text: "Global\nPour Ada".into()
            }
        );
        assert!(request.tools.contains_key("inspect_json"));
    }
    #[tokio::test]
    async fn input_after_tool_response_reaches_the_model_even_when_text_is_unchanged() {
        let captured = Arc::new(Mutex::new(None));
        let params = parameters(&json!({"provider":"fixture","tools":["inspect_json"]})).unwrap();
        let model = ModelNode {
            id: "model".into(),
            model: Arc::new(RecordingModel(captured.clone())),
            params,
        };
        let mut response = Content::new("user");
        response.parts.push(Part::FunctionResponse {
            function_response: adk_core::FunctionResponseData::new(
                "inspect_json",
                json!({"ok":true}),
            ),
            id: Some("call-1".into()),
            annotations: None,
        });
        let state = State::from([
            ("input".into(), json!("Ada")),
            ("answer:ask".into(), json!("Ada")),
            (
                "messages".into(),
                json!([Content::new("user").with_text("Ada"), response]),
            ),
            ("__zedflow:input:input".into(), json!(0)),
            ("__zedflow:model-input:model".into(), json!(0)),
        ]);
        let input_ctx = NodeContext::new(state, ExecutionConfig::new("human-after-tool"), 7);
        let answer = crate::operations::execute(
            "input",
            &json!({"nodeId":"ask","field":"input"}),
            NodeContext::new(
                input_ctx.state.clone(),
                ExecutionConfig::new("human-after-tool"),
                7,
            ),
        )
        .await
        .unwrap();
        let mut next_state = input_ctx.state;
        next_state.extend(answer.updates);
        model
            .execute(&NodeContext::new(
                next_state,
                ExecutionConfig::new("human-after-tool"),
                8,
            ))
            .await
            .unwrap();
        let request = captured.lock().unwrap().take().unwrap();
        assert_eq!(
            request.contents.last().unwrap().parts,
            vec![Part::Text { text: "Ada".into() }]
        );
        assert_eq!(
            request.contents.len(),
            4,
            "system, earlier user, tool response, new user turn"
        );
    }
}
