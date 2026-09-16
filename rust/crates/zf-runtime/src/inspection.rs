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
        return runtime
            .flows
            .keys()
            .map(|instance| {
                Ok((
                    instance.clone(),
                    RevisionDefinition::from_prepared(&runtime, instance)?,
                ))
            })
            .collect::<anyhow::Result<_>>();
    }
    let package = field(store, run, "flowPackage").await?;
    let composition = field(store, run, "composition").await?;
    let source = field(store, run, "flowSource").await?;
    let source = source
        .as_str()
        .context("Exact historical flow source is unavailable")?
        .to_owned();
    Ok(BTreeMap::from([(
        String::new(),
        RevisionDefinition {
            package: if package.is_null() {
                None
            } else {
                Some(serde_json::from_value(package)?)
            },
            context_selections: Default::default(),
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
    let base_revision = base.revision();
    let pinned_revision = pin.and_then(|p| {
        p["definitionRevision"]
            .as_str()
            .or_else(|| p["hash"].as_str())
    });
    let revision = query
        .hash
        .as_deref()
        .or(pinned_revision)
        .unwrap_or(&base_revision);
    if let (Some(requested), Some(pinned)) = (&query.hash, pinned_revision) {
        ensure!(requested == pinned, "Passage used another flow revision");
    }
    let definition = if let Some(reference) = pin.and_then(|p| p["definitionRef"].as_str()) {
        serde_json::from_value::<RevisionDefinition>(store.resolve(reference).await?)?
    } else if revision == base_revision {
        base.clone()
    } else {
        let key = zf_storage::context_store::hash(format!("{instance}\0{revision}").as_bytes());
        let value = store
            .record(id, "revision-definitions", &key)
            .await?
            .context("Definition revision is not owned by this run instance")?;
        serde_json::from_value::<RevisionDefinition>(value)?
    };
    super::revisions::validate_definition(&definition)?;
    ensure!(
        definition.key == base.key && definition.revision() == revision,
        "Definition revision mismatch"
    );
    let graph_ref = pin.and_then(|p| p["graphRef"].as_str());
    let (runtime, definition_matches_graph) = if let Some(reference) = graph_ref {
        let runtime: PreparedRuntime = serde_json::from_value(store.resolve(reference).await?)?;
        runtime.validate(&RuntimePrimitives)?;
        let captured = RevisionDefinition::from_prepared(&runtime, instance)?;
        // A pending invocation may retain an older definition than this graph's
        // catalogue. Both captures are exact; do not fabricate a mixed graph.
        (
            Some(runtime.summary()),
            Some(captured.revision() == definition.revision()),
        )
    } else {
        (None, None)
    };
    Ok(
        json!({"runId":id,"instance":instance,"nodePath":node_path,"occurrenceId":activity.map(|a|&a["occurrenceId"]),
        "key":definition.key,"hash":definition.hash,"definitionRevision":definition.revision(),"package":definition.package,"source":definition.source,"composition":definition.composition,
        "flowRevision":pin,"graphRef":graph_ref,"runtime":runtime,"definitionMatchesGraph":definition_matches_graph,"exact":true}),
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
            definition
        } else {
            base.clone()
        };
        let published_revision = published.revision();
        let scopes:Vec<_>=active.iter().filter(|pin|pin["instance"]==instance).map(|pin|json!({
            "scope":pin["scope"],"threadId":pin["threadId"],"step":pin["step"],"hash":pin["hash"],"definitionRevision":pin["definitionRevision"],"packageRevision":pin["packageRevision"],"graphRef":pin["graphRef"],
            "diagnostic":pin["diagnostic"],"pending":pin.get("definitionRevision").unwrap_or(&pin["hash"])!=&json!(published_revision)
        })).collect();
        instances.push(json!({"instance":instance,"key":base.key,"initialHash":base.hash,"publishedHash":published.hash,"initialRevision":base.revision(),"publishedRevision":published_revision,"scopes":scopes}));
    }
    let graph = store.record(id, "runtime-graph-heads", "current").await?;
    Ok(
        json!({"runId":id,"instances":instances,"publishedGraphRef":graph.as_ref().map(|g|&g["graphRef"])}),
    )
}
