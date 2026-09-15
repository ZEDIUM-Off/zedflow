//! Structural validation independent of ADK graph materialization.
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use zf_flows::schema::{Composition, RetrySettings};

// Reserved identities in the retained ADK 2.2.0 edge protocol.
pub const START: &str = "__start__";
pub const END: &str = "__end__";

/// Provider/tool contracts injected by the composition root. Implementations
/// must be deterministic and inspect configuration only: no credentials, IO,
/// model invocation or tool execution. The runtime supplies its real parameter
/// validators; tests can declare a narrower explicit fixture catalogue.
/// No permissive default is provided.
pub trait PrimitiveContracts: Send + Sync {
    fn validate_model(&self, config: &Value) -> Result<()>;
    fn has_tool(&self, name: &str) -> bool;
}

/// Canonical complete graph validation, including required primitive contracts.
pub struct GraphValidator<'a> {
    primitives: &'a dyn PrimitiveContracts,
}
impl<'a> GraphValidator<'a> {
    pub fn new(primitives: &'a dyn PrimitiveContracts) -> Self {
        Self { primitives }
    }
}
impl zf_flows::flow_format::SourceValidator for GraphValidator<'_> {
    fn validate(&self, document: &Composition) -> Result<()> {
        validate(document, self.primitives)
    }
}

/// Model settings overlay only the explicitly associated context configuration.
/// Both semantic validation and runtime preparation use this same merge.
pub fn combined_config(model: &Value, context: &Value) -> Value {
    let mut combined = context.clone();
    if let (Some(target), Some(source)) = (combined.as_object_mut(), model.as_object()) {
        for (key, value) in source {
            target.insert(key.clone(), value.clone());
        }
    }
    combined
}

pub fn validate(doc: &Composition, primitives: &dyn PrimitiveContracts) -> Result<()> {
    validate_depth(doc, 0, primitives)
}
fn validate_depth(
    doc: &Composition,
    depth: usize,
    primitives: &dyn PrimitiveContracts,
) -> Result<()> {
    ensure!(
        [1, 2, 3, 4].contains(&doc.format_version),
        "Version de flow non prise en charge"
    );
    ensure!(depth <= 8, "Imbrication limitée à huit niveaux");
    ensure!(
        depth == 0 || doc.settings.recursion_limit == 100,
        "ADK 2.2.0 impose une limite native de 50 étapes aux sous-graphes ; une limite personnalisée n'est disponible que pour une composition racine"
    );
    ensure!(!doc.name.trim().is_empty(), "Le nom est obligatoire");
    ensure!(
        doc.nodes.len() <= 250,
        "250 nœuds maximum pour cette version"
    );
    validate_settings(doc)?;
    let ids: HashSet<_> = doc.nodes.iter().map(|n| n.id.as_str()).collect();
    ensure!(
        ids.len() == doc.nodes.len(),
        "Identifiants de nœuds dupliqués"
    );
    ensure!(
        doc.nodes.iter().filter(|n| n.data.kind == "start").count() == 1,
        "Un nœud Début est requis"
    );
    for node in &doc.nodes {
        ensure!(
            doc.format_version < 4 || node.data.kind != "agent",
            "{} : le format v4 exige deux nœuds distincts Contexte et Modèle",
            node.data.label
        );
        ensure!(
            !node.id.is_empty()
                && !node.id.contains('/')
                && ![START, END].contains(&node.id.as_str()),
            "Identifiant réservé : {}",
            node.id
        );
        ensure!(
            [
                "start",
                "end",
                "set",
                "output",
                "input",
                "condition",
                "agent",
                "model",
                "subgraph",
                "tool",
                "context",
                "steering",
                "inbox",
                "route",
                "await_route"
            ]
            .contains(&node.data.kind.as_str()),
            "Nature non disponible : {}",
            node.data.kind
        );
        validate_node(node, doc.format_version, primitives)?;
        if node.data.kind == "subgraph" {
            let child: Composition =
                serde_json::from_value(node.data.config["composition"].clone()).map_err(|_| {
                    anyhow::anyhow!("{} : choisissez une sous-composition", node.data.label)
                })?;
            ensure!(
                child.format_version == doc.format_version,
                "Les sous-graphes doivent utiliser la même version que le flow parent"
            );
            validate_depth(&child, depth + 1, primitives)?;
        }
        let outgoing: Vec<_> = doc.edges.iter().filter(|e| e.source == node.id).collect();
        if node.data.kind == "end" {
            ensure!(outgoing.is_empty(), "Fin ne peut avoir de sortie");
        } else {
            ensure!(
                !outgoing.is_empty(),
                "{} : sortie manquante",
                node.data.label
            );
        }
        if node.data.kind == "condition" {
            ensure!(
                outgoing.len() == 2
                    && outgoing
                        .iter()
                        .any(|e| e.source_handle.as_deref() == Some("true"))
                    && outgoing
                        .iter()
                        .any(|e| e.source_handle.as_deref() == Some("false")),
                "{} : sorties true et false requises",
                node.data.label
            );
        }
    }
    let mut edges = HashSet::new();
    for edge in &doc.edges {
        ensure!(edges.insert(&edge.id), "Identifiant de connexion dupliqué");
        ensure!(
            ids.contains(edge.source.as_str()) && ids.contains(edge.target.as_str()),
            "Connexion vers un nœud absent"
        );
        ensure!(
            doc.nodes
                .iter()
                .find(|n| n.id == edge.target)
                .is_some_and(|n| n.data.kind != "start"),
            "Début ne peut recevoir de connexion"
        );
    }
    let start = doc
        .nodes
        .iter()
        .find(|n| n.data.kind == "start")
        .ok_or_else(|| anyhow::anyhow!("Début absent"))?;
    let mut reachable = HashSet::from([start.id.as_str()]);
    loop {
        let size = reachable.len();
        for edge in &doc.edges {
            if reachable.contains(edge.source.as_str()) {
                reachable.insert(edge.target.as_str());
            }
        }
        if reachable.len() == size {
            break;
        }
    }
    for node in &doc.nodes {
        ensure!(
            reachable.contains(node.id.as_str()),
            "{} : nœud inaccessible",
            node.data.label
        );
    }
    if doc.format_version >= 4 {
        let analysis = zf_flows::node_contracts::analyze(doc);
        ensure!(
            analysis.diagnostics.is_empty(),
            "{}",
            analysis
                .diagnostics
                .iter()
                .map(|d| d.message.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
    validate_context_model_pairs(doc, primitives)?;
    validate_parallel_waits(doc)?;
    zf_flows::flow_contract::validate(doc)?;
    Ok(())
}

/// A model consumes one preparation. Every control-flow arrival, including a
/// back edge, must cross its context node first; parallel joins cannot bypass it.
fn validate_context_model_pairs(
    doc: &Composition,
    primitives: &dyn PrimitiveContracts,
) -> Result<()> {
    for node in &doc.nodes {
        if node.data.kind == "model" {
            let context_id = node.data.config["contextNode"].as_str().unwrap_or_default();
            let context = doc
                .nodes
                .iter()
                .find(|candidate| candidate.id == context_id)
                .ok_or_else(|| {
                    anyhow::anyhow!("{} : nœud Contexte {context_id} absent", node.data.label)
                })?;
            ensure!(
                context.data.kind == "context" && context.data.config["modelNode"] == node.id,
                "{} : le nœud Contexte doit désigner ce Modèle",
                node.data.label
            );
            let incoming: Vec<_> = doc
                .edges
                .iter()
                .filter(|edge| edge.target == node.id)
                .collect();
            ensure!(
                incoming.len() == 1 && incoming[0].source == context.id,
                "{} : chaque passage doit suivre directement son nœud Contexte ; raccordez les boucles au Contexte",
                node.data.label
            );
            primitives.validate_model(&combined_config(
                &config(node, doc.format_version),
                &config(context, doc.format_version),
            ))?;
        } else if node.data.kind == "context" && doc.format_version >= 3 {
            let model_id = node.data.config["modelNode"].as_str().unwrap_or_default();
            ensure!(
                doc.nodes.iter().any(|candidate| candidate.id == model_id
                    && candidate.data.kind == "model"
                    && candidate.data.config["contextNode"] == node.id),
                "{} : nœud Modèle associé absent ou incompatible",
                node.data.label
            );
            let outgoing: Vec<_> = doc
                .edges
                .iter()
                .filter(|edge| edge.source == node.id)
                .collect();
            ensure!(
                outgoing.len() == 1 && outgoing[0].target == model_id,
                "{} : la sortie du Contexte doit rejoindre uniquement son Modèle",
                node.data.label
            );
        }
    }
    Ok(())
}

fn pause_path(node: &zf_flows::schema::Node) -> Result<Option<String>> {
    if ["input", "inbox"].contains(&node.data.kind.as_str())
        || (matches!(node.data.kind.as_str(), "agent" | "model")
            && node.data.config["modelBinding"] == "runtime")
    {
        return Ok(Some(node.id.clone()));
    }
    if node.data.kind == "subgraph" {
        let child: Composition = serde_json::from_value(node.data.config["composition"].clone())?;
        for inner in &child.nodes {
            if let Some(path) = pause_path(inner)? {
                return Ok(Some(format!("{}/{path}", node.id)));
            }
        }
    }
    Ok(None)
}

/// A conditional route chooses one successor; other fan-outs can admit several
/// branches. This release exposes one durable wait at a time, so refuse parallel
/// waits unless a native all-predecessor join proves they have synchronized first.
fn validate_parallel_waits(doc: &Composition) -> Result<()> {
    let mut pauses = HashMap::new();
    for node in &doc.nodes {
        if let Some(path) = pause_path(node)? {
            pauses.insert(node.id.as_str(), path);
        }
    }
    if pauses.is_empty() {
        return Ok(());
    }
    let mut outgoing: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in &doc.edges {
        outgoing.entry(&edge.source).or_default().push(&edge.target);
    }
    let no_barriers = HashSet::new();
    for fork in doc
        .nodes
        .iter()
        .filter(|node| node.data.kind != "condition")
    {
        let roots: HashSet<_> = outgoing
            .get(fork.id.as_str())
            .into_iter()
            .flatten()
            .copied()
            .collect();
        if roots.len() < 2 {
            continue;
        }
        let mut roots: Vec<_> = roots.into_iter().collect();
        roots.sort_unstable();
        let regions: Vec<_> = roots
            .iter()
            .map(|root| branch_region(root, &fork.id, &outgoing, &no_barriers).0)
            .collect();
        let mut barriers = HashSet::new();
        for join in doc
            .nodes
            .iter()
            .filter(|node| !alternative_arrival(doc, &node.id))
        {
            let mut required_branches = HashSet::new();
            for edge in doc.edges.iter().filter(|edge| edge.target == join.id) {
                // The compiler lowers condition edges and fanIn:any to routes,
                // which do not provide native all-predecessor synchronization.
                if doc
                    .nodes
                    .iter()
                    .any(|node| node.id == edge.source && node.data.kind == "condition")
                {
                    continue;
                }
                if edge.source == fork.id {
                    if let Some(index) = roots.iter().position(|root| *root == edge.target) {
                        required_branches.insert(index);
                    }
                } else {
                    let origins: Vec<_> = regions
                        .iter()
                        .enumerate()
                        .filter_map(|(index, region)| {
                            region.contains(edge.source.as_str()).then_some(index)
                        })
                        .collect();
                    // A required predecessor reachable from exactly one branch
                    // cannot be supplied by an earlier arrival from another one.
                    if origins.len() == 1 {
                        required_branches.insert(origins[0]);
                    }
                }
            }
            if required_branches.len() == roots.len() {
                barriers.insert(join.id.as_str());
            }
        }
        let mut waits = Vec::new();
        let mut reenters = false;
        for root in roots {
            let (region, reentry) = branch_region(root, &fork.id, &outgoing, &barriers);
            reenters |= reentry;
            if let Some(wait) = region.iter().filter_map(|node| pauses.get(node)).min() {
                waits.push(wait.as_str());
            }
        }
        ensure!(
            waits.len() < 2 && (!reenters || waits.is_empty()),
            "{} : attentes parallèles non prises en charge ({}) ; sérialiser ces branches ou les réunir par une jointure all avant l'attente",
            fork.id,
            waits.join(", ")
        );
    }
    Ok(())
}

fn branch_region<'a>(
    root: &'a str,
    fork: &str,
    outgoing: &HashMap<&'a str, Vec<&'a str>>,
    barriers: &HashSet<&str>,
) -> (HashSet<&'a str>, bool) {
    let mut region = HashSet::new();
    let mut pending = vec![root];
    let mut reentry = false;
    while let Some(node) = pending.pop() {
        if node == fork {
            reentry = true;
            continue;
        }
        if barriers.contains(node) || !region.insert(node) {
            continue;
        }
        pending.extend(outgoing.get(node).into_iter().flatten().copied());
    }
    (region, reentry)
}
pub(crate) fn target(doc: &Composition, id: &str) -> String {
    if doc.nodes.iter().any(|n| n.id == id && n.data.kind == "end") {
        END.into()
    } else {
        id.into()
    }
}
pub fn config(node: &zf_flows::schema::Node, version: u32) -> Value {
    let mut config = if node.data.config.is_object() {
        node.data.config.clone()
    } else {
        json!({})
    };
    config["nodeId"] = json!(node.id);
    if version >= 2 {
        config["__zedflowVersion"] = json!(version);
    } else if let Some(object) = config.as_object_mut() {
        object.remove("__zedflowVersion");
    }
    config
}
fn validate_retry(retry: &RetrySettings) -> Result<()> {
    ensure!(
        (1..=20).contains(&retry.max_attempts),
        "maxAttempts doit être compris entre 1 et 20 (première tentative incluse)"
    );
    ensure!(
        retry.initial_delay_ms <= 600_000
            && retry.max_delay_ms <= 600_000
            && retry.initial_delay_ms <= retry.max_delay_ms,
        "Délais de retry invalides (0 à 600000 ms et initial <= maximum)"
    );
    ensure!(
        retry.backoff_factor.is_finite() && (1.0..=16.0).contains(&retry.backoff_factor),
        "backoffFactor doit être compris entre 1 et 16"
    );
    ensure!(
        retry.jitter.is_finite() && (0.0..=1.0).contains(&retry.jitter),
        "jitter doit être compris entre 0 et 1"
    );
    ensure!(
        ["any", "timeout"].contains(&retry.retry_on.as_str()),
        "retryOn attendu : any ou timeout"
    );
    Ok(())
}
fn validate_settings(doc: &Composition) -> Result<()> {
    let settings = &doc.settings;
    ensure!(
        (1..=10_000).contains(&settings.recursion_limit),
        "recursionLimit doit être compris entre 1 et 10000"
    );
    ensure!(
        settings
            .max_concurrency
            .is_none_or(|v| (1..=128).contains(&v)),
        "maxConcurrency doit être compris entre 1 et 128"
    );
    for timeout in [settings.timeout_ms, settings.idle_timeout_ms]
        .into_iter()
        .flatten()
    {
        ensure!(
            (1..=3_600_000).contains(&timeout),
            "Timeout doit être compris entre 1 et 3600000 ms"
        );
    }
    if let Some(retry) = &settings.retry {
        validate_retry(retry)?;
    }
    let mut names = HashSet::new();
    for channel in &doc.channels {
        ensure!(
            !channel.name.trim().is_empty()
                && !channel.name.starts_with("answer:")
                && !channel.name.starts_with("__zedflow:"),
            "Nom de canal vide ou réservé : {}",
            channel.name
        );
        ensure!(
            names.insert(&channel.name),
            "Canal dupliqué : {}",
            channel.name
        );
        ensure!(
            ["overwrite", "append", "sum"].contains(&channel.reducer.as_str()),
            "Reducer inconnu : {}",
            channel.reducer
        );
        if channel.reducer == "sum" {
            ensure!(
                channel.default.as_ref().is_none_or(Value::is_number),
                "Le défaut d'un compteur doit être un nombre"
            );
        }
        if channel.reducer == "append" {
            ensure!(
                channel.default.as_ref().is_none_or(Value::is_array),
                "Le défaut d'une liste doit être un tableau"
            );
        }
    }
    // Internal bookkeeping values cannot use aggregating reducers.
    for channel in &doc.channels {
        if [
            "messages",
            "toolCalls",
            "toolResults",
            "hasToolCalls",
            "modelResponse",
        ]
        .contains(&channel.name.as_str())
        {
            ensure!(
                channel.reducer == "overwrite",
                "Le canal {} doit utiliser overwrite",
                channel.name
            );
        }
    }
    for node in &doc.nodes {
        if ["agent", "model", "tool"].contains(&node.data.kind.as_str()) {
            for field in ["historyField", "toolCallsField"] {
                if let Some(name) = node.data.config[field].as_str()
                    && let Some(channel) = doc.channels.iter().find(|channel| channel.name == name)
                {
                    ensure!(
                        channel.reducer == "overwrite",
                        "{name} doit utiliser overwrite pour conserver l'historique ou les appels complets"
                    );
                }
            }
        }
    }
    Ok(())
}
fn validate_node(
    node: &zf_flows::schema::Node,
    version: u32,
    primitives: &dyn PrimitiveContracts,
) -> Result<()> {
    let cfg = &node.data.config;
    ensure!(
        cfg.get("__zedflowVersion").is_none(),
        "Champ de version runtime réservé"
    );
    ensure!(
        version != 2 || node.data.kind != "context",
        "Le contexte v2 se configure dans les pièces des agents ; convertissez le flow v1 explicitement"
    );
    if version >= 2 && node.data.kind == "condition" {
        zf_flows::node_contracts::parse_predicate(&cfg["predicate"])?;
    }
    if let Some(mode) = cfg.get("fanIn") {
        ensure!(
            ["any", "all"].contains(&mode.as_str().unwrap_or("")),
            "fanIn attendu : any ou all"
        );
    }
    ensure!(
        cfg.is_object() || cfg.is_null(),
        "{} : configuration objet requise",
        node.data.label
    );
    if let Some(retry) = cfg.get("retry").filter(|value| !value.is_null()) {
        validate_retry(&serde_json::from_value(retry.clone())?)?;
    }
    for key in ["field", "inputField", "historyField", "toolCallsField"] {
        if let Some(value) = cfg.get(key) {
            ensure!(
                value.as_str().is_some_and(|s| !s.trim().is_empty()
                    && !s.starts_with("__zedflow:")
                    && !s.starts_with("answer:")),
                "{} : {key} doit être un nom de canal",
                node.data.label
            );
        }
    }
    if let Some(ui) = cfg.get("ui").filter(|v| !v.is_null()) {
        ensure!(ui.is_object(), "ui doit être un objet de présentation");
        ensure!(
            ["json", "table", "code", "markdown"]
                .contains(&ui["renderer"].as_str().unwrap_or("json")),
            "Renderer indisponible"
        );
        ensure!(
            ui.as_object().is_some_and(|v| v
                .keys()
                .all(|key| ["renderer", "title", "language"].contains(&key.as_str()))),
            "ui accepte seulement renderer, title et language (pas de code exécutable)"
        );
    }
    match node.data.kind.as_str() {
        "route" => {
            ensure!(
                cfg["branch"].as_str().is_some_and(|s| !s.trim().is_empty()),
                "Route node requires a public branch point"
            );
            if let Some(invocation) = cfg.get("invocation") {
                ensure!(
                    matches!(invocation.as_str(), Some("node" | "condition")),
                    "Route node invocation must be node or condition"
                );
            }
        }
        "await_route" => {
            ensure!(
                cfg["inputField"].as_str().is_some_and(|s| !s.is_empty()),
                "Await node requires a visit handle channel"
            );
        }
        "context" if version >= 3 => {
            ensure!(
                cfg["modelNode"].as_str().is_some_and(|s| !s.is_empty()),
                "{} : choisissez le nœud Modèle associé",
                node.data.label
            );
            ensure!(
                cfg.get("contextProgram").is_some_and(|v| !v.is_null())
                    || cfg.get("contextStrategy").is_some_and(|v| !v.is_null()),
                "{} : choisissez une stratégie de contexte",
                node.data.label
            );
            if let Some(program) = cfg.get("contextProgram") {
                zf_context::frozen_context::validate_frozen(program)?;
            }
        }
        "model" => {
            ensure!(version >= 3, "Le nœud Modèle requiert le format Rust v3");
            ensure!(
                cfg["contextNode"].as_str().is_some_and(|s| !s.is_empty()),
                "{} : choisissez le nœud Contexte associé",
                node.data.label
            );
            ensure!(
                cfg.as_object()
                    .is_some_and(|fields| fields.iter().all(|(key, value)| value.is_null()
                        || !(key.starts_with("context") && key != "contextNode"
                            || [
                                "attachments",
                                "instructions",
                                "globalInstructions",
                                "tools",
                                "modelNode"
                            ]
                            .contains(&key.as_str())))),
                "{} : instructions, ressources et outils se configurent dans le nœud Contexte",
                node.data.label
            );
        }
        "agent" => {
            if let Some(program) = cfg.get("contextProgram") {
                zf_context::frozen_context::validate_frozen(program)?;
            }
            primitives.validate_model(&config(node, version))?;
        }
        "tool" => {
            let name = cfg["tool"].as_str().unwrap_or("inspect_json");
            ensure!(
                ["execute_calls", "execute_next_call"].contains(&name) || primitives.has_tool(name),
                "Outil indisponible : {name}"
            );
            if !["execute_calls", "execute_next_call"].contains(&name)
                && cfg["inputField"].as_str().is_none()
            {
                ensure!(
                    cfg.get("arguments").is_some_and(Value::is_object),
                    "Les arguments d'outil doivent être un objet JSON"
                );
                if name == "delay" {
                    ensure!(
                        cfg["arguments"]["milliseconds"]
                            .as_u64()
                            .is_some_and(|v| v <= 60_000),
                        "milliseconds doit être compris entre 0 et 60000"
                    );
                }
            }
        }
        "input" => ensure!(
            ["text", "confirmation"].contains(&cfg["responseType"].as_str().unwrap_or("text")),
            "Type d'attente non encore disponible"
        ),
        _ => {}
    }
    Ok(())
}

/// Alternative arrivals become ADK routes rather than automatic Direct-edge joins.
/// The choice is explicit on the target node; native all-predecessor joins remain intact.
pub(crate) fn alternative_arrival(doc: &Composition, target: &str) -> bool {
    doc.nodes
        .iter()
        .any(|node| node.id == target && node.data.config["fanIn"] == "any")
}
