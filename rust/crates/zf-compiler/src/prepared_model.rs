//! Frozen file-backed inputs for one native ADK runtime composition.
use crate::{
    graph_compiler::{GraphValidator, PrimitiveContracts},
    resolve::{self, RuntimeGraph},
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use zf_flows::{
    composition::{CompositionCatalog, ResolveRequest},
    flow_contract::{self, FlowExports},
    package::PackageSnapshot,
    schema::Composition,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrozenFlow {
    pub key: String,
    pub hash: String,
    pub source: String,
    pub composition: Composition,
    pub exports: FlowExports,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreparedRuntime {
    pub graph: RuntimeGraph,
    /// Each independent flow instance owns execution state, while resources are
    /// references into the common entity universe. Definitions here are frozen.
    pub flows: BTreeMap<String, FrozenFlow>,
    #[serde(default)]
    pub definitions: DefinitionPins,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DefinitionPins {
    /// Authored file hashes differ from executable hashes after context linking.
    pub flow_hashes: BTreeMap<String, String>,
    /// Authored package closures are shared by all instances of the same flow.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub flow_packages: BTreeMap<String, PackageSnapshot>,
    pub bridge_hashes: BTreeMap<String, String>,
    pub bridge_sources: BTreeMap<String, String>,
    /// Initial per-instance choices. Effective program revisions live in each
    /// frozen flow; subsequent accepted source revisions may advance these pins.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub context_selections: BTreeMap<String, ContextSelection>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextSelection {
    pub key: String,
    pub hash: String,
}
impl PreparedRuntime {
    /// Effective interaction follows reachable route entries. Merely importing
    /// an interactive flow does not make an autonomous composition interactive.
    /// Declarations remain capabilities, including conditional human paths.
    pub fn interactive(&self) -> bool {
        let mut pending = vec![(
            self.graph.entry.instance.clone(),
            self.graph.entry.port.clone(),
        )];
        let mut visited = BTreeSet::new();
        while let Some((instance, entry)) = pending.pop() {
            if !visited.insert((instance.clone(), entry.clone())) {
                continue;
            }
            // Invalid/incomplete definitions cannot establish autonomy. Normal
            // callers validate the prepared graph before exposing or executing it.
            let Some(flow) = self.flows.get(&instance) else {
                return true;
            };
            if flow.exports.interactive {
                return true;
            }
            let Ok(projection) = flow_contract::at_entry(&flow.composition, &entry) else {
                return true;
            };
            if human_boundary(&projection) {
                return true;
            }
            for route in self
                .graph
                .routes
                .values()
                .filter(|route| route.from.instance == instance)
            {
                if flow
                    .exports
                    .branches
                    .get(&route.from.port)
                    .is_some_and(|node| projection.nodes.iter().any(|spec| spec.id == *node))
                {
                    pending.push((route.to.instance.clone(), route.to.port.clone()));
                }
            }
        }
        false
    }

    pub fn summary(&self) -> serde_json::Value {
        use serde_json::json;
        let instances:BTreeMap<_,_>=self.flows.iter().map(|(id,flow)|(id.clone(),json!({"flow":flow.key,"name":flow.composition.name,"hash":flow.hash,"interactive":flow.exports.interactive,"workingDirectory":flow.composition.settings.working_directory,"entries":flow.exports.contract.entries}))).collect();
        fn inference_nodes(
            doc: &Composition,
            instance: &str,
            scope: &str,
            out: &mut BTreeMap<String, serde_json::Value>,
        ) {
            for node in &doc.nodes {
                let path = format!("{scope}{}", node.id);
                if matches!(node.data.kind.as_str(), "agent" | "model") {
                    let context_config = if node.data.kind == "model" {
                        doc.nodes
                            .iter()
                            .find(|n| {
                                Some(n.id.as_str()) == node.data.config["contextNode"].as_str()
                            })
                            .map(|n| &n.data.config)
                            .unwrap_or(&node.data.config)
                    } else {
                        &node.data.config
                    };
                    let mut config = json!({});
                    for key in [
                        "provider",
                        "model",
                        "reasoningEffort",
                        "reasoning",
                        "modelBinding",
                        "contextStrategy",
                    ] {
                        if let Some(value) = node.data.config.get(key) {
                            config[key] = value.clone();
                        }
                    }
                    if let Some(value) = context_config.get("contextStrategy") {
                        config["contextStrategy"] = value.clone();
                    }
                    config["contextNode"] = node.data.config["contextNode"].clone();
                    let context_node = (node.data.kind == "model")
                        .then(|| {
                            doc.nodes.iter().find(|n| {
                                Some(n.id.as_str()) == node.data.config["contextNode"].as_str()
                            })
                        })
                        .flatten();
                    out.insert(path.clone(),json!({"instance":instance,"node":node.id,"label":node.data.label,"config":config,"contextProgramHash":context_config["contextProgram"]["hash"],"context":context_node.map(|n| json!({"node":n.id,"label":n.data.label,"config":n.data.config})),"contextPath":context_node.map(|n| format!("{scope}{}",n.id))}));
                }
                if node.data.kind == "subgraph"
                    && let Ok(child) =
                        serde_json::from_value(node.data.config["composition"].clone())
                {
                    inference_nodes(&child, instance, &format!("{path}/"), out);
                }
            }
        }
        let mut inferences = BTreeMap::new();
        for (id, flow) in &self.flows {
            inference_nodes(&flow.composition, id, &format!("{id}/"), &mut inferences);
        }
        let package_revisions: BTreeMap<_, _> = self
            .definitions
            .flow_packages
            .iter()
            .map(|(key, package)| (key, &package.root))
            .collect();
        json!({"packageRevisions":package_revisions,"interactive":self.interactive(),"types":self.graph.types,"entry":self.graph.entry,"instances":instances,"inferences":inferences,"routes":self.graph.routes,"aliases":self.graph.aliases,"dataBindings":self.graph.data_bindings,"bridges":self.graph.bridges.keys().collect::<Vec<_>>(),"flowHashes":self.definitions.flow_hashes,"bridgeHashes":self.definitions.bridge_hashes})
    }
    pub fn root(&self) -> Result<&FrozenFlow> {
        self.flows
            .get(&self.graph.entry.instance)
            .context("Runtime entry instance is absent")
    }
    pub fn validate(&self, primitives: &dyn PrimitiveContracts) -> Result<()> {
        let mut authored_packages = BTreeMap::new();
        let mut package_identities = BTreeMap::new();
        for (key, package) in &self.definitions.flow_packages {
            ensure!(
                self.flows.values().any(|flow| &flow.key == key),
                "Orphan package pin: {key}"
            );
            let authored = package_authored(package, primitives)?;
            for (revision, node) in &package.packages {
                let id = node.package_id()?;
                if let Some(previous) = package_identities.insert(id.clone(), revision) {
                    ensure!(
                        previous == revision,
                        "Concurrent package revisions across selected flows: {}",
                        id.as_str()
                    );
                }
            }
            let source = package.root_node()?.entry_source()?;
            ensure!(
                self.definitions.flow_hashes.get(key)
                    == Some(&crate::programs::hash(source.as_bytes())),
                "Package authored source hash mismatch: {key}"
            );

            authored_packages.insert(key, authored);
        }
        if !self.definitions.context_selections.is_empty() {
            let summary = self.summary();
            for (path, selection) in &self.definitions.context_selections {
                let config = &summary["inferences"][path]["config"];
                let reference = &config["contextStrategy"];
                ensure!(
                    reference.as_str().or(reference["key"].as_str())
                        == Some(selection.key.as_str()),
                    "Runtime context choice does not match inference {path}"
                );
                ensure!(
                    selection.hash.len() == 64
                        && selection.hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
                    "Runtime context choice has an invalid initial hash: {path}"
                );
            }
        }
        ensure!(
            self.definitions
                .bridge_hashes
                .keys()
                .eq(self.definitions.bridge_sources.keys()),
            "Bridge pins and sources differ"
        );
        for (key, source) in &self.definitions.bridge_sources {
            ensure!(
                format!("{:x}", Sha256::digest(source.as_bytes()))
                    == self.definitions.bridge_hashes[key],
                "Frozen bridge source hash mismatch: {key}"
            );
            let parsed =
                zf_flows::bridge_source::parse(source).map_err(|d| anyhow::anyhow!("{d:?}"))?;
            ensure!(
                self.graph
                    .bridges
                    .get(key)
                    .is_some_and(|bridge| serde_json::to_value(bridge).ok()
                        == serde_json::to_value(&parsed).ok()),
                "Frozen bridge source and definition disagree: {key}"
            );
        }
        for (instance, flow) in &self.flows {
            ensure!(
                self.graph
                    .instances
                    .get(instance)
                    .is_some_and(|resolved| resolved.flow == flow.key),
                "Frozen flow key and resolved instance disagree: {instance}"
            );
            if let Some(authored) = authored_packages.get(&flow.key) {
                let linked = validate_package_execution(
                    authored,
                    &flow.composition,
                    instance,
                    &self.definitions.context_selections,
                )?;
                let authored_source = self.definitions.flow_packages[&flow.key]
                    .root_node()?
                    .entry_source()?;
                validate_package_source(
                    authored_source,
                    &flow.source,
                    &flow.composition,
                    linked,
                    primitives,
                )?;
            }
            ensure!(
                format!("{:x}", Sha256::digest(flow.source.as_bytes())) == flow.hash,
                "Frozen flow source hash mismatch: {instance}"
            );
            let parsed =
                zf_flows::flow_format::parse(&flow.source, &GraphValidator::new(primitives))?;
            ensure!(
                serde_json::to_value(&parsed)? == serde_json::to_value(&flow.composition)?,
                "Frozen source and flow disagree: {instance}"
            );
            let exports = flow_contract::validate(&flow.composition)?
                .context("Frozen flow has no exports")?;
            ensure!(
                serde_json::to_value(exports)? == serde_json::to_value(&flow.exports)?,
                "Frozen exports disagree: {instance}"
            );
            ensure!(
                self.graph.instances.contains_key(instance),
                "Instance {instance} is not in the resolved graph"
            );
        }
        ensure!(
            self.flows.keys().eq(self.graph.instances.keys()),
            "Resolved graph and frozen instance sets differ"
        );
        let mut catalog = CompositionCatalog {
            types: self.graph.types.clone(),
            bridges: self.graph.bridges.clone(),
            ..Default::default()
        };
        for (instance, resolved) in &self.graph.instances {
            let declared = &self.flows[instance].exports.contract;
            ensure!(
                serde_json::to_value(declared)? == serde_json::to_value(&resolved.definition)?,
                "Resolved contract and frozen flow differ: {instance}"
            );
            if let Some(previous) = catalog
                .flows
                .insert(resolved.flow.clone(), declared.clone())
            {
                ensure!(
                    serde_json::to_value(previous)? == serde_json::to_value(declared)?,
                    "Conflicting definitions of the same flow"
                );
            }
        }
        let root = self
            .graph
            .instances
            .get(&self.graph.entry.instance)
            .context("Missing runtime entry")?;
        let request = ResolveRequest {
            flow: root.flow.clone(),
            entry: self.graph.entry.port.clone(),
            bridges: self.graph.bridges.keys().cloned().collect(),
        };
        let resolved = resolve::resolve(&catalog, &request)
            .map_err(|d| anyhow::anyhow!("Invalid frozen composition: {d:?}"))?;
        ensure!(
            serde_json::to_value(resolved)? == serde_json::to_value(&self.graph)?,
            "Frozen routes, aliases or grants do not match their bridge definitions"
        );
        for (id, route) in &self.graph.routes {
            let source = &self.flows[&route.from.instance];
            ensure!(
                flow_contract::requesters(&source.exports, &route.from.port)
                    .into_iter()
                    .any(|node| flow_contract::requester_accepts(
                        &source.composition,
                        node,
                        route.invocation
                    )),
                "Route {id}: no authorized requester accepts this trigger"
            );
        }
        Ok(())
    }
}

fn human_boundary(doc: &Composition) -> bool {
    if flow_contract::read(doc)
        .ok()
        .flatten()
        .is_some_and(|exports| exports.interactive)
    {
        return true;
    }
    doc.nodes.iter().any(|node| {
        matches!(node.data.kind.as_str(), "input" | "inbox")
            || node.data.kind == "subgraph"
                && serde_json::from_value::<Composition>(node.data.config["composition"].clone())
                    .map_or(true, |child| human_boundary(&child))
    })
}

/// Validate an executable definition against its captured authored package.
/// `selections` contains explicit context choices with paths relative to this
/// definition, such as `model` or `child/model`. The caller authenticates source
/// lineage and hashes stored outside this package; no live catalogue is read.
///
/// # Errors
/// Rejects corrupt closures, mismatched flow identity or executable source,
/// invalid selections, and edits beyond supported context linking/overrides.
pub fn validate_package_definition(
    package: &PackageSnapshot,
    source: &str,
    composition: &Composition,
    selections: &BTreeMap<String, ContextSelection>,
    primitives: &dyn PrimitiveContracts,
) -> Result<()> {
    let authored = package_authored(package, primitives)?;
    let selections: BTreeMap<String, ContextSelection> = selections
        .iter()
        .map(|(path, selection)| (format!("/{path}"), selection.clone()))
        .collect();
    let linked = validate_package_execution(&authored, composition, "", &selections)?;
    let parsed = zf_flows::flow_format::parse(source, &GraphValidator::new(primitives))?;
    ensure!(
        serde_json::to_value(parsed)? == serde_json::to_value(composition)?,
        "Frozen source and flow disagree"
    );
    validate_package_source(
        package.root_node()?.entry_source()?,
        source,
        composition,
        linked,
        primitives,
    )
}

fn package_authored(
    package: &PackageSnapshot,
    primitives: &dyn PrimitiveContracts,
) -> Result<Composition> {
    crate::package_sources::validate_package_sources(package)?;
    let authored = zf_flows::flow_format::parse(
        package.root_node()?.entry_source()?,
        &GraphValidator::new(primitives),
    )?;
    ensure!(
        package.root_manifest()?.id.as_str() == authored.id,
        "Package identity and authored flow disagree"
    );
    Ok(authored)
}

fn validate_package_source(
    authored_source: &str,
    source: &str,
    composition: &Composition,
    linked: bool,
    primitives: &dyn PrimitiveContracts,
) -> Result<()> {
    let expected_source = if linked {
        zf_flows::flow_format::render(composition, &GraphValidator::new(primitives))?
    } else {
        authored_source.to_owned()
    };
    ensure!(
        source == expected_source,
        "Package executable source differs from captured source or canonical context linking"
    );
    Ok(())
}

/// Rebuild only the transformations the compiler permits between authored and
/// executable documents. In particular, a valid executable hash alone cannot
/// substitute another graph, tool configuration, or context binding for a pin.
fn validate_package_execution(
    authored: &Composition,
    executable: &Composition,
    instance: &str,
    selections: &BTreeMap<String, ContextSelection>,
) -> Result<bool> {
    let mut expected = authored.clone();
    let prefix = format!("{instance}/");
    let selections: BTreeMap<String, ContextSelection> = selections
        .iter()
        .filter_map(|(path, selection)| {
            path.strip_prefix(&prefix)
                .map(|path| (path.to_owned(), selection.clone()))
        })
        .collect();
    for (path, selection) in &selections {
        ensure!(
            selection.hash.len() == 64
                && selection.hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "Runtime context choice has an invalid initial hash: {path}"
        );
    }
    crate::prepared::apply_context_selections(&mut expected, &selections, Some(executable))?;
    let linked = relink_captured_programs(&mut expected, executable)?;
    ensure!(
        serde_json::to_value(expected)? == serde_json::to_value(executable)?,
        "Package authored flow and executable differ beyond context linking: {instance}"
    );
    Ok(linked || !selections.is_empty())
}

fn relink_captured_programs(expected: &mut Composition, executable: &Composition) -> Result<bool> {
    use crate::programs::{self, ProgramSources, SourceKind, SourceSnapshot};
    let mut linked = false;
    for node in &mut expected.nodes {
        let actual = executable
            .nodes
            .iter()
            .find(|actual| actual.id == node.id)
            .with_context(|| format!("Package executable node absent: {}", node.id))?;
        if node.data.kind == "subgraph" {
            let mut child = serde_json::from_value(node.data.config["composition"].clone())?;
            let actual_child = serde_json::from_value(actual.data.config["composition"].clone())?;
            linked |= relink_captured_programs(&mut child, &actual_child)?;
            node.data.config["composition"] = serde_json::to_value(child)?;
        }
        if !matches!(node.data.kind.as_str(), "agent" | "context")
            || node
                .data
                .config
                .get("contextStrategy")
                .is_none_or(serde_json::Value::is_null)
        {
            continue;
        }
        let program = &actual.data.config["contextProgram"];
        programs::validate_frozen(program)?;
        let mut sources = ProgramSources::default();
        let source = program["source"]
            .as_str()
            .context("Frozen strategy source absent")?;
        let key = program["strategy"]["id"]
            .as_str()
            .context("Frozen strategy identity absent")?;
        sources
            .strategies
            .insert(key.to_owned(), SourceSnapshot::capture(source.to_owned()));
        for (field, files) in [
            ("librarySources", &mut sources.libraries),
            ("typeSources", &mut sources.types),
        ] {
            if let Some(captures) = program[field].as_array() {
                for capture in captures {
                    let key = capture["key"]
                        .as_str()
                        .context("Frozen context dependency key absent")?;
                    let source = capture["source"]
                        .as_str()
                        .context("Frozen context dependency source absent")?;
                    ensure!(
                        files
                            .insert(key.to_owned(), SourceSnapshot::capture(source.to_owned()))
                            .is_none(),
                        "Duplicate frozen context dependency: {key}"
                    );
                }
            }
        }
        // Source lineage is authenticated by the capturing host. Frozen runtime
        // validation permits an accepted later revision of the same reference,
        // while reconstructing every effective program field from captured bytes.
        let mut scope = Composition {
            nodes: vec![node.clone()],
            format_version: expected.format_version,
            id: expected.id.clone(),
            name: expected.name.clone(),
            revision: expected.revision,
            edges: Vec::new(),
            settings: expected.settings.clone(),
            channels: Vec::new(),
        };
        for reference in programs::references(&scope)? {
            let files = match reference.kind {
                SourceKind::Strategy => &mut sources.strategies,
                SourceKind::Library => &mut sources.libraries,
                SourceKind::Types => &mut sources.types,
            };
            if let Some(hash) = reference.hash
                && let Some(file) = files.get_mut(&reference.key)
            {
                file.accepted_references.insert(hash);
            }
        }
        programs::freeze(&mut scope, &sources)?;
        linked = true;
        node.data.config = scope.nodes.remove(0).data.config;
    }
    Ok(linked)
}
