//! Read-only navigation from a passage to its exact executable definition.
use crate::{materialize::RuntimePrimitives, revisions::RevisionDefinition};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use zf_compiler::prepared_model::PreparedRuntime;
use zf_storage::content_store::ContentStore;

#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefinitionQuery {
    pub node_path: Option<String>,
    pub occurrence_id: Option<String>,
    pub hash: Option<String>,
}
async fn field(store: &ContentStore, run: &Value, name: &str) -> Result<Value> {
    if let Some(reference) = run[format!("{name}Ref")].as_str() {
        store.resolve(reference).await
    } else {
        Ok(run[name].clone())
    }
}
async fn initial(
    store: &ContentStore,
    run: &Value,
) -> Result<BTreeMap<String, RevisionDefinition>> {
    let runtime = field(store, run, "runtimeGraph").await?;
    if !runtime.is_null() {
        let runtime: PreparedRuntime = serde_json::from_value(runtime)?;
        runtime.validate(&RuntimePrimitives)?;
        return Ok(runtime
            .flows
            .into_iter()
            .map(|(instance, flow)| {
                (
                    instance,
                    RevisionDefinition {
                        key: flow.key,
                        source: flow.source,
                        hash: flow.hash,
                        composition: flow.composition,
                    },
                )
            })
            .collect());
    }
    let composition = field(store, run, "composition").await?;
    let source = field(store, run, "flowSource").await?;
    let source = source
        .as_str()
        .context("Exact historical flow source is unavailable")?
        .to_owned();
    Ok(BTreeMap::from([(
        String::new(),
        RevisionDefinition {
            key: run["flowRef"]["key"]
                .as_str()
                .or(composition["id"].as_str())
                .unwrap_or_default()
                .into(),
            hash: zf_storage::flow_store::hash(source.as_bytes()),
            source,
            composition: serde_json::from_value(composition)?,
        },
    )]))
}
fn inside(path: &str, instance: &str) -> bool {
    instance.is_empty()
        || path == instance
        || path
            .strip_prefix(instance)
            .is_some_and(|tail| tail.starts_with('/'))
}

pub async fn definition(
    store: &ContentStore,
    run: &Value,
    query: DefinitionQuery,
) -> Result<Value> {
    let id = run["id"].as_str().context("Run identity absent")?;
    let bases = initial(store, run).await?;
    let activity = if let Some(occurrence) = &query.occurrence_id {
        Some(
            run["activities"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|a| a["occurrenceId"] == *occurrence)
                .context("Passage absent from this run")?,
        )
    } else if query.hash.is_none() {
        run["activities"]
            .as_array()
            .into_iter()
            .flatten()
            .rev()
            .find(|a| {
                query
                    .node_path
                    .as_deref()
                    .is_none_or(|path| a["path"] == path)
            })
    } else {
        None
    };
    if let (Some(path), Some(activity)) = (&query.node_path, activity) {
        ensure!(activity["path"] == *path, "Passage belongs to another node");
    }
    let node_path = query
        .node_path
        .as_deref()
        .or_else(|| activity.and_then(|a| a["path"].as_str()))
        .unwrap_or_default();
    let pin = activity.map(|a| &a["flowRevision"]);
    if query.occurrence_id.is_some() && pin.is_none_or(Value::is_null) {
        return Ok(
            json!({"runId":id,"nodePath":node_path,"occurrenceId":query.occurrence_id,"exact":false,
            "diagnostic":{"code":"legacy_definition_origin_missing","message":"Ce passage ne conserve pas sa version exacte de définition."}}),
        );
    }
    let instance = pin
        .and_then(|p| p["instance"].as_str())
        .or_else(|| {
            bases
                .keys()
                .filter(|instance| inside(node_path, instance))
                .max_by_key(|i| i.len())
                .map(String::as_str)
        })
        .or_else(|| bases.contains_key("root").then_some("root"))
        .unwrap_or_default();
    let base = bases.get(instance).context("Flow instance absent")?;
    let hash = query
        .hash
        .as_deref()
        .or_else(|| pin.and_then(|p| p["hash"].as_str()))
        .unwrap_or(&base.hash);
    if let (Some(requested), Some(pin_hash)) = (&query.hash, pin.and_then(|p| p["hash"].as_str())) {
        ensure!(requested == pin_hash, "Passage used another flow revision");
    }
    let definition = if hash == base.hash {
        base.clone()
    } else {
        let key = zf_storage::context_store::hash(format!("{instance}\0{hash}").as_bytes());
        let value = store
            .record(id, "revision-definitions", &key)
            .await?
            .context("Definition revision is not owned by this run instance")?;
        serde_json::from_value::<RevisionDefinition>(value)?
    };
    super::revisions::validate_definition(&definition)?;
    ensure!(definition.hash == hash, "Definition hash mismatch");
    let graph_ref = pin.and_then(|p| p["graphRef"].as_str());
    let runtime = if let Some(reference) = graph_ref {
        let mut runtime: PreparedRuntime = serde_json::from_value(store.resolve(reference).await?)?;
        if let Some(flow) = runtime.flows.get_mut(instance) {
            flow.composition = definition.composition.clone();
            flow.source = definition.source.clone();
            flow.hash = definition.hash.clone();
            flow.exports = zf_flows::flow_contract::validate(&flow.composition)?
                .context("Instance exports absent")?;
        }
        runtime.validate(&RuntimePrimitives)?;
        Some(runtime.summary())
    } else {
        None
    };
    Ok(
        json!({"runId":id,"instance":instance,"nodePath":node_path,"occurrenceId":activity.map(|a|&a["occurrenceId"]),
        "key":definition.key,"hash":definition.hash,"source":definition.source,"composition":definition.composition,
        "flowRevision":pin,"graphRef":graph_ref,"runtime":runtime,"exact":true}),
    )
}

pub async fn revisions(store: &ContentStore, run: &Value) -> Result<Value> {
    let id = run["id"].as_str().context("Run identity absent")?;
    let bases = initial(store, run).await?;
    let mut instances = Vec::new();
    let mut active = Vec::new();
    for record in store.records_of_kind(id, "revision-active").await? {
        active.push(store.resolve(&record.value_ref).await?);
    }
    for (instance, base) in bases {
        let head = store
            .record(
                id,
                "revision-heads",
                &zf_storage::context_store::hash(instance.as_bytes()),
            )
            .await?;
        let published = if let Some(head) = head {
            let definition: RevisionDefinition = serde_json::from_value(
                store
                    .resolve(
                        head["definitionRef"]
                            .as_str()
                            .context("Definition head is incomplete")?,
                    )
                    .await?,
            )?;
            definition.hash
        } else {
            base.hash.clone()
        };
        let scopes:Vec<_>=active.iter().filter(|pin|pin["instance"]==instance).map(|pin|json!({
            "scope":pin["scope"],"threadId":pin["threadId"],"step":pin["step"],"hash":pin["hash"],"graphRef":pin["graphRef"],
            "diagnostic":pin["diagnostic"],"pending":pin["hash"]!=published
        })).collect();
        instances.push(json!({"instance":instance,"key":base.key,"initialHash":base.hash,"publishedHash":published,"scopes":scopes}));
    }
    let graph = store.record(id, "runtime-graph-heads", "current").await?;
    Ok(
        json!({"runId":id,"instances":instances,"publishedGraphRef":graph.as_ref().map(|g|&g["graphRef"])}),
    )
}
