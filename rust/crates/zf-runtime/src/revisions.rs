//! Published definitions are selected once per native ADK super-step. The graph
//! executor remains alive; only a statically sequential structural boundary may
//! request an internal checkpoint/rebuild.
use crate::materialize::RuntimePrimitives;
use adk_graph::{Node, NodeContext, NodeOutput, StateSchema, error::GraphError};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tokio::sync::Mutex as AsyncMutex;
use zf_compiler::prepared_model::{ContextSelection, PreparedRuntime};
use zf_flows::{
    package::PackageSnapshot,
    schema::{Composition, Node as SchemaNode},
};
use zf_storage::content_store::ContentStore;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevisionDefinition {
    pub key: String,
    pub hash: String,
    pub source: String,
    pub composition: Composition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<PackageSnapshot>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub context_selections: BTreeMap<String, ContextSelection>,
}
impl RevisionDefinition {
    /// Full executable identity; legacy source-only definitions retain their hash.
    pub fn revision(&self) -> String {
        if self.package.is_none() && self.context_selections.is_empty() {
            return self.hash.clone();
        }
        let mut hash = Sha256::new();
        hash.update(b"zedflow.definition.v1\0");
        for part in std::iter::once(self.hash.as_str())
            .chain(std::iter::once(
                self.package.as_ref().map_or("", |p| p.root.as_str()),
            ))
            .chain(self.context_selections.iter().flat_map(|(path, choice)| {
                [path.as_str(), choice.key.as_str(), choice.hash.as_str()]
            }))
        {
            hash.update((part.len() as u64).to_be_bytes());
            hash.update(part.as_bytes());
        }
        format!("{:x}", hash.finalize())
    }
    pub fn from_prepared(runtime: &PreparedRuntime, instance: &str) -> Result<Self> {
        let flow = runtime
            .flows
            .get(instance)
            .context("Revision instance absent")?;
        let mut definition = Self::from_frozen(flow, &runtime.definitions, instance);
        definition.context_selections.retain(|path, _| {
            let full_path = format!("{instance}/{path}");
            runtime
                .flows
                .keys()
                .filter(|candidate| full_path.starts_with(&format!("{candidate}/")))
                .max_by_key(|candidate| candidate.len())
                .is_some_and(|owner| owner == instance)
        });
        Ok(definition)
    }
    fn from_frozen(
        flow: &zf_compiler::prepared_model::FrozenFlow,
        pins: &zf_compiler::prepared_model::DefinitionPins,
        instance: &str,
    ) -> Self {
        let prefix = format!("{instance}/");
        Self {
            key: flow.key.clone(),
            hash: flow.hash.clone(),
            source: flow.source.clone(),
            composition: flow.composition.clone(),
            package: pins.flow_packages.get(&flow.key).cloned(),
            context_selections: pins
                .context_selections
                .iter()
                .filter_map(|(path, selection)| {
                    path.strip_prefix(&prefix)
                        .map(|path| (path.to_owned(), selection.clone()))
                })
                .collect(),
        }
    }
    /// Install one selected definition and its authored provenance together.
    pub fn apply_to(&self, plan: &mut PreparedRuntime, instance: &str) -> Result<()> {
        validate_definition(self)?;
        let flow = plan
            .flows
            .get_mut(instance)
            .context("Revision instance absent")?;
        ensure!(
            flow.key == self.key,
            "Published flow identity disagrees with runtime instance"
        );
        flow.exports = zf_flows::flow_contract::validate(&self.composition)?
            .context("Published flow exports absent")?;
        flow.composition = self.composition.clone();
        flow.source = self.source.clone();
        flow.hash = self.hash.clone();
        if let Some(package) = &self.package {
            plan.definitions
                .flow_hashes
                .insert(self.key.clone(), key(package.root_node()?.entry_source()?));
            plan.definitions
                .flow_packages
                .insert(self.key.clone(), package.clone());
        } else {
            ensure!(
                !plan.definitions.flow_packages.contains_key(&self.key),
                "Revision cannot discard a captured package"
            );
        }
        let prefix = format!("{instance}/");
        plan.definitions.context_selections.retain(|path, _| {
            plan.flows
                .keys()
                .filter(|candidate| path.starts_with(&format!("{candidate}/")))
                .max_by_key(|candidate| candidate.len())
                .is_none_or(|owner| owner != instance)
        });
        plan.definitions.context_selections.extend(
            self.context_selections
                .iter()
                .map(|(path, choice)| (format!("{prefix}{path}"), choice.clone())),
        );
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Compatibility {
    Live,
    SequentialBoundary { reasons: Vec<String> },
    Incompatible { reasons: Vec<String> },
}
impl Compatibility {
    pub fn can_publish(&self) -> bool {
        !matches!(self, Self::Incompatible { .. })
    }
}
pub type NodeFactory =
    Arc<dyn Fn(&Composition, &SchemaNode) -> Result<Arc<dyn Node>> + Send + Sync>;

tokio::task_local! {static CURRENT_REVISION:Value;}
pub fn current_revision() -> Option<Value> {
    CURRENT_REVISION.try_with(Clone::clone).ok()
}
fn key(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

pub struct RevisionRuntime {
    store: ContentStore,
    run_id: String,
    bases: BTreeMap<String, Arc<RevisionDefinition>>,
    selection: AsyncMutex<()>,
    definitions: Mutex<BTreeMap<String, Arc<RevisionDefinition>>>,
    graph: Mutex<Option<RuntimeGraphSnapshot>>,
}
impl RevisionRuntime {
    pub async fn new(
        store: ContentStore,
        run_id: impl Into<String>,
        initial: BTreeMap<String, RevisionDefinition>,
    ) -> Result<Arc<Self>> {
        let run_id = run_id.into();
        ensure!(
            !run_id.is_empty() && !initial.is_empty(),
            "Revision runtime requires a run and definitions"
        );
        let mut bases = BTreeMap::new();
        for (instance, definition) in initial {
            validate_definition(&definition)?;
            let value = json!(definition);
            let reference = store.intern(&value).await?;
            store
                .claim_record(
                    &run_id,
                    "revision-definitions",
                    &key(&format!("{instance}\0{}", definition.revision())),
                    &value,
                )
                .await?;
            store
                .claim_record(
                    &run_id,
                    "revision-heads",
                    &key(&instance),
                    &json!({"instance":instance,"definitionRef":reference}),
                )
                .await?;
            bases.insert(instance, Arc::new(definition));
        }
        Ok(Arc::new(Self {
            store,
            run_id,
            bases,
            selection: AsyncMutex::new(()),
            definitions: Mutex::new(BTreeMap::new()),
            graph: Mutex::new(None),
        }))
    }
    /// A native executor may rebuild only after a verified sequential boundary.
    /// Other live executors retain their own controller and captured node Arcs.
    pub async fn rebased(
        &self,
        instance: &str,
        definition: RevisionDefinition,
    ) -> Result<Arc<Self>> {
        let previous = self
            .bases
            .get(instance)
            .context("Rebuilt instance is not registered")?;
        ensure!(
            previous.key == definition.key,
            "Rebuild must retain definition identity"
        );
        let mut definitions = self
            .bases
            .iter()
            .map(|(key, value)| (key.clone(), value.as_ref().clone()))
            .collect::<BTreeMap<_, _>>();
        definitions.insert(instance.into(), definition);
        Self::new(self.store.clone(), self.run_id.clone(), definitions).await
    }
    pub fn run_id(&self) -> &str {
        &self.run_id
    }
    pub async fn publish(
        &self,
        instance: &str,
        definition: RevisionDefinition,
    ) -> Result<Compatibility> {
        let baseline = self
            .bases
            .get(instance)
            .context("Published instance is absent from this runtime")?
            .composition
            .clone();
        Ok(publish_batch(
            &self.store,
            &[RevisionPublication {
                run_id: self.run_id.clone(),
                instance: instance.into(),
                baseline,
                definition,
            }],
        )
        .await?
        .remove(0))
    }
    pub fn wrap(
        self: &Arc<Self>,
        scope: &str,
        node_id: &str,
        initial: Arc<dyn Node>,
        factory: NodeFactory,
    ) -> Arc<dyn Node> {
        Arc::new(RevisionNode {
            runtime: self.clone(),
            scope: scope.trim_end_matches('/').to_owned(),
            id: node_id.to_owned(),
            initial,
            factory,
            cache: Mutex::new(None),
        })
    }
    fn base(&self, scope: &str) -> Result<(&str, &Arc<RevisionDefinition>)> {
        self.bases
            .iter()
            .filter(|(instance, _)| {
                instance.is_empty()
                    || scope == instance.as_str()
                    || scope
                        .strip_prefix(instance.as_str())
                        .is_some_and(|suffix| suffix.starts_with('/'))
            })
            .max_by_key(|(instance, _)| instance.len())
            .map(|(instance, base)| (instance.as_str(), base))
            .context("Node scope has no published definition")
    }
    async fn definition(&self, reference: &str) -> Result<Arc<RevisionDefinition>> {
        if let Some(definition) = self
            .definitions
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(reference)
            .cloned()
        {
            return Ok(definition);
        }
        let definition = serde_json::from_value(self.store.resolve(reference).await?)?;
        validate_definition(&definition)?;
        let definition = Arc::new(definition);
        let mut cache = self.definitions.lock().unwrap_or_else(|p| p.into_inner());
        // Immutable CAS references are safe to reuse within this controller.
        // Bound retained revisions; eviction changes only read cost.
        if cache.len() >= 32 {
            cache.clear();
        }
        cache.insert(reference.to_owned(), definition.clone());
        Ok(definition)
    }
    async fn select(&self, scope: &str, node_id: &str, ctx: &NodeContext) -> Result<Selection> {
        let _guard = self.selection.lock().await;
        let (instance, base) = self.base(scope)?;
        let pin_key = key(&format!("{}\0{scope}\0{}", ctx.config.thread_id, ctx.step));
        if let Some(pin) = self
            .store
            .record(&self.run_id, "revision-steps", &pin_key)
            .await?
        {
            return Ok(Selection::Node {
                definition: self
                    .definition(
                        pin["definitionRef"]
                            .as_str()
                            .context("Step definition absent")?,
                    )
                    .await?,
                metadata: pin,
            });
        }
        let graph_pin = pin_runtime_graph(
            &self.store,
            &self.run_id,
            &ctx.config.thread_id,
            ctx.step,
            &self.graph,
        )
        .await?;
        let active_key = key(&format!("{}\0{scope}", ctx.config.thread_id));
        let active = self
            .store
            .record(&self.run_id, "revision-active", &active_key)
            .await?;
        let fallback = if let Some(active) = &active {
            self.definition(
                active["definitionRef"]
                    .as_str()
                    .context("Active definition absent")?,
            )
            .await?
        } else {
            base.clone()
        };
        let (head, candidate) = if let Some((_, graph)) = &graph_pin
            && graph.flows.contains_key(instance)
        {
            let definition = Arc::new(RevisionDefinition::from_prepared(graph, instance)?);
            let reference = self.store.intern(&json!(definition)).await?;
            (json!({"definitionRef":reference}), definition)
        } else {
            let head = self
                .store
                .record(&self.run_id, "revision-heads", &key(instance))
                .await?
                .context("Published head missing")?;
            let definition = self
                .definition(
                    head["definitionRef"]
                        .as_str()
                        .context("Published definition absent")?,
                )
                .await?;
            (head, definition)
        };
        let mut chosen = candidate.clone();
        let mut diagnostic = None;
        let base_doc = at_scope(&base.composition, instance, scope)?;
        let candidate_doc = at_scope(&candidate.composition, instance, scope);
        let pending_calls = ctx.state.iter().any(|(name, value)| {
            (name == "toolCalls" || name.ends_with("ToolCalls"))
                && value.as_array().is_some_and(|calls| !calls.is_empty())
        });
        let state_issue = if candidate.revision() != fallback.revision() {
            let headers = zf_storage::contracts::CheckpointStore::new(self.store.clone())
                .await?
                .list_headers(&ctx.config.thread_id)
                .await?;
            let frontier = headers
                .iter()
                .rev()
                .find(|header| header.step == ctx.step)
                .map(|header| header.pending_nodes.clone())
                .unwrap_or_else(|| {
                    if ctx.step == 0 {
                        base_doc
                            .edges
                            .iter()
                            .filter(|edge| {
                                base_doc
                                    .nodes
                                    .iter()
                                    .any(|n| n.id == edge.source && n.data.kind == "start")
                            })
                            .map(|edge| edge.target.clone())
                            .collect()
                    } else {
                        vec![node_id.into()]
                    }
                });
            candidate_doc
                .as_ref()
                .ok()
                .and_then(|doc| state_issue(doc, &frontier, &ctx.state))
        } else {
            None
        };
        if candidate.revision() != fallback.revision() && pending_calls {
            chosen = fallback.clone();
            diagnostic = Some(
                json!({"code":"pending_tool_calls","message":"An existing model invocation must resolve its pending tool calls before adoption"}),
            );
        } else if let Some(issue) = state_issue {
            chosen = fallback.clone();
            diagnostic = Some(issue);
        } else {
            match candidate_doc.and_then(|doc| compatibility(&base_doc, &doc)) {
                Ok(Compatibility::Live) => {}
                Ok(Compatibility::SequentialBoundary { reasons }) => {
                    if at_scope(&candidate.composition, instance, scope)?
                        .nodes
                        .iter()
                        .any(|node| node.id == node_id)
                    {
                        let request = json!({"kind":"revision_boundary","scope":scope,"instance":instance,"nodePath":if scope.is_empty(){node_id.to_owned()}else{format!("{scope}/{node_id}")},"node":node_id,"threadId":ctx.config.thread_id,"step":ctx.step,"fromHash":base.hash,"toHash":candidate.hash,"fromRevision":base.revision(),"toRevision":candidate.revision(),"definitionRef":head["definitionRef"],"reasons":reasons});
                        self.store
                            .put_record(&self.run_id, "revision-boundaries", &pin_key, &request)
                            .await?;
                        return Ok(Selection::Boundary(request));
                    }
                    chosen = fallback.clone();
                    diagnostic = Some(json!({"code":"pending_node_removed","node":node_id}));
                }
                Ok(Compatibility::Incompatible { reasons }) => {
                    chosen = fallback.clone();
                    diagnostic = Some(json!({"code":"frontier_incompatible","reasons":reasons}));
                }
                Err(error) => {
                    chosen = fallback.clone();
                    diagnostic =
                        Some(json!({"code":"scope_incompatible","message":error.to_string()}));
                }
            }
        }
        let definition_ref = self.store.intern(&json!(chosen)).await?;
        let source_ref = self.store.intern(&json!(chosen.source)).await?;
        let pin = json!({"graphRef":graph_pin.as_ref().map(|(reference,_)|reference),"scope":scope,"instance":instance,"threadId":ctx.config.thread_id,"step":ctx.step,"key":chosen.key,"hash":chosen.hash,"definitionRevision":chosen.revision(),"packageRevision":chosen.package.as_ref().map(|package|&package.root),"sourceRef":source_ref,"definitionRef":definition_ref,"diagnostic":diagnostic});
        self.store
            .claim_record(&self.run_id, "revision-steps", &pin_key, &pin)
            .await?;
        let pin = self
            .store
            .record(&self.run_id, "revision-steps", &pin_key)
            .await?
            .context("Step publication disappeared")?;
        if active
            .as_ref()
            .and_then(|v| v["step"].as_u64())
            .is_none_or(|step| step <= ctx.step as u64)
        {
            self.store
                .put_record(&self.run_id, "revision-active", &active_key, &pin)
                .await?;
        }
        Ok(Selection::Node {
            definition: self
                .definition(
                    pin["definitionRef"]
                        .as_str()
                        .context("Step definition absent")?,
                )
                .await?,
            metadata: pin,
        })
    }
}

enum Selection {
    Node {
        definition: Arc<RevisionDefinition>,
        metadata: Value,
    },
    Boundary(Value),
}
struct RevisionNode {
    runtime: Arc<RevisionRuntime>,
    scope: String,
    id: String,
    initial: Arc<dyn Node>,
    factory: NodeFactory,
    cache: Mutex<Option<(String, Arc<dyn Node>)>>,
}
#[async_trait::async_trait]
impl Node for RevisionNode {
    fn name(&self) -> &str {
        &self.id
    }
    fn validate(&self) -> adk_graph::error::Result<()> {
        self.initial.validate()
    }
    fn validate_against(&self, schema: &StateSchema) -> adk_graph::error::Result<()> {
        self.initial.validate_against(schema)
    }
    async fn execute(&self, ctx: &NodeContext) -> adk_graph::error::Result<NodeOutput> {
        let error = |error: anyhow::Error| GraphError::NodeExecutionFailed {
            node: self.id.clone(),
            message: format!("{error:#}"),
        };
        let selection = self
            .runtime
            .select(&self.scope, &self.id, ctx)
            .await
            .map_err(error)?;
        let Selection::Node {
            definition,
            metadata,
        } = selection
        else {
            let Selection::Boundary(request) = selection else {
                unreachable!()
            };
            return Ok(NodeOutput::new().with_interrupt(
                adk_graph::interrupt::interrupt_with_data(
                    "Published definition reached a compatible boundary",
                    request,
                ),
            ));
        };
        let (instance, base) = self.runtime.base(&self.scope).map_err(error)?;
        let cached = {
            self.cache
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .as_ref()
                .filter(|(hash, _)| hash == &definition.revision())
                .map(|(_, node)| node.clone())
        };
        let node = if definition.revision() == base.revision() {
            self.initial.clone()
        } else if let Some(node) = cached {
            node
        } else {
            let doc = at_scope(&definition.composition, instance, &self.scope).map_err(error)?;
            let spec = doc
                .nodes
                .iter()
                .find(|node| node.id == self.id)
                .context("Pinned node is absent")
                .map_err(error)?;
            let node = (self.factory)(&doc, spec).map_err(error)?;
            *self.cache.lock().unwrap_or_else(|p| p.into_inner()) =
                Some((definition.revision(), node.clone()));
            node
        };
        CURRENT_REVISION.scope(metadata, node.execute(ctx)).await
    }
}
fn at_scope(doc: &Composition, instance: &str, scope: &str) -> Result<Composition> {
    let mut current = doc.clone();
    let suffix = scope
        .strip_prefix(instance)
        .context("Scope outside definition")?
        .trim_start_matches('/');
    if !suffix.is_empty() {
        for id in suffix.split('/') {
            let node = current
                .nodes
                .iter()
                .find(|node| node.id == id && node.data.kind == "subgraph")
                .context("Nested scope absent from revision")?;
            current = serde_json::from_value(node.data.config["composition"].clone())?;
        }
    }
    Ok(current)
}
pub fn validate_definition(definition: &RevisionDefinition) -> Result<()> {
    if let Some(package) = &definition.package {
        zf_compiler::prepared_model::validate_package_definition(
            package,
            &definition.source,
            &definition.composition,
            &definition.context_selections,
            &RuntimePrimitives,
        )?;
    }
    ensure!(
        key(&definition.source) == definition.hash,
        "Revision source hash mismatch"
    );
    let parsed = zf_flows::flow_format::parse(
        &definition.source,
        &zf_compiler::graph_compiler::GraphValidator::new(&RuntimePrimitives),
    )
    .map_err(|diagnostics| anyhow::anyhow!("Revision source invalid: {diagnostics:?}"))?;
    ensure!(
        serde_json::to_value(&parsed)? == serde_json::to_value(&definition.composition)?,
        "Revision source differs from its executable document"
    );
    zf_compiler::graph_compiler::validate(&definition.composition, &RuntimePrimitives)
}

/// Static proof for adopting node behavior inside the existing native executor.
/// Structural changes require a single sequential frontier, never a parallel join.
pub fn compatibility(before: &Composition, after: &Composition) -> Result<Compatibility> {
    zf_compiler::graph_compiler::validate(after, &RuntimePrimitives)?;
    let mut reasons = Vec::new();
    if before.id != after.id {
        reasons.push("flow_identity_changed".into());
    }
    if before.format_version != after.format_version {
        reasons.push("flow_format_changed".into());
    }
    if zf_flows::schema::runtime_channels(before)? != zf_flows::schema::runtime_channels(after)? {
        reasons.push("channel_schema_changed".into());
    }
    if serde_json::to_value(&before.settings)? != serde_json::to_value(&after.settings)? {
        reasons.push("executor_policy_changed".into());
    }
    let exports = |doc: &Composition| {
        doc.nodes
            .iter()
            .find(|n| n.data.kind == "start")
            .map(|n| n.data.config.get("exports").cloned().unwrap_or(Value::Null))
    };
    if exports(before) != exports(after) {
        reasons.push("public_contract_changed".into());
    }
    if !reasons.is_empty() {
        return Ok(Compatibility::Incompatible { reasons });
    }
    let structure = |doc: &Composition| {
        let nodes: BTreeMap<_, _> = doc
            .nodes
            .iter()
            .map(|n| {
                let policies: BTreeMap<_, _> = [
                    "fanIn",
                    "retry",
                    "timeoutMs",
                    "idleTimeoutMs",
                    "interruptBefore",
                    "interruptAfter",
                ]
                .into_iter()
                .map(|key| (key, n.data.config.get(key).cloned().unwrap_or(Value::Null)))
                .collect();
                let native = if doc.format_version == 1 && n.data.kind == "condition" {
                    n.data.config.clone()
                } else {
                    json!(policies)
                };
                (&n.id, json!({"kind":n.data.kind,"native":native}))
            })
            .collect();
        let edges: BTreeMap<_, _> = doc
            .edges
            .iter()
            .map(|e| {
                (
                    (e.source.clone(), e.target.clone(), e.source_handle.clone()),
                    (),
                )
            })
            .collect();
        json!({"nodes":nodes,"edges":edges.keys().collect::<Vec<_>>()})
    };
    let mut child_live = true;
    for node in before.nodes.iter().filter(|n| n.data.kind == "subgraph") {
        if let Some(next) = after
            .nodes
            .iter()
            .find(|n| n.id == node.id && n.data.kind == "subgraph")
        {
            let left: Composition =
                serde_json::from_value(node.data.config["composition"].clone())?;
            let right: Composition =
                serde_json::from_value(next.data.config["composition"].clone())?;
            child_live &= compatibility(&left, &right)? == Compatibility::Live;
        }
    }
    if structure(before) == structure(after) && child_live {
        return Ok(Compatibility::Live);
    }
    reasons.push("native_graph_structure_changed".into());
    if sequential(before) && sequential(after) {
        Ok(Compatibility::SequentialBoundary { reasons })
    } else {
        reasons.push("parallel_frontier_not_serialized_by_adk".into());
        Ok(Compatibility::Incompatible { reasons })
    }
}
fn sequential(doc: &Composition) -> bool {
    !doc.nodes
        .iter()
        .any(|n| n.data.kind == "subgraph" || n.data.config["fanIn"] == "all")
        && doc.nodes.iter().all(|n| {
            n.data.kind == "condition" || doc.edges.iter().filter(|e| e.source == n.id).count() <= 1
        })
}

fn state_issue(doc: &Composition, frontier: &[String], state: &adk_graph::State) -> Option<Value> {
    for node in doc.nodes.iter().filter(|node| frontier.contains(&node.id)) {
        if node.data.kind == "model" {
            let context = node.data.config["contextNode"].as_str().unwrap_or_default();
            let prepared = state.get(&format!("__zedflow:prepared:{context}"));
            let consumed = state.get(&format!("__zedflow:prepared-consumed:{}", node.id));
            if prepared.is_some_and(Value::is_string) && prepared != consumed {
                return Some(
                    json!({"code":"prepared_context_inflight","node":node.id,"contextNode":context,"message":"Le Modèle doit consommer la préparation déjà figée avant d’adopter la nouvelle définition."}),
                );
            }
        }
        let Some(raw) = node.data.config.get("contextProgram") else {
            continue;
        };
        let Ok(program) = super::inference::program(raw) else {
            continue;
        };
        let mut resources = BTreeMap::new();
        let mut state_names = Vec::new();
        for (name, binding) in &program.bindings {
            if let zf_context::resources::ResourceBinding::State { field, pointer, .. } = binding {
                state_names.push(name.clone());
                if let Some(value) = state.get(field).and_then(|value| {
                    pointer
                        .as_ref()
                        .map_or(Some(value), |pointer| value.pointer(pointer))
                }) {
                    resources.insert(name.clone(), Arc::new(value.clone()));
                }
            }
        }
        let evaluation = zf_context::context::evaluate_with_library(
            &program.strategy,
            &resources,
            &program.types,
            &program.library,
        );
        let missing: Vec<_> = evaluation
            .needs
            .iter()
            .filter(|need| state_names.contains(&need.resource))
            .collect();
        if !missing.is_empty() {
            return Some(
                json!({"code":"context_state_incompatible","node":node.id,"needs":missing}),
            );
        }
        if !evaluation.diagnostics.is_empty()
            && evaluation
                .reads
                .iter()
                .all(|name| state_names.contains(name))
        {
            return Some(
                json!({"code":"context_state_incompatible","node":node.id,"diagnostics":evaluation.diagnostics}),
            );
        }
    }
    None
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevisionPublication {
    pub run_id: String,
    pub instance: String,
    pub baseline: Composition,
    pub definition: RevisionDefinition,
}
/// Validate the complete publication set before one atomic transaction installs
/// its definition contents and all run/instance heads. Active steps are untouched.
pub async fn publish_batch(
    store: &ContentStore,
    publications: &[RevisionPublication],
) -> Result<Vec<Compatibility>> {
    publish_batch_impl(store, None, publications, &[]).await
}
/// Recovery identity is committed with the heads. Replaying an old batch returns
/// its prior outcome and cannot move a subsequently advanced head backwards.
pub async fn publish_batch_unique(
    store: &ContentStore,
    batch_id: &str,
    publications: &[RevisionPublication],
) -> Result<Vec<Compatibility>> {
    uuid::Uuid::parse_str(batch_id).context("Publication batch identity must be a UUID")?;
    publish_batch_impl(store, Some(batch_id), publications, &[]).await
}
/// A graph publication changes future route selections; existing visits retain
/// their captured plan. This does not replace the native ADK executor.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGraphPublication {
    pub run_id: String,
    pub baseline: PreparedRuntime,
    pub prepared: PreparedRuntime,
}

pub fn runtime_graph_compatibility(
    before: &PreparedRuntime,
    after: &PreparedRuntime,
) -> Result<Compatibility> {
    before.validate(&RuntimePrimitives)?;
    after.validate(&RuntimePrimitives)?;
    let mut reasons = Vec::new();
    let a = serde_json::to_value(&before.graph)?;
    let b = serde_json::to_value(&after.graph)?;
    for field in [
        "entry",
        "types",
        "instances",
        "aliases",
        "dataBindings",
        "inferences",
    ] {
        if a[field] != b[field] {
            reasons.push(format!("runtime_{field}_requires_rebuild"));
        }
    }
    if !before.graph.routes.keys().eq(after.graph.routes.keys()) {
        reasons.push("runtime_route_set_requires_rebuild".into());
    }
    for (id, route) in &before.graph.routes {
        if let Some(next) = after.graph.routes.get(id) {
            let a = serde_json::to_value(route)?;
            let b = serde_json::to_value(next)?;
            for field in [
                "bridge",
                "from",
                "invocation",
                "toolName",
                "input",
                "output",
            ] {
                if a[field] != b[field] {
                    reasons.push(format!("runtime_route_contract_changed:{id}:{field}"));
                }
            }
        }
    }
    for (instance, flow) in &before.flows {
        if let Some(next) = after.flows.get(instance)
            && (flow.key != next.key
                || !compatibility(&flow.composition, &next.composition)?.can_publish()
                || zf_flows::schema::answer_paths(&flow.composition)?
                    != zf_flows::schema::answer_paths(&next.composition)?)
        {
            reasons.push(format!("runtime_instance_incompatible:{instance}"));
        }
    }
    Ok(if reasons.is_empty() {
        Compatibility::Live
    } else {
        Compatibility::Incompatible { reasons }
    })
}

pub fn validate_runtime_graph_publications(
    publications: &[RuntimeGraphPublication],
) -> Result<Vec<Compatibility>> {
    let mut seen = std::collections::BTreeSet::new();
    publications
        .iter()
        .map(|publication| {
            ensure!(
                !publication.run_id.is_empty() && seen.insert(&publication.run_id),
                "Runtime graph publication targets must be unique"
            );
            let result = runtime_graph_compatibility(&publication.baseline, &publication.prepared)?;
            ensure!(
                result.can_publish(),
                "Runtime graph cannot be adopted by {}: {}",
                publication.run_id,
                serde_json::to_string(&result)?
            );
            Ok(result)
        })
        .collect()
}

/// One recovery identity, CAS insertion and SQL commit for both kinds of heads.
/// There is no interval in which a running step can observe half a publication.
pub async fn publish_mixed_unique(
    store: &ContentStore,
    batch_id: &str,
    publications: &[RevisionPublication],
    runtime_graphs: &[RuntimeGraphPublication],
) -> Result<Vec<Compatibility>> {
    uuid::Uuid::parse_str(batch_id).context("Publication batch identity must be a UUID")?;
    publish_batch_impl(store, Some(batch_id), publications, runtime_graphs).await
}

async fn publish_batch_impl(
    store: &ContentStore,
    batch_id: Option<&str>,
    publications: &[RevisionPublication],
    runtime_graphs: &[RuntimeGraphPublication],
) -> Result<Vec<Compatibility>> {
    // Keep historical flow-only receipts replayable.
    let digest = key(&if runtime_graphs.is_empty() {
        serde_json::to_string(publications)?
    } else {
        serde_json::to_string(&(publications, runtime_graphs))?
    });
    let mut compatibility_results = validate_publications(publications)?;
    compatibility_results.extend(validate_runtime_graph_publications(runtime_graphs)?);
    let definitions: Vec<_> = publications
        .iter()
        .map(|item| json!(item.definition))
        .collect();
    let graphs: Vec<_> = runtime_graphs
        .iter()
        .map(|item| json!(item.prepared))
        .collect();
    let revisions: Vec<_> = publications
        .iter()
        .map(|item| item.definition.revision())
        .collect();
    let definition_writes: Vec<_> = publications
        .iter()
        .zip(&definitions)
        .zip(&revisions)
        .map(
            |((item, value), revision)| zf_storage::revision_publications::DefinitionWrite {
                run_id: &item.run_id,
                instance: &item.instance,
                hash: revision,
                value,
            },
        )
        .collect();
    let graph_writes: Vec<_> = runtime_graphs
        .iter()
        .zip(&graphs)
        .map(
            |(item, value)| zf_storage::revision_publications::RuntimeGraphWrite {
                run_id: &item.run_id,
                value,
            },
        )
        .collect();
    let outcomes = zf_storage::revision_publications::publish(
        store,
        batch_id,
        &digest,
        &definition_writes,
        &graph_writes,
        &json!(compatibility_results),
    )
    .await?;
    Ok(serde_json::from_value(outcomes)?)
}

/// Recover the source already selected at a committed checkpoint. This does not
/// adopt a published head: future passages still select through the wrapper.
pub async fn checkpoint_definition(
    store: &ContentStore,
    run_id: &str,
    thread_id: &str,
    step: usize,
    scope: &str,
) -> Result<Option<RevisionDefinition>> {
    let pin_key = key(&format!("{thread_id}\0{scope}\0{step}"));
    let pin = if let Some(pin) = store.record(run_id, "revision-steps", &pin_key).await? {
        Some(pin)
    } else {
        store
            .record(
                run_id,
                "revision-active",
                &key(&format!("{thread_id}\0{scope}")),
            )
            .await?
            .filter(|pin| {
                pin["step"]
                    .as_u64()
                    .is_some_and(|selected| selected <= step as u64)
            })
    };
    let Some(pin) = pin else { return Ok(None) };
    ensure!(
        pin["threadId"] == thread_id && pin["scope"] == scope,
        "Checkpoint revision belongs to another execution scope"
    );
    let reference = pin["definitionRef"]
        .as_str()
        .context("Checkpoint definition reference absent")?;
    let definition: RevisionDefinition = serde_json::from_value(store.resolve(reference).await?)?;
    validate_definition(&definition)?;
    ensure!(
        pin["hash"] == definition.hash
            && pin.get("definitionRevision").map_or(
                definition.package.is_none() && definition.context_selections.is_empty(),
                |revision| revision == &definition.revision()
            ),
        "Checkpoint definition revision mismatch"
    );
    Ok(Some(definition))
}

pub async fn load_boundary(
    store: &ContentStore,
    run_id: &str,
    thread_id: &str,
    step: usize,
    scope: &str,
) -> Result<Option<Value>> {
    let pin_key = key(&format!("{thread_id}\0{scope}\0{step}"));
    // A real node may subsequently interrupt at this same step. Once pinned,
    // its earlier rebuild request must no longer mask that user-visible wait.
    if store
        .record(run_id, "revision-steps", &pin_key)
        .await?
        .is_some()
    {
        return Ok(None);
    }
    store.record(run_id, "revision-boundaries", &pin_key).await
}

pub fn validate_publications(publications: &[RevisionPublication]) -> Result<Vec<Compatibility>> {
    let mut seen = std::collections::BTreeSet::new();
    publications
        .iter()
        .map(|publication| {
            ensure!(
                !publication.run_id.is_empty()
                    && seen.insert((&publication.run_id, &publication.instance)),
                "Publication targets must be unique"
            );
            validate_definition(&publication.definition)?;
            let compatible =
                compatibility(&publication.baseline, &publication.definition.composition)?;
            ensure!(
                compatible.can_publish(),
                "Definition cannot be adopted by runtime {}: {}",
                publication.run_id,
                serde_json::to_string(&compatible)?
            );
            Ok(compatible)
        })
        .collect()
}

/// Read graph and flow heads from one SQLite snapshot, then resolve immutable
/// values outside the statement. No authoring filesystem is consulted here.
pub async fn latest_runtime_graph(
    store: &ContentStore,
    run_id: &str,
) -> Result<Option<PreparedRuntime>> {
    latest_runtime_graph_cached(store, run_id, &Mutex::new(None)).await
}

struct RuntimeGraphSnapshot {
    heads: Vec<(String, String, String)>,
    plan: Option<PreparedRuntime>,
}

async fn latest_runtime_graph_cached(
    store: &ContentStore,
    run_id: &str,
    cache: &Mutex<Option<RuntimeGraphSnapshot>>,
) -> Result<Option<PreparedRuntime>> {
    // Always sample all mutable heads together. Cached content is reusable only
    // when every immutable reference matches this new database snapshot.
    let heads: Vec<(String, String, String)> = sqlx::query_as("SELECT kind,key,value_ref FROM zf_records WHERE scope=? AND kind IN ('runtime-graph-heads','revision-heads') ORDER BY kind,key")
        .bind(run_id).fetch_all(store.pool()).await?;
    if let Some(snapshot) = cache.lock().unwrap_or_else(|p| p.into_inner()).as_ref()
        && snapshot.heads == heads
    {
        return Ok(snapshot.plan.clone());
    }
    let plan = resolve_runtime_graph_heads(store, &heads).await?;
    *cache.lock().unwrap_or_else(|p| p.into_inner()) = Some(RuntimeGraphSnapshot {
        heads,
        plan: plan.clone(),
    });
    Ok(plan)
}

async fn resolve_runtime_graph_heads(
    store: &ContentStore,
    heads: &[(String, String, String)],
) -> Result<Option<PreparedRuntime>> {
    let mut plan = None;
    let mut definitions = Vec::new();
    for (kind, _, reference) in heads {
        let value = store.resolve(reference).await?;
        if kind == "runtime-graph-heads" {
            plan = Some(serde_json::from_value::<PreparedRuntime>(
                store
                    .resolve(
                        value["graphRef"]
                            .as_str()
                            .context("Runtime graph head missing")?,
                    )
                    .await?,
            )?);
        } else {
            definitions.push(value);
        }
    }
    let Some(mut plan) = plan else {
        return Ok(None);
    };
    for head in definitions {
        let instance = head["instance"]
            .as_str()
            .context("Revision instance absent")?;
        if !plan.flows.contains_key(instance) {
            continue;
        }
        let definition: RevisionDefinition = serde_json::from_value(
            store
                .resolve(
                    head["definitionRef"]
                        .as_str()
                        .context("Revision reference absent")?,
                )
                .await?,
        )?;
        definition.apply_to(&mut plan, instance)?;
    }
    plan.validate(&RuntimePrimitives)?;
    Ok(Some(plan))
}

/// First native node in a super-step wins across all parallel scopes. This also
/// captures the graph before a model or route node is eventually reached.
async fn pin_runtime_graph(
    store: &ContentStore,
    run_id: &str,
    thread_id: &str,
    step: usize,
    cache: &Mutex<Option<RuntimeGraphSnapshot>>,
) -> Result<Option<(String, PreparedRuntime)>> {
    let pin_key = key(&format!("{thread_id}\0{step}"));
    if let Some(pin) = store
        .record(run_id, "runtime-graph-steps", &pin_key)
        .await?
    {
        let reference = pin["graphRef"]
            .as_str()
            .context("Graph step reference missing")?
            .to_owned();
        return Ok(Some((
            reference.clone(),
            serde_json::from_value(store.resolve(&reference).await?)?,
        )));
    }
    let Some(plan) = latest_runtime_graph_cached(store, run_id, cache).await? else {
        return Ok(None);
    };
    let value = json!(plan);
    let reference = store.intern(&value).await?;
    store
        .claim_record(run_id, "runtime-graph-definitions", &reference, &value)
        .await?;
    store
        .claim_record(
            run_id,
            "runtime-graph-steps",
            &pin_key,
            &json!({"graphRef":reference,"threadId":thread_id,"step":step}),
        )
        .await?;
    let pin = store
        .record(run_id, "runtime-graph-steps", &pin_key)
        .await?
        .context("Graph step claim absent")?;
    let claimed_reference = pin["graphRef"]
        .as_str()
        .context("Graph step reference missing")?
        .to_owned();
    // A concurrent scope may have won the claim. Reuse our validated plan only
    // when the durable winner has the same content identity.
    let plan = if claimed_reference == reference {
        plan
    } else {
        serde_json::from_value(store.resolve(&claimed_reference).await?)?
    };
    Ok(Some((claimed_reference, plan)))
}
