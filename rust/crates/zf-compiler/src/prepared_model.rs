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
        json!({"interactive":self.interactive(),"types":self.graph.types,"entry":self.graph.entry,"instances":instances,"inferences":inferences,"routes":self.graph.routes,"aliases":self.graph.aliases,"dataBindings":self.graph.data_bindings,"bridges":self.graph.bridges.keys().collect::<Vec<_>>(),"flowHashes":self.definitions.flow_hashes,"bridgeHashes":self.definitions.bridge_hashes})
    }
    pub fn root(&self) -> Result<&FrozenFlow> {
        self.flows
            .get(&self.graph.entry.instance)
            .context("Runtime entry instance is absent")
    }
    pub fn validate(&self, primitives: &dyn PrimitiveContracts) -> Result<()> {
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
