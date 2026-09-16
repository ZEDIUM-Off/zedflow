//! Capture, validate and admit one definition through the shared execution owner.
use crate::{
    commands::{
        Actor, CommandKind, command_ack, expand_run_skill, validate_bindings, validate_run_bindings,
    },
    preparation::{self, RuntimeSelection},
    service::{ExecutionService, launch, now},
};
use adk_graph::State;
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use uuid::Uuid;
use zf_compiler::{
    graph_compiler::{self, GraphValidator},
    programs,
};
use zf_flows::{flow_format, schema::Composition};
use zf_runtime::{materialize::RuntimePrimitives, workspace_context::ContextSnapshot};
use zf_storage::workspaces;

/// These choices are exclusive. Inline definitions are for callers which already
/// own the document; stored definitions require the revision the caller selected.
pub enum StartDefinition {
    Inline(Composition),
    Stored { key: String, expected_hash: String },
    Composition(RuntimeSelection),
}
pub struct StartRequest {
    pub definition: StartDefinition,
    pub input: State,
    pub model_bindings: Value,
    pub node_path: Option<String>,
    /// Only a trusted local preview caller supplies a captured source context.
    pub prepared_context: Option<ContextSnapshot>,
    pub preview_metadata: Option<Value>,
}
impl ExecutionService {
    pub async fn prepare(
        &self,
        actor: &Actor,
        selection: &RuntimeSelection,
    ) -> Result<zf_compiler::prepared_model::PreparedRuntime> {
        let b = self.admit(actor, CommandKind::Read, None).await?;
        let _authoring = b.authoring_writer.lock().await;
        let workspace = workspaces::get(&b.db, &actor.workspace_id).await?;
        crate::live_files::recover(&b.db, &b.home).await?;
        if workspace.path != b.home {
            crate::live_files::recover(&b.db, &workspace.path).await?;
        }
        preparation::prepare(&b.flows, &workspace, selection).await
    }
    pub async fn start(&self, actor: &Actor, request: StartRequest) -> Result<Value> {
        let b = self.admit(actor, CommandKind::Start, None).await?;
        let authoring = b.authoring_writer.clone();
        let _authoring = authoring.lock().await;
        let workspace = workspaces::get(&b.db, &actor.workspace_id).await?;
        crate::live_files::recover(&b.db, &b.home).await?;
        if workspace.path != b.home {
            crate::live_files::recover(&b.db, &workspace.path).await?;
        }
        let selection = match &request.definition {
            StartDefinition::Composition(s) => Some(s.clone()),
            _ => None,
        };
        let prepared = match &selection {
            Some(s) => Some(preparation::prepare(&b.flows, &workspace, s).await?),
            None => None,
        };
        let mut flow_package = None;
        let mut package_conditions = Vec::new();
        let mut package_root = None;
        let (mut composition, mut source, flow_ref) = match &request.definition {
            StartDefinition::Composition(_) => {
                let runtime = prepared.as_ref().context("Prepared composition absent")?;
                let root = runtime.root()?;
                (
                    zf_flows::flow_contract::at_entry(
                        &root.composition,
                        &runtime.graph.entry.port,
                    )?,
                    root.source.clone(),
                    json!({"key":root.key,"hash":root.hash,"runtimeGraph":true}),
                )
            }
            StartDefinition::Stored { key, expected_hash } => {
                let file = b.flows.get(&workspace, key).await?;
                ensure!(
                    file.hash == *expected_hash,
                    zf_storage::flow_store::Conflict(
                        "Le flow a changé ; actualisez avant de lancer cette version."
                    )
                );
                ensure!(
                    file.diagnostics.is_empty(),
                    "Flow non exécutable : {:?}",
                    file.diagnostics
                );
                package_root = file
                    .package
                    .as_ref()
                    .map(|package| (file.path.clone(), package.root.clone()));
                flow_package = file.package;
                package_conditions = file.preconditions;
                let reference = json!({"key":file.key,"id":file.id,"name":file.name,"path":file.path,"scope":file.scope,"hash":file.hash,"fileVersion":file.file_version});
                (
                    file.composition.context("Flow non exécutable")?,
                    file.source.context("Source du flow inaccessible")?,
                    reference,
                )
            }
            StartDefinition::Inline(doc) => {
                let source = flow_format::render(doc, &GraphValidator::new(&RuntimePrimitives))?;
                let reference = json!({"id":doc.id,"name":doc.name,"hash":zf_storage::flow_store::hash(source.as_bytes()),"fileVersion":doc.format_version,"preview":true});
                (doc.clone(), source, reference)
            }
        };
        let context_dependencies = if prepared.is_some() {
            vec![]
        } else {
            let sources =
                crate::sources::program_sources(&composition, &workspace.path, &[]).await?;
            programs::freeze(&mut composition, &sources)?
        };
        if !context_dependencies.is_empty() {
            source = flow_format::render(&composition, &GraphValidator::new(&RuntimePrimitives))?;
        }
        graph_compiler::validate(&composition, &RuntimePrimitives)?;
        if let Some(runtime) = &prepared {
            validate_run_bindings(
                &json!({"composition":composition,"runtimeGraph":runtime}),
                &request.model_bindings,
            )?;
        } else {
            validate_bindings(&composition, &request.model_bindings)?;
        }
        let id = Uuid::new_v4().to_string();
        let mut context = if let Some(context) = request.prepared_context {
            context
        } else {
            ContextSnapshot::load_with_home(
                &workspace.path,
                &b.skill_dirs,
                b.context_home.as_deref(),
            )
            .await?
        };
        let mut input = request.input;
        if let Some(runtime) = &prepared {
            let root = runtime.root()?;
            let binding = &root.exports.entries[&runtime.graph.entry.port];
            let value = input
                .get(&binding.input_field)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("Entrée {} absente", binding.input_field))?;
            zf_flows::flow_contract::entry_input(&root.exports, &runtime.graph.entry.port, value)?;
        }
        let original_text = input
            .get("input")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if composition.format_version < 2
            && let Some(text) = input.get("input").and_then(Value::as_str)
        {
            let (expanded, loaded) = context.expand_skill_with_metadata(text)?;
            if let Some(loaded) = loaded {
                context.loaded_skills.push(loaded);
            }
            input.insert("input".into(), json!(expanded));
        }
        let name = original_text
            .as_deref()
            .filter(|text| !text.trim().is_empty())
            .map(|text| {
                text.split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .chars()
                    .take(100)
                    .collect::<String>()
            })
            .unwrap_or_else(|| composition.name.clone());
        let mut value = json!({"id":id,"name":name,"workspaceId":workspace.id,"workspacePath":workspace.path,"flowRef":flow_ref,"flowSource":source,"composition":composition,"createdAt":now(),"updatedAt":now(),"status":"running","input":input,"state":{},"messages":original_text.map(|text|vec![json!({"id":Uuid::new_v4().to_string(),"role":"user","text":text})]).unwrap_or_default(),"activities":[],"toolActivities":[],"activeNodes":[],"wait":null,"modelBindings":request.model_bindings,"modelRevision":0,"queue":[],"context":context});
        if let Some(package) = flow_package {
            value["flowPackage"] = serde_json::to_value(package)?;
        }
        value["executedSourceHash"] = json!(zf_storage::flow_store::hash(source.as_bytes()));
        if let Some(preview) = request.preview_metadata {
            value["preview"] = preview;
        }
        let interactive = prepared
            .as_ref()
            .map(|runtime| runtime.interactive())
            .unwrap_or_else(|| {
                zf_storage::session_store::composition_interactive(&value["composition"])
            });
        value["interactive"] = json!(interactive);
        if !interactive {
            value["messages"] = json!([]);
        }
        if let Some(runtime) = prepared {
            value["runtimeGraph"] = serde_json::to_value(runtime)?;
            value["runtimeSelection"] = serde_json::to_value(selection.as_ref().map(|s|json!({"flow":s.flow,"entry":s.entry,"bridges":s.bridges,"flowHashes":s.flow_hashes,"bridgeHashes":s.bridge_hashes,"contexts":s.contexts})))?;
        }
        if !context_dependencies.is_empty() {
            value["contextDependencies"] = serde_json::to_value(context_dependencies)?;
        }
        if value["composition"]["formatVersion"].as_u64().unwrap_or(1) >= 2
            && let Some(text) = input
                .get("input")
                .and_then(Value::as_str)
                .map(str::to_owned)
        {
            let expanded = expand_run_skill(&mut value, &text, request.node_path.as_deref())?;
            input.insert("input".into(), json!(expanded));
            value["input"] = json!(input);
        }
        zf_storage::timeline::reconcile(&mut value, &json!({"type":"run_started"}), 0);
        value["startedBy"] = json!({"id":b.actor.id,"workspaceId":b.actor.workspace_id});
        if let Some((path, revision)) = package_root {
            ensure!(
                zf_storage::flow_packages::capture(&path).await?.root == revision,
                zf_storage::flow_store::Conflict("Flow package changed before admission")
            );
        }
        preparation::recheck(
            &package_conditions
                .into_iter()
                .map(|condition| (condition.path, condition.hash))
                .collect(),
        )
        .await?;
        zf_storage::session_store::save(&b.writer_db, &id, &value).await?;
        b.sync.seed(&id).await?;
        let acknowledgement = command_ack(&b, &id).await?;
        drop(_authoring);
        launch(b, id, input, None, None);
        Ok(acknowledgement)
    }
}
