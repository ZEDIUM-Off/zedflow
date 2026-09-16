//! Materialize canonical compiler plans as native ADK graphs. Execution,
//! checkpoint scheduling, retries and node contexts remain owned by ADK.
use crate::operations;
use adk_graph::{
    CompiledGraph, StateGraph,
    checkpoint::Checkpointer,
    edge::{Edge, EdgeTarget},
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{collections::HashMap, sync::Arc};
use zf_compiler::{
    compiler::CompiledPlan,
    graph_compiler::{self, PrimitiveContracts},
    plan::{self, GraphPlan, PlannedEdge, PlannedNode},
};
use zf_flows::schema::{Composition, Node};

/// Concrete, side-effect-free primitive catalogue used at the compiler boundary.
/// Validation delegates to the same model and tool implementations that execute.
pub struct RuntimePrimitives;

impl PrimitiveContracts for RuntimePrimitives {
    fn validate_model(&self, config: &Value) -> Result<()> {
        crate::models::validate(config)
    }

    fn has_tool(&self, name: &str) -> bool {
        operations::tool_declarations().contains_key(name)
    }
}

/// Materialize the root public entry captured by compilation, without repeating
/// graph lowering. Other instances require `build_compiled_entry`.
///
/// # Errors
/// Returns an error for a non-root instance, unsupported runtime primitives or
/// an ADK construction error. No model or tool is invoked while building.
pub fn build_compiled(
    compiled: &CompiledPlan,
    instance: &str,
    services: Option<Arc<crate::runtime::RunServices>>,
    sender: Option<crate::event_sink::EventSink>,
    checkpointer: Option<Arc<dyn Checkpointer>>,
) -> Result<CompiledGraph> {
    let entry = &compiled.prepared().graph.entry;
    ensure!(
        instance == entry.instance,
        "A non-root instance requires an explicit public entry"
    );
    build_compiled_entry(
        compiled,
        instance,
        &entry.port,
        services,
        sender,
        checkpointer,
    )
}

/// Materialize one public entry captured in the immutable compilation plan.
/// The compiler owns projection; runtime never recreates an authored start edge.
pub fn build_compiled_entry(
    compiled: &CompiledPlan,
    instance: &str,
    entry: &str,
    services: Option<Arc<crate::runtime::RunServices>>,
    sender: Option<crate::event_sink::EventSink>,
    checkpointer: Option<Arc<dyn Checkpointer>>,
) -> Result<CompiledGraph> {
    let entry = compiled
        .entry(instance, entry)
        .context("Compiled public entry is absent")?;
    let doc = entry.composition();
    let plan = entry.graph();
    // A plan may have been compiled against another primitive catalogue. Require
    // the real runtime contracts before any native node is constructed.
    graph_compiler::validate(doc, &RuntimePrimitives)?;
    let revisions = services.as_ref().and_then(|services| services.revisions());
    let checkpointer = checkpointer.or_else(|| {
        services.as_ref().map(|_| {
            Arc::new(adk_graph::checkpoint::MemoryCheckpointer::new()) as Arc<dyn Checkpointer>
        })
    });
    materialize_scope(
        doc,
        plan,
        BuildScope {
            sender,
            scope: &format!("{instance}/"),
            services,
            checkpointer,
            native: None,
            revisions,
        },
    )
}

pub fn build(doc: &Composition) -> Result<CompiledGraph> {
    build_observed(doc, None)
}
pub fn build_observed(
    doc: &Composition,
    sender: Option<crate::event_sink::EventSink>,
) -> Result<CompiledGraph> {
    build_scope(doc, sender, "", None, None)
}

pub fn build_with_services(
    doc: &Composition,
    services: Arc<crate::runtime::RunServices>,
    sender: Option<crate::event_sink::EventSink>,
    checkpointer: Option<Arc<dyn Checkpointer>>,
) -> Result<CompiledGraph> {
    let checkpointer =
        checkpointer.unwrap_or_else(|| Arc::new(adk_graph::checkpoint::MemoryCheckpointer::new()));
    build_scope(doc, sender, "", Some(services), Some(checkpointer))
}

fn runtime_node(
    doc: &Composition,
    node: &Node,
    planned: &PlannedNode,
    sender: Option<crate::event_sink::EventSink>,
    scope: &str,
    services: Option<Arc<crate::runtime::RunServices>>,
    checkpointer: Option<Arc<dyn Checkpointer>>,
) -> Result<Arc<dyn adk_graph::Node>> {
    let kind = node.data.kind.clone();
    let config = planned.config.clone();
    let path = format!("{scope}{}", node.id);
    let inner: Arc<dyn adk_graph::Node> = if kind == "subgraph" {
        let child: Composition = serde_json::from_value(config["composition"].clone())?;
        let child_plan = planned
            .child
            .as_deref()
            .context("Subgraph plan is absent")?;
        let child_scope = format!("{scope}{}/", node.id);
        let revisions = services.as_ref().and_then(|services| services.revisions());
        let compiled = Arc::new(materialize_scope(
            &child,
            child_plan,
            BuildScope {
                sender: sender.clone(),
                scope: &child_scope,
                services: services.clone(),
                checkpointer,
                native: None,
                revisions,
            },
        )?);
        if services.is_some() {
            Arc::new(crate::subgraphs::ResumableSubgraph::new(
                &node.id,
                compiled,
                child_plan.answer_paths().to_vec(),
            ))
        } else {
            Arc::new(
                adk_graph::subgraph::SubgraphNode::new(&node.id, compiled)
                    .isolated()
                    .with_input("input", "input")
                    .with_output("response", "output"),
            )
        }
    } else if kind == "model" || (kind == "context" && doc.format_version >= 3) {
        let services = services.clone().ok_or_else(|| {
            anyhow::anyhow!("Les nœuds Contexte et Modèle nécessitent les services d’exécution")
        })?;
        let peer_key = if kind == "model" {
            "contextNode"
        } else {
            "modelNode"
        };
        let peer = doc
            .nodes
            .iter()
            .find(|peer| Some(peer.id.as_str()) == config[peer_key].as_str())
            .ok_or_else(|| anyhow::anyhow!("{} : nœud associé absent", node.data.label))?;
        let peer_config = graph_compiler::config(peer, doc.format_version);
        if kind == "model" {
            Arc::new(crate::models::inference_node_with_services(
                &node.id,
                &config,
                &peer_config,
                &path,
                services,
            )?)
        } else {
            Arc::new(crate::models::context_node_with_services(
                &node.id,
                &config,
                &peer_config,
                &path,
                services,
            )?)
        }
    } else if kind == "agent" {
        if let Some(services) = services.clone() {
            Arc::new(crate::models::node_with_services(
                &node.id, &config, &path, services,
            )?)
        } else {
            ensure!(
                config["modelBinding"] != "runtime",
                "Un modèle runtime nécessite les services d'exécution"
            );
            Arc::new(crate::models::node(&node.id, &config)?)
        }
    } else {
        let captured_services = services.clone();
        let captured_path = path.clone();
        Arc::new(adk_graph::node::FunctionNode::new(&node.id, move |ctx| {
            let kind = kind.clone();
            let config = config.clone();
            let services = captured_services.clone();
            let path = captured_path.clone();
            async move {
                match services {
                    Some(services) => {
                        operations::execute_with_services(&kind, &config, ctx, &path, services)
                            .await
                    }
                    None => operations::execute(&kind, &config, ctx).await,
                }
            }
        }))
    };
    Ok(if let Some(sender) = sender {
        crate::observation::wrap(
            inner,
            sender,
            node.data.label.clone(),
            node.data.kind.clone(),
            node.data.config.clone(),
            path,
            services.as_ref().and_then(|s| s.content_store()),
        )
    } else {
        inner
    })
}

pub fn build_scope(
    doc: &Composition,
    sender: Option<crate::event_sink::EventSink>,
    scope: &str,
    services: Option<Arc<crate::runtime::RunServices>>,
    checkpointer: Option<Arc<dyn Checkpointer>>,
) -> Result<CompiledGraph> {
    build_scope_with_native(doc, sender, scope, services, checkpointer, None)
}

/// Reuse nodes compiled from an exact structured Rust module while projecting
/// only the selected public entry's control graph. Observation and revision
/// wrappers are applied in the same order as the ordinary daemon compiler.
pub fn build_scope_with_native(
    doc: &Composition,
    sender: Option<crate::event_sink::EventSink>,
    scope: &str,
    services: Option<Arc<crate::runtime::RunServices>>,
    checkpointer: Option<Arc<dyn Checkpointer>>,
    native: Option<&CompiledGraph>,
) -> Result<CompiledGraph> {
    let revisions = services.as_ref().and_then(|services| services.revisions());
    build_scope_with_native_and_revisions(
        doc,
        sender,
        scope,
        services,
        checkpointer,
        native,
        revisions,
    )
}

/// A routed visit retains the baseline of its own compiled source. Rebasing one
/// child must not change the controller used to compile another child.
pub fn build_scope_with_native_and_revisions(
    doc: &Composition,
    sender: Option<crate::event_sink::EventSink>,
    scope: &str,
    services: Option<Arc<crate::runtime::RunServices>>,
    checkpointer: Option<Arc<dyn Checkpointer>>,
    native: Option<&CompiledGraph>,
    revisions: Option<Arc<crate::revisions::RevisionRuntime>>,
) -> Result<CompiledGraph> {
    let plan = plan::lower(doc, &RuntimePrimitives)?;
    materialize_scope(
        doc,
        &plan,
        BuildScope {
            sender,
            scope,
            services,
            checkpointer,
            native,
            revisions,
        },
    )
}

struct BuildScope<'a> {
    sender: Option<crate::event_sink::EventSink>,
    scope: &'a str,
    services: Option<Arc<crate::runtime::RunServices>>,
    checkpointer: Option<Arc<dyn Checkpointer>>,
    native: Option<&'a CompiledGraph>,
    revisions: Option<Arc<crate::revisions::RevisionRuntime>>,
}

fn materialize_scope(
    doc: &Composition,
    plan: &GraphPlan,
    build: BuildScope<'_>,
) -> Result<CompiledGraph> {
    let BuildScope {
        sender,
        scope,
        services,
        checkpointer,
        native,
        revisions,
    } = build;
    let services = services
        .map(|services| services.for_composition(doc, scope))
        .transpose()?;
    let mut channels = plan.channels().clone();
    if let Some(host) = services
        .as_ref()
        .and_then(|services| services.dynamic_capabilities())
    {
        let channels = channels
            .as_array_mut()
            .ok_or_else(|| anyhow::anyhow!("Canaux absents"))?;
        for name in host.resume_channels() {
            if !channels.iter().any(|c| c["name"] == name) {
                channels.push(json!({"name":name,"reducer":"overwrite"}));
            }
        }
    }
    let mut graph = StateGraph::new(operations::state_schema(&channels)?);
    for planned in plan.nodes() {
        let node = doc
            .nodes
            .iter()
            .find(|node| node.id == planned.id)
            .context("Planned node is absent from frozen composition")?;
        let initial = if let Some(native) = native {
            let inner = native
                .node(&node.id)
                .ok_or_else(|| anyhow::anyhow!("native source has no node {}", node.id))?;
            if let Some(sender) = sender.clone() {
                crate::observation::wrap(
                    inner,
                    sender,
                    node.data.label.clone(),
                    node.data.kind.clone(),
                    node.data.config.clone(),
                    format!("{scope}{}", node.id),
                    services.as_ref().and_then(|s| s.content_store()),
                )
            } else {
                inner
            }
        } else {
            runtime_node(
                doc,
                node,
                planned,
                sender.clone(),
                scope,
                services.clone(),
                checkpointer.clone(),
            )?
        };
        let inner = if let Some(revisions) = &revisions {
            let sender = sender.clone();
            let scope = scope.to_owned();
            let services = services.clone();
            let checkpointer = checkpointer.clone();
            let factory_scope = scope.clone();
            let factory: crate::revisions::NodeFactory = Arc::new(move |doc, node| {
                let plan = plan::lower(doc, &RuntimePrimitives)?;
                let planned = plan
                    .nodes()
                    .iter()
                    .find(|planned| planned.id == node.id)
                    .context("Revision node is absent from compiled plan")?;
                runtime_node(
                    doc,
                    node,
                    planned,
                    sender.clone(),
                    &factory_scope,
                    services.clone(),
                    checkpointer.clone(),
                )
            });
            revisions.wrap(&scope, &node.id, initial, factory)
        } else {
            initial
        };
        let inner = if revisions.is_some()
            && let Some(sender) = sender.clone()
        {
            crate::observation::wrap_preparation(
                inner,
                sender,
                node.data.label.clone(),
                node.data.kind.clone(),
                format!("{scope}{}", node.id),
            )
        } else {
            inner
        };
        graph.nodes.insert(node.id.clone(), inner);
    }
    for edge in plan.edges() {
        match edge {
            PlannedEdge::Direct { source, target } => {
                graph = graph.add_edge(source, target);
            }
            PlannedEdge::Alternative { source, target } => {
                graph.edges.push(Edge::Conditional {
                    source: source.clone(),
                    router: Arc::new(|_| "next".into()),
                    targets: HashMap::from([("next".into(), EdgeTarget::from(target.as_str()))]),
                });
            }
            PlannedEdge::Conditional {
                source,
                field,
                expected,
                yes,
                no,
            } => {
                let field = field.clone();
                let expected = expected.clone();
                graph.edges.push(Edge::Conditional {
                    source: source.clone(),
                    router: Arc::new(move |state| {
                        if state.get(&field) == Some(&expected) {
                            "true".into()
                        } else {
                            "false".into()
                        }
                    }),
                    targets: HashMap::from([
                        ("true".into(), EdgeTarget::from(yes.as_str())),
                        ("false".into(), EdgeTarget::from(no.as_str())),
                    ]),
                });
            }
        }
    }
    let retries: Vec<_> = plan
        .nodes()
        .iter()
        .filter_map(|node| {
            node.config
                .get("retry")
                .filter(|retry| retry.is_object())
                .map(|retry| (node.id.as_str(), retry.clone()))
        })
        .collect();
    let mut compiled = operations::configure(
        graph.compile()?,
        &serde_json::to_value(plan.settings())?,
        &retries,
    );
    if let Some(checkpointer) = checkpointer {
        compiled = compiled.with_checkpointer_arc(checkpointer);
    }
    Ok(compiled)
}
