//! Pure producer descriptors, input evaluation and explicit production requests.
//! Execution validates route eligibility/contracts and performs any effects.
use crate::{
    context::{
        self, ContextBlock, ContextEvaluation, ContextExpr, ContextItem, FragmentFormat,
        FragmentRole,
    },
    resources::{ContextProgram, ResourceBinding},
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
use zf_core::types::DataType;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceProducer {
    pub branch: String,
    pub route_id: String,
    pub input: ContextExpr,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_pointer: Option<String>,
}
impl ResourceProducer {
    pub fn validate(&self, program: &ContextProgram) -> Result<()> {
        ensure!(
            !self.branch.is_empty() && !self.route_id.is_empty(),
            "A producer requires an explicit branch and routeId"
        );
        ensure!(
            self.output_pointer
                .as_ref()
                .is_none_or(|p| p.is_empty() || p.starts_with('/')),
            "Invalid producer output pointer"
        );
        context::expression_type(
            &self.input,
            &program.strategy.requirements,
            &program.types,
            &program.library,
        )
        .map_err(|errors| anyhow::anyhow!("Invalid producer input: {errors:?}"))?;
        Ok(())
    }
}

pub struct PreparedResources {
    pub evaluation: ContextEvaluation,
    pub statuses: BTreeMap<String, Value>,
    pub wait: Option<Value>,
}

pub fn input(
    program: &ContextProgram,
    producer: &ResourceProducer,
    resources: &BTreeMap<String, Arc<Value>>,
) -> ContextEvaluation {
    let mut strategy = program.strategy.clone();
    strategy.capabilities.clear();
    strategy.program = vec![ContextBlock::emit(
        "producer-input",
        FragmentRole::Data,
        FragmentFormat::Json,
        producer.input.clone(),
    )];
    context::evaluate_with_library(&strategy, resources, &program.types, &program.library)
}

pub fn selected_output_type(
    producer: &ResourceProducer,
    descriptor: &Value,
    program: &ContextProgram,
) -> Result<DataType> {
    let mut data_type: DataType = serde_json::from_value(descriptor["output"].clone())?;
    if let Some(pointer) = &producer.output_pointer {
        for field in pointer.split('/').skip(1) {
            for _ in 0..64 {
                if let DataType::Named { name } = &data_type {
                    data_type = program
                        .types
                        .get(name)
                        .context("Producer output type alias is missing")?
                        .clone();
                } else {
                    break;
                }
            }
            let field = field.replace("~1", "/").replace("~0", "~");
            data_type = match data_type {
                DataType::Record { mut fields } => fields
                    .remove(&field)
                    .context("Producer output pointer selects an unknown field")?,
                DataType::List { item } if field.parse::<usize>().is_ok() => *item,
                _ => anyhow::bail!("Producer output pointer is incompatible with its route type"),
            };
        }
    }
    Ok(data_type)
}

/// A request is not an authorization or an invocation. The execution domain
/// must resolve the route, check eligibility and persist its durable identity.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductionRequest {
    pub resource: String,
    pub producer: ResourceProducer,
    pub input: Value,
    pub sources: BTreeMap<String, Value>,
}

pub fn request(
    program: &ContextProgram,
    resource: &str,
    resources: &BTreeMap<String, Arc<Value>>,
    provenance: &BTreeMap<String, Value>,
) -> Result<ProductionRequest> {
    let Some(ResourceBinding::Produced { producer }) = program.bindings.get(resource) else {
        anyhow::bail!("Resource has no producer binding: {resource}");
    };
    ensure!(
        program.strategy.requirements.contains_key(resource),
        "Producer resource is not declared: {resource}"
    );
    producer.validate(program)?;
    let evaluated = input(program, producer, resources);
    ensure!(
        evaluated.complete,
        "Producer input is incomplete: {:?}; {:?}",
        evaluated.needs,
        evaluated.diagnostics
    );
    let Some(ContextItem::Fragment { value, .. }) = evaluated.items.first() else {
        anyhow::bail!("Producer input is incomplete");
    };
    let sources = evaluated
        .reads
        .iter()
        .map(|name| {
            Ok((
                name.clone(),
                match provenance.get(name) {
                    Some(source) => source.clone(),
                    None if !resources.contains_key(name) => json!({"absent":true}),
                    None => anyhow::bail!("Captured provenance is missing for resource: {name}"),
                },
            ))
        })
        .collect::<Result<_>>()?;
    Ok(ProductionRequest {
        resource: resource.into(),
        producer: producer.clone(),
        input: serde_json::to_value(value)?,
        sources,
    })
}
