//! Resolve declared bridges into routes without storage, scheduling or execution.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use zf_core::types::{DataType, Diagnostic, TypeRegistry, compatible, validate_type};
use zf_flows::composition::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlowInstance {
    pub flow: String,
    pub definition: FlowDefinition,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Route {
    pub bridge: String,
    pub from: Endpoint,
    pub to: Endpoint,
    pub mode: RouteMode,
    pub invocation: InvocationKind,
    pub tool_name: Option<String>,
    pub condition: Option<Value>,
    pub input: DataType,
    pub output: Option<DataType>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedDataBinding {
    pub bridge: String,
    pub from: Endpoint,
    pub to: Endpoint,
    pub permissions: DataPermissions,
    pub data_type: DataType,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedInference {
    pub instance: String,
    pub node: String,
    pub definition: InferenceDefinition,
}

/// A resolved plan, not a scheduler or an executable ADK graph.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeGraph {
    pub entry: Endpoint,
    pub types: TypeRegistry,
    pub bridges: BTreeMap<String, BridgeDefinition>,
    pub instances: BTreeMap<String, FlowInstance>,
    /// Global import alias -> canonical instance. Explicit reuse keeps one instance.
    pub aliases: BTreeMap<String, String>,
    pub routes: BTreeMap<String, Route>,
    pub data_bindings: BTreeMap<String, ResolvedDataBinding>,
    pub inferences: BTreeMap<String, ResolvedInference>,
}

fn diagnostic(errors: &mut Vec<Diagnostic>, code: &str, path: &str, message: impl Into<String>) {
    errors.push(Diagnostic::new(code, path, message));
}
fn name(name: &str, path: &str, errors: &mut Vec<Diagnostic>) {
    if name.is_empty()
        || name.len() > 160
        || name.contains('/')
        || name.chars().any(char::is_control)
        || name.trim() != name
    {
        diagnostic(
            errors,
            "invalid_name",
            path,
            "Names must be nonempty path segments of at most 160 bytes",
        );
    }
}
fn data_type(value: &DataType, types: &TypeRegistry, path: &str, errors: &mut Vec<Diagnostic>) {
    if let Err(found) = validate_type(value, types) {
        errors.extend(found.into_iter().map(|mut error| {
            error.path = format!(
                "{path}{}",
                error.path.strip_prefix('$').unwrap_or(&error.path)
            );
            error
        }));
    }
}
fn contract(value: &PortContract, types: &TypeRegistry, path: &str, errors: &mut Vec<Diagnostic>) {
    data_type(&value.input, types, &format!("{path}.input"), errors);
    if let Some(output) = &value.output {
        data_type(output, types, &format!("{path}.output"), errors);
    }
}
fn permissions(value: DataPermissions, path: &str, errors: &mut Vec<Diagnostic>) {
    if !value.read && !value.write {
        diagnostic(
            errors,
            "empty_permissions",
            path,
            "At least one data permission is required",
        );
    } else if value.write && !value.read {
        diagnostic(
            errors,
            "permissions_unsupported",
            path,
            "Write-only access is unsupported; explicitly grant read and write access or read access",
        );
    }
}
fn validate_flow(
    id: &str,
    flow: &FlowDefinition,
    types: &TypeRegistry,
    errors: &mut Vec<Diagnostic>,
) {
    let path = format!("flows.{id}");
    if flow.entries.is_empty() {
        diagnostic(
            errors,
            "missing_entry",
            &path,
            "A flow must declare at least one public entry",
        );
    }
    for (id, value) in &flow.entries {
        let path = format!("{path}.entries.{id}");
        name(id, &path, errors);
        contract(value, types, &path, errors);
    }
    for (id, value) in &flow.branches {
        let path = format!("{path}.branches.{id}");
        name(id, &path, errors);
        contract(&value.contract, types, &path, errors);
        if value.invocations.is_empty() && value.requesters.is_empty() {
            diagnostic(
                errors,
                "missing_invocation",
                &path,
                "A branch must declare how it can be invoked",
            );
        }
        if !value.invocations.is_empty() && !value.requesters.is_empty() {
            diagnostic(
                errors,
                "ambiguous_branch_contract",
                &path,
                "A neutral plug declares requesters; legacy invocation policy cannot be combined with it",
            );
        }
        for requester in &value.requesters {
            name(requester, &format!("{path}.requesters"), errors);
        }
    }
    for (id, value) in &flow.data {
        let path = format!("{path}.data.{id}");
        name(id, &path, errors);
        data_type(&value.data_type, types, &path, errors);
        permissions(value.permissions, &path, errors);
        if flow.requires.contains_key(id) {
            diagnostic(
                errors,
                "ambiguous_data",
                &path,
                "A data name cannot be both exposed and required",
            );
        }
    }
    for (id, value) in &flow.requires {
        let path = format!("{path}.requires.{id}");
        name(id, &path, errors);
        data_type(&value.data_type, types, &path, errors);
        permissions(value.permissions, &path, errors);
    }
    for (id, value) in &flow.inference_nodes {
        let path = format!("{path}.inferenceNodes.{id}");
        name(id, &path, errors);
        if let ModelBinding::Fixed { provider, model } = &value.model
            && (provider.trim().is_empty() || model.trim().is_empty())
        {
            diagnostic(
                errors,
                "missing_model",
                &path,
                "A fixed inference requires provider and model",
            );
        }
        if value
            .context_strategy
            .as_ref()
            .is_some_and(|s| s.trim().is_empty())
        {
            diagnostic(
                errors,
                "invalid_strategy",
                &path,
                "Context strategy must be named when specified",
            );
        }
        for resource in &value.resources {
            if !flow.data.contains_key(resource) && !flow.requires.contains_key(resource) {
                diagnostic(
                    errors,
                    "unknown_resource",
                    &path,
                    format!("Undeclared inference resource: {resource}"),
                );
            }
        }
        for capability in &value.capabilities {
            name(capability, &format!("{path}.capabilities"), errors);
        }
    }
}

fn dependencies(
    id: &str,
    catalog: &CompositionCatalog,
    visiting: &mut BTreeSet<String>,
    active: &mut BTreeSet<String>,
    errors: &mut Vec<Diagnostic>,
) {
    if active.contains(id) {
        return;
    }
    if !visiting.insert(id.into()) {
        diagnostic(
            errors,
            "bridge_cycle",
            &format!("bridges.{id}.requires"),
            "Cyclic bridge dependencies",
        );
        return;
    }
    if visiting.len() > 128 {
        diagnostic(
            errors,
            "composition_depth",
            &format!("bridges.{id}"),
            "Bridge dependency depth exceeds 128",
        );
    } else if let Some(bridge) = catalog.bridges.get(id) {
        name(id, &format!("bridges.{id}"), errors);
        for dependency in &bridge.requires {
            dependencies(dependency, catalog, visiting, active, errors);
        }
        active.insert(id.into());
    } else {
        diagnostic(
            errors,
            "missing_bridge",
            &format!("bridges.{id}"),
            "Required bridge is absent",
        );
    }
    visiting.remove(id);
}

fn can_reuse(owner: &str, target: &str, bridges: &BTreeMap<String, BridgeDefinition>) -> bool {
    if target == "root" {
        return true;
    }
    let Some((scope, _)) = target.split_once('/') else {
        return false;
    };
    let mut pending = vec![owner];
    let mut seen = BTreeSet::new();
    while let Some(next) = pending.pop() {
        if next == scope {
            return true;
        }
        if seen.insert(next)
            && let Some(bridge) = bridges.get(next)
        {
            pending.extend(bridge.requires.iter().map(String::as_str));
        }
    }
    false
}

struct InstanceResolver<'a> {
    catalog: &'a CompositionCatalog,
    imports: BTreeMap<String, &'a FlowImport>,
    aliases: BTreeMap<String, String>,
    instances: BTreeMap<String, FlowInstance>,
    visiting: BTreeSet<String>,
    errors: Vec<Diagnostic>,
}
impl InstanceResolver<'_> {
    fn resolve(&mut self, alias: &str) -> Option<String> {
        if let Some(instance) = self.aliases.get(alias) {
            return Some(instance.clone());
        }
        if !self.visiting.insert(alias.into()) {
            diagnostic(
                &mut self.errors,
                "reuse_cycle",
                alias,
                "Cyclic instance reuse",
            );
            return None;
        }
        if self.visiting.len() > 128 {
            diagnostic(
                &mut self.errors,
                "composition_depth",
                alias,
                "Instance reuse depth exceeds 128",
            );
            self.visiting.remove(alias);
            return None;
        }
        let result = if let Some(import) = self.imports.get(alias).copied() {
            if !self.catalog.flows.contains_key(&import.flow) {
                diagnostic(
                    &mut self.errors,
                    "missing_flow",
                    alias,
                    format!("Flow {} is absent", import.flow),
                );
                None
            } else if let Some(target) = &import.reuse {
                let owner = alias.split_once('/').map(|v| v.0).unwrap_or("");
                if !can_reuse(owner, target, &self.catalog.bridges) {
                    diagnostic(
                        &mut self.errors,
                        "reuse_scope",
                        alias,
                        "Reuse must target this bridge, root, or a declared dependency",
                    );
                    None
                } else if let Some(instance) = self.resolve(target) {
                    if self.instances[&instance].flow != import.flow {
                        diagnostic(
                            &mut self.errors,
                            "reuse_flow_mismatch",
                            alias,
                            "Reused instance has a different flow definition",
                        );
                        None
                    } else {
                        Some(instance)
                    }
                } else {
                    None
                }
            } else {
                self.instances.insert(
                    alias.into(),
                    FlowInstance {
                        flow: import.flow.clone(),
                        definition: self.catalog.flows[&import.flow].clone(),
                    },
                );
                Some(alias.into())
            }
        } else {
            diagnostic(
                &mut self.errors,
                "unknown_alias",
                alias,
                "Instance alias is absent",
            );
            None
        };
        self.visiting.remove(alias);
        if let Some(instance) = &result {
            self.aliases.insert(alias.into(), instance.clone());
        }
        result
    }
}

fn endpoint(
    bridge: &str,
    value: &Endpoint,
    aliases: &BTreeMap<String, String>,
    path: &str,
    errors: &mut Vec<Diagnostic>,
) -> Option<Endpoint> {
    let alias = if value.instance == "root" {
        "root".into()
    } else {
        format!("{bridge}/{}", value.instance)
    };
    if value.instance.contains('/') {
        diagnostic(
            errors,
            "alias_scope",
            path,
            "Use a local import alias; cross-bridge sharing requires explicit reuse",
        );
        return None;
    }
    match aliases.get(&alias) {
        Some(instance) => Some(Endpoint {
            instance: instance.clone(),
            port: value.port.clone(),
        }),
        None => {
            diagnostic(
                errors,
                "unknown_alias",
                path,
                format!("Unknown instance alias: {}", value.instance),
            );
            None
        }
    }
}

/// Deterministic resolution: request order and duplicate selected bridges never create instances.
/// Inactive flow/bridge definitions are not resolved. Type declarations are validated as a registry.
pub fn resolve(
    catalog: &CompositionCatalog,
    request: &ResolveRequest,
) -> Result<RuntimeGraph, Vec<Diagnostic>> {
    let mut errors = Vec::new();
    for id in catalog.types.keys() {
        data_type(
            &DataType::Named { name: id.clone() },
            &catalog.types,
            &format!("types.{id}"),
            &mut errors,
        );
    }
    let mut active = BTreeSet::new();
    for bridge in request.bridges.iter().collect::<BTreeSet<_>>() {
        dependencies(
            bridge,
            catalog,
            &mut BTreeSet::new(),
            &mut active,
            &mut errors,
        );
    }
    let Some(root) = catalog.flows.get(&request.flow) else {
        diagnostic(
            &mut errors,
            "missing_flow",
            "request.flow",
            "Selected root flow is absent",
        );
        return Err(errors);
    };
    if !root.entries.contains_key(&request.entry) {
        diagnostic(
            &mut errors,
            "unknown_entry",
            "request.entry",
            "Selected entry is not exposed by the root flow",
        );
    }
    let mut imports = BTreeMap::new();
    for bridge in &active {
        for (alias, import) in &catalog.bridges[bridge].imports {
            name(
                alias,
                &format!("bridges.{bridge}.imports.{alias}"),
                &mut errors,
            );
            if alias == "root" {
                diagnostic(
                    &mut errors,
                    "reserved_alias",
                    &format!("bridges.{bridge}.imports.root"),
                    "root is reserved for the selected instance",
                );
            }
            imports.insert(format!("{bridge}/{alias}"), import);
        }
    }
    let mut resolver = InstanceResolver {
        catalog,
        imports,
        aliases: BTreeMap::from([("root".into(), "root".into())]),
        instances: BTreeMap::from([(
            "root".into(),
            FlowInstance {
                flow: request.flow.clone(),
                definition: root.clone(),
            },
        )]),
        visiting: BTreeSet::new(),
        errors,
    };
    for alias in resolver.imports.keys().cloned().collect::<Vec<_>>() {
        resolver.resolve(&alias);
    }
    let mut errors = resolver.errors;
    let mut validated = BTreeSet::new();
    for instance in resolver.instances.values() {
        if validated.insert(&instance.flow) {
            validate_flow(
                &instance.flow,
                &instance.definition,
                &catalog.types,
                &mut errors,
            );
        }
    }
    let mut graph = RuntimeGraph {
        entry: Endpoint {
            instance: "root".into(),
            port: request.entry.clone(),
        },
        types: catalog.types.clone(),
        bridges: active
            .iter()
            .map(|id| (id.clone(), catalog.bridges[id].clone()))
            .collect(),
        instances: resolver.instances,
        aliases: resolver.aliases,
        routes: BTreeMap::new(),
        data_bindings: BTreeMap::new(),
        inferences: BTreeMap::new(),
    };
    let mut bound = BTreeSet::new();
    let mut tools = BTreeSet::new();
    for bridge_id in &active {
        let bridge = &catalog.bridges[bridge_id];
        for (id, connection) in &bridge.connections {
            let path = format!("bridges.{bridge_id}.connections.{id}");
            name(id, &path, &mut errors);
            let from = endpoint(
                bridge_id,
                &connection.from,
                &graph.aliases,
                &format!("{path}.from"),
                &mut errors,
            );
            let to = endpoint(
                bridge_id,
                &connection.to,
                &graph.aliases,
                &format!("{path}.to"),
                &mut errors,
            );
            let (Some(from), Some(to)) = (from, to) else {
                continue;
            };
            let Some(source) = graph.instances[&from.instance]
                .definition
                .branches
                .get(&from.port)
            else {
                diagnostic(
                    &mut errors,
                    "unknown_branch",
                    &format!("{path}.from"),
                    "Source is not an exposed branch point",
                );
                continue;
            };
            let Some(target) = graph.instances[&to.instance]
                .definition
                .entries
                .get(&to.port)
            else {
                diagnostic(
                    &mut errors,
                    "unknown_entry",
                    &format!("{path}.to"),
                    "Target is not an exposed entry point",
                );
                continue;
            };
            if source.requesters.is_empty() && !source.invocations.contains(&connection.invocation)
            {
                diagnostic(
                    &mut errors,
                    "invocation_not_allowed",
                    &path,
                    "This branch does not permit the requested invocation kind",
                );
            }
            if !compatible(&source.contract.input, &target.input, &catalog.types) {
                diagnostic(
                    &mut errors,
                    "route_input_type",
                    &path,
                    "Branch arguments are incompatible with the destination input",
                );
            }
            if connection.mode == RouteMode::CallAwait
                && let Some(expected) = &source.contract.output
                && !target
                    .output
                    .as_ref()
                    .is_some_and(|output| compatible(output, expected, &catalog.types))
            {
                diagnostic(
                    &mut errors,
                    "route_output_type",
                    &path,
                    "Destination result does not satisfy the awaited output contract",
                );
            }
            match connection.invocation {
                InvocationKind::Tool => match &connection.tool_name {
                    Some(tool) if !tool.trim().is_empty() => {
                        name(tool, &format!("{path}.toolName"), &mut errors);
                        if !tools.insert((from.instance.clone(), tool.clone())) {
                            diagnostic(
                                &mut errors,
                                "duplicate_tool",
                                &path,
                                "Two routes expose the same tool name on one instance",
                            );
                        }
                    }
                    _ => diagnostic(
                        &mut errors,
                        "missing_tool_name",
                        &path,
                        "Tool routes require an explicit tool name",
                    ),
                },
                _ if connection.tool_name.is_some() => diagnostic(
                    &mut errors,
                    "unexpected_tool_name",
                    &path,
                    "Only tool routes declare toolName",
                ),
                _ => {}
            }
            match (connection.invocation, &connection.condition) {
                (InvocationKind::Condition, Some(predicate)) => {
                    if let Err(error) = zf_flows::node_contracts::parse_predicate(predicate) {
                        diagnostic(&mut errors, "invalid_condition", &path, error.to_string());
                    }
                }
                (InvocationKind::Condition, None) => diagnostic(
                    &mut errors,
                    "missing_condition",
                    &path,
                    "Conditional routes require a typed predicate",
                ),
                (_, Some(_)) => diagnostic(
                    &mut errors,
                    "unexpected_condition",
                    &path,
                    "Only conditional routes declare a predicate",
                ),
                _ => {}
            }
            graph.routes.insert(
                format!("{bridge_id}/{id}"),
                Route {
                    bridge: bridge_id.clone(),
                    from,
                    to,
                    mode: connection.mode,
                    invocation: connection.invocation,
                    tool_name: connection.tool_name.clone(),
                    condition: connection.condition.clone(),
                    input: target.input.clone(),
                    output: target.output.clone(),
                },
            );
        }
        for (id, binding) in &bridge.bindings {
            let path = format!("bridges.{bridge_id}.bindings.{id}");
            name(id, &path, &mut errors);
            permissions(binding.permissions, &path, &mut errors);
            let from = endpoint(
                bridge_id,
                &binding.from,
                &graph.aliases,
                &format!("{path}.from"),
                &mut errors,
            );
            let to = endpoint(
                bridge_id,
                &binding.to,
                &graph.aliases,
                &format!("{path}.to"),
                &mut errors,
            );
            let (Some(from), Some(to)) = (from, to) else {
                continue;
            };
            let Some(source) = graph.instances[&from.instance]
                .definition
                .data
                .get(&from.port)
            else {
                diagnostic(
                    &mut errors,
                    "unknown_exposure",
                    &format!("{path}.from"),
                    "Source dataset is not exposed",
                );
                continue;
            };
            let Some(target) = graph.instances[&to.instance]
                .definition
                .requires
                .get(&to.port)
            else {
                diagnostic(
                    &mut errors,
                    "unknown_requirement",
                    &format!("{path}.to"),
                    "Target dataset requirement is absent",
                );
                continue;
            };
            if !bound.insert((to.instance.clone(), to.port.clone())) {
                diagnostic(
                    &mut errors,
                    "duplicate_binding",
                    &path,
                    "A dataset requirement has more than one binding",
                );
            }
            if !source.permissions.contains(binding.permissions)
                || binding.permissions != target.permissions
            {
                diagnostic(
                    &mut errors,
                    "binding_permissions",
                    &path,
                    "Binding must satisfy exactly the declared requirement within the exposure's grants",
                );
            }
            if (binding.permissions.read
                && !compatible(&source.data_type, &target.data_type, &catalog.types))
                || (binding.permissions.write
                    && !compatible(&target.data_type, &source.data_type, &catalog.types))
            {
                diagnostic(
                    &mut errors,
                    "binding_type",
                    &path,
                    "Dataset types are incompatible for the requested read/write directions",
                );
            }
            graph.data_bindings.insert(
                format!("{bridge_id}/{id}"),
                ResolvedDataBinding {
                    bridge: bridge_id.clone(),
                    from,
                    to,
                    permissions: binding.permissions,
                    data_type: source.data_type.clone(),
                },
            );
        }
    }
    for (instance_id, instance) in &graph.instances {
        for (id, requirement) in &instance.definition.requires {
            if !requirement.optional && !bound.contains(&(instance_id.clone(), id.clone())) {
                diagnostic(
                    &mut errors,
                    "unbound_data",
                    &format!("instances.{instance_id}.requires.{id}"),
                    "Required dataset has no binding",
                );
            }
        }
        for (node, definition) in &instance.definition.inference_nodes {
            graph.inferences.insert(
                format!("{instance_id}/{node}"),
                ResolvedInference {
                    instance: instance_id.clone(),
                    node: node.clone(),
                    definition: definition.clone(),
                },
            );
        }
    }
    if errors.is_empty() {
        Ok(graph)
    } else {
        Err(errors)
    }
}
