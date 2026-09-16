//! Runtime-owned interpretation of frozen archive and checkpoint documents.
use adk_graph::state::Checkpoint;
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::Path;
use uuid::Uuid;
use zf_flows::schema::Composition;
use zf_storage::contracts::CheckpointCodec;
use zf_storage::{
    content_store::ContentStore,
    contracts::{ArchiveRuntime, ArchiveSnapshot, DependencyInspection, RuntimeInspection},
};

/// Uses the exact installed ADK schema rather than duplicating its defaults and
/// serialization rules in the SQLite reader. No node is executed by this codec.
pub struct AdkCheckpointCodec;
impl CheckpointCodec for AdkCheckpointCodec {
    fn decode_legacy_checkpoint(&self, value: Value) -> Result<Value> {
        let checkpoint: Checkpoint = serde_json::from_value(value)?;
        Ok(serde_json::to_value(checkpoint)?)
    }
    fn validate_checkpoint(&self, value: &Value) -> Result<()> {
        let _: Checkpoint = serde_json::from_value(value.clone())?;
        Ok(())
    }
}

/// Interprets captured definitions and sealed capabilities with this runtime's
/// real primitives. Inspection and import never dispatch a graph or an effect.
pub struct RuntimeArchiveValidation;
impl CheckpointCodec for RuntimeArchiveValidation {
    fn decode_legacy_checkpoint(&self, value: Value) -> Result<Value> {
        AdkCheckpointCodec.decode_legacy_checkpoint(value)
    }
    fn validate_checkpoint(&self, value: &Value) -> Result<()> {
        AdkCheckpointCodec.validate_checkpoint(value)
    }
}
impl RuntimeInspection for RuntimeArchiveValidation {
    fn summary(&self, definition: &Value) -> Result<Value> {
        let prepared: zf_compiler::prepared_model::PreparedRuntime =
            serde_json::from_value(definition.clone())?;
        prepared.validate(&crate::materialize::RuntimePrimitives)?;
        Ok(prepared.summary())
    }
    fn interactive(&self, definition: &Value) -> Result<bool> {
        let prepared: zf_compiler::prepared_model::PreparedRuntime =
            serde_json::from_value(definition.clone())?;
        prepared.validate(&crate::materialize::RuntimePrimitives)?;
        Ok(prepared.interactive())
    }
}
impl ArchiveRuntime for RuntimeArchiveValidation {
    fn definition_diagnostics(&self, run: &Value) -> Vec<String> {
        let mut blocked = Vec::new();
        let validator = zf_compiler::graph_compiler::GraphValidator::new(
            &crate::materialize::RuntimePrimitives,
        );
        match serde_json::from_value::<Composition>(run["composition"].clone()) {
            Ok(doc) => {
                if let Err(error) = zf_compiler::graph_compiler::validate(
                    &doc,
                    &crate::materialize::RuntimePrimitives,
                ) {
                    blocked.push(format!("Flow incompatible : {error}"));
                }
                if let Some(source) = run["flowSource"].as_str() {
                    if run
                        .get("executedSourceHash")
                        .unwrap_or(&run["flowRef"]["hash"])
                        != &json!(digest(source.as_bytes()))
                    {
                        blocked.push("Empreinte du Rust figé invalide".into());
                    }
                    let parsed =
                        zf_flows::flow_format::parse(source, &validator).and_then(|parsed| {
                            if let Some(runtime) =
                                run.get("runtimeGraph").filter(|value| !value.is_null())
                            {
                                let runtime: zf_compiler::prepared_model::PreparedRuntime =
                                    serde_json::from_value(runtime.clone())?;
                                runtime.validate(&crate::materialize::RuntimePrimitives)?;
                                ensure!(
                                    runtime.root()?.source == source,
                                    "La source racine diffère du graphe runtime"
                                );
                                zf_flows::flow_contract::at_entry(
                                    &parsed,
                                    &runtime.graph.entry.port,
                                )
                            } else {
                                Ok(parsed)
                            }
                        });
                    match parsed {
                        Ok(parsed)
                            if zf_flows::flow_format::render(&parsed, &validator)
                                .and_then(|rendered| {
                                    Ok(rendered == zf_flows::flow_format::render(&doc, &validator)?)
                                })
                                .unwrap_or(false) => {}
                        Ok(_) => blocked.push(
                            "Le Rust figé ne correspond pas à la composition exécutable".into(),
                        ),
                        Err(error) => blocked.push(format!("Rust figé incompatible : {error}")),
                    }
                } else {
                    blocked.push("Source Rust figée absente".into());
                }
            }
            Err(error) => blocked.push(format!("Flow incompatible : {error}")),
        }

        blocked.sort();
        blocked.dedup();
        blocked
    }
    fn dependencies(&self, run: &Value) -> DependencyInspection {
        let resources = match (
            serde_json::from_value::<Composition>(run["composition"].clone()),
            run["workspacePath"].as_str(),
        ) {
            (Ok(composition), Some(cwd)) => {
                crate::resources::dependency_diagnostics(&composition, Path::new(cwd))
            }
            _ => vec![],
        };
        DependencyInspection {
            blocked: dependency_diagnostics(run),
            resources,
        }
    }
    fn resumable_internal_receipt<'a>(
        &'a self,
        store: &'a ContentStore,
        snapshot: ArchiveSnapshot<'a>,
        receipt: &'a Value,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<bool>> + Send + 'a>> {
        Box::pin(async move { resumable_internal_receipt(store, &snapshot, receipt).await })
    }
}
const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MISSING_PIECE: &str = "Pièce indisponible dans le workspace cible";
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
async fn resumable_internal_receipt(
    store: &ContentStore,
    bundle: &ArchiveSnapshot<'_>,
    receipt: &Value,
) -> Result<bool> {
    async fn record(
        store: &ContentStore,
        bundle: &ArchiveSnapshot<'_>,
        kind: &str,
        key: &str,
    ) -> Result<Option<Value>> {
        let Some(record) = bundle.records.iter().find(|record| {
            record.kind == kind && record.key == key && record.scope == bundle.session_id
        }) else {
            return Ok(None);
        };
        Ok(Some(
            store
                .resolve_with_limit(&record.value_ref, MAX_BYTES)
                .await?,
        ))
    }
    async fn proof(
        store: &ContentStore,
        bundle: &ArchiveSnapshot<'_>,
        receipt: &Value,
    ) -> Result<()> {
        let name = receipt["name"].as_str().context("Receipt tool absent")?;
        ensure!(
            !crate::operations::tool_declarations().contains_key(name),
            "External effects cannot resume an uncertain receipt"
        );
        let path = receipt["nodePath"]
            .as_str()
            .context("Receipt owner absent")?;
        let call_id = receipt["id"].as_str().context("Receipt identity absent")?;
        let (invocation, index) = call_id
            .rsplit_once(':')
            .context("Unsealed runtime receipt")?;
        Uuid::parse_str(invocation)?;
        let index: usize = index.parse()?;
        ensure!(
            call_id == format!("{invocation}:{index}"),
            "Noncanonical call identity"
        );
        let model = record(store, bundle, "model-calls", invocation)
            .await?
            .context("Sealed invocation absent")?;
        let call = model["calls"].get(index).context("Sealed call absent")?;
        ensure!(
            model["invocationId"] == invocation
                && model["agentPath"] == path
                && call["name"] == name
                && call["args"] == receipt["arguments"]
                && call["provenance"]["invocationId"] == invocation
                && call["provenance"]["agentPath"] == path
                && call["provenance"]["callIndex"] == index
                && model["tools"]
                    .as_array()
                    .is_some_and(|tools| tools.contains(&json!(name))),
            "Receipt differs from sealed model capability"
        );
        if zf_context::window::is_tool(name) {
            let snapshot = record(store, bundle, "capability-snapshots", invocation)
                .await?
                .context("Window grants absent")?;
            ensure!(
                snapshot["agentPath"] == path,
                "Window grants belong to another agent"
            );
            let args = &receipt["arguments"];
            let alias = args["alias"].as_str().context("Window alias absent")?;
            let grants = zf_context::window::grants(
                &json!({"windowGrants":snapshot["prepared"]["windowGrants"]}),
            )?;
            let write = name == "context_window_patch";
            ensure!(
                grants.iter().any(|grant| grant.alias == alias
                    && (!write || grant.permission == zf_core::identity::Permission::Write)),
                "Window permission absent"
            );
            let scope = path
                .rsplit_once('/')
                .map_or("root", |(instance, _)| instance);
            let binding = bundle
                .registry
                .aliases
                .iter()
                .find(|item| {
                    item.scope_kind == "flow" && item.scope_id == scope && item.name == alias
                })
                .context("Window scoped alias absent")?;
            ensure!(
                !write || binding.permission == "write",
                "Window alias is not writable"
            );
            let revision = if write {
                args["expectedRevision"].as_str()
            } else {
                args["revision"].as_str()
            };
            let requested_revision = revision
                .or_else(|| {
                    bundle
                        .registry
                        .entities
                        .iter()
                        .find(|item| item.id == binding.entity)
                        .map(|item| item.head.as_str())
                })
                .context("Window head absent")?;
            let content = bundle
                .registry
                .revisions
                .iter()
                .find(|item| item.id == requested_revision && item.entity == binding.entity)
                .context("Window revision absent or foreign")?;
            let window: zf_context::window::PreparedWindow = serde_json::from_value(
                store
                    .resolve_with_limit(&content.content_ref, MAX_BYTES)
                    .await?,
            )?;
            zf_context::window::validate(&window)?;
            if write {
                let expected = revision.context("Window expected revision absent")?;
                ensure!(args["patches"].is_array(), "Window patches absent");
                let publication = format!("window:{path}:{call_id}");
                let committed = bundle.registry.publications.iter().any(|item| {
                    item.id == publication
                        && item.entity == binding.entity
                        && item.scope_kind == "flow"
                        && item.scope_id == scope
                        && item.alias == alias
                        && item.expected_revision.as_deref() == Some(expected)
                });
                let untouched = bundle
                    .registry
                    .entities
                    .iter()
                    .any(|item| item.id == binding.entity && item.head == expected);
                ensure!(
                    committed || untouched,
                    "Window patch frontier changed without a matching publication"
                );
            }
            return Ok(());
        }
        let captured = record(store, bundle, "route-inputs", invocation)
            .await?
            .context("Route input capture absent")?;
        ensure!(
            captured["agentPath"] == path && captured["state"].is_object(),
            "Route capture belongs to another agent"
        );
        let plan = if let Some(reference) = captured["graphRef"].as_str() {
            store.resolve_with_limit(reference, MAX_BYTES).await?
        } else {
            bundle.run["runtimeGraph"].clone()
        };
        let plan: zf_compiler::prepared_model::PreparedRuntime = serde_json::from_value(plan)?;
        plan.validate(&crate::materialize::RuntimePrimitives)?;
        let (instance, node) = path
            .rsplit_once('/')
            .context("Route owner has no instance")?;
        let flow = plan.flows.get(instance).context("Route owner absent")?;
        let routes = plan
            .graph
            .routes
            .iter()
            .filter(|(_, route)| {
                route.invocation == zf_flows::composition::InvocationKind::Tool
                    && route.from.instance == instance
                    && route.tool_name.as_deref() == Some(name)
                    && flow
                        .exports
                        .branches
                        .get(&route.from.port)
                        .is_some_and(|owner| owner == node)
            })
            .collect::<Vec<_>>();
        ensure!(routes.len() == 1, "Captured tool route absent or ambiguous");
        let (route_id, route) = routes[0];
        let args = &receipt["arguments"];
        ensure!(
            args.as_object()
                .is_some_and(|args| args.len() == 1 && args.contains_key("input")),
            "Invalid route arguments"
        );
        zf_core::types::validate_value(&route.input, &args["input"], &plan.graph.types)
            .map_err(|errors| anyhow::anyhow!("Invalid route input: {errors:?}"))?;
        let visit_id = digest(format!("{path}\0{route_id}\0{call_id}").as_bytes());
        if let Some(visit) = record(store, bundle, "route-visits", &visit_id).await? {
            ensure!(
                visit["routeId"] == *route_id
                    && visit["path"] == path
                    && visit["callId"] == call_id
                    && visit["input"] == args["input"]
                    && visit["instance"] == route.to.instance
                    && visit["entry"] == route.to.port
                    && visit["targetHash"] == plan.flows[&route.to.instance].hash
                    && visit["mode"] == json!(route.mode)
                    && visit["threadId"] == format!("{}/route/{visit_id}", bundle.session_id),
                "Route visit differs from its captured resolution"
            );
        }
        Ok(())
    }
    Ok(proof(store, bundle, receipt).await.is_ok())
}

fn dependency_diagnostics(run: &Value) -> Vec<String> {
    use crate::agent_capabilities::{Activation, InstructionSource, SkillSource};
    let mut blocked = Vec::new();
    let Some(cwd) = run["workspacePath"].as_str() else {
        return blocked;
    };
    let check_file = |path: &Path, skill: bool| -> Result<()> {
        let path = if path.is_absolute() {
            path.to_owned()
        } else {
            Path::new(cwd).join(path)
        };
        let metadata = std::fs::metadata(&path)?;
        ensure!(
            metadata.is_file() && metadata.len() <= 1024 * 1024,
            "fichier ordinaire UTF-8 requis, 1 Mio maximum"
        );
        if skill {
            crate::workspace_context::read_skill(&path)?;
        } else {
            std::fs::read_to_string(&path)?;
        }
        Ok(())
    };
    let mut pending = vec![(&run["composition"], String::new())];
    while let Some((composition, prefix)) = pending.pop() {
        for node in composition["nodes"].as_array().into_iter().flatten() {
            let Some(id) = node["id"].as_str() else {
                continue;
            };
            let path = if prefix.is_empty() {
                id.to_owned()
            } else {
                format!("{prefix}/{id}")
            };
            let config = &node["data"]["config"];
            if node["data"]["kind"] == "subgraph" {
                pending.push((&config["composition"], path));
                continue;
            }
            if !matches!(node["data"]["kind"].as_str(), Some("agent" | "context"))
                || composition["formatVersion"].as_u64().unwrap_or(1) < 2
            {
                continue;
            }
            let Ok(pieces) = crate::agent_capabilities::attachments(config) else {
                continue;
            };
            let activation_path = if node["data"]["kind"] == "context" {
                config["modelNode"]
                    .as_str()
                    .map(|model| {
                        if prefix.is_empty() {
                            model.to_owned()
                        } else {
                            format!("{prefix}/{model}")
                        }
                    })
                    .unwrap_or_else(|| path.clone())
            } else {
                path.clone()
            };
            let active = run["capabilityActivations"][&activation_path].as_array();
            let enabled_now = |id: &str, activation: Activation| {
                activation == Activation::Always
                    || active.is_some_and(|keys| keys.iter().any(|key| key == id))
            };
            let mut check = |id: &str, file: &Path, skill: bool| {
                if let Err(error) = check_file(file, skill) {
                    blocked.push(format!(
                        "{MISSING_PIECE} : {path}/{id} ({}) : {error}",
                        file.display()
                    ));
                }
            };
            for item in pieces
                .instructions
                .iter()
                .flat_map(|piece| &piece.items)
                .filter(|item| item.enabled && enabled_now(&item.id, item.activation))
            {
                if let InstructionSource::File { path } = &item.source {
                    check(&item.id, path, false);
                }
            }
            for item in pieces
                .files
                .iter()
                .flat_map(|piece| &piece.items)
                .filter(|item| item.enabled && enabled_now(&item.id, item.activation))
            {
                check(&item.id, &item.path, false);
            }
            for item in pieces
                .skills
                .iter()
                .flat_map(|piece| &piece.items)
                .filter(|item| item.enabled)
            {
                match &item.source {
                    SkillSource::File { path } => check(&item.id, path, true),
                    SkillSource::Workspace => {
                        for skill in run["context"]["skills"].as_array().into_iter().flatten() {
                            if let (Some(name), Some(path)) =
                                (skill["name"].as_str(), skill["path"].as_str())
                                && item.name.as_ref().is_none_or(|selected| selected == name)
                                && enabled_now(&format!("{}::{name}", item.id), item.activation)
                            {
                                check(&item.id, Path::new(path), true);
                            }
                        }
                    }
                }
            }
        }
    }
    blocked.sort();
    blocked.dedup();
    blocked
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    #[tokio::test]
    async fn internal_receipt_proof_is_read_only_and_never_allows_uncertain_external_effects() {
        use zf_context::window::PreparedWindow;
        use zf_core::identity::Scope;
        use zf_storage::{
            data::{DataRegistry, WindowRegistry},
            data_archive,
        };
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let store = ContentStore::new(pool).await.unwrap();
        let run = Uuid::new_v4().to_string();
        let registry = DataRegistry::new(store.pool().clone(), store.clone(), &run)
            .await
            .unwrap();
        let scope = Scope::Flow("root".into());
        let window = PreparedWindow {
            strategy_id: "test".into(),
            strategy_revision: "first".into(),
            program_revision: None,
            items: vec![],
            source_revisions: BTreeMap::new(),
            capabilities: vec![],
        };
        let initial = WindowRegistry::new(registry.clone())
            .create(&scope, "view", &window)
            .await
            .unwrap();
        let invocation = Uuid::new_v4().to_string();
        let call_id = format!("{invocation}:0");
        let args = json!({"alias":"view","expectedRevision":initial.revision,"patches":[]});
        let receipt = json!({"id":call_id,"nodePath":"root/agent","name":"context_window_patch","arguments":args,"status":"started"});
        store.put_record(&run,"model-calls",&invocation,&json!({"invocationId":invocation,"agentPath":"root/agent","tools":["context_window_patch"],"calls":[{"name":"context_window_patch","args":args,"provenance":{"invocationId":invocation,"agentPath":"root/agent","callIndex":0}}]})).await.unwrap();
        store.put_record(&run,"capability-snapshots",&invocation,&json!({"agentPath":"root/agent","prepared":{"windowGrants":[{"alias":"view","permission":"write"}]}})).await.unwrap();
        let records_snapshot = store.records(&run).await.unwrap();
        let mut registry_snapshot = data_archive::capture(store.pool(), &run).await.unwrap();
        let run_snapshot = json!({});
        let before = json!(registry_snapshot);
        let records = json!(store.records(&run).await.unwrap());
        assert!(
            resumable_internal_receipt(
                &store,
                &ArchiveSnapshot {
                    session_id: &run,
                    run: &run_snapshot,
                    records: &records_snapshot,
                    registry: &registry_snapshot
                },
                &receipt
            )
            .await
            .unwrap()
        );
        for (field, value) in [
            ("name", json!("exec")),
            ("nodePath", json!("root/foreign")),
            ("arguments", json!({"alias":"other"})),
        ] {
            let mut forged = receipt.clone();
            forged[field] = value;
            assert!(
                !resumable_internal_receipt(
                    &store,
                    &ArchiveSnapshot {
                        session_id: &run,
                        run: &run_snapshot,
                        records: &records_snapshot,
                        registry: &registry_snapshot
                    },
                    &forged
                )
                .await
                .unwrap()
            );
        }
        assert_eq!(
            before,
            json!(data_archive::capture(store.pool(), &run).await.unwrap())
        );
        assert_eq!(records, json!(store.records(&run).await.unwrap()));
        let mut next = window.clone();
        next.strategy_revision = "second".into();
        let published = registry
            .publish_unique(
                &scope,
                "view",
                Some(&initial.revision),
                &json!(window),
                &format!("window:root/agent:{call_id}"),
            )
            .await
            .unwrap();
        registry
            .publish(&scope, "view", &published.revision, &json!(next))
            .await
            .unwrap();
        registry_snapshot = data_archive::capture(store.pool(), &run).await.unwrap();
        assert!(
            resumable_internal_receipt(
                &store,
                &ArchiveSnapshot {
                    session_id: &run,
                    run: &run_snapshot,
                    records: &records_snapshot,
                    registry: &registry_snapshot
                },
                &receipt
            )
            .await
            .unwrap(),
            "committed publication remains recoverable after a later head"
        );
        registry_snapshot.publications.clear();
        assert!(
            !resumable_internal_receipt(
                &store,
                &ArchiveSnapshot {
                    session_id: &run,
                    run: &run_snapshot,
                    records: &records_snapshot,
                    registry: &registry_snapshot
                },
                &receipt
            )
            .await
            .unwrap(),
            "an advanced head without the unique publication cannot replay the patch"
        );
    }
    #[test]
    fn frozen_definition_validation_rejects_hash_and_semantic_mismatches() {
        let composition: Composition = serde_json::from_value(json!({
            "id":"frozen", "name":"Frozen", "nodes":[
                {"id":"s","position":{"x":0,"y":0},"data":{"kind":"start","label":"Start","config":{}}},
                {"id":"o","position":{"x":100,"y":0},"data":{"kind":"output","label":"Output","config":{"text":"first"}}},
                {"id":"e","position":{"x":200,"y":0},"data":{"kind":"end","label":"End","config":{}}}
            ], "edges":[{"id":"a","source":"s","target":"o"},{"id":"b","source":"o","target":"e"}]
        })).unwrap();
        let validator = zf_compiler::graph_compiler::GraphValidator::new(
            &crate::materialize::RuntimePrimitives,
        );
        let source = zf_flows::flow_format::render(&composition, &validator).unwrap();
        let mut run = json!({"composition":composition, "flowSource":source,"flowRef":{"hash":digest(source.as_bytes())}});
        let provider = RuntimeArchiveValidation;
        assert!(provider.definition_diagnostics(&run).is_empty());
        run["flowRef"]["hash"] = json!("wrong");
        assert!(
            provider
                .definition_diagnostics(&run)
                .iter()
                .any(|d| d.contains("Empreinte"))
        );
        run["flowRef"]["hash"] = json!(digest(source.as_bytes()));
        run["composition"]["nodes"][1]["data"]["config"]["text"] = json!("changed after capture");
        assert!(
            provider
                .definition_diagnostics(&run)
                .iter()
                .any(|d| d.contains("ne correspond pas"))
        );
        run["composition"] = json!(composition);
        run["flowSource"] = Value::Null;
        assert!(
            provider
                .definition_diagnostics(&run)
                .iter()
                .any(|d| d.contains("Source Rust figée absente"))
        );
        assert!(provider.summary(&json!({})).is_err());
        assert!(provider.interactive(&json!({})).is_err());
    }
}
