//! Adapt captured context programs into concrete ADK requests, preserving acquisition provenance.
use crate::{agent_capabilities::EffectiveContext, runtime::RunServices};
use adk_core::{Content, Part};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};
use zf_context::{
    context::{
        self, ContextCapability, ContextItem, ContextStrategy, FragmentFormat, FragmentRole,
    },
    resources::{ContextProgram, InputEncoding, ResourceBinding},
};
use zf_core::{
    identity::Scope,
    types::{Diagnostic, TypeRegistry, compatible},
};
use zf_storage::data::DataError;

/// Make an explicit copy. Legacy JSON depended on the binding's message
/// encoding; ambiguous derived expressions require a choice from the author.
pub fn convert_context_v1(
    strategy: &ContextStrategy,
    bindings: Option<&BTreeMap<String, ResourceBinding>>,
    formats: &BTreeMap<String, FragmentFormat>,
) -> Result<ContextStrategy, Vec<Diagnostic>> {
    context::validate_structure(strategy)?;
    if strategy.version != 1 {
        return Err(vec![Diagnostic::new(
            "context_conversion",
            "version",
            "Only a version-one strategy requires this conversion",
        )]);
    }
    fn resources(value: &Value, out: &mut std::collections::BTreeSet<String>) {
        match value {
            Value::Object(fields) => {
                if fields.get("kind").is_some_and(|kind| kind == "literal") {
                    return;
                }
                if fields.get("kind").is_some_and(|kind| kind == "resource")
                    && let Some(name) = fields.get("name").and_then(Value::as_str)
                {
                    out.insert(name.into());
                }
                for value in fields.values() {
                    resources(value, out);
                }
            }
            Value::Array(values) => {
                for value in values {
                    resources(value, out);
                }
            }
            _ => {}
        }
    }
    fn visit(
        blocks: &mut [context::ContextBlock],
        bindings: Option<&BTreeMap<String, ResourceBinding>>,
        formats: &BTreeMap<String, FragmentFormat>,
        used: &mut HashSet<String>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for block in blocks {
            match block {
                context::ContextBlock::Group { items, .. }
                | context::ContextBlock::ForEach { items, .. } => {
                    visit(items, bindings, formats, used, diagnostics)
                }
                context::ContextBlock::If {
                    then, otherwise, ..
                } => {
                    visit(then, bindings, formats, used, diagnostics);
                    visit(otherwise, bindings, formats, used, diagnostics);
                }
                context::ContextBlock::Emit {
                    id, format, value, ..
                } if *format == FragmentFormat::Json => {
                    if let Some(choice) = formats.get(id) {
                        used.insert(id.clone());
                        if matches!(choice, FragmentFormat::Json | FragmentFormat::AdkMessages) {
                            *format = *choice;
                        } else {
                            diagnostics.push(Diagnostic::new(
                                "context_conversion",
                                format!("program.{id}"),
                                "Choose JSON data or ADK messages explicitly",
                            ));
                        }
                        continue;
                    }
                    let mut names = std::collections::BTreeSet::new();
                    resources(
                        &serde_json::to_value(&*value).expect("Context expression is serializable"),
                        &mut names,
                    );
                    let encoded = |name: &str| {
                        matches!(
                            bindings.and_then(|b| b.get(name)),
                            Some(ResourceBinding::State {
                                encoding: Some(InputEncoding::AdkMessages),
                                ..
                            })
                        )
                    };
                    if let context::ContextExpr::Resource { name } = value
                        && encoded(name)
                    {
                        *format = FragmentFormat::AdkMessages;
                        continue;
                    }
                    if names
                        .iter()
                        .any(|name| bindings.and_then(|b| b.get(name)).is_none() || encoded(name))
                    {
                        diagnostics.push(Diagnostic::new("context_conversion",format!("program.{id}"),"Choose JSON data or ADK messages for this derived/unknown resource; conversion cannot infer its representation"));
                    }
                }
                _ => {}
            }
        }
    }
    let mut copy = strategy.clone();
    let mut diagnostics = Vec::new();
    let mut used = HashSet::new();
    visit(
        &mut copy.program,
        bindings,
        formats,
        &mut used,
        &mut diagnostics,
    );
    for id in formats.keys().filter(|id| !used.contains(*id)) {
        diagnostics.push(Diagnostic::new(
            "context_conversion",
            format!("formats.{id}"),
            "A representation choice must target an existing JSON fragment",
        ));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    copy.version = 2;
    copy.id = format!("{}-v2", copy.id.chars().take(157).collect::<String>());
    copy.name = format!("{} (v2)", copy.name);
    context::validate_structure(&copy)?;
    Ok(copy)
}

pub struct PreparedInference {
    pub snapshot: EffectiveContext,
    pub contents: Vec<Content>,
    pub tools: HashMap<String, Value>,
    pub needs: Vec<context::ResourceNeed>,
    pub wait: Option<Value>,
}

pub fn program(value: &Value) -> Result<ContextProgram> {
    zf_context::frozen_context::validate_frozen(value)?;
    let mut program: ContextProgram = serde_json::from_value(value.clone())?;
    program.types = context::resolved_types(&program.strategy, &program.types)
        .map_err(|errors| anyhow::anyhow!("{errors:?}"))?;
    for (name, binding) in &program.bindings {
        ensure!(
            program.strategy.requirements.contains_key(name),
            "Unknown resource binding: {name}"
        );
        match binding {
            ResourceBinding::State { field, pointer, .. } => {
                ensure!(!field.is_empty(), "Empty state field for {name}");
                ensure!(
                    pointer
                        .as_ref()
                        .is_none_or(|p| p.is_empty() || p.starts_with('/')),
                    "Invalid JSON pointer for {name}"
                );
            }
            ResourceBinding::Conversation {
                history_field,
                input_field,
            } => {
                ensure!(
                    !history_field.is_empty() && !input_field.is_empty(),
                    "Conversation fields must not be empty"
                );
            }
            ResourceBinding::Attachments { slot } => {
                ensure!(
                    ["instructions", "skills", "files"].contains(&slot.as_str()),
                    "Unknown attachment category: {slot}"
                );
            }
            ResourceBinding::Attachment { item_id, .. } => {
                ensure!(!item_id.is_empty(), "Empty attachment identity for {name}")
            }
            ResourceBinding::Entity { alias, .. } => {
                ensure!(!alias.is_empty(), "Empty entity alias for {name}")
            }
            ResourceBinding::Produced { producer } => producer.validate(&program)?,
            ResourceBinding::Reader { reader, input } => {
                ensure!(!reader.is_empty(), "Reader identity absent");
                input.validate()?;
            }
        }
    }
    if let Some(window) = &program.window {
        window.validate()?;
    }
    Ok(program)
}

/// Static capability validation is separate from adapter availability, which is
/// checked against the concrete bridge runtime at the invocation boundary.
pub fn validate_grants(config: &Value, program: &ContextProgram) -> Result<()> {
    let builtin = crate::agent_capabilities::tools(config)?;
    let window_tools = zf_context::window::declarations(config)?;
    let grants: Vec<ContextCapability> =
        serde_json::from_value(config.get("capabilityGrants").cloned().unwrap_or(json!([])))?;
    let mut names = HashSet::new();
    for grant in &grants {
        ensure!(
            names.insert(&grant.id),
            "Duplicate capability grant: {}",
            grant.id
        );
    }
    for requested in &program.strategy.capabilities {
        if let Some(adapter) = zf_context::window::capability(&requested.id) {
            ensure!(
                window_tools.iter().any(|tool| tool["name"] == requested.id),
                "No alias permission grants this window capability: {}",
                requested.id
            );
            ensure!(
                compatible(&requested.input, &adapter.input, &program.types)
                    && compatible(&adapter.output, &requested.output, &program.types),
                "Window capability contract differs from its adapter: {}",
                requested.id
            );
        }
        if let Some(grant) = grants.iter().find(|grant| grant.id == requested.id) {
            ensure!(
                compatible(&requested.input, &grant.input, &program.types)
                    && compatible(&grant.output, &requested.output, &program.types),
                "Incompatible capability contract: {}",
                requested.id
            );
        } else {
            ensure!(
                builtin.contains(&requested.id)
                    || window_tools.iter().any(|tool| tool["name"] == requested.id),
                "Capability not granted to this agent: {}",
                requested.id
            );
        }
    }
    Ok(())
}

struct Acquired {
    value: Arc<Value>,
    provenance: Value,
}

async fn acquire(
    binding: &ResourceBinding,
    config: &Value,
    services: &RunServices,
    path: &str,
    state: &HashMap<String, Value>,
    expected: &zf_core::types::DataType,
    types: &TypeRegistry,
) -> Result<Option<Acquired>> {
    match binding {
        ResourceBinding::State { field, pointer, .. } => {
            let value = state.get(field).and_then(|value| match pointer { Some(pointer) => value.pointer(pointer), None => Some(value) });
            Ok(value.map(|value| Acquired {value:Arc::new(value.clone()),provenance:json!({"kind":"state","field":field,"pointer":pointer})}))
        }
        ResourceBinding::Conversation { history_field, input_field } => {
            let mut history: Vec<Content> = state.get(history_field).cloned().map(serde_json::from_value).transpose()?.unwrap_or_default();
            let model = path.rsplit('/').next().unwrap_or(path);
            let marker = state.get(&format!("__zedflow:input:{input_field}")).cloned().unwrap_or(Value::Null);
            let fresh = !marker.is_null() && state.get(&format!("__zedflow:model-input:{model}")) != Some(&marker);
            let after_tool = history.last().is_some_and(|content|content.parts.iter().any(|part| matches!(part,Part::FunctionResponse {..})));
            if (fresh || !after_tool) && let Some(input) = state.get(input_field) {
                history.push(Content::new("user").with_text(input.as_str().map(str::to_owned).unwrap_or_else(||input.to_string())));
            }
            Ok(Some(Acquired { value: Arc::new(serde_json::to_value(history)?), provenance: json!({"kind":"conversation","historyField":history_field,"inputField":input_field,"modelNodePath":path,"inputMarker":marker}) }))
        }
        ResourceBinding::Attachments { slot } => {
            let mut captured = crate::agent_capabilities::acquire_attachments(config, services, path, state, slot)?;
            Ok(Some(Acquired { value: Arc::new(captured["value"].take()), provenance: json!({"kind":"attachments","slot":slot,"sources":captured["sources"],"skillCatalog":captured["skillCatalog"]}) }))
        }
        ResourceBinding::Attachment { item_id, skill_name } => {
            Ok(crate::agent_capabilities::acquire_attachment(config, services, path, state, item_id, skill_name.as_deref())?.map(|mut captured| Acquired {value:Arc::new(captured["value"].take()),provenance:json!({"kind":"attachment","itemId":item_id,"skillName":skill_name,"sources":captured["sources"]})}))
        }
        ResourceBinding::Entity { scope, alias, revision } => {
            let instance=path.rsplit_once('/').map_or("root",|(instance,_)|instance);
            ensure!(*scope==Scope::Flow(instance.into()),"Entity bindings must use this flow's own aliases: {instance}");
            let Some(registry) = services.data_registry() else { return Ok(None); };
            let snapshot = match revision {
                Some(revision) => registry.revision(scope, alias, revision).await,
                None => registry.snapshot(scope, alias).await,
            };
            match snapshot {
                Ok(snapshot) => Ok(Some(Acquired {value:snapshot.value,provenance:json!({"kind":"entity","universe":registry.universe(),"scope":scope,"alias":alias,"entityId":snapshot.entity_id,"revision":snapshot.revision,"contentRef":snapshot.content_ref})})),
                Err(DataError::NotFound) => Ok(None),
                Err(error) => Err(error.into()),
            }
        }
        ResourceBinding::Produced { .. } => Ok(None),
        ResourceBinding::Reader{reader,input}=>{
            let Some(input_value)=input.capture(state)? else{return Ok(None)};
            let result = services.resource_reads().read(
                services.resource_readers()?.as_ref(), reader, &input_value, expected, types,
            ).await?;
            Ok(result.map(|r|Acquired{value:r.value,provenance:json!({"binding":input,"read":r.provenance})}))
        }
    }
}

pub async fn prepare(
    config: &Value,
    program: &ContextProgram,
    services: &RunServices,
    path: &str,
    state: &HashMap<String, Value>,
    provider: &str,
) -> Result<PreparedInference> {
    prepare_inner(config, program, services, path, state, provider, None).await
}

pub async fn prepare_at(
    config: &Value,
    program: &ContextProgram,
    services: &RunServices,
    path: &str,
    ctx: &adk_graph::NodeContext,
    provider: &str,
) -> Result<PreparedInference> {
    prepare_inner(
        config,
        program,
        services,
        path,
        &ctx.state,
        provider,
        Some(ctx),
    )
    .await
}

async fn prepare_inner(
    config: &Value,
    program: &ContextProgram,
    services: &RunServices,
    path: &str,
    state: &HashMap<String, Value>,
    provider: &str,
    ctx: Option<&adk_graph::NodeContext>,
) -> Result<PreparedInference> {
    validate_grants(config, program)?;
    let invocation_id = uuid::Uuid::new_v4().to_string();
    let preparation_node = config["contextNode"].as_str().map(|context| {
        path.rsplit_once('/').map_or_else(
            || context.to_owned(),
            |(scope, _)| format!("{scope}/{context}"),
        )
    });
    let preparation_path = preparation_node.as_deref().unwrap_or(path);
    if let Some(dynamic) = services.dynamic_capabilities() {
        dynamic.capture(path, &invocation_id, state).await?;
    }
    let mut available = crate::operations::tool_declarations();
    for tool in zf_context::window::declarations(config)? {
        available.insert(
            tool["name"]
                .as_str()
                .context("Missing window capability name")?
                .into(),
            tool,
        );
    }
    if let Some(dynamic) = services.dynamic_capabilities() {
        for declaration in dynamic.tools(path) {
            let name = declaration["name"]
                .as_str()
                .context("Dynamic capability name is missing")?
                .to_owned();
            ensure!(
                !available.contains_key(&name),
                "Dynamic capability shadows an existing tool: {name}"
            );
            ensure!(
                declaration["parameters"].is_object(),
                "Capability schema is absent: {name}"
            );
            available.insert(name, declaration);
        }
    }
    let mut tools = HashMap::new();
    for requested in &program.strategy.capabilities {
        let schema = available
            .get(&requested.id)
            .with_context(|| format!("No model adapter for capability: {}", requested.id))?;
        tools.insert(requested.id.clone(), schema.clone());
    }
    let mut resources = BTreeMap::new();
    let mut provenance = BTreeMap::new();
    let mut failures = BTreeMap::new();
    for (name, binding) in &program.bindings {
        if matches!(binding, ResourceBinding::Reader { .. }) {
            continue;
        }
        match acquire(
            binding,
            config,
            services,
            path,
            state,
            &program.strategy.requirements[name],
            &program.types,
        )
        .await
        {
            Ok(Some(acquired)) => {
                resources.insert(name.clone(), acquired.value);
                provenance.insert(name.clone(), acquired.provenance);
            }
            Ok(None) => {}
            Err(error) => {
                failures.insert(name.clone(), format!("{error:#}"));
            }
        }
    }
    let mut read_attempts = HashSet::new();
    let mut producer_reads = Vec::new();
    let prepared_resources = loop {
        let current = context::evaluate_with_library(
            &program.strategy,
            &resources,
            &program.types,
            &program.library,
        );
        let mut acquired = false;
        for name in current
            .reads
            .into_iter()
            .chain(std::mem::take(&mut producer_reads))
        {
            let Some(binding @ ResourceBinding::Reader { .. }) = program.bindings.get(&name) else {
                continue;
            };
            if !read_attempts.insert(name.clone()) {
                continue;
            }
            acquired = true;
            match acquire(
                binding,
                config,
                services,
                path,
                state,
                &program.strategy.requirements[&name],
                &program.types,
            )
            .await
            {
                Ok(Some(value)) => {
                    resources.insert(name.clone(), value.value);
                    provenance.insert(name, value.provenance);
                }
                Ok(None) => {}
                Err(error) => {
                    failures.insert(name, format!("{error:#}"));
                }
            }
        }
        if acquired {
            continue;
        }
        let prepared = crate::resources::producers::prepare(
            program,
            services,
            preparation_path,
            state,
            ctx,
            &mut resources,
            &mut provenance,
        )
        .await?;
        // A producer may enable another reader branch. Each reader is acquired
        // at most once for this invocation; durable producers retain their own
        // exact identity and are never replayed by this reevaluation.
        if prepared.wait.is_none()
            && prepared.evaluation.reads.iter().any(|name| {
                !read_attempts.contains(name)
                    && matches!(
                        program.bindings.get(name),
                        Some(ResourceBinding::Reader { .. })
                    )
            })
        {
            producer_reads.clone_from(&prepared.evaluation.reads);
            continue;
        }
        break prepared;
    };
    let mut evaluation = prepared_resources.evaluation;
    let mut wait = prepared_resources.wait;
    for name in &evaluation.reads {
        if let Some(error) = failures.get(name) {
            evaluation.diagnostics.push(Diagnostic::new(
                "resource_acquisition",
                format!("bindings.{name}"),
                error,
            ));
        }
    }
    if !evaluation.diagnostics.is_empty() {
        evaluation.complete = false;
    }
    let origin = crate::runtime::current_origin()
        .unwrap_or_else(|| json!({"nodePath":path,"occurrenceId":invocation_id}));
    let captured: Vec<_> = resources
        .iter()
        .filter(|(name, _)| evaluation.reads.contains(name))
        .map(|(name, value)| json!({"name":name,"value":value,"provenance":provenance[name]}))
        .collect();
    let mut snapshot = EffectiveContext {
        invocation_id,
        agent_path: path.into(),
        origin,
        tools: program
            .strategy
            .capabilities
            .iter()
            .map(|c| c.id.clone())
            .collect(),
        system: String::new(),
        files: String::new(),
        resources: captured,
        skill_catalog: provenance
            .values()
            .filter_map(|entry| entry["skillCatalog"].as_array())
            .flatten()
            .cloned()
            .collect(),
        prepared: Some(
            json!({"version":1,"program":program,"flowRevision":crate::revisions::current_revision(),"evaluation":evaluation,"resourceStatus":prepared_resources.statuses,"windowGrants":zf_context::window::grants(config)?,"adapter":{"kind":"llm","provider":provider}}),
        ),
    };
    let mut contents = Vec::new();
    if evaluation.complete {
        if let (Some(window), Some(ctx)) = (&program.window, ctx) {
            let prepared = crate::resources::window_preparation::prepare(
                window,
                program,
                services,
                preparation_path,
                crate::resources::window_preparation::PreparationInputs {
                    ctx,
                    evaluation: &evaluation,
                    resources: &resources,
                    provenance: &provenance,
                },
            )
            .await?;
            snapshot
                .prepared
                .as_mut()
                .context("Prepared snapshot absent")?["window"] = prepared.manifest;
            wait = prepared.wait;
            if let Some(items) = prepared.items {
                evaluation.items = items;
            }
        } else {
            ensure!(
                program.window.is_none(),
                "Preparing a runtime window requires an actual ADK NodeContext"
            );
        }
        if wait.is_some() {
            crate::agent_capabilities::persist_snapshot(services, &snapshot).await?;
            return Ok(PreparedInference {
                snapshot,
                contents,
                tools,
                needs: evaluation.needs,
                wait,
            });
        }
        // Preserve the actual post-intervention projection separately from the
        // initial evaluation trace. Both share their immutable CAS contents.
        snapshot
            .prepared
            .as_mut()
            .context("Prepared snapshot absent")?["consumedItems"] = json!(evaluation.items);
        match adapt(
            &evaluation.items,
            program,
            &resources,
            MediaSource::Runtime(services),
            provider,
        )
        .await
        {
            Ok(prepared) => contents = prepared,
            Err(error) => {
                if let Some(prepared) = &mut snapshot.prepared {
                    prepared["adapter"]["error"] = json!(error.to_string());
                }
                crate::agent_capabilities::persist_snapshot(services, &snapshot).await?;
                return Err(error);
            }
        }
        snapshot.system = contents
            .iter()
            .filter(|content| content.role == "system")
            .flat_map(|content| &content.parts)
            .filter_map(|part| {
                if let Part::Text { text } = part {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    crate::agent_capabilities::persist_snapshot(services, &snapshot).await?;
    ensure!(
        evaluation.diagnostics.is_empty(),
        "Invalid prepared context: {}",
        serde_json::to_string(&evaluation.diagnostics)?
    );
    Ok(PreparedInference {
        snapshot,
        contents,
        tools,
        needs: evaluation.needs,
        wait,
    })
}

/// Trial media are supplied explicitly; the authoring adapter never acquires
/// bytes from a workspace, a URL, or a producer.
pub(crate) enum MediaSource<'a> {
    Runtime(&'a RunServices),
    Trial(&'a BTreeMap<String, Value>),
}

pub(crate) async fn adapt(
    items: &[ContextItem],
    program: &ContextProgram,
    resources: &BTreeMap<String, Arc<Value>>,
    media: MediaSource<'_>,
    provider: &str,
) -> Result<Vec<Content>> {
    // Iterative traversal preserves group/fragment order without retaining a
    // lock or recursively boxing futures for deeply nested context programs.
    let mut stack: Vec<_> = items.iter().rev().collect();
    let mut contents = Vec::new();
    while let Some(item) = stack.pop() {
        match item {
            ContextItem::Group { items, .. } => stack.extend(items.iter().rev()),
            ContextItem::Fragment {
                id,
                role,
                format,
                value,
                sources,
            } => {
                let mut serialized = serde_json::to_value(value)?;
                let adk_messages = sources.iter().find(|name| {
                    program.strategy.version == 1
                        && *format == FragmentFormat::Json
                        && matches!(
                            program.bindings.get(*name),
                            Some(ResourceBinding::State {
                                encoding: Some(InputEncoding::AdkMessages),
                                ..
                            })
                        )
                });
                if *format == FragmentFormat::AdkMessages || adk_messages.is_some() {
                    ensure!(
                        *role == FragmentRole::Data
                            && adk_messages.is_none_or(|name| resources.contains_key(name)),
                        "adkMessages requires an explicitly selected data fragment: {id}"
                    );
                    let messages: Vec<Content> = serde_json::from_value(serialized)
                        .context("Invalid explicitly selected ADK messages")?;
                    ensure!(
                        messages.iter().flat_map(|m| &m.parts).all(|part| matches!(
                            part,
                            Part::Text { .. }
                                | Part::Thinking { .. }
                                | Part::FunctionCall { .. }
                                | Part::FunctionResponse { .. }
                        )),
                        "Binary history must be supplied as explicit media references"
                    );
                    contents.extend(messages);
                    continue;
                }
                let role = if *role == FragmentRole::Instruction {
                    "system"
                } else {
                    "user"
                };
                let content = match format {
                    FragmentFormat::Text => Content::new(role).with_text(
                        serialized
                            .as_str()
                            .context("Text fragment must remain text")?,
                    ),
                    FragmentFormat::Json => {
                        Content::new(role).with_text(serde_json::to_string(&serialized)?)
                    }
                    FragmentFormat::AdkMessages => {
                        unreachable!("Messages handled before scalar representations")
                    }
                    FragmentFormat::Media => {
                        let media_type = serialized["mediaType"]
                            .as_str()
                            .context("Media type missing")?
                            .to_owned();
                        let supported = match provider {
                            "codex" => matches!(
                                media_type.as_str(),
                                "image/png" | "image/jpeg" | "image/webp" | "image/gif"
                            ),
                            "gemini" | "fixture" => {
                                media_type.starts_with("image/")
                                    || media_type.starts_with("audio/")
                                    || media_type.starts_with("video/")
                                    || media_type == "application/pdf"
                            }
                            _ => false,
                        };
                        ensure!(
                            supported,
                            "Unsupported modality for {provider}: {media_type} (fragment {id})"
                        );
                        let reference = serialized["contentRef"].take();
                        let reference = reference
                            .as_str()
                            .context("Media content reference missing")?;
                        let payload = match &media {
                            MediaSource::Runtime(services) => services
                                .content_store()
                                .context("Media content store unavailable")?
                                .resolve_with_limit(reference, 20 * 1024 * 1024)
                                .await?,
                            MediaSource::Trial(values) => values.get(reference)
                                .with_context(|| format!("Octets d’essai absents pour le média {reference} (fragment {id})"))?.clone(),
                        };
                        let bytes = zf_storage::content_store::decode_full_output(&payload)?;
                        ensure!(
                            bytes.len() <= 10 * 1024 * 1024,
                            "Media exceeds ADK inline limit of 10 MiB"
                        );
                        Content::new(role).with_inline_data(media_type, bytes)
                    }
                };
                contents.push(content);
            }
        }
    }
    // Selection and projection are explicit program operations. Validate the
    // final message sequence, so split fragments can retain a complete tool
    // exchange while omitted calls/results and repeated identities are refused.
    validate_messages(&contents)?;
    Ok(contents)
}

fn validate_messages(messages: &[Content]) -> Result<()> {
    let mut pending = HashMap::new();
    let mut seen = HashSet::new();
    for content in messages {
        ensure!(
            matches!(
                content.role.as_str(),
                "user" | "model" | "assistant" | "system" | "developer"
            ),
            "Unsupported ADK message role"
        );
        for part in &content.parts {
            match part {
                Part::FunctionCall { name, id, args, .. } => {
                    let id = id
                        .as_deref()
                        .context("Tool call identity missing from history")?;
                    ensure!(
                        args.is_object() && seen.insert(id),
                        "Invalid or repeated tool call identity in history: {id}"
                    );
                    pending.insert(id, name);
                }
                Part::FunctionResponse {
                    function_response,
                    id,
                    ..
                } => {
                    let id = id
                        .as_deref()
                        .context("Tool result identity missing from history")?;
                    ensure!(
                        pending
                            .remove(id)
                            .is_some_and(|name| name == &function_response.name),
                        "Tool result without its matching call in history: {id}"
                    );
                }
                Part::Text { .. } | Part::Thinking { .. } => {}
                // Binary parts here came only from the explicit media adapter;
                // encoded histories reject them before joining the sequence.
                _ => {}
            }
        }
    }
    ensure!(
        pending.is_empty(),
        "Selected history has pending tool calls without results"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zf_context::{
        context::{
            ContextBlock as B, ContextExpr as E, ContextStrategy, FragmentFormat as F,
            FragmentRole as R,
        },
        context_source::{generate, parse},
    };
    use zf_core::types::DataType as T;
    #[test]
    fn conversion_is_a_copy_preserves_legacy_source_and_demands_explicit_ambiguous_formats() {
        let legacy = ContextStrategy::new("legacy", "Legacy")
            .require(
                "history",
                T::List {
                    item: Box::new(T::Record {
                        fields: BTreeMap::new(),
                    }),
                },
            )
            .with_program(vec![B::emit(
                "messages",
                R::Data,
                F::Json,
                E::resource("history"),
            )]);
        let source = generate(&legacy).unwrap();
        assert!(source.starts_with("// @zedflow-context 1\n"));
        assert!(convert_context_v1(&legacy, None, &BTreeMap::new()).is_err());
        let bindings: BTreeMap<String, ResourceBinding> = serde_json::from_value(
            json!({"history":{"kind":"state","field":"history","encoding":"adkMessages"}}),
        )
        .unwrap();
        let converted = convert_context_v1(&legacy, Some(&bindings), &BTreeMap::new()).unwrap();
        assert_eq!(converted.version, 2);
        assert_eq!(converted.id, "legacy-v2");
        assert!(matches!(
            converted.program[0],
            B::Emit {
                format: F::AdkMessages,
                ..
            }
        ));
        assert_eq!(generate(&legacy).unwrap(), source);
        let mut derived = legacy.clone();
        derived.program = vec![B::emit(
            "count",
            R::Data,
            F::Json,
            E::measure(
                E::resource("history"),
                zf_context::context::MeasureUnit::Items,
            ),
        )];
        assert!(convert_context_v1(&derived, Some(&bindings), &BTreeMap::new()).is_err());
        assert!(
            convert_context_v1(
                &derived,
                Some(&bindings),
                &BTreeMap::from([("count".into(), F::Json)])
            )
            .is_ok()
        );
        assert!(
            convert_context_v1(
                &legacy,
                Some(&bindings),
                &BTreeMap::from([("unknown".into(), F::Json)])
            )
            .is_err()
        );
        let source = generate(&converted).unwrap();
        assert!(source.contains("ContextStrategy::new_v2"));
        assert!(source.starts_with("// @zedflow-context 2\n"));
        assert_eq!(parse(&source).unwrap(), converted);
        assert!(
            parse(&source.replacen("// @zedflow-context 2", "// @zedflow-context 1", 1)).is_err()
        );
    }
}
