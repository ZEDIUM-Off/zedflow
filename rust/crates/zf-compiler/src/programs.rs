//! Link strategies, libraries and types using captured inputs only.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use zf_context::{context, context_library::ContextLibrary, context_source};
use zf_core::types::TypeRegistry;
use zf_flows::schema::Composition;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StrategyReference {
    pub key: String,
    #[serde(default)]
    pub hash: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramDependency {
    pub node_path: String,
    pub key: String,
    pub hash: String,
}

fn diagnostics(errors: Vec<zf_core::types::Diagnostic>) -> anyhow::Error {
    anyhow::anyhow!(
        "{}",
        errors
            .into_iter()
            .map(|e| format!("{}: {} [{}]", e.path, e.message, e.code))
            .collect::<Vec<_>>()
            .join("; ")
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    Strategy,
    Library,
    Types,
}
#[derive(Clone, Debug)]
pub struct SourceOverride {
    pub kind: SourceKind,
    pub key: String,
    pub source: String,
    pub expected_hash: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyReference {
    pub node_path: String,
    pub kind: SourceKind,
    pub key: String,
    pub hash: Option<String>,
}

/// Captured bytes and accepted reference ancestry. Storage/execution must check
/// the actual catalogue head under its lock before supplying this snapshot.
/// Compiler checks bytes and pins; it cannot authenticate external edits.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceSnapshot {
    pub source: String,
    pub hash: String,
    #[serde(default)]
    pub accepted_references: BTreeSet<String>,
}
impl SourceSnapshot {
    pub fn capture(source: String) -> Self {
        let hash = hash(source.as_bytes());
        Self {
            source,
            hash,
            accepted_references: BTreeSet::new(),
        }
    }
    fn accepts(&self, requested: Option<&str>) -> bool {
        requested.is_none_or(|pin| pin == self.hash || self.accepted_references.contains(pin))
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            hash(self.source.as_bytes()) == self.hash,
            "Captured source hash mismatch"
        );
        ensure!(
            self.accepted_references
                .iter()
                .all(|pin| pin.len() == 64 && pin.bytes().all(|c| c.is_ascii_hexdigit())),
            "Invalid accepted source reference"
        );
        Ok(())
    }
}
pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProgramSources {
    pub strategies: BTreeMap<String, SourceSnapshot>,
    pub libraries: BTreeMap<String, SourceSnapshot>,
    pub types: BTreeMap<String, SourceSnapshot>,
}
impl ProgramSources {
    pub fn get(&self, kind: SourceKind, key: &str) -> Option<&SourceSnapshot> {
        match kind {
            SourceKind::Strategy => self.strategies.get(key),
            SourceKind::Library => self.libraries.get(key),
            SourceKind::Types => self.types.get(key),
        }
    }
}

pub fn freeze(doc: &mut Composition, sources: &ProgramSources) -> Result<Vec<ProgramDependency>> {
    freeze_with_overrides(doc, sources, &[])
}
pub fn freeze_with_overrides(
    doc: &mut Composition,
    sources: &ProgramSources,
    overrides: &[SourceOverride],
) -> Result<Vec<ProgramDependency>> {
    // No ambiguous first override wins policy.
    let mut seen = BTreeSet::new();
    for candidate in overrides {
        ensure!(
            seen.insert((candidate.kind, &candidate.key)),
            "Duplicate source override"
        );
    }
    let mut candidate = doc.clone();
    let dependencies = freeze_scope(&mut candidate, sources, "", overrides)?;
    *doc = candidate;
    Ok(dependencies)
}
fn selected_source(
    sources: &ProgramSources,
    kind: SourceKind,
    reference: &StrategyReference,
    overrides: &[SourceOverride],
) -> Result<SourceSnapshot> {
    ensure!(
        context::valid_id(&reference.key),
        "Invalid source reference identity"
    );
    let current = sources.get(kind, &reference.key);
    let candidate = overrides
        .iter()
        .find(|candidate| candidate.kind == kind && candidate.key == reference.key);
    if let Some(candidate) = candidate {
        let next = SourceSnapshot::capture(candidate.source.clone());
        if let Some(expected) = &candidate.expected_hash {
            let current = current.context("Expected source is absent from snapshot")?;
            current.validate()?;
            ensure!(
                &current.hash == expected && current.accepts(reference.hash.as_deref()),
                "Source changed during dependent preflight"
            );
        } else {
            ensure!(
                reference.hash.as_ref().is_none_or(|pin| *pin == next.hash),
                "New source reference has an unrelated hash"
            );
        }
        return Ok(next);
    }
    let current =
        current.with_context(|| format!("Source {} is absent from snapshot", reference.key))?;
    current.validate()?;
    ensure!(
        current.accepts(reference.hash.as_deref()),
        "Unaccepted source reference: {}",
        reference.key
    );
    Ok(current.clone())
}

fn freeze_scope(
    doc: &mut Composition,
    sources: &ProgramSources,
    scope: &str,
    overrides: &[SourceOverride],
) -> Result<Vec<ProgramDependency>> {
    let mut dependencies = Vec::new();
    for node in &mut doc.nodes {
        let path = format!("{scope}{}", node.id);
        if node.data.kind == "subgraph" {
            let mut child: Composition =
                serde_json::from_value(node.data.config["composition"].clone())?;
            dependencies.extend(freeze_scope(
                &mut child,
                sources,
                &format!("{path}/"),
                overrides,
            )?);
            node.data.config["composition"] = serde_json::to_value(child)?;
        }
        if !matches!(node.data.kind.as_str(), "agent" | "context") {
            continue;
        }
        let Some(reference) = node
            .data
            .config
            .get("contextStrategy")
            .filter(|v| !v.is_null())
        else {
            if let Some(program) = node.data.config.get("contextProgram") {
                validate_frozen(program).with_context(|| path.clone())?;
            }
            continue;
        };
        let reference: StrategyReference = if let Some(key) = reference.as_str() {
            StrategyReference {
                key: key.into(),
                hash: None,
            }
        } else {
            serde_json::from_value(reference.clone())
                .context("Invalid context strategy reference")?
        };
        let file = selected_source(sources, SourceKind::Strategy, &reference, overrides)
            .with_context(|| format!("{path}: stratégie {}", reference.key))?;
        let strategy = context_source::parse(&file.source).map_err(diagnostics)?;
        let source = file.source.clone();
        ensure!(
            strategy.id == reference.key,
            "Strategy identity differs from its selected key"
        );
        let mut types: TypeRegistry = serde_json::from_value(
            node.data
                .config
                .get("contextTypes")
                .cloned()
                .unwrap_or(json!({})),
        )?;
        let mut type_sources = Vec::<Value>::new();
        if let Some(raw) = node
            .data
            .config
            .get("contextTypesRef")
            .filter(|v| !v.is_null())
        {
            let reference = self::reference(raw)?;
            let file = selected_source(sources, SourceKind::Types, &reference, overrides)?;
            types = context_source::parse_types(&file.source).map_err(diagnostics)?;
            type_sources.push(json!({"key":reference.key,"hash":file.hash,"source":file.source}));
        }
        let mut library: ContextLibrary = serde_json::from_value(
            node.data
                .config
                .get("contextLibrary")
                .cloned()
                .unwrap_or(json!({})),
        )?;
        let mut library_sources = Vec::<Value>::new();
        if let Some(reference) = node
            .data
            .config
            .get("contextLibraryRef")
            .filter(|v| !v.is_null())
        {
            let reference: StrategyReference = serde_json::from_value(reference.clone())?;
            let file = selected_source(sources, SourceKind::Library, &reference, overrides)?;
            library = context_source::parse_library(&file.source).map_err(diagnostics)?;
            library_sources
                .push(json!({"key":reference.key,"hash":file.hash,"source":file.source}));
        }
        types = context::resolved_types(&strategy, &types).map_err(diagnostics)?;
        context::validate_strategy_with_library(&strategy, &types, &library)
            .map_err(diagnostics)?;
        let bindings: BTreeMap<String, Value> = serde_json::from_value(
            node.data
                .config
                .get("contextBindings")
                .cloned()
                .unwrap_or(json!({})),
        )?;
        for key in bindings.keys() {
            ensure!(
                strategy.requirements.contains_key(key),
                "{path}: binding vers une ressource non déclarée : {key}"
            );
        }
        for required in strategy.requirements.keys() {
            ensure!(
                bindings.contains_key(required),
                "{path}: ressource {required} déclarée sans binding de source"
            );
        }
        node.data.config["contextProgram"] = json!({"strategy":strategy,"types":types,"typeSources":type_sources,"source":source,"hash":file.hash,"bindings":bindings,"library":library,"librarySources":library_sources});
        if let Some(window) = node.data.config.get("contextWindow").cloned() {
            node.data.config["contextProgram"]["window"] = window;
        }
        dependencies.push(ProgramDependency {
            node_path: path,
            key: reference.key,
            hash: file.hash,
        });
    }
    Ok(dependencies)
}

fn reference(value: &Value) -> Result<StrategyReference> {
    if let Some(key) = value.as_str() {
        Ok(StrategyReference {
            key: key.into(),
            hash: None,
        })
    } else {
        Ok(serde_json::from_value(value.clone())?)
    }
}
pub fn references(doc: &Composition) -> Result<Vec<DependencyReference>> {
    fn collect(doc: &Composition, prefix: &str, out: &mut Vec<DependencyReference>) -> Result<()> {
        for node in &doc.nodes {
            let path = format!("{prefix}{}", node.id);
            if node.data.kind == "subgraph" {
                collect(
                    &serde_json::from_value(node.data.config["composition"].clone())?,
                    &format!("{path}/"),
                    out,
                )?;
            }
            if !matches!(node.data.kind.as_str(), "agent" | "context") {
                continue;
            }
            for (field, kind) in [
                ("contextStrategy", SourceKind::Strategy),
                ("contextLibraryRef", SourceKind::Library),
                ("contextTypesRef", SourceKind::Types),
            ] {
                if let Some(value) = node.data.config.get(field).filter(|v| !v.is_null()) {
                    let reference = reference(value)?;
                    out.push(DependencyReference {
                        node_path: path.clone(),
                        kind,
                        key: reference.key,
                        hash: reference.hash,
                    });
                }
            }
        }
        Ok(())
    }
    let mut out = vec![];
    collect(doc, "", &mut out)?;
    Ok(out)
}
/// This inexpensive first pass does not make an unrelated malformed reference
/// prevent saving another source; a dependent flow is fully validated afterwards.
pub fn depends_on(doc: &Composition, kind: SourceKind, key: &str) -> bool {
    doc.nodes.iter().any(|node| {
        if node.data.kind == "subgraph" {
            return serde_json::from_value(node.data.config["composition"].clone())
                .is_ok_and(|child| depends_on(&child, kind, key));
        }
        let field = match kind {
            SourceKind::Strategy => "contextStrategy",
            SourceKind::Library => "contextLibraryRef",
            SourceKind::Types => "contextTypesRef",
        };
        matches!(node.data.kind.as_str(), "agent" | "context")
            && node
                .data
                .config
                .get(field)
                .is_some_and(|value| value.as_str().or_else(|| value["key"].as_str()) == Some(key))
    })
}

pub use zf_context::frozen_context::validate_frozen;

/// Logical catalogue precondition. Execution resolves its storage location;
/// compiler never constructs workspace paths or reads the current head.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapturedDependency {
    pub kind: SourceKind,
    pub key: String,
    pub hash: String,
}

pub fn captured_sources(doc: &Composition) -> Result<Vec<CapturedDependency>> {
    fn collect(doc: &Composition, out: &mut BTreeMap<(SourceKind, String), String>) -> Result<()> {
        for node in &doc.nodes {
            if node.data.kind == "subgraph" {
                collect(
                    &serde_json::from_value(node.data.config["composition"].clone())?,
                    out,
                )?;
            }
            if !matches!(node.data.kind.as_str(), "agent" | "context") {
                continue;
            }
            let Some(raw) = node
                .data
                .config
                .get("contextStrategy")
                .filter(|v| !v.is_null())
            else {
                continue;
            };
            let selected = reference(raw)?;
            let program = &node.data.config["contextProgram"];
            validate_frozen(program)?;
            insert(
                out,
                SourceKind::Strategy,
                &selected.key,
                program["hash"]
                    .as_str()
                    .context("Captured strategy hash absent")?,
            )?;
            for (field, kind) in [
                ("librarySources", SourceKind::Library),
                ("typeSources", SourceKind::Types),
            ] {
                if let Some(sources) = program[field].as_array() {
                    for source in sources {
                        insert(
                            out,
                            kind,
                            source["key"].as_str().context("Captured key absent")?,
                            source["hash"].as_str().context("Captured hash absent")?,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }
    fn insert(
        out: &mut BTreeMap<(SourceKind, String), String>,
        kind: SourceKind,
        key: &str,
        hash: &str,
    ) -> Result<()> {
        ensure!(context::valid_id(key), "Invalid captured dependency key");
        if let Some(previous) = out.insert((kind, key.into()), hash.into()) {
            ensure!(previous == hash, "Conflicting captured revisions of {key}");
        }
        Ok(())
    }
    let mut captures = BTreeMap::new();
    collect(doc, &mut captures)?;
    Ok(captures
        .into_iter()
        .map(|((kind, key), hash)| CapturedDependency { kind, key, hash })
        .collect())
}
