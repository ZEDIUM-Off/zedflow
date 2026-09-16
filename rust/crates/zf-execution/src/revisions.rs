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
            package: run
                .get("flowPackage")
                .filter(|v| !v.is_null())
                .map(|v| serde_json::from_value(v.clone()))
                .transpose()?,
            context_selections: Default::default(),
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
