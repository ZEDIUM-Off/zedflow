//! Declared flow interfaces and bridge composition definitions.
//! A bridge defines connections; a route is one resolved connection. Nothing executes here.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use zf_core::types::{DataType, TypeRegistry};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompositionCatalog {
    #[serde(default)]
    pub types: TypeRegistry,
    #[serde(default)]
    pub flows: BTreeMap<String, FlowDefinition>,
    #[serde(default)]
    pub bridges: BTreeMap<String, BridgeDefinition>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolveRequest {
    pub flow: String,
    pub entry: String,
    #[serde(default)]
    pub bridges: Vec<String>,
}

/// Contracts belong to public ports, not to an implicitly shared state map.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortContract {
    pub input: DataType,
    #[serde(default)]
    pub output: Option<DataType>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InvocationKind {
    Tool,
    Node,
    Condition,
    Context,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BranchPoint {
    pub contract: PortContract,
    #[serde(default)]
    pub invocations: BTreeSet<InvocationKind>,
    /// Neutral v4 plugs authorize named nodes. Trigger policy belongs to the bridge.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub requesters: BTreeSet<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataPermissions {
    #[serde(default)]
    pub read: bool,
    #[serde(default)]
    pub write: bool,
}
impl DataPermissions {
    /// Whether these permissions include every permission requested by `other`.
    pub fn contains(self, other: Self) -> bool {
        (!other.read || self.read) && (!other.write || self.write)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataExposure {
    pub data_type: DataType,
    pub permissions: DataPermissions,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataRequirement {
    pub data_type: DataType,
    pub permissions: DataPermissions,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ModelBinding {
    Fixed { provider: String, model: String },
    Runtime,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InferenceDefinition {
    pub model: ModelBinding,
    #[serde(default)]
    pub context_strategy: Option<String>,
    /// Names of this flow's exposed or required datasets.
    #[serde(default)]
    pub resources: BTreeSet<String>,
    /// Capability names are metadata; their provider validates them at integration.
    #[serde(default)]
    pub capabilities: BTreeSet<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlowDefinition {
    #[serde(default)]
    pub entries: BTreeMap<String, PortContract>,
    #[serde(default)]
    pub branches: BTreeMap<String, BranchPoint>,
    #[serde(default)]
    pub data: BTreeMap<String, DataExposure>,
    #[serde(default)]
    pub requires: BTreeMap<String, DataRequirement>,
    #[serde(default)]
    pub inference_nodes: BTreeMap<String, InferenceDefinition>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FlowImport {
    pub flow: String,
    /// Explicit global alias: root or bridge/alias in this bridge or a dependency.
    #[serde(default)]
    pub reuse: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Endpoint {
    /// Within a bridge, root or one of its local import aliases.
    pub instance: String,
    pub port: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RouteMode {
    CallAwait,
    Launch,
    Handoff,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Connection {
    pub from: Endpoint,
    pub to: Endpoint,
    pub mode: RouteMode,
    pub invocation: InvocationKind,
    #[serde(default)]
    pub tool_name: Option<String>,
    /// Uses the existing typed predicate grammar; resolution validates, never evaluates it.
    #[serde(default)]
    pub condition: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataBinding {
    pub from: Endpoint,
    pub to: Endpoint,
    pub permissions: DataPermissions,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BridgeDefinition {
    #[serde(default)]
    pub requires: BTreeSet<String>,
    #[serde(default)]
    pub imports: BTreeMap<String, FlowImport>,
    #[serde(default)]
    pub connections: BTreeMap<String, Connection>,
    #[serde(default)]
    pub bindings: BTreeMap<String, DataBinding>,
}

impl Endpoint {
    pub fn new(instance: &str, port: &str) -> Self {
        Self {
            instance: instance.into(),
            port: port.into(),
        }
    }
}
impl DataPermissions {
    pub fn read_only() -> Self {
        Self {
            read: true,
            write: false,
        }
    }
    pub fn read_write() -> Self {
        Self {
            read: true,
            write: true,
        }
    }
}
impl Connection {
    pub fn new(from: Endpoint, to: Endpoint, mode: RouteMode, invocation: InvocationKind) -> Self {
        Self {
            from,
            to,
            mode,
            invocation,
            tool_name: None,
            condition: None,
        }
    }
    pub fn tool(mut self, name: &str) -> Self {
        self.tool_name = Some(name.into());
        self
    }
    pub fn when(mut self, predicate: Value) -> Self {
        self.condition = Some(predicate);
        self
    }
}
impl BridgeDefinition {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn require(mut self, bridge: &str) -> Self {
        self.requires.insert(bridge.into());
        self
    }
    pub fn import(mut self, alias: &str, flow: &str) -> Self {
        self.imports.insert(
            alias.into(),
            FlowImport {
                flow: flow.into(),
                reuse: None,
            },
        );
        self
    }
    pub fn reuse(mut self, alias: &str, flow: &str, instance: &str) -> Self {
        self.imports.insert(
            alias.into(),
            FlowImport {
                flow: flow.into(),
                reuse: Some(instance.into()),
            },
        );
        self
    }
    pub fn connect(mut self, name: &str, connection: Connection) -> Self {
        self.connections.insert(name.into(), connection);
        self
    }
    pub fn bind(
        mut self,
        name: &str,
        from: Endpoint,
        to: Endpoint,
        permissions: DataPermissions,
    ) -> Self {
        self.bindings.insert(
            name.into(),
            DataBinding {
                from,
                to,
                permissions,
            },
        );
        self
    }
}
