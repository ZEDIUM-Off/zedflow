//! Initial frozen definitions for native revision controllers.
use serde_json::Value;
use zf_flows::schema::Composition;
pub(crate) fn revision_definitions(
    run: &Value,
) -> anyhow::Result<std::collections::BTreeMap<String, zf_runtime::revisions::RevisionDefinition>> {
    use zf_runtime::revisions::RevisionDefinition;
    if let Some(runtime) = run.get("runtimeGraph").filter(|v| !v.is_null()) {
        let runtime: zf_compiler::prepared_model::PreparedRuntime =
            serde_json::from_value(runtime.clone())?;
        runtime.validate(&zf_runtime::materialize::RuntimePrimitives)?;
        return Ok(runtime
            .flows
            .into_iter()
            .map(|(instance, flow)| {
                (
                    instance,
                    RevisionDefinition {
                        key: flow.key,
                        hash: flow.hash,
                        source: flow.source,
                        composition: flow.composition,
                    },
                )
            })
            .collect());
    }
    let composition: Composition = serde_json::from_value(run["composition"].clone())?;
    let source = if let Some(source) = run["flowSource"].as_str() {
        source.into()
    } else {
        zf_flows::flow_format::render(
            &composition,
            &zf_compiler::graph_compiler::GraphValidator::new(
                &zf_runtime::materialize::RuntimePrimitives,
            ),
        )?
    };
    let composition = zf_flows::flow_format::parse(
        &source,
        &zf_compiler::graph_compiler::GraphValidator::new(
            &zf_runtime::materialize::RuntimePrimitives,
        ),
    )?;
    Ok(std::collections::BTreeMap::from([(
        String::new(),
        RevisionDefinition {
            key: run["flowRef"]["key"]
                .as_str()
                .unwrap_or(&composition.id)
                .into(),
            hash: zf_storage::flow_store::hash(source.as_bytes()),
            source,
            composition,
        },
    )]))
}
