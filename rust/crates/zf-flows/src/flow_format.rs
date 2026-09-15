//! A deliberately bounded Rust source format, parsed without executing Rust.
//!
//! Native ADK node registrations and edge expressions are the executable source
//! of truth. Constants contain presentation data only; literal `json!` values
//! are the configurations actually supplied to the existing runtime primitives.
use crate::schema::*;
use anyhow::{Context, Result, bail, ensure};
use proc_macro2::{Span, TokenStream};
use quote::ToTokens;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use syn::{
    Expr, Item, Lit, Stmt, Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned,
    visit::Visit,
    visit_mut::VisitMut,
};

const MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;
// Context types add a `fields` object per semantic level, inside the frozen
// node configuration. This representation budget does not relax their limit.
const MAX_CONFIG_JSON_DEPTH: usize = 256;
const LEGACY_PRELUDE: &str = "#![allow(dead_code, unused_imports)]\nuse adk_graph::prelude::*;\nuse adk_graph::edge::{Edge, EdgeTarget};\nuse std::{collections::HashMap, sync::Arc};\nuse crate::{models, operations, runtime, subgraphs};\n";
const PRELUDE: &str = "#![allow(dead_code, unused_imports)]\nuse adk_graph::prelude::*;\nuse adk_graph::edge::{Edge, EdgeTarget};\nuse std::{collections::HashMap, sync::Arc};\nuse zf_runtime::{models, operations, runtime, subgraphs};\n";
const SIGNATURE: &str = "services: Arc<runtime::RunServices>, checkpointer: Arc<dyn adk_graph::checkpoint::Checkpointer>";

/// Complete semantic validation supplied by the compiler. Reading or rendering
/// a source never silently skips this boundary. Implementations must validate
/// nested graphs as well as the root; the codec itself owns syntax and limits.
pub trait SourceValidator: Send + Sync {
    fn validate(&self, document: &Composition) -> Result<()>;
}

impl<F> SourceValidator for F
where
    F: Fn(&Composition) -> Result<()> + Send + Sync,
{
    fn validate(&self, document: &Composition) -> Result<()> {
        self(document)
    }
}

/// Render the complete supported catalog, including inline nested compositions.
pub fn render(doc: &Composition, validator: &dyn SourceValidator) -> Result<String> {
    validator.validate(doc)?;
    let source = render_scope(doc, false)?;
    ensure!(
        source.len() <= MAX_SOURCE_BYTES,
        "Source Rust limitée à 2 Mio"
    );
    Ok(source)
}

/// Parse only the documented structured subset. No macro, compiler, process,
/// external module or user expression is executed, including on invalid files.
pub fn parse(source: &str, validator: &dyn SourceValidator) -> Result<Composition> {
    ensure!(
        source.len() <= MAX_SOURCE_BYTES,
        "Source Rust limitée à 2 Mio"
    );
    zf_context::context_source::parse_on_source_stack(source, |source| {
        parse_inner(source, validator).map_err(|error| {
            vec![zf_core::diagnostics::Diagnostic::new(
                "flow_source",
                "$source",
                format!("{error:#}"),
            )]
        })
    })
    .map_err(|diagnostics| {
        anyhow::anyhow!(
            "{}",
            diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.message)
                .collect::<Vec<_>>()
                .join("; ")
        )
    })
}

// The complete AST lifetime, including normalization and destruction, stays on
// the checked parsing stack; only the owned composition crosses its boundary.
fn parse_inner(source: &str, validator: &dyn SourceValidator) -> Result<Composition> {
    let file = syn::parse_file(source).map_err(syntax_error)?;
    let doc = extract_scope(&file.items, 0)?;
    validator.validate(&doc)?;
    // Extraction is intentionally small. A second, exhaustive comparison makes
    // every other expression, item, attribute and closure body fail closed.
    let expected = syn::parse_file(&render_scope(&doc, false)?).map_err(syntax_error)?;
    let actual = normalized(file)?;
    let legacy = syn::parse_file(&render_scope(&doc, true)?).map_err(syntax_error)?;
    ensure!(
        actual == normalized(expected)? || actual == normalized(legacy)?,
        "Rust hors du format Zedflow structuré : appel, attribut, déclaration ou corps de fonction non reconnu ; le fichier reste consultable sans être exécuté ni réécrit"
    );
    Ok(doc)
}

/// Explicit conversion produces a new draft; it never rewrites the original flow
/// or the source/checkpoints of a session which already started.
pub fn convert_v1(doc: &Composition, validator: &dyn SourceValidator) -> Result<Composition> {
    validator.validate(doc)?;
    ensure!(doc.format_version == 1, "Ce flow est déjà en version 2");
    let mut copy = doc.clone();
    copy.id = uuid::Uuid::new_v4().to_string();
    copy.name = format!("{} · v2", doc.name);
    copy.revision = 0;
    copy.format_version = 2;
    while let Some(index) = copy
        .nodes
        .iter()
        .position(|node| node.data.kind == "context")
    {
        let node = &copy.nodes[index];
        let incoming: Vec<_> = copy
            .edges
            .iter()
            .filter(|edge| edge.target == node.id)
            .cloned()
            .collect();
        let outgoing: Vec<_> = copy
            .edges
            .iter()
            .filter(|edge| edge.source == node.id)
            .cloned()
            .collect();
        ensure!(
            incoming.len() == 1
                && outgoing.len() == 1
                && incoming[0].source != node.id
                && outgoing[0].target != node.id,
            "{} : conversion du contexte impossible avec un branchement ou une jonction ; adaptez une copie manuellement",
            node.data.label
        );
        ensure!(
            node.data
                .config
                .as_object()
                .is_none_or(|config| config.keys().all(|key| key == "nodeId")),
            "{} : le contexte possède une configuration à transposer manuellement",
            node.data.label
        );
        ensure!(
            !copy
                .nodes
                .iter()
                .filter(|other| other.id != node.id)
                .any(|other| other.data.config.to_string().contains("__zedflow:context")),
            "Le flow utilise l'état de l'ancien nœud contexte ; conversion automatique impossible"
        );
        let removed = node.id.clone();
        copy.edges.retain(|edge| edge.source != removed);
        for edge in &mut copy.edges {
            if edge.target == removed {
                edge.target = outgoing[0].target.clone();
            }
        }
        copy.nodes.remove(index);
    }
    for node in &mut copy.nodes {
        if ["agent", "condition"].contains(&node.data.kind.as_str()) && node.data.config.is_null() {
            node.data.config = serde_json::json!({});
        }
        if node.data.kind == "subgraph" {
            let child: Composition =
                serde_json::from_value(node.data.config["composition"].clone())?;
            node.data.config["composition"] = serde_json::to_value(convert_v1(&child, validator)?)?;
        } else if node.data.kind == "condition" {
            ensure!(
                node.data.config.get("predicate").is_none(),
                "Une condition v1 contient déjà un prédicat v2 ambigu"
            );
            let field = node.data.config["field"]
                .as_str()
                .unwrap_or("input")
                .to_owned();
            let value = node
                .data
                .config
                .get("equals")
                .cloned()
                .unwrap_or(Value::Bool(true));
            // A legacy field beginning with '/' is an exact key, not a pointer.
            let field = if field.starts_with('/') {
                format!("/{}", field.replace('~', "~0").replace('/', "~1"))
            } else {
                field
            };
            node.data.config["predicate"] =
                serde_json::json!({"kind":"compare","field":field,"operator":"eq","value":value});
            if let Some(config) = node.data.config.as_object_mut() {
                config.remove("equals");
                config.remove("field");
            }
        } else if node.data.kind == "agent" {
            ensure!(
                node.data.config.get("attachments").is_none(),
                "{} : pièces déjà présentes dans une configuration v1 ambiguë",
                node.data.label
            );
            let config = node
                .data
                .config
                .as_object_mut()
                .context("Configuration d'agent objet requise pour la conversion")?;
            let tools = config
                .remove("tools")
                .unwrap_or_else(|| serde_json::json!([]));
            let global = config
                .remove("globalInstructions")
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_default();
            let instruction = config
                .remove("instructions")
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| "Réponds de manière concise en français.".into());
            let mut attachments = serde_json::json!({"instructions":{"items":[
                {"id":"workspace-instructions","source":{"kind":"workspace"}},
                {"id":"authored-instructions","source":{"kind":"text","text":format!("{global}\n{instruction}")},"mode":"template"}
            ]},"tools":{"items":tools.as_array().into_iter().flatten().enumerate().map(|(index,name)|serde_json::json!({"id":format!("tool-{index}"),"name":name})).collect::<Vec<_>>()}});
            if tools
                .as_array()
                .is_some_and(|tools| tools.iter().any(|name| name == "read" || name == "exec"))
            {
                attachments["skills"] = serde_json::json!({"items":[{"id":"workspace-skills","source":{"kind":"workspace"},"activation":"explicit"}]});
            }
            config.insert("attachments".into(), attachments);
        }
    }
    validator.validate(&copy)?;
    Ok(copy)
}

fn rust_string(value: &str) -> String {
    format!("{value:?}")
}

fn optional_string(value: Option<&str>) -> String {
    value.map_or_else(
        || "None".into(),
        |text| format!("Some({})", rust_string(text)),
    )
}

// JSON values use Rust string escapes inside the actual json! macro. JSON text
// alone is not valid Rust for control characters such as \u0000, \b or \f.
fn rust_json(value: &Value) -> String {
    match value {
        Value::String(value) => rust_string(value),
        Value::Array(values) => format!(
            "[{}]",
            values.iter().map(rust_json).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(values) => format!(
            "{{{}}}",
            values
                .iter()
                .map(|(key, value)| format!("{}: {}", rust_string(key), rust_json(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Number(value)
            if value
                .as_i64()
                .is_some_and(|value| i32::try_from(value).is_err()) =>
        {
            format!("{value}_i64")
        }
        Value::Number(value) if value.as_u64().is_some_and(|value| value > i32::MAX as u64) => {
            format!("{value}_u64")
        }
        _ => value.to_string(),
    }
}

fn render_scope(doc: &Composition, legacy_imports: bool) -> Result<String> {
    let prelude = if legacy_imports {
        LEGACY_PRELUDE
    } else {
        PRELUDE
    };
    let mut source = format!(
        "// @zedflow format v{}. See docs/flow-format.md.\n{prelude}\nconst ZEDFLOW_FORMAT: u32 = {};\n// @zedflow flow-view: identity, name, revision\nconst FLOW: (&str, &str, i64) = ({}, {}, {});\n",
        doc.format_version,
        doc.format_version,
        rust_string(&doc.id),
        rust_string(&doc.name),
        doc.revision
    );
    for (index, node) in doc.nodes.iter().enumerate() {
        ensure!(
            node.position.x.is_finite() && node.position.y.is_finite(),
            "{} : position non finie",
            node.id
        );
        source.push_str(&format!(
            "// @zedflow node-view {index}: identity, renderer, label, x, y\nconst NODE_{index}: (&str, &str, &str, f64, f64) = ({}, {}, {}, {:?}, {:?});\n",
            rust_string(&node.id),
            rust_string(&node.node_type),
            rust_string(&node.data.label),
            node.position.x,
            node.position.y
        ));
    }
    for (index, edge) in doc.edges.iter().enumerate() {
        if doc.format_version >= 4 {
            source.push_str(&format!("// @zedflow edge-view {index}: identity, output port, input port, label\nconst EDGE_{index}: (&str, Option<&str>, Option<&str>, Option<&str>) = ({}, {}, {}, {});\n",rust_string(&edge.id),optional_string(edge.source_handle.as_deref()),optional_string(edge.target_handle.as_deref()),optional_string(edge.label.as_deref())));
        } else {
            source.push_str(&format!(
            "// @zedflow edge-view {index}: identity, source handle, label\nconst EDGE_{index}: (&str, Option<&str>, Option<&str>) = ({}, {}, {});\n",
            rust_string(&edge.id),
            optional_string(edge.source_handle.as_deref()),
            optional_string(edge.label.as_deref())
        ));
        }
    }
    for (index, node) in doc.nodes.iter().enumerate() {
        if node.data.kind == "subgraph" {
            let child: Composition =
                serde_json::from_value(node.data.config["composition"].clone())?;
            source.push_str(&format!(
                "\nmod subgraph_{index} {{\n{}\n}}\n",
                render_scope(&child, legacy_imports)?
            ));
        }
    }
    source.push_str(&format!(
        "\npub fn build({SIGNATURE}) -> anyhow::Result<CompiledGraph> {{\n    build_scope(services, checkpointer, \"\")\n}}\n\npub(super) fn build_scope({SIGNATURE}, scope: &str) -> anyhow::Result<CompiledGraph> {{\n"
    ));
    if let Some(directory) = &doc.settings.working_directory {
        source.push_str(&format!(
            "    let services = services.for_working_directory(Some({}))?;\n",
            rust_string(directory)
        ));
    }
    for (index, node) in doc.nodes.iter().enumerate() {
        let id = match node.data.kind.as_str() {
            "start" => "START".to_owned(),
            "end" => "END".to_owned(),
            _ => format!("NODE_{index}.0"),
        };
        let mut config = node.data.config.clone();
        if node.data.kind == "subgraph" {
            config
                .as_object_mut()
                .context("Configuration enfant invalide")?
                .remove("composition");
        }
        source.push_str(&format!(
            "    let node_{index} = {id};\n    let config_{index} = json!({});\n    let _ = &config_{index};\n", rust_json(&config)
        ));
    }
    source.push_str(&format!(
        "    let channels = json!({});\n    let settings = json!({});\n",
        rust_json(&serde_json::to_value(&doc.channels)?),
        rust_json(&serde_json::to_value(&doc.settings)?)
    ));
    let all_channels = runtime_channels(doc)?;
    let private_channels =
        &all_channels.as_array().context("Canaux invalides")?[doc.channels.len()..];
    source.push_str(&format!(
        "    let mut runtime_channels: Vec<Value> = serde_json::from_value(channels)?;\n    runtime_channels.extend(serde_json::from_value::<Vec<Value>>(json!({}))?);\n    let mut graph = StateGraph::new(operations::state_schema(&Value::Array(runtime_channels))?);\n",
        rust_json(&serde_json::to_value(private_channels)?)
    ));
    for (index, node) in doc.nodes.iter().enumerate() {
        let kind = node.data.kind.as_str();
        if ["start", "end"].contains(&kind) {
            continue;
        }
        source.push_str(&format!(
            "    let path_{index} = format!(\"{{scope}}{{}}\", node_{index});\n"
        ));
        if kind == "subgraph" {
            let child: Composition =
                serde_json::from_value(node.data.config["composition"].clone())?;
            source.push_str(&format!(
                "    let child_{index} = subgraph_{index}::build_scope(services.clone(), checkpointer.clone(), &format!(\"{{path_{index}}}/\"))?;\n    graph = graph.add_node(subgraphs::ResumableSubgraph::new(node_{index}, Arc::new(child_{index}), serde_json::from_value(json!({}))?));\n",
                rust_json(&serde_json::to_value(answer_paths(&child)?)?)
            ));
            continue;
        }
        source.push_str(&format!(
            "    let mut runtime_config_{index} = if config_{index}.is_object() {{ config_{index}.clone() }} else {{ json!({{}}) }};\n    runtime_config_{index}[\"nodeId\"] = json!(node_{index});\n"
        ));
        if doc.format_version >= 2 {
            source.push_str(&format!(
                "    runtime_config_{index}[\"__zedflowVersion\"] = json!({version});\n",
                version = doc.format_version
            ));
        }
        if kind == "model" || (kind == "context" && doc.format_version >= 3) {
            let field = if kind == "model" {
                "contextNode"
            } else {
                "modelNode"
            };
            let peer = node_index(
                doc,
                node.data.config[field]
                    .as_str()
                    .context("Nœud associé absent")?,
            )?;
            let constructor = if kind == "model" {
                "inference_node_with_services"
            } else {
                "context_node_with_services"
            };
            source.push_str(&format!("    let mut peer_config_{index} = config_{peer}.clone();\n    peer_config_{index}[\"nodeId\"] = json!(node_{peer});\n    peer_config_{index}[\"__zedflowVersion\"] = json!({version});\n    graph = graph.add_node(models::{constructor}(node_{index}, &runtime_config_{index}, &peer_config_{index}, &path_{index}, services.clone())?);\n", version = doc.format_version));
        } else if kind == "agent" {
            source.push_str(&format!("    graph = graph.add_node(models::node_with_services(node_{index}, &runtime_config_{index}, &path_{index}, services.clone())?);\n"));
        } else {
            source.push_str(&format!(
                "    graph = graph.add_node_fn(node_{index}, {{\n        let services = services.clone();\n        move |ctx| {{\n            let config = runtime_config_{index}.clone();\n            let services = services.clone();\n            let path = path_{index}.clone();\n            async move {{ operations::execute_with_services({}, &config, ctx, &path, services).await }}\n        }}\n    }});\n",
                rust_string(kind)
            ));
        }
    }
    for (index, edge) in doc.edges.iter().enumerate() {
        let from = node_index(doc, &edge.source)?;
        let to = node_index(doc, &edge.target)?;
        source.push_str(&format!(
            "    let edge_{index} = (node_{from}, node_{to});\n"
        ));
    }
    for (node_index, node) in doc.nodes.iter().enumerate() {
        let edges: Vec<_> = doc
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| edge.source == node.id)
            .collect();
        if node.data.kind == "condition" {
            let yes = edges
                .iter()
                .find(|(_, edge)| edge.source_handle.as_deref() == Some("true"))
                .context("Sortie true absente")?
                .0;
            let no = edges
                .iter()
                .find(|(_, edge)| edge.source_handle.as_deref() == Some("false"))
                .context("Sortie false absente")?
                .0;
            if doc.format_version >= 2 {
                source.push_str(&format!(
                    "    let field_{node_index} = format!(\"__zedflow:condition:{{}}\", node_{node_index});\n    graph.edges.push(Edge::Conditional {{\n        source: edge_{yes}.0.to_owned(),\n        router: Arc::new(move |state| if state.get(&field_{node_index}) == Some(&json!(true)) {{ \"true\".into() }} else {{ \"false\".into() }}),\n        targets: HashMap::from([(EDGE_{yes}.1.unwrap_or(\"\").to_owned(), EdgeTarget::from(edge_{yes}.1)), (EDGE_{no}.1.unwrap_or(\"\").to_owned(), EdgeTarget::from(edge_{no}.1))]),\n    }});\n"
                ));
            } else {
                source.push_str(&format!(
                "    let field_{node_index} = config_{node_index}[\"field\"].as_str().unwrap_or(\"input\").to_owned();\n    let expected_{node_index} = config_{node_index}.get(\"equals\").cloned().unwrap_or(Value::Bool(true));\n    graph.edges.push(Edge::Conditional {{\n        source: edge_{yes}.0.to_owned(),\n        router: Arc::new(move |state| if state.get(&field_{node_index}) == Some(&expected_{node_index}) {{ \"true\".into() }} else {{ \"false\".into() }}),\n        targets: HashMap::from([(EDGE_{yes}.1.unwrap_or(\"\").to_owned(), EdgeTarget::from(edge_{yes}.1)), (EDGE_{no}.1.unwrap_or(\"\").to_owned(), EdgeTarget::from(edge_{no}.1))]),\n    }});\n"
            ));
            }
        } else {
            for (index, edge) in edges {
                let target = &doc.nodes[node_index_for(doc, &edge.target)?];
                if node.data.kind != "start" && target.data.config["fanIn"] == "any" {
                    source.push_str(&format!(
                        "    graph.edges.push(Edge::Conditional {{ source: edge_{index}.0.to_owned(), router: Arc::new(|_| \"next\".into()), targets: HashMap::from([(\"next\".into(), EdgeTarget::from(edge_{index}.1))]) }});\n"
                    ));
                } else {
                    source.push_str(&format!(
                        "    graph = graph.add_edge(edge_{index}.0, edge_{index}.1);\n"
                    ));
                }
            }
        }
    }
    let retries: Vec<_> = doc
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.data.config["retry"].is_object())
        .map(|(index, _)| format!("(NODE_{index}.0, config_{index}[\"retry\"].clone())"))
        .collect();
    source.push_str(&format!("    Ok(operations::configure(graph.compile()?, &settings, &[{}]).with_checkpointer_arc(checkpointer))\n}}\n", retries.join(", ")));
    Ok(source)
}

fn node_index(doc: &Composition, id: &str) -> Result<usize> {
    doc.nodes
        .iter()
        .position(|node| node.id == id)
        .with_context(|| format!("Nœud absent : {id}"))
}

// A differently named alias keeps generated loop indices unambiguous.
fn node_index_for(doc: &Composition, id: &str) -> Result<usize> {
    node_index(doc, id)
}

fn syntax_error(error: syn::Error) -> anyhow::Error {
    let start = error.span().start();
    anyhow::anyhow!(
        "Rust non reconnu, ligne {}, colonne {} : {}",
        start.line,
        start.column + 1,
        error
    )
}

fn extract_scope(items: &[Item], depth: usize) -> Result<Composition> {
    ensure!(depth <= 8, "Imbrication limitée à huit niveaux");
    let mut constants = BTreeMap::new();
    let mut children = BTreeMap::new();
    let mut function = None;
    for item in items {
        match item {
            Item::Const(item) => {
                ensure!(
                    constants
                        .insert(item.ident.to_string(), constant_value(&item.expr)?)
                        .is_none(),
                    "Constante dupliquée : {}",
                    item.ident
                );
            }
            Item::Mod(item) => {
                let index = indexed(&item.ident.to_string(), "subgraph_")?;
                let (_, inner) = item
                    .content
                    .as_ref()
                    .context("Les modules externes ne sont pas pris en charge")?;
                ensure!(
                    children
                        .insert(index, extract_scope(inner, depth + 1)?)
                        .is_none(),
                    "Sous-graphe dupliqué"
                );
            }
            Item::Fn(item) if item.sig.ident == "build_scope" => {
                ensure!(function.replace(item).is_none(), "build_scope dupliqué");
            }
            _ => {} // Exhaustively checked by normalized AST comparison below.
        }
    }
    let format_version = constants
        .get("ZEDFLOW_FORMAT")
        .and_then(Value::as_u64)
        .filter(|version| [1, 2, 3, 4].contains(version))
        .context("Format Rust absent ou version non prise en charge")?
        as u32;
    let (id, name, revision): (String, String, i64) =
        serde_json::from_value(constants.remove("FLOW").context("Constante FLOW absente")?)
            .context("Métadonnées FLOW invalides")?;
    let function = function.context("Fonction build_scope absente")?;
    let mut locals = BTreeMap::new();
    let mut kinds = BTreeMap::new();
    for statement in &function.block.stmts {
        if let Stmt::Local(local) = statement
            && let syn::Pat::Ident(pattern) = &local.pat
            && let Some(init) = &local.init
        {
            ensure!(
                locals
                    .insert(pattern.ident.to_string(), init.expr.as_ref())
                    .is_none(),
                "Variable dupliquée : {}",
                pattern.ident
            );
        }
        extract_registration(statement, &mut kinds)?;
    }
    let mut nodes = Vec::new();
    while let Some(metadata) = constants.remove(&format!("NODE_{}", nodes.len())) {
        let index = nodes.len();
        let (id, node_type, label, x, y): (String, String, String, f64, f64) =
            serde_json::from_value(metadata).context("Métadonnées NODE invalides")?;
        let native = locals
            .get(&format!("node_{index}"))
            .context("Déclaration native du nœud absente")?;
        let kind = match expression_path(native).as_deref() {
            Some("START") => "start".into(),
            Some("END") => "end".into(),
            _ => kinds
                .remove(&index)
                .context("Enregistrement ADK du nœud absent")?,
        };
        let mut config = macro_value(
            locals
                .get(&format!("config_{index}"))
                .context("Configuration du nœud absente")?,
        )?;
        if kind == "subgraph" {
            let child = children
                .remove(&index)
                .context("Module du sous-graphe absent")?;
            ensure!(
                config.get("composition").is_none(),
                "La composition enfant doit être exprimée dans son module Rust"
            );
            config
                .as_object_mut()
                .context("Configuration enfant invalide")?
                .insert("composition".into(), serde_json::to_value(child)?);
        }
        nodes.push(Node {
            id,
            node_type,
            position: Position { x, y },
            data: NodeData {
                label,
                kind,
                config,
            },
        });
    }
    ensure!(
        children.is_empty() && kinds.is_empty(),
        "Enregistrement sans nœud ou module enfant inutilisé"
    );
    let mut edges = Vec::new();
    while let Some(metadata) = constants.remove(&format!("EDGE_{}", edges.len())) {
        let index = edges.len();
        let (id, source_handle, target_handle, label): (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        ) = if format_version >= 4 {
            serde_json::from_value(metadata).context("Métadonnées EDGE v4 invalides")?
        } else {
            let (id, source, label): (String, Option<String>, Option<String>) =
                serde_json::from_value(metadata).context("Métadonnées EDGE invalides")?;
            (id, source, None, label)
        };
        let native = locals
            .get(&format!("edge_{index}"))
            .context("Connexion native absente")?;
        let Expr::Tuple(tuple) = native else {
            bail!("Une connexion doit être une paire de nœuds Rust");
        };
        ensure!(
            tuple.elems.len() == 2,
            "Une connexion doit avoir deux extrémités"
        );
        let endpoint = |expression: &Expr| -> Result<String> {
            let index = indexed(
                &expression_path(expression).context("Extrémité calculée non prise en charge")?,
                "node_",
            )?;
            Ok(nodes
                .get(index)
                .context("Extrémité de connexion absente")?
                .id
                .clone())
        };
        edges.push(Edge {
            id,
            source: endpoint(&tuple.elems[0])?,
            target: endpoint(&tuple.elems[1])?,
            source_handle,
            target_handle,
            label,
        });
    }
    let settings = serde_json::from_value(macro_value(
        locals.get("settings").context("Réglages absents")?,
    )?)?;
    let channels = serde_json::from_value(macro_value(
        locals.get("channels").context("Canaux absents")?,
    )?)?;
    Ok(Composition {
        format_version,
        id,
        name,
        revision,
        nodes,
        edges,
        settings,
        channels,
    })
}

fn indexed(name: &str, prefix: &str) -> Result<usize> {
    name.strip_prefix(prefix)
        .context("Identifiant Rust hors format")?
        .parse()
        .context("Indice Rust non valide")
}

fn expression_path(expression: &Expr) -> Option<String> {
    if let Expr::Path(path) = expression {
        Some(
            path.path
                .segments
                .iter()
                .map(|part| part.ident.to_string())
                .collect::<Vec<_>>()
                .join("::"),
        )
    } else {
        None
    }
}

fn extract_registration(statement: &Stmt, kinds: &mut BTreeMap<usize, String>) -> Result<()> {
    let Stmt::Expr(Expr::Assign(assignment), _) = statement else {
        return Ok(());
    };
    let Expr::MethodCall(call) = assignment.right.as_ref() else {
        return Ok(());
    };
    if call.method == "add_node_fn" {
        let node = call
            .args
            .first()
            .and_then(expression_path)
            .context("Identifiant ADK absent")?;
        let index = indexed(&node, "node_")?;
        struct KindVisitor {
            kinds: Vec<String>,
        }
        impl<'ast> Visit<'ast> for KindVisitor {
            fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
                if expression_path(&call.func).as_deref()
                    == Some("operations::execute_with_services")
                    && let Some(Expr::Lit(syn::ExprLit {
                        lit: Lit::Str(kind),
                        ..
                    })) = call.args.first()
                {
                    self.kinds.push(kind.value());
                }
                syn::visit::visit_expr_call(self, call);
            }
        }
        let mut visitor = KindVisitor { kinds: Vec::new() };
        visitor.visit_expr_method_call(call);
        ensure!(
            visitor.kinds.len() == 1,
            "Une opération native connue est requise par nœud"
        );
        ensure!(
            kinds.insert(index, visitor.kinds.remove(0)).is_none(),
            "Nœud ADK dupliqué"
        );
    } else if call.method == "add_node" {
        let mut expression = call.args.first().context("Nœud ADK absent")?;
        if let Expr::Try(value) = expression {
            expression = &value.expr;
        }
        let Expr::Call(native) = expression else {
            bail!("Construction de nœud inconnue");
        };
        let kind = match expression_path(&native.func).as_deref() {
            Some("models::node_with_services") => "agent",
            Some("models::inference_node_with_services") => "model",
            Some("models::context_node_with_services") => "context",
            Some("subgraphs::ResumableSubgraph::new") => "subgraph",
            _ => bail!("Constructeur de nœud hors catalogue"),
        };
        let index = indexed(
            &native
                .args
                .first()
                .and_then(expression_path)
                .context("Identifiant ADK absent")?,
            "node_",
        )?;
        ensure!(
            kinds.insert(index, kind.into()).is_none(),
            "Nœud ADK dupliqué"
        );
    }
    Ok(())
}

fn constant_value(expression: &Expr) -> Result<Value> {
    match expression {
        Expr::Tuple(tuple) => Ok(Value::Array(
            tuple
                .elems
                .iter()
                .map(constant_value)
                .collect::<Result<_>>()?,
        )),
        Expr::Lit(value) => literal_value(&value.lit),
        Expr::Unary(value) if matches!(value.op, syn::UnOp::Neg(_)) => {
            let value = constant_value(&value.expr)?;
            serde_json::from_str(&format!("-{value}")).context("Nombre négatif invalide")
        }
        Expr::Path(_) if expression_path(expression).as_deref() == Some("None") => Ok(Value::Null),
        Expr::Call(call)
            if expression_path(&call.func).as_deref() == Some("Some") && call.args.len() == 1 =>
        {
            constant_value(&call.args[0])
        }
        _ => bail!(
            "Les métadonnées doivent être des littéraux Rust (ligne {})",
            expression.span().start().line
        ),
    }
}

fn literal_value(literal: &Lit) -> Result<Value> {
    match literal {
        Lit::Str(value) => Ok(Value::String(value.value())),
        Lit::Bool(value) => Ok(Value::Bool(value.value)),
        Lit::Int(_) => numeric_value(literal, false),
        Lit::Float(value) if value.suffix().is_empty() => {
            serde_json::from_str(value.base10_digits()).context("Nombre JSON invalide")
        }
        _ => bail!("Littéral hors format : chaînes, nombres, booléens et null uniquement"),
    }
}

fn numeric_value(literal: &Lit, negative: bool) -> Result<Value> {
    let (digits, suffix) = match literal {
        Lit::Int(value) => (value.base10_digits(), value.suffix()),
        Lit::Float(value) if value.suffix().is_empty() => (value.base10_digits(), ""),
        _ => bail!("Nombre littéral attendu"),
    };
    let number = format!("{}{digits}", if negative { "-" } else { "" });
    match suffix {
        "" => {}
        "i64" => {
            number.parse::<i64>().context("Entier i64 hors limites")?;
        }
        "u64" if !negative => {
            number.parse::<u64>().context("Entier u64 hors limites")?;
        }
        _ => bail!("Suffixe numérique non pris en charge ; seuls i64 et u64 sont admis"),
    }
    serde_json::from_str(&number).context("Nombre JSON invalide")
}

fn json_numeric_value(literal: &Lit, negative: bool) -> Result<Value> {
    let value = numeric_value(literal, negative)?;
    if let Lit::Int(literal) = literal
        && literal.suffix().is_empty()
    {
        ensure!(
            value
                .as_i64()
                .is_some_and(|value| i32::try_from(value).is_ok()),
            "Les grands entiers dans json! nécessitent un suffixe i64 ou u64 pour compiler sans débordement Rust"
        );
    }
    Ok(value)
}

fn macro_value(expression: &Expr) -> Result<Value> {
    let Expr::Macro(expression) = expression else {
        bail!("Une configuration doit utiliser json! avec des valeurs littérales");
    };
    ensure!(
        expression.mac.path.is_ident("json"),
        "Seule la macro json! littérale est admise pour la configuration"
    );
    Ok(syn::parse2::<LiteralJson>(expression.mac.tokens.clone())
        .map_err(syntax_error)?
        .0)
}

struct LiteralJson(Value);
impl Parse for LiteralJson {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        parse_json(input, 0).map(Self)
    }
}

fn parse_json(input: ParseStream<'_>, depth: usize) -> syn::Result<Value> {
    if depth >= MAX_CONFIG_JSON_DEPTH
        && (input.peek(syn::token::Brace) || input.peek(syn::token::Bracket))
    {
        return Err(input.error("Configuration JSON limitée à 256 niveaux"));
    }
    if input.peek(syn::token::Brace) {
        let content;
        syn::braced!(content in input);
        let mut object = Map::new();
        while !content.is_empty() {
            let key = content.parse::<syn::LitStr>()?.value();
            content.parse::<Token![:]>()?;
            let value = parse_json(&content, depth + 1)?;
            if object.insert(key, value).is_some() {
                return Err(content.error("Clé JSON dupliquée"));
            }
            if content.is_empty() {
                break;
            }
            content.parse::<Token![,]>()?;
        }
        Ok(Value::Object(object))
    } else if input.peek(syn::token::Bracket) {
        let content;
        syn::bracketed!(content in input);
        let mut values = Vec::new();
        while !content.is_empty() {
            values.push(parse_json(&content, depth + 1)?);
            if content.is_empty() {
                break;
            }
            content.parse::<Token![,]>()?;
        }
        Ok(Value::Array(values))
    } else if input.peek(Token![-]) {
        input.parse::<Token![-]>()?;
        let literal = input.parse::<Lit>()?;
        if !matches!(literal, Lit::Int(_) | Lit::Float(_)) {
            return Err(input.error("Nombre attendu après -"));
        }
        json_numeric_value(&literal, true).map_err(|error| input.error(error.to_string()))
    } else if input.peek(syn::Ident) && !input.peek(syn::LitBool) {
        let value = input.parse::<syn::Ident>()?;
        if value == "null" {
            Ok(Value::Null)
        } else {
            Err(input.error("Les expressions Rust calculées ne sont pas admises dans json!"))
        }
    } else {
        let literal = input.parse::<Lit>()?;
        if matches!(literal, Lit::Int(_) | Lit::Float(_)) {
            json_numeric_value(&literal, false).map_err(|error| input.error(error.to_string()))
        } else {
            literal_value(&literal).map_err(|error| input.error(error.to_string()))
        }
    }
}

fn trim_punctuation<T, P>(values: &mut Punctuated<T, P>) {
    values.pop_punct();
}

fn sort_imports(items: &mut [Item]) {
    let mut start = 0;
    while start < items.len() {
        if matches!(items[start], Item::Use(_)) {
            let mut end = start + 1;
            while end < items.len() && matches!(items[end], Item::Use(_)) {
                end += 1;
            }
            items[start..end].sort_by_cached_key(|item| item.to_token_stream().to_string());
            start = end;
        } else {
            start += 1;
        }
    }
}

/// Ignore formatting, ordinary comments, raw-string spelling and optional
/// trailing commas. Preserve every executable AST item and attribute.
fn normalized(mut file: syn::File) -> Result<String> {
    #[derive(Default)]
    struct Normalize {
        error: Option<anyhow::Error>,
    }
    impl VisitMut for Normalize {
        fn visit_file_mut(&mut self, file: &mut syn::File) {
            syn::visit_mut::visit_file_mut(self, file);
            sort_imports(&mut file.items);
        }
        fn visit_item_mod_mut(&mut self, module: &mut syn::ItemMod) {
            syn::visit_mut::visit_item_mod_mut(self, module);
            if let Some((_, items)) = &mut module.content {
                sort_imports(items);
            }
        }
        fn visit_lit_str_mut(&mut self, literal: &mut syn::LitStr) {
            *literal = syn::LitStr::new(&literal.value(), Span::call_site());
        }
        fn visit_expr_array_mut(&mut self, value: &mut syn::ExprArray) {
            trim_punctuation(&mut value.elems);
            syn::visit_mut::visit_expr_array_mut(self, value);
        }
        fn visit_expr_tuple_mut(&mut self, value: &mut syn::ExprTuple) {
            if value.elems.len() != 1 {
                trim_punctuation(&mut value.elems);
            }
            syn::visit_mut::visit_expr_tuple_mut(self, value);
        }
        fn visit_expr_call_mut(&mut self, value: &mut syn::ExprCall) {
            trim_punctuation(&mut value.args);
            syn::visit_mut::visit_expr_call_mut(self, value);
        }
        fn visit_expr_method_call_mut(&mut self, value: &mut syn::ExprMethodCall) {
            trim_punctuation(&mut value.args);
            syn::visit_mut::visit_expr_method_call_mut(self, value);
        }
        fn visit_expr_struct_mut(&mut self, value: &mut syn::ExprStruct) {
            trim_punctuation(&mut value.fields);
            syn::visit_mut::visit_expr_struct_mut(self, value);
        }
        fn visit_expr_closure_mut(&mut self, value: &mut syn::ExprClosure) {
            // rustfmt wraps a long `|state| if ...` router in an otherwise
            // empty block. Only that exact, expression-only wrapper is optional.
            if let Expr::Block(block) = value.body.as_ref()
                && block.attrs.is_empty()
                && block.label.is_none()
                && let [Stmt::Expr(expression @ Expr::If(_), None)] = block.block.stmts.as_slice()
            {
                *value.body = expression.clone();
            }
            syn::visit_mut::visit_expr_closure_mut(self, value);
        }
        fn visit_signature_mut(&mut self, value: &mut syn::Signature) {
            trim_punctuation(&mut value.inputs);
            syn::visit_mut::visit_signature_mut(self, value);
        }
        fn visit_use_group_mut(&mut self, value: &mut syn::UseGroup) {
            syn::visit_mut::visit_use_group_mut(self, value);
            let mut items: Vec<_> = value.items.iter().cloned().collect();
            items.sort_by_cached_key(|item| item.to_token_stream().to_string());
            value.items = items.into_iter().collect();
        }
        fn visit_macro_mut(&mut self, value: &mut syn::Macro) {
            if value.path.is_ident("json") {
                // Scaffolding also uses json!(node_N). Those identifiers are
                // checked byte-structurally against the generated AST instead.
                match syn::parse2::<LiteralJson>(value.tokens.clone()) {
                    Ok(literal) => match rust_json(&literal.0).parse::<TokenStream>() {
                        Ok(tokens) => value.tokens = tokens,
                        Err(error) => self.error = Some(anyhow::anyhow!(error.to_string())),
                    },
                    Err(_) => {
                        // Do not normalize arbitrary macros or expressions.
                    }
                }
            }
            syn::visit_mut::visit_macro_mut(self, value);
        }
    }
    let mut visitor = Normalize::default();
    visitor.visit_file_mut(&mut file);
    if let Some(error) = visitor.error {
        return Err(error);
    }
    Ok(file.into_token_stream().to_string())
}
