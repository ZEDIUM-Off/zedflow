//! Typed context preparation. Evaluation reads a fixed resource snapshot and
//! never executes producers, grants tools, mutates input values or retains memory.
pub use super::context_library::{ContextFunction, ContextLibrary, LibraryKind};
use serde::{Deserialize, Serialize, Serializer, ser::SerializeMap};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use zf_core::types::{DataType, Diagnostic, TypeRegistry, validate_type, validate_value};

pub const CONTEXT_VERSION: u32 = 1;
pub const EXPLICIT_CONTEXT_VERSION: u32 = 2;
const MAX_DEPTH: usize = 64;
const MAX_BLOCKS: usize = 4096;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextStrategy {
    pub version: u32,
    pub id: String,
    pub name: String,
    /// Named schemas selected by this strategy, independent of any flow binding.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub types: TypeRegistry,
    #[serde(default)]
    pub requirements: BTreeMap<String, DataType>,
    /// Requested selections only. The enclosing flow/bridge owns actual grants.
    #[serde(default)]
    pub capabilities: Vec<ContextCapability>,
    #[serde(default)]
    pub program: Vec<ContextBlock>,
}

impl ContextStrategy {
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            version: CONTEXT_VERSION,
            id: id.into(),
            name: name.into(),
            types: TypeRegistry::new(),
            requirements: BTreeMap::new(),
            capabilities: vec![],
            program: vec![],
        }
    }
    /// Version two gives JSON and structured messages distinct representations.
    /// Keep `new` stable so previously exported version-one Rust retains meaning.
    pub fn new_v2(id: &str, name: &str) -> Self {
        Self {
            version: EXPLICIT_CONTEXT_VERSION,
            ..Self::new(id, name)
        }
    }
    pub fn require(mut self, name: &str, data_type: DataType) -> Self {
        self.requirements.insert(name.into(), data_type);
        self
    }
    pub fn define_type(mut self, name: &str, data_type: DataType) -> Self {
        self.types.insert(name.into(), data_type);
        self
    }
    pub fn capability(mut self, capability: ContextCapability) -> Self {
        self.capabilities.push(capability);
        self
    }
    pub fn with_program(mut self, program: Vec<ContextBlock>) -> Self {
        self.program = program;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextCapability {
    pub id: String,
    pub input: DataType,
    pub output: DataType,
}
impl ContextCapability {
    pub fn new(id: &str, input: DataType, output: DataType) -> Self {
        Self {
            id: id.into(),
            input,
            output,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FragmentRole {
    Instruction,
    Data,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FragmentFormat {
    Text,
    Json,
    Media,
    AdkMessages,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Comparison {
    Lt,
    Lte,
    Gt,
    Gte,
    Ne,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MeasureUnit {
    Bytes,
    Items,
    Media,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ContextExpr {
    /// Assign a named business identity explicitly, after checking its shape.
    Construct {
        name: String,
        value: Box<ContextExpr>,
    },
    Variable {
        name: String,
    },
    Filter {
        value: Box<ContextExpr>,
        item: String,
        condition: Box<ContextPredicate>,
    },
    Sort {
        value: Box<ContextExpr>,
        item: String,
        key: Box<ContextExpr>,
        descending: bool,
    },
    Take {
        value: Box<ContextExpr>,
        count: usize,
    },
    /// Keep at most `count` Unicode scalar values without splitting UTF-8.
    Truncate {
        value: Box<ContextExpr>,
        count: usize,
    },
    Map {
        value: Box<ContextExpr>,
        item: String,
        body: Box<ContextExpr>,
    },
    GroupBy {
        value: Box<ContextExpr>,
        item: String,
        key: Box<ContextExpr>,
    },
    Dedup {
        value: Box<ContextExpr>,
        item: String,
        key: Box<ContextExpr>,
    },
    Record {
        fields: BTreeMap<String, ContextExpr>,
    },
    List {
        #[serde(rename = "itemType")]
        item_type: DataType,
        items: Vec<ContextExpr>,
    },
    Template {
        template: String,
        values: BTreeMap<String, ContextExpr>,
    },
    ToJson {
        value: Box<ContextExpr>,
    },
    Measure {
        value: Box<ContextExpr>,
        unit: MeasureUnit,
    },
    Call {
        catalog: LibraryKind,
        name: String,
        arguments: BTreeMap<String, ContextExpr>,
    },
    Resource {
        name: String,
    },
    Field {
        value: Box<ContextExpr>,
        field: String,
    },
    Project {
        value: Box<ContextExpr>,
        fields: Vec<String>,
    },
    Literal {
        #[serde(rename = "dataType")]
        data_type: DataType,
        value: Value,
    },
}
impl ContextExpr {
    pub fn list(item_type: DataType, items: Vec<Self>) -> Self {
        Self::List { item_type, items }
    }
    pub fn truncate(value: Self, count: usize) -> Self {
        Self::Truncate {
            value: Box::new(value),
            count,
        }
    }
    pub fn construct(name: &str, value: Self) -> Self {
        Self::Construct {
            name: name.into(),
            value: Box::new(value),
        }
    }
    pub fn variable(name: &str) -> Self {
        Self::Variable { name: name.into() }
    }
    pub fn filter(value: Self, item: &str, condition: ContextPredicate) -> Self {
        Self::Filter {
            value: Box::new(value),
            item: item.into(),
            condition: Box::new(condition),
        }
    }
    pub fn sort(value: Self, item: &str, key: Self, descending: bool) -> Self {
        Self::Sort {
            value: Box::new(value),
            item: item.into(),
            key: Box::new(key),
            descending,
        }
    }
    pub fn take(value: Self, count: usize) -> Self {
        Self::Take {
            value: Box::new(value),
            count,
        }
    }
    pub fn map(value: Self, item: &str, body: Self) -> Self {
        Self::Map {
            value: Box::new(value),
            item: item.into(),
            body: Box::new(body),
        }
    }
    pub fn group_by(value: Self, item: &str, key: Self) -> Self {
        Self::GroupBy {
            value: Box::new(value),
            item: item.into(),
            key: Box::new(key),
        }
    }
    pub fn dedup(value: Self, item: &str, key: Self) -> Self {
        Self::Dedup {
            value: Box::new(value),
            item: item.into(),
            key: Box::new(key),
        }
    }
    pub fn record(fields: BTreeMap<String, Self>) -> Self {
        Self::Record { fields }
    }
    pub fn template(template: &str, values: BTreeMap<String, Self>) -> Self {
        Self::Template {
            template: template.into(),
            values,
        }
    }
    pub fn to_json(value: Self) -> Self {
        Self::ToJson {
            value: Box::new(value),
        }
    }
    pub fn measure(value: Self, unit: MeasureUnit) -> Self {
        Self::Measure {
            value: Box::new(value),
            unit,
        }
    }
    pub fn call(catalog: LibraryKind, name: &str, arguments: BTreeMap<String, Self>) -> Self {
        Self::Call {
            catalog,
            name: name.into(),
            arguments,
        }
    }
    pub fn resource(name: &str) -> Self {
        Self::Resource { name: name.into() }
    }
    pub fn field(value: Self, field: &str) -> Self {
        Self::Field {
            value: Box::new(value),
            field: field.into(),
        }
    }
    pub fn project(value: Self, fields: &[&str]) -> Self {
        Self::Project {
            value: Box::new(value),
            fields: fields.iter().map(|s| (*s).into()).collect(),
        }
    }
    pub fn literal(data_type: DataType, value: Value) -> Self {
        Self::Literal { data_type, value }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ContextPredicate {
    Compare {
        left: ContextExpr,
        operator: Comparison,
        right: ContextExpr,
    },
    Contains {
        value: ContextExpr,
        item: ContextExpr,
    },
    Present {
        value: ContextExpr,
    },
    Eq {
        left: ContextExpr,
        right: ContextExpr,
    },
    And {
        items: Vec<ContextPredicate>,
    },
    Or {
        items: Vec<ContextPredicate>,
    },
    Not {
        item: Box<ContextPredicate>,
    },
}
impl ContextPredicate {
    pub fn compare(left: ContextExpr, operator: Comparison, right: ContextExpr) -> Self {
        Self::Compare {
            left,
            operator,
            right,
        }
    }
    pub fn contains(value: ContextExpr, item: ContextExpr) -> Self {
        Self::Contains { value, item }
    }
    pub fn present(value: ContextExpr) -> Self {
        Self::Present { value }
    }
    pub fn equal(left: ContextExpr, right: ContextExpr) -> Self {
        Self::Eq { left, right }
    }
    pub fn all(items: Vec<Self>) -> Self {
        Self::And { items }
    }
    pub fn any(items: Vec<Self>) -> Self {
        Self::Or { items }
    }
    pub fn negate(item: Self) -> Self {
        Self::Not {
            item: Box::new(item),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ContextBlock {
    ForEach {
        id: String,
        value: ContextExpr,
        item: String,
        items: Vec<ContextBlock>,
    },
    Group {
        id: String,
        label: String,
        items: Vec<ContextBlock>,
    },
    Emit {
        id: String,
        role: FragmentRole,
        format: FragmentFormat,
        value: ContextExpr,
    },
    If {
        id: String,
        condition: ContextPredicate,
        then: Vec<ContextBlock>,
        #[serde(rename = "else")]
        otherwise: Vec<ContextBlock>,
    },
}
impl ContextBlock {
    pub fn for_each(id: &str, value: ContextExpr, item: &str, items: Vec<Self>) -> Self {
        Self::ForEach {
            id: id.into(),
            value,
            item: item.into(),
            items,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Group { id, .. }
            | Self::Emit { id, .. }
            | Self::If { id, .. }
            | Self::ForEach { id, .. } => id,
        }
    }
    pub fn group(id: &str, label: &str, items: Vec<Self>) -> Self {
        Self::Group {
            id: id.into(),
            label: label.into(),
            items,
        }
    }
    pub fn emit(id: &str, role: FragmentRole, format: FragmentFormat, value: ContextExpr) -> Self {
        Self::Emit {
            id: id.into(),
            role,
            format,
            value,
        }
    }
    pub fn branch(
        id: &str,
        condition: ContextPredicate,
        then: Vec<Self>,
        otherwise: Vec<Self>,
    ) -> Self {
        Self::If {
            id: id.into(),
            condition,
            then,
            otherwise,
        }
    }
}

/// Validate the independently editable source; named types may remain unbound.
pub fn validate_structure(strategy: &ContextStrategy) -> Result<(), Vec<Diagnostic>> {
    check(strategy, None, &ContextLibrary::default())
}

/// Validate against the resolved graph's type registry, before any evaluation.
pub fn validate_strategy(
    strategy: &ContextStrategy,
    types: &TypeRegistry,
) -> Result<(), Vec<Diagnostic>> {
    validate_strategy_with_library(strategy, types, &ContextLibrary::default())
}

pub fn validate_strategy_with_library(
    strategy: &ContextStrategy,
    types: &TypeRegistry,
    library: &ContextLibrary,
) -> Result<(), Vec<Diagnostic>> {
    check(strategy, Some(types), library)
}

/// Combine authored schemas and the selected scope without silently changing a
/// named business identity. Identical definitions may be shared by both sides.
pub fn resolved_types(
    strategy: &ContextStrategy,
    external: &TypeRegistry,
) -> Result<TypeRegistry, Vec<Diagnostic>> {
    let mut types = external.clone();
    let mut diagnostics = vec![];
    for (name, data_type) in &strategy.types {
        if types
            .get(name)
            .is_some_and(|existing| existing != data_type)
        {
            diagnostics.push(Diagnostic::new(
                "type_conflict",
                format!("types.{name}"),
                "The strategy and its scope define this named type differently",
            ));
        } else {
            types.insert(name.clone(), data_type.clone());
        }
    }
    if diagnostics.is_empty() {
        Ok(types)
    } else {
        Err(diagnostics)
    }
}

/// Type an authored expression with the same lexical rules as a context block.
/// Used to validate explicit producer inputs against their composed route ports.
pub fn expression_type(
    expression: &ContextExpr,
    requirements: &BTreeMap<String, DataType>,
    types: &TypeRegistry,
    library: &ContextLibrary,
) -> Result<DataType, Vec<Diagnostic>> {
    let strategy = ContextStrategy {
        requirements: requirements.clone(),
        ..ContextStrategy::new("expression", "Expression")
    };
    let mut check = Checker {
        strategy: &strategy,
        types,
        linked: true,
        diagnostics: vec![],
        ids: BTreeSet::new(),
        nodes: 0,
        library,
        variables: BTreeMap::new(),
        functions_done: BTreeSet::new(),
        functions_active: BTreeSet::new(),
        in_function: false,
    };
    let inferred = check.expression(expression, "expression", 0);
    if check.diagnostics.is_empty() {
        inferred.ok_or_else(|| {
            vec![Diagnostic::new(
                "expression_type",
                "expression",
                "Expression has no concrete type",
            )]
        })
    } else {
        Err(check.diagnostics)
    }
}

pub fn validate_library_structure(library: &ContextLibrary) -> Result<(), Vec<Diagnostic>> {
    check(
        &ContextStrategy::new("library", "Context library"),
        None,
        library,
    )
}

pub fn validate_library(
    library: &ContextLibrary,
    types: &TypeRegistry,
) -> Result<(), Vec<Diagnostic>> {
    validate_strategy_with_library(
        &ContextStrategy::new("library", "Context library"),
        types,
        library,
    )
}

fn check(
    strategy: &ContextStrategy,
    types: Option<&TypeRegistry>,
    library: &ContextLibrary,
) -> Result<(), Vec<Diagnostic>> {
    let empty = TypeRegistry::new();
    let merged = resolved_types(strategy, types.unwrap_or(&empty))?;
    let mut check = Checker {
        strategy,
        types: &merged,
        linked: types.is_some(),
        diagnostics: vec![],
        ids: BTreeSet::new(),
        nodes: 0,
        library,
        variables: BTreeMap::new(),
        functions_done: BTreeSet::new(),
        functions_active: BTreeSet::new(),
        in_function: false,
    };
    if ![CONTEXT_VERSION, EXPLICIT_CONTEXT_VERSION].contains(&strategy.version) {
        check.error(
            "context_version",
            "version",
            "Unsupported context strategy version",
        );
    }
    if !valid_id(&strategy.id) {
        check.error(
            "context_id",
            "id",
            "Use 1–160 ASCII letters, digits, hyphens or underscores",
        );
    }
    if strategy.name.trim().is_empty() {
        check.error("context_name", "name", "Strategy name cannot be empty");
    }
    for (name, data_type) in &strategy.types {
        let path = format!("types.{name}");
        if name.trim().is_empty() {
            check.error("type_name", &path, "Named type cannot be empty");
        }
        check.data_type(data_type, &path);
    }
    for (name, data_type) in &strategy.requirements {
        let path = format!("requirements.{name}");
        if name.trim().is_empty() {
            check.error("resource_name", &path, "Resource name cannot be empty");
        }
        check.data_type(data_type, &path);
    }
    let mut capabilities = BTreeSet::new();
    for (index, capability) in strategy.capabilities.iter().enumerate() {
        let path = format!("capabilities[{index}]");
        if capability.id.trim().is_empty() || !capabilities.insert(&capability.id) {
            check.error(
                "capability_id",
                &path,
                "Capability identifiers must be nonempty and unique",
            );
        }
        check.data_type(&capability.input, &format!("{path}.input"));
        check.data_type(&capability.output, &format!("{path}.output"));
    }
    for (kind, entries) in [
        (LibraryKind::Projection, &library.projections),
        (LibraryKind::Subprogram, &library.subprograms),
    ] {
        for name in entries.keys() {
            check.function(kind, name, "library", 0);
        }
    }
    check.blocks(&strategy.program, "program", 0);
    if check.diagnostics.is_empty() {
        Ok(())
    } else {
        Err(check.diagnostics)
    }
}

/// Shared identity rule for authored context artifacts and their catalogue keys.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 160
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

struct Checker<'a> {
    strategy: &'a ContextStrategy,
    types: &'a TypeRegistry,
    linked: bool,
    diagnostics: Vec<Diagnostic>,
    ids: BTreeSet<String>,
    nodes: usize,
    library: &'a ContextLibrary,
    variables: BTreeMap<String, DataType>,
    functions_done: BTreeSet<(LibraryKind, String)>,
    functions_active: BTreeSet<(LibraryKind, String)>,
    in_function: bool,
}
impl Checker<'_> {
    fn error(&mut self, code: &str, path: &str, message: &str) {
        if self.diagnostics.len() < 64 {
            self.diagnostics.push(Diagnostic::new(code, path, message));
        }
    }
    fn append(&mut self, errors: Vec<Diagnostic>, path: &str) {
        for mut error in errors {
            if !self.linked && error.code == "unknown_type" {
                continue;
            }
            error.path = format!(
                "{path}{}",
                error.path.strip_prefix('$').unwrap_or(&error.path)
            );
            if self.diagnostics.len() < 64 {
                self.diagnostics.push(error);
            }
        }
    }
    fn depth(&mut self, depth: usize, path: &str) -> bool {
        self.nodes += 1;
        if depth > MAX_DEPTH || self.nodes > MAX_BLOCKS {
            self.error(
                "context_limit",
                path,
                "Context program exceeds its depth or node limit",
            );
            false
        } else {
            true
        }
    }
    fn data_type(&mut self, value: &DataType, path: &str) {
        if let Err(errors) = validate_type(value, self.types) {
            self.append(errors, path);
        }
        if let DataType::Named { name } = value
            && name.trim().is_empty()
        {
            self.error("type_name", path, "Named type cannot be empty");
        }
    }
    fn resolved(&self, value: DataType) -> Option<DataType> {
        let mut value = value;
        for _ in 0..=MAX_DEPTH {
            if let DataType::Named { name } = &value {
                value = self.types.get(name)?.clone();
            } else {
                return Some(value);
            }
        }
        None
    }
    fn function(
        &mut self,
        catalog: LibraryKind,
        name: &str,
        path: &str,
        depth: usize,
    ) -> Option<DataType> {
        if !self.depth(depth, path) {
            return None;
        }
        let Some(function) = self.library.get(catalog, name).cloned() else {
            if self.linked
                || !self.library.projections.is_empty()
                || !self.library.subprograms.is_empty()
            {
                self.error(
                    "unknown_function",
                    path,
                    &format!("Unknown {catalog:?}: {name}"),
                );
            }
            return None;
        };
        let key = (catalog, name.to_owned());
        if self.functions_active.contains(&key) {
            self.error(
                "function_cycle",
                path,
                &format!("Recursive context function: {name}"),
            );
            return None;
        }
        if self.functions_done.contains(&key) {
            return Some(function.output);
        }
        self.functions_active.insert(key.clone());
        if !valid_id(name) {
            self.error("function_name", path, "Invalid context function name");
        }
        for (name, data_type) in &function.parameters {
            if !valid_id(name) {
                self.error("variable_name", path, "Invalid parameter name");
            }
            self.data_type(data_type, path);
        }
        self.data_type(&function.output, path);
        let variables = std::mem::replace(&mut self.variables, function.parameters.clone());
        let in_function = std::mem::replace(&mut self.in_function, true);
        if let Some(actual) = self.expression(
            &function.body,
            &format!("library.{catalog:?}.{name}.body"),
            depth + 1,
        ) && (self.linked
            || (validate_type(&actual, self.types).is_ok()
                && validate_type(&function.output, self.types).is_ok()))
            && !zf_core::types::compatible(&actual, &function.output, self.types)
        {
            self.error(
                "function_output",
                path,
                "Function body does not match its declared output type",
            );
        }
        self.variables = variables;
        self.in_function = in_function;
        self.functions_active.remove(&key);
        self.functions_done.insert(key);
        Some(function.output)
    }
    fn collection_item(
        &mut self,
        value: &ContextExpr,
        item: &str,
        path: &str,
        depth: usize,
    ) -> Option<DataType> {
        if !valid_id(item) {
            self.error("variable_name", path, "Invalid collection variable name");
        }
        let source = self.expression(value, path, depth + 1)?;
        if let DataType::List { item } = self.resolved(source)? {
            Some(*item)
        } else {
            self.error(
                "collection_type",
                path,
                "Collection operation requires a list",
            );
            None
        }
    }
    fn restore_variable(&mut self, name: &str, old: Option<DataType>) {
        if let Some(old) = old {
            self.variables.insert(name.into(), old);
        } else {
            self.variables.remove(name);
        }
    }
    fn expression(&mut self, value: &ContextExpr, path: &str, depth: usize) -> Option<DataType> {
        if !self.depth(depth, path) {
            return None;
        }
        match value {
            ContextExpr::Construct { name, value } => {
                let target = DataType::Named { name: name.clone() };
                self.data_type(&target, path);
                let actual = self.expression(value, &format!("{path}.value"), depth + 1)?;
                // Only this explicit constructor unwraps the outer nominal
                // identity. Nested nominal fields still require their own mapping.
                if let (Some(actual), Some(expected)) =
                    (self.resolved(actual), self.resolved(target.clone()))
                    && !zf_core::types::compatible(&actual, &expected, self.types)
                {
                    self.error("construct_type", path, "Construction is incompatible with the named type; no implicit field conversion is available");
                }
                Some(target)
            }
            ContextExpr::Variable { name } => {
                let result = self.variables.get(name).cloned();
                if result.is_none() {
                    self.error(
                        "unknown_variable",
                        path,
                        &format!("Variable is not in scope: {name}"),
                    );
                }
                result
            }
            ContextExpr::Filter {
                value,
                item,
                condition,
            } => {
                let ty = self.collection_item(value, item, path, depth)?;
                let old = self.variables.insert(item.clone(), ty.clone());
                self.predicate(condition, &format!("{path}.condition"), depth + 1);
                self.restore_variable(item, old);
                Some(DataType::List { item: Box::new(ty) })
            }
            ContextExpr::Map {
                value: source,
                item,
                body,
            }
            | ContextExpr::Sort {
                value: source,
                item,
                key: body,
                ..
            }
            | ContextExpr::GroupBy {
                value: source,
                item,
                key: body,
            }
            | ContextExpr::Dedup {
                value: source,
                item,
                key: body,
            } => {
                let ty = self.collection_item(source, item, path, depth)?;
                let old = self.variables.insert(item.clone(), ty.clone());
                let result = self.expression(body, &format!("{path}.body"), depth + 1);
                self.restore_variable(item, old);
                let result = result?;
                if matches!(value, ContextExpr::Sort { .. })
                    && !matches!(
                        self.resolved(result.clone()),
                        Some(DataType::Text | DataType::Number)
                    )
                {
                    self.error("sort_type", path, "Sort keys must be text or numbers");
                }
                let output = match value {
                    ContextExpr::Map { .. } => result,
                    ContextExpr::GroupBy { .. } => DataType::Record {
                        fields: BTreeMap::from([
                            ("key".into(), result),
                            ("items".into(), DataType::List { item: Box::new(ty) }),
                        ]),
                    },
                    _ => ty,
                };
                Some(DataType::List {
                    item: Box::new(output),
                })
            }
            ContextExpr::Take { value, .. } => {
                let source = self.expression(value, path, depth + 1)?;
                if !matches!(self.resolved(source.clone()), Some(DataType::List { .. })) {
                    self.error("collection_type", path, "Take requires a list");
                }
                Some(source)
            }
            ContextExpr::Truncate { value, .. } => {
                let source = self.expression(value, path, depth + 1)?;
                if self.resolved(source) != Some(DataType::Text) {
                    self.error("text_type", path, "Truncate requires text");
                }
                Some(DataType::Text)
            }
            ContextExpr::Record { fields } => {
                let mut result = BTreeMap::new();
                for (name, value) in fields {
                    if let Some(ty) = self.expression(value, &format!("{path}.{name}"), depth + 1) {
                        result.insert(name.clone(), ty);
                    }
                }
                Some(DataType::Record { fields: result })
            }
            ContextExpr::List { item_type, items } => {
                self.data_type(item_type, &format!("{path}.itemType"));
                for (index, value) in items.iter().enumerate() {
                    let path = format!("{path}.items[{index}]");
                    if let Some(actual) = self.expression(value, &path, depth + 1)
                        && (self.linked
                            || (validate_type(&actual, self.types).is_ok()
                                && validate_type(item_type, self.types).is_ok()))
                        && !zf_core::types::compatible(&actual, item_type, self.types)
                    {
                        self.error(
                            "list_item_type",
                            &path,
                            "List element is incompatible with its declared item type",
                        );
                    }
                }
                Some(DataType::List {
                    item: Box::new(item_type.clone()),
                })
            }
            ContextExpr::Template { template, values } => {
                match template_parts(template) {
                    Ok(parts) => {
                        let needed: BTreeSet<_> = parts
                            .iter()
                            .filter_map(|p| {
                                if let TemplatePart::Variable(v) = p {
                                    Some(*v)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        if needed != values.keys().map(String::as_str).collect() {
                            self.error(
                                "template_arguments",
                                path,
                                "Template placeholders and arguments must match exactly",
                            );
                        }
                    }
                    Err(message) => self.error("template_syntax", path, message),
                }
                for (name, value) in values {
                    if let Some(ty) = self.expression(value, &format!("{path}.{name}"), depth + 1)
                        && self.resolved(ty) != Some(DataType::Text)
                    {
                        self.error("template_type", path, "Template arguments must be text; use to_json explicitly for other types");
                    }
                }
                Some(DataType::Text)
            }
            ContextExpr::ToJson { value } => {
                self.expression(value, path, depth + 1);
                Some(DataType::Text)
            }
            ContextExpr::Measure { value, unit } => {
                let ty = self.expression(value, path, depth + 1)?;
                let ty = self.resolved(ty)?;
                let accepted = match unit {
                    MeasureUnit::Bytes => true,
                    MeasureUnit::Items => matches!(ty, DataType::List { .. }),
                    MeasureUnit::Media => match ty {
                        DataType::Media { .. } => true,
                        DataType::List { item } => {
                            matches!(self.resolved(*item), Some(DataType::Media { .. }))
                        }
                        _ => false,
                    },
                };
                if !accepted {
                    self.error(
                        "measure_type",
                        path,
                        "Measurement unit is incompatible with its value",
                    );
                }
                Some(DataType::Number)
            }
            ContextExpr::Call {
                catalog,
                name,
                arguments,
            } => {
                let output = self.function(*catalog, name, path, depth + 1);
                let expected = self
                    .library
                    .get(*catalog, name)
                    .map(|f| f.parameters.clone());
                if let Some(expected) = &expected
                    && expected.keys().collect::<Vec<_>>() != arguments.keys().collect::<Vec<_>>()
                {
                    self.error(
                        "function_arguments",
                        path,
                        "Function argument names must match its parameters",
                    );
                }
                for (name, argument) in arguments {
                    let actual =
                        self.expression(argument, &format!("{path}.arguments.{name}"), depth + 1);
                    if let (Some(actual), Some(expected)) =
                        (actual, expected.as_ref().and_then(|p| p.get(name)))
                        && (self.linked
                            || (validate_type(&actual, self.types).is_ok()
                                && validate_type(expected, self.types).is_ok()))
                        && !zf_core::types::compatible(&actual, expected, self.types)
                    {
                        self.error(
                            "function_argument_type",
                            path,
                            "Function argument has an incompatible type",
                        );
                    }
                }
                output
            }
            ContextExpr::Resource { name } => {
                if self.in_function {
                    self.error(
                        "function_resource",
                        path,
                        "Functions receive explicit parameters; ambient resources are unavailable",
                    );
                    return None;
                }
                let data_type = self.strategy.requirements.get(name).cloned();
                if data_type.is_none() {
                    self.error(
                        "undeclared_resource",
                        path,
                        &format!("Resource is not declared: {name}"),
                    );
                }
                data_type
            }
            ContextExpr::Literal { data_type, value } => {
                self.data_type(data_type, path);
                if let Err(errors) = validate_value(data_type, value, self.types) {
                    self.append(errors, path);
                }
                Some(data_type.clone())
            }
            ContextExpr::Field { value, field } => {
                let source = self.expression(value, &format!("{path}.value"), depth + 1)?;
                match self.resolved(source)? {
                    DataType::Record { fields } => {
                        let result = fields.get(field).cloned();
                        if result.is_none() {
                            self.error(
                                "unknown_field",
                                path,
                                &format!("Field is not declared: {field}"),
                            );
                        }
                        result
                    }
                    _ => {
                        self.error("field_type", path, "Field access requires a record");
                        None
                    }
                }
            }
            ContextExpr::Project {
                value,
                fields: selected,
            } => {
                let source = self.expression(value, &format!("{path}.value"), depth + 1)?;
                let DataType::Record { fields } = self.resolved(source)? else {
                    self.error("projection_type", path, "Projection requires a record");
                    return None;
                };
                let mut output = BTreeMap::new();
                if selected.is_empty() {
                    self.error("projection_fields", path, "Select at least one field");
                }
                for field in selected {
                    if let Some(data_type) = fields.get(field) {
                        if output.insert(field.clone(), data_type.clone()).is_some() {
                            self.error(
                                "projection_fields",
                                path,
                                &format!("Duplicate field: {field}"),
                            );
                        }
                    } else {
                        self.error(
                            "unknown_field",
                            path,
                            &format!("Field is not declared: {field}"),
                        );
                    }
                }
                Some(DataType::Record { fields: output })
            }
        }
    }
    fn predicate(&mut self, value: &ContextPredicate, path: &str, depth: usize) {
        if !self.depth(depth, path) {
            return;
        }
        match value {
            ContextPredicate::Compare {
                left,
                operator,
                right,
            } => {
                let left = self.expression(left, &format!("{path}.left"), depth + 1);
                let right = self.expression(right, &format!("{path}.right"), depth + 1);
                if let (Some(left), Some(right)) = (left, right)
                    && (left != right
                        || (*operator != Comparison::Ne
                            && self.resolved(left) != Some(DataType::Number)))
                {
                    self.error("comparison_type", path, "Ordering requires identical numeric types; inequality requires identical types");
                }
            }
            ContextPredicate::Contains { value, item } => {
                let value = self.expression(value, &format!("{path}.value"), depth + 1);
                let item = self.expression(item, &format!("{path}.item"), depth + 1);
                if let (Some(value), Some(item)) = (value.and_then(|v| self.resolved(v)), item) {
                    let accepted = match value {
                        DataType::Text => item == DataType::Text,
                        DataType::List { item: expected } => *expected == item,
                        _ => false,
                    };
                    if !accepted {
                        self.error(
                            "contains_type",
                            path,
                            "Contains requires text/text or a list and its exact item type",
                        );
                    }
                }
            }
            ContextPredicate::Present { value } => {
                self.expression(value, &format!("{path}.value"), depth + 1);
            }
            ContextPredicate::Eq { left, right } => {
                let left = self.expression(left, &format!("{path}.left"), depth + 1);
                let right = self.expression(right, &format!("{path}.right"), depth + 1);
                if let (Some(left), Some(right)) = (left, right)
                    && left != right
                {
                    self.error(
                        "comparison_type",
                        path,
                        "Equality requires identical types; there is no coercion",
                    );
                }
            }
            ContextPredicate::And { items } | ContextPredicate::Or { items } => {
                if items.is_empty() {
                    self.error(
                        "empty_predicate",
                        path,
                        "Boolean groups require at least one condition",
                    );
                }
                for (index, item) in items.iter().enumerate() {
                    self.predicate(item, &format!("{path}.items[{index}]"), depth + 1);
                }
            }
            ContextPredicate::Not { item } => {
                self.predicate(item, &format!("{path}.item"), depth + 1)
            }
        }
    }
    fn blocks(&mut self, blocks: &[ContextBlock], path: &str, depth: usize) {
        for (index, block) in blocks.iter().enumerate() {
            let path = format!("{path}[{index}]");
            if !self.depth(depth, &path) {
                return;
            }
            let id = block.id();
            if id.trim().is_empty() || !self.ids.insert(id.to_owned()) {
                self.error(
                    "block_id",
                    &path,
                    "Block identifiers must be nonempty and unique",
                );
            }
            match block {
                ContextBlock::ForEach {
                    value, item, items, ..
                } => {
                    if let Some(data_type) =
                        self.collection_item(value, item, &format!("{path}.value"), depth)
                    {
                        let previous = self.variables.insert(item.clone(), data_type);
                        self.blocks(items, &format!("{path}.items"), depth + 1);
                        self.restore_variable(item, previous);
                    }
                }
                ContextBlock::Group { label, items, .. } => {
                    if label.trim().is_empty() {
                        self.error("group_label", &path, "Group label cannot be empty");
                    }
                    self.blocks(items, &format!("{path}.items"), depth + 1);
                }
                ContextBlock::Emit {
                    role,
                    format,
                    value,
                    ..
                } => {
                    let data_type = self
                        .expression(value, &format!("{path}.value"), depth + 1)
                        .and_then(|t| self.resolved(t));
                    if *role == FragmentRole::Instruction && *format != FragmentFormat::Text {
                        self.error(
                            "instruction_format",
                            &path,
                            "Instructions must be text fragments",
                        );
                    }
                    if let Some(data_type) = data_type {
                        let compatible = match format {
                            FragmentFormat::Text => data_type == DataType::Text,
                            FragmentFormat::Json => !matches!(data_type, DataType::Media { .. }),
                            FragmentFormat::Media => matches!(data_type, DataType::Media { .. }),
                            FragmentFormat::AdkMessages => match data_type {
                                DataType::List { item } => {
                                    matches!(self.resolved(*item), Some(DataType::Record { .. }))
                                }
                                _ => false,
                            },
                        };
                        if !compatible {
                            self.error(
                                "fragment_type",
                                &path,
                                "Fragment representation does not match its input type",
                            );
                        }
                    }
                }
                ContextBlock::If {
                    condition,
                    then,
                    otherwise,
                    ..
                } => {
                    self.predicate(condition, &format!("{path}.condition"), depth + 1);
                    self.blocks(then, &format!("{path}.then"), depth + 1);
                    self.blocks(otherwise, &format!("{path}.else"), depth + 1);
                }
            }
        }
    }
}

/// Shared values and field views retain the original Arc. Projecting a record
/// allocates only its selected keys/views, not copies of the resource payload.
#[derive(Clone, Debug)]
pub enum ContextValue {
    Shared {
        root: Arc<Value>,
        pointer: String,
    },
    Object {
        fields: BTreeMap<String, ContextValue>,
    },
    Array {
        items: Vec<ContextValue>,
    },
}
impl ContextValue {
    pub fn as_value(&self) -> Option<&Value> {
        match self {
            Self::Shared { root, pointer } => root.pointer(pointer),
            Self::Object { .. } | Self::Array { .. } => None,
        }
    }
    fn field(&self, name: &str) -> Option<Self> {
        match self {
            Self::Object { fields } => fields.get(name).cloned(),
            Self::Array { .. } => None,
            Self::Shared { root, pointer } => {
                self.as_value()?.as_object()?.get(name)?;
                Some(Self::Shared {
                    root: Arc::clone(root),
                    pointer: format!("{pointer}/{}", name.replace('~', "~0").replace('/', "~1")),
                })
            }
        }
    }
    fn equivalent(&self, other: &Self) -> bool {
        if let (Some(left), Some(right)) = (self.array_len(), other.array_len()) {
            return left == right
                && (0..left).all(|i| match (self.index(i), other.index(i)) {
                    (Some(a), Some(b)) => a.equivalent(&b),
                    _ => false,
                });
        }
        match (self.as_value(), other.as_value()) {
            (Some(left), Some(right)) => left == right,
            _ => {
                let keys = |value: &Self| -> Option<Vec<String>> {
                    match value {
                        Self::Object { fields } => Some(fields.keys().cloned().collect()),
                        _ => Some(value.as_value()?.as_object()?.keys().cloned().collect()),
                    }
                };
                let (Some(left), Some(right)) = (keys(self), keys(other)) else {
                    return false;
                };
                left.len() == right.len()
                    && left
                        .iter()
                        .all(|key| match (self.field(key), other.field(key)) {
                            (Some(a), Some(b)) => a.equivalent(&b),
                            _ => false,
                        })
            }
        }
    }
    fn array_len(&self) -> Option<usize> {
        match self {
            Self::Array { items } => Some(items.len()),
            _ => self.as_value()?.as_array().map(Vec::len),
        }
    }
    fn index(&self, index: usize) -> Option<Self> {
        match self {
            Self::Array { items } => items.get(index).cloned(),
            Self::Shared { root, pointer } => {
                self.as_value()?.as_array()?.get(index)?;
                Some(Self::Shared {
                    root: Arc::clone(root),
                    pointer: format!("{pointer}/{index}"),
                })
            }
            _ => None,
        }
    }
    fn owned(value: Value) -> Self {
        Self::Shared {
            root: Arc::new(value),
            pointer: String::new(),
        }
    }
}
impl Serialize for ContextValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Shared { .. } => self
                .as_value()
                .ok_or_else(|| serde::ser::Error::custom("Invalid context value reference"))?
                .serialize(serializer),
            Self::Object { fields } => {
                let mut map = serializer.serialize_map(Some(fields.len()))?;
                for (key, value) in fields {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
            Self::Array { items } => items.serialize(serializer),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ContextItem {
    Group {
        id: String,
        label: String,
        items: Vec<ContextItem>,
    },
    Fragment {
        id: String,
        role: FragmentRole,
        format: FragmentFormat,
        value: ContextValue,
        sources: Vec<String>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceNeed {
    pub resource: String,
    pub data_type: DataType,
    pub required_by: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextIteration {
    pub block_id: String,
    pub index: usize,
}

/// Links the exact evaluated occurrence back to its authored block without
/// changing the historical window item codec or copying any source payload.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextTrace {
    pub id: String,
    pub block_id: String,
    pub path: String,
    pub sources: Vec<String>,
    pub iterations: Vec<ContextIteration>,
    pub output_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<bool>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextEvaluation {
    pub complete: bool,
    pub items: Vec<ContextItem>,
    /// Requested capabilities, never an authorization grant.
    pub capabilities: Vec<ContextCapability>,
    pub needs: Vec<ResourceNeed>,
    pub diagnostics: Vec<Diagnostic>,
    /// Resources consulted by live expressions, including unsuccessful presence probes.
    pub reads: Vec<String>,
    pub trace: Vec<ContextTrace>,
}

/// Evaluate a fresh context against immutable, already acquired resources.
/// Missing resources are demands only when a live branch actually reads them.
pub fn evaluate(
    strategy: &ContextStrategy,
    resources: &BTreeMap<String, Arc<Value>>,
    types: &TypeRegistry,
) -> ContextEvaluation {
    evaluate_with_library(strategy, resources, types, &ContextLibrary::default())
}

#[derive(Clone, Copy, Debug)]
pub struct EvaluationLimits {
    pub max_steps: usize,
    pub max_items: usize,
    pub max_bytes: usize,
}
impl Default for EvaluationLimits {
    fn default() -> Self {
        Self {
            max_steps: 100_000,
            max_items: 10_000,
            max_bytes: 16 * 1024 * 1024,
        }
    }
}

pub fn evaluate_with_library(
    strategy: &ContextStrategy,
    resources: &BTreeMap<String, Arc<Value>>,
    types: &TypeRegistry,
    library: &ContextLibrary,
) -> ContextEvaluation {
    evaluate_with_limits(
        strategy,
        resources,
        types,
        library,
        EvaluationLimits::default(),
    )
}

pub fn evaluate_with_limits(
    strategy: &ContextStrategy,
    resources: &BTreeMap<String, Arc<Value>>,
    types: &TypeRegistry,
    library: &ContextLibrary,
    limits: EvaluationLimits,
) -> ContextEvaluation {
    let mut output = ContextEvaluation {
        complete: false,
        items: vec![],
        capabilities: strategy.capabilities.clone(),
        needs: vec![],
        diagnostics: vec![],
        reads: vec![],
        trace: vec![],
    };
    if let Err(errors) = validate_strategy_with_library(strategy, types, library) {
        output.diagnostics = errors;
        return output;
    }
    let types = match resolved_types(strategy, types) {
        Ok(types) => types,
        Err(errors) => {
            output.diagnostics = errors;
            return output;
        }
    };
    fn block_ids(blocks: &[ContextBlock], ids: &mut BTreeSet<String>) {
        for block in blocks {
            ids.insert(block.id().to_owned());
            match block {
                ContextBlock::Group { items, .. } | ContextBlock::ForEach { items, .. } => {
                    block_ids(items, ids)
                }
                ContextBlock::If {
                    then, otherwise, ..
                } => {
                    block_ids(then, ids);
                    block_ids(otherwise, ids);
                }
                ContextBlock::Emit { .. } => {}
            }
        }
    }
    let mut ids = BTreeSet::new();
    block_ids(&strategy.program, &mut ids);
    let mut evaluator = Evaluator {
        strategy,
        resources,
        types: &types,
        needs: BTreeMap::new(),
        diagnostics: vec![],
        validated: BTreeSet::new(),
        library,
        variables: BTreeMap::new(),
        limits,
        steps: 0,
        output_bytes: 0,
        derived_bytes: 0,
        reads: BTreeSet::new(),
        all_reads: BTreeSet::new(),
        variable_sources: BTreeMap::new(),
        trace: vec![],
        iterations: vec![],
        output_ids: ids,
        output_items: 0,
    };
    output.items = evaluator.blocks(&strategy.program, "program");
    output.needs = evaluator.needs.into_values().collect();
    output.diagnostics = evaluator.diagnostics;
    output.reads = evaluator.all_reads.into_iter().collect();
    output.trace = evaluator.trace;
    output.complete = output.needs.is_empty() && output.diagnostics.is_empty();
    output
}

enum AccessError {
    Missing(String),
    Invalid,
}
struct Evaluator<'a> {
    strategy: &'a ContextStrategy,
    resources: &'a BTreeMap<String, Arc<Value>>,
    types: &'a TypeRegistry,
    needs: BTreeMap<String, ResourceNeed>,
    diagnostics: Vec<Diagnostic>,
    validated: BTreeSet<String>,
    library: &'a ContextLibrary,
    variables: BTreeMap<String, ContextValue>,
    limits: EvaluationLimits,
    steps: usize,
    output_bytes: usize,
    derived_bytes: usize,
    reads: BTreeSet<String>,
    all_reads: BTreeSet<String>,
    variable_sources: BTreeMap<String, BTreeSet<String>>,
    trace: Vec<ContextTrace>,
    iterations: Vec<ContextIteration>,
    output_ids: BTreeSet<String>,
    output_items: usize,
}
impl Evaluator<'_> {
    fn error(&mut self, path: &str, message: &str) -> AccessError {
        if self.diagnostics.len() < 64 {
            self.diagnostics
                .push(Diagnostic::new("evaluation_limit", path, message));
        }
        AccessError::Invalid
    }
    fn tick(&mut self, path: &str) -> Result<(), AccessError> {
        if self.steps >= self.limits.max_steps {
            return Err(self.error(path, "Context evaluation exceeded its operation budget"));
        }
        self.steps += 1;
        Ok(())
    }
    fn items(&mut self, value: &ContextValue, path: &str) -> Result<usize, AccessError> {
        let count = value.array_len().ok_or(AccessError::Invalid)?;
        if count > self.limits.max_items {
            return Err(self.error(path, "Collection exceeds the item budget"));
        }
        Ok(count)
    }
    fn bytes(&mut self, value: &ContextValue, path: &str) -> Result<usize, AccessError> {
        measured_bytes(value, self.limits.max_bytes)
            .map_err(|_| self.error(path, "Context value exceeds its byte budget"))
    }
    fn charge(&mut self, bytes: usize, path: &str) -> Result<(), AccessError> {
        if bytes > self.limits.max_bytes.saturating_sub(self.derived_bytes) {
            return Err(self.error(path, "Derived values exceed the allocation budget"));
        }
        self.derived_bytes += bytes;
        Ok(())
    }
    fn keyed(
        &mut self,
        expression: &ContextExpr,
        source: &ContextExpr,
        item: &str,
        key: &ContextExpr,
        path: &str,
    ) -> Result<ContextValue, AccessError> {
        let source = self.expression(source, path)?;
        let count = self.items(&source, path)?;
        let source_reads = self.reads.clone();
        let mut values = Vec::with_capacity(count);
        for index in 0..count {
            self.tick(path)?;
            let value = source.index(index).ok_or(AccessError::Invalid)?;
            let previous = self.variables.insert(item.into(), value.clone());
            let previous_sources = self
                .variable_sources
                .insert(item.into(), source_reads.clone());
            let result = self.expression(key, path);
            self.restore_variable(item, previous);
            self.restore_variable_sources(item, previous_sources);
            let result = result?;
            if matches!(expression, ContextExpr::Map { .. }) {
                values.push((result, value));
            } else {
                let bytes = self.bytes(&result, path)?;
                self.charge(bytes, path)?;
                values.push((result, value));
            }
        }
        match expression {
            ContextExpr::Map { .. } => Ok(ContextValue::Array {
                items: values.into_iter().map(|(v, _)| v).collect(),
            }),
            ContextExpr::Sort { descending, .. } => {
                values.sort_by(|(left, _), (right, _)| {
                    let order = compare_values(left, right).unwrap_or(std::cmp::Ordering::Equal);
                    if *descending { order.reverse() } else { order }
                });
                Ok(ContextValue::Array {
                    items: values.into_iter().map(|(_, v)| v).collect(),
                })
            }
            ContextExpr::Dedup { .. } => {
                let mut seen = BTreeSet::new();
                let mut output = vec![];
                for (key, value) in values {
                    let key = serde_json::to_string(&key).map_err(|_| AccessError::Invalid)?;
                    self.charge(key.len(), path)?;
                    if seen.insert(key) {
                        output.push(value);
                    }
                }
                Ok(ContextValue::Array { items: output })
            }
            ContextExpr::GroupBy { .. } => {
                let mut groups: Vec<(ContextValue, Vec<ContextValue>)> = vec![];
                let mut positions = BTreeMap::new();
                for (key, value) in values {
                    let encoded = serde_json::to_string(&key).map_err(|_| AccessError::Invalid)?;
                    self.charge(encoded.len(), path)?;
                    let position = *positions.entry(encoded).or_insert_with(|| {
                        groups.push((key, vec![]));
                        groups.len() - 1
                    });
                    groups[position].1.push(value);
                }
                Ok(ContextValue::Array {
                    items: groups
                        .into_iter()
                        .map(|(key, items)| ContextValue::Object {
                            fields: BTreeMap::from([
                                ("key".into(), key),
                                ("items".into(), ContextValue::Array { items }),
                            ]),
                        })
                        .collect(),
                })
            }
            _ => Err(AccessError::Invalid),
        }
    }
    fn restore_variable(&mut self, name: &str, previous: Option<ContextValue>) {
        if let Some(previous) = previous {
            self.variables.insert(name.into(), previous);
        } else {
            self.variables.remove(name);
        }
    }
    fn restore_variable_sources(&mut self, name: &str, previous: Option<BTreeSet<String>>) {
        if let Some(previous) = previous {
            self.variable_sources.insert(name.into(), previous);
        } else {
            self.variable_sources.remove(name);
        }
    }
    fn expression(
        &mut self,
        expression: &ContextExpr,
        path: &str,
    ) -> Result<ContextValue, AccessError> {
        self.tick(path)?;
        match expression {
            ContextExpr::Construct { name, value } => {
                let value = self.expression(value, path)?;
                let target = DataType::Named { name: name.clone() };
                let checked = if let Some(raw) = value.as_value() {
                    validate_value(&target, raw, self.types)
                } else {
                    let bytes = self.bytes(&value, path)?;
                    self.charge(bytes, path)?;
                    let raw = serde_json::to_value(&value).map_err(|_| AccessError::Invalid)?;
                    validate_value(&target, &raw, self.types)
                };
                if let Err(errors) = checked {
                    self.diagnostics
                        .extend(errors.into_iter().take(64).map(|mut error| {
                            error.path = format!("{path}:{}", error.path);
                            error
                        }));
                    return Err(AccessError::Invalid);
                }
                Ok(value)
            }
            ContextExpr::Variable { name } => {
                if let Some(sources) = self.variable_sources.get(name) {
                    self.reads.extend(sources.iter().cloned());
                }
                self.variables
                    .get(name)
                    .cloned()
                    .ok_or(AccessError::Invalid)
            }
            ContextExpr::Filter {
                value,
                item,
                condition,
            } => {
                let source = self.expression(value, path)?;
                let count = self.items(&source, path)?;
                let source_reads = self.reads.clone();
                let mut output = vec![];
                for index in 0..count {
                    self.tick(path)?;
                    let value = source.index(index).ok_or(AccessError::Invalid)?;
                    let previous = self.variables.insert(item.clone(), value.clone());
                    let previous_sources = self
                        .variable_sources
                        .insert(item.clone(), source_reads.clone());
                    let selected = self.predicate(condition, path);
                    self.restore_variable(item, previous);
                    self.restore_variable_sources(item, previous_sources);
                    if selected? {
                        output.push(value);
                    }
                }
                Ok(ContextValue::Array { items: output })
            }
            ContextExpr::Map { value, item, body }
            | ContextExpr::Sort {
                value,
                item,
                key: body,
                ..
            }
            | ContextExpr::GroupBy {
                value,
                item,
                key: body,
            }
            | ContextExpr::Dedup {
                value,
                item,
                key: body,
            } => self.keyed(expression, value, item, body, path),
            ContextExpr::Take { value, count } => {
                let value = self.expression(value, path)?;
                let count = self.items(&value, path)?.min(*count);
                Ok(ContextValue::Array {
                    items: (0..count).filter_map(|index| value.index(index)).collect(),
                })
            }
            ContextExpr::Truncate { value, count } => {
                let value = self.expression(value, path)?;
                let text = value
                    .as_value()
                    .and_then(Value::as_str)
                    .ok_or(AccessError::Invalid)?;
                let end = text
                    .char_indices()
                    .nth(*count)
                    .map_or(text.len(), |(index, _)| index);
                self.charge(end, path)?;
                Ok(ContextValue::owned(Value::String(text[..end].to_owned())))
            }
            ContextExpr::Record { fields } => {
                let mut output = BTreeMap::new();
                for (name, value) in fields {
                    output.insert(name.clone(), self.expression(value, path)?);
                }
                Ok(ContextValue::Object { fields: output })
            }
            ContextExpr::List { items, .. } => {
                if items.len() > self.limits.max_items {
                    return Err(self.error(path, "List construction exceeds the item budget"));
                }
                let mut output = Vec::with_capacity(items.len());
                for (index, item) in items.iter().enumerate() {
                    output.push(self.expression(item, &format!("{path}.items[{index}]"))?);
                }
                Ok(ContextValue::Array { items: output })
            }
            ContextExpr::Template { template, values } => {
                let parts = template_parts(template).map_err(|_| AccessError::Invalid)?;
                let mut arguments = BTreeMap::new();
                for (name, value) in values {
                    arguments.insert(name.clone(), self.expression(value, path)?);
                }
                let mut text = String::new();
                for part in parts {
                    let value = match part {
                        TemplatePart::Text(text) => text,
                        TemplatePart::Variable(name) => arguments
                            .get(name)
                            .and_then(ContextValue::as_value)
                            .and_then(Value::as_str)
                            .ok_or(AccessError::Invalid)?,
                    };
                    if value.len() > self.limits.max_bytes.saturating_sub(text.len()) {
                        return Err(self.error(path, "Template exceeds its byte budget"));
                    }
                    text.push_str(value);
                }
                self.charge(text.len(), path)?;
                Ok(ContextValue::owned(Value::String(text)))
            }
            ContextExpr::ToJson { value } => {
                let value = self.expression(value, path)?;
                self.bytes(&value, path)?;
                let text = serde_json::to_string(&value).map_err(|_| AccessError::Invalid)?;
                self.charge(text.len(), path)?;
                Ok(ContextValue::owned(Value::String(text)))
            }
            ContextExpr::Measure { value, unit } => {
                let value = self.expression(value, path)?;
                let count = match unit {
                    MeasureUnit::Bytes => self.bytes(&value, path)?,
                    MeasureUnit::Items => self.items(&value, path)?,
                    MeasureUnit::Media => {
                        if value.array_len().is_some() {
                            self.items(&value, path)?
                        } else {
                            1
                        }
                    }
                };
                Ok(ContextValue::owned(Value::from(count)))
            }
            ContextExpr::Call {
                catalog,
                name,
                arguments,
            } => {
                let function = self
                    .library
                    .get(*catalog, name)
                    .ok_or(AccessError::Invalid)?;
                let mut bindings = BTreeMap::new();
                let mut sources = BTreeMap::new();
                for (name, argument) in arguments {
                    bindings.insert(name.clone(), self.expression(argument, path)?);
                    sources.insert(name.clone(), self.reads.clone());
                }
                let previous = std::mem::replace(&mut self.variables, bindings);
                let previous_sources = std::mem::replace(&mut self.variable_sources, sources);
                let result = self.expression(&function.body, &format!("{path}.call.{name}"));
                self.variables = previous;
                self.variable_sources = previous_sources;
                result
            }
            ContextExpr::Resource { name } => {
                self.reads.insert(name.clone());
                self.all_reads.insert(name.clone());
                let value = self
                    .resources
                    .get(name)
                    .ok_or_else(|| AccessError::Missing(name.clone()))?;
                if !self.validated.contains(name) {
                    self.bytes(
                        &ContextValue::Shared {
                            root: Arc::clone(value),
                            pointer: String::new(),
                        },
                        path,
                    )?;
                    let data_type = &self.strategy.requirements[name];
                    if let Err(mut errors) = validate_value(data_type, value, self.types) {
                        for error in &mut errors {
                            error.path = format!(
                                "{path}:resources.{name}{}",
                                error.path.strip_prefix('$').unwrap_or(&error.path)
                            );
                        }
                        self.diagnostics.extend(errors);
                        return Err(AccessError::Invalid);
                    }
                    self.validated.insert(name.clone());
                }
                Ok(ContextValue::Shared {
                    root: Arc::clone(value),
                    pointer: String::new(),
                })
            }
            ContextExpr::Literal { value, .. } => Ok(ContextValue::Shared {
                root: Arc::new(value.clone()),
                pointer: String::new(),
            }),
            ContextExpr::Field { value, field } => {
                self.expression(value, path)?.field(field).ok_or_else(|| {
                    self.diagnostics.push(Diagnostic::new(
                        "missing_field",
                        path,
                        format!("Field unavailable: {field}"),
                    ));
                    AccessError::Invalid
                })
            }
            ContextExpr::Project { value, fields } => {
                let value = self.expression(value, path)?;
                let mut selected = BTreeMap::new();
                for field in fields {
                    let part = value.field(field).ok_or_else(|| {
                        self.diagnostics.push(Diagnostic::new(
                            "missing_field",
                            path,
                            format!("Field unavailable: {field}"),
                        ));
                        AccessError::Invalid
                    })?;
                    selected.insert(field.clone(), part);
                }
                Ok(ContextValue::Object { fields: selected })
            }
        }
    }
    fn predicate(&mut self, condition: &ContextPredicate, path: &str) -> Result<bool, AccessError> {
        self.tick(path)?;
        match condition {
            ContextPredicate::Compare {
                left,
                operator,
                right,
            } => {
                let left = self.expression(left, path)?;
                let right = self.expression(right, path)?;
                if *operator == Comparison::Ne {
                    return Ok(!left.equivalent(&right));
                }
                let order = compare_values(&left, &right).ok_or(AccessError::Invalid)?;
                use std::cmp::Ordering;
                Ok(match operator {
                    Comparison::Lt => order == Ordering::Less,
                    Comparison::Lte => order != Ordering::Greater,
                    Comparison::Gt => order == Ordering::Greater,
                    Comparison::Gte => order != Ordering::Less,
                    Comparison::Ne => unreachable!(),
                })
            }
            ContextPredicate::Contains { value, item } => {
                let value = self.expression(value, path)?;
                let item = self.expression(item, path)?;
                if let Some(text) = value.as_value().and_then(Value::as_str) {
                    return Ok(text.contains(
                        item.as_value()
                            .and_then(Value::as_str)
                            .ok_or(AccessError::Invalid)?,
                    ));
                }
                let count = self.items(&value, path)?;
                for index in 0..count {
                    self.tick(path)?;
                    if value.index(index).is_some_and(|v| v.equivalent(&item)) {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            ContextPredicate::Present { value } => match self.expression(value, path) {
                Ok(_) => Ok(true),
                Err(AccessError::Missing(_)) => Ok(false),
                Err(error) => Err(error),
            },
            ContextPredicate::Eq { left, right } => Ok(self
                .expression(left, path)?
                .equivalent(&self.expression(right, path)?)),
            ContextPredicate::And { items } => {
                for item in items {
                    if !self.predicate(item, path)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            ContextPredicate::Or { items } => {
                for item in items {
                    if self.predicate(item, path)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            ContextPredicate::Not { item } => Ok(!self.predicate(item, path)?),
        }
    }
    fn demand(&mut self, error: AccessError, path: &str) {
        if let AccessError::Missing(resource) = error {
            let need = self
                .needs
                .entry(resource.clone())
                .or_insert_with(|| ResourceNeed {
                    data_type: self.strategy.requirements[&resource].clone(),
                    resource,
                    required_by: vec![],
                });
            if !need.required_by.iter().any(|value| value == path) {
                need.required_by.push(path.into());
            }
        }
    }
    fn occurrence_id(&mut self, block_id: &str, path: &str) -> String {
        if self.iterations.is_empty() {
            return block_id.to_owned();
        }
        // Reserve every authored ID before evaluation, so even an authored name
        // resembling this representation cannot collide with a loop occurrence.
        let mut id = format!("{block_id}@{path}");
        while !self.output_ids.insert(id.clone()) {
            id.push('_');
        }
        id
    }
    fn count_output(&mut self, path: &str) -> bool {
        if self.output_items >= self.limits.max_items {
            self.error(path, "Prepared window exceeds its item budget");
            false
        } else {
            self.output_items += 1;
            true
        }
    }
    fn blocks(&mut self, blocks: &[ContextBlock], path: &str) -> Vec<ContextItem> {
        let mut output = vec![];
        for (index, block) in blocks.iter().enumerate() {
            let path = format!("{path}[{index}]");
            if self.tick(&path).is_err() {
                break;
            }
            if self.trace.len() >= self.limits.max_items {
                self.error(&path, "Context trace exceeds its item budget");
                break;
            }
            let id = self.occurrence_id(block.id(), &path);
            let previous_reads = std::mem::take(&mut self.reads);
            let begin = output.len();
            let mut outcome = None;
            match block {
                ContextBlock::Group { label, items, .. } => {
                    if self.count_output(&path) {
                        output.push(ContextItem::Group {
                            id: id.clone(),
                            label: label.clone(),
                            items: self.blocks(items, &format!("{path}.items")),
                        });
                    }
                }
                ContextBlock::Emit {
                    role,
                    format,
                    value,
                    ..
                } => match self.expression(value, &path) {
                    Ok(resolved) => {
                        if let Ok(bytes) = self.bytes(&resolved, &path) {
                            if bytes > self.limits.max_bytes.saturating_sub(self.output_bytes) {
                                self.error(&path, "Prepared window exceeds its byte budget");
                            } else if self.count_output(&path) {
                                self.output_bytes += bytes;
                                output.push(ContextItem::Fragment {
                                    id: id.clone(),
                                    role: *role,
                                    format: *format,
                                    value: resolved,
                                    sources: self.reads.iter().cloned().collect(),
                                });
                            }
                        }
                    }
                    Err(error) => self.demand(error, &path),
                },
                ContextBlock::If {
                    condition,
                    then,
                    otherwise,
                    ..
                } => match self.predicate(condition, &format!("{path}.condition")) {
                    Ok(selected) => {
                        outcome = Some(selected);
                        output.extend(self.blocks(
                            if selected { then } else { otherwise },
                            &format!("{path}.{}", if selected { "then" } else { "else" }),
                        ));
                    }
                    Err(error) => self.demand(error, &path),
                },
                ContextBlock::ForEach {
                    value, item, items, ..
                } => {
                    let source = self.expression(value, &format!("{path}.value"));
                    match source {
                        Ok(source) => {
                            if let Ok(count) = self.items(&source, &path) {
                                let sources = self.reads.clone();
                                let old_sources =
                                    self.variable_sources.insert(item.clone(), sources);
                                let old_value = self.variables.remove(item);
                                for index in 0..count {
                                    if self.tick(&path).is_err()
                                        || self.trace.len() >= self.limits.max_items
                                    {
                                        self.error(
                                            &path,
                                            "Loop exceeds the context evaluation budget",
                                        );
                                        break;
                                    }
                                    if let Some(value) = source.index(index) {
                                        self.variables.insert(item.clone(), value);
                                        self.iterations.push(ContextIteration {
                                            block_id: block.id().into(),
                                            index,
                                        });
                                        output.extend(self.blocks(
                                            items,
                                            &format!("{path}.iterations[{index}].items"),
                                        ));
                                        self.iterations.pop();
                                    }
                                }
                                self.restore_variable(item, old_value);
                                self.restore_variable_sources(item, old_sources);
                            }
                        }
                        Err(error) => self.demand(error, &path),
                    }
                }
            }
            if self.trace.len() < self.limits.max_items {
                self.trace.push(ContextTrace {
                    id,
                    block_id: block.id().into(),
                    path,
                    sources: self.reads.iter().cloned().collect(),
                    iterations: self.iterations.clone(),
                    output_ids: output[begin..]
                        .iter()
                        .map(|item| match item {
                            ContextItem::Group { id, .. } | ContextItem::Fragment { id, .. } => {
                                id.clone()
                            }
                        })
                        .collect(),
                    outcome,
                });
            } else {
                self.error(&path, "Context trace exceeds its item budget");
            }
            self.reads.extend(previous_reads);
        }
        output
    }
}

enum TemplatePart<'a> {
    Text(&'a str),
    Variable(&'a str),
}
fn template_parts(template: &str) -> Result<Vec<TemplatePart<'_>>, &'static str> {
    let mut parts = vec![];
    let mut tail = template;
    while let Some(start) = tail.find("{{") {
        parts.push(TemplatePart::Text(&tail[..start]));
        tail = &tail[start + 2..];
        let end = tail.find("}}").ok_or("Unclosed template placeholder")?;
        let name = &tail[..end];
        if !valid_id(name) {
            return Err("Template placeholders must be simple variable names");
        }
        parts.push(TemplatePart::Variable(name));
        tail = &tail[end + 2..];
    }
    parts.push(TemplatePart::Text(tail));
    Ok(parts)
}

/// Bytes means UTF-8 bytes for text, compact JSON bytes for structured values.
/// Media counts measure references; binary media size is never guessed.
fn measured_bytes(value: &ContextValue, max: usize) -> Result<usize, ()> {
    if let Some(text) = value.as_value().and_then(Value::as_str) {
        return if text.len() <= max {
            Ok(text.len())
        } else {
            Err(())
        };
    }
    struct Counter {
        bytes: usize,
        max: usize,
    }
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > self.max.saturating_sub(self.bytes) {
                return Err(std::io::Error::other("byte budget exceeded"));
            }
            self.bytes += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter { bytes: 0, max };
    serde_json::to_writer(&mut counter, value).map_err(|_| ())?;
    Ok(counter.bytes)
}

fn compare_values(left: &ContextValue, right: &ContextValue) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering;
    let (left, right) = (left.as_value()?, right.as_value()?);
    if let (Some(left), Some(right)) = (left.as_str(), right.as_str()) {
        return Some(left.cmp(right));
    }
    if !left.is_number() || !right.is_number() {
        return None;
    }
    let integer = |value: &Value| {
        value
            .as_i64()
            .map(i128::from)
            .or_else(|| value.as_u64().map(i128::from))
    };
    if let (Some(left), Some(right)) = (integer(left), integer(right)) {
        return Some(left.cmp(&right));
    }
    fn int_float(integer: i128, float: f64) -> Ordering {
        if float >= i128::MAX as f64 {
            return Ordering::Less;
        }
        if float <= i128::MIN as f64 {
            return Ordering::Greater;
        }
        integer.cmp(&(float as i128)).then_with(|| {
            if float.fract() > 0.0 {
                Ordering::Less
            } else if float.fract() < 0.0 {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        })
    }
    if let (Some(left), Some(right)) = (integer(left), right.as_f64()) {
        return Some(int_float(left, right));
    }
    if let (Some(left), Some(right)) = (left.as_f64(), integer(right)) {
        return Some(int_float(right, left).reverse());
    }
    left.as_f64()?.partial_cmp(&right.as_f64()?)
}
