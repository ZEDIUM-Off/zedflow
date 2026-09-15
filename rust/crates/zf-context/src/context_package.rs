//! Pure source bundle validation. Catalogue reads and transactional installation
//! belong to storage; bridge parsing is injected from the flow domain.
use super::{
    context::{self, ContextLibrary, ContextStrategy, valid_id},
    context_source,
};
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use zf_core::{diagnostics::Diagnostic, types::TypeRegistry};

pub const PACKAGE_VERSION: u32 = 1;
const MAX_FILES: usize = 256;
const MAX_BYTES: usize = 64 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactKind {
    Strategy,
    Library,
    Bridge,
    Types,
    Example,
}
impl ArtifactKind {
    pub fn segments(self) -> Vec<String> {
        match self {
            Self::Strategy => vec!["context".into()],
            Self::Library => vec!["context".into(), "libraries".into()],
            Self::Bridge => vec!["bridges".into()],
            Self::Types => vec!["types".into()],
            Self::Example => vec!["examples".into()],
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceArtifact {
    pub kind: ArtifactKind,
    pub key: String,
    pub source: String,
    pub hash: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextPackage {
    pub version: u32,
    pub artifacts: Vec<SourceArtifact>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactSelection {
    pub kind: ArtifactKind,
    pub key: String,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackagePrerequisite {
    pub kind: String,
    pub key: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageValidation {
    pub valid: bool,
    pub prerequisites: Vec<PackagePrerequisite>,
    pub diagnostics: Vec<Diagnostic>,
}
/// Pure dependency summary supplied by the flow domain after parsing a bridge.
#[derive(Clone, Debug, Default)]
pub struct BridgeDependencies {
    pub requires: Vec<String>,
    pub flows: Vec<String>,
}
/// Parsing belongs to flows, keeping context independent of the composition model.
pub trait BridgeArtifactValidator {
    fn validate(&self, source: &str) -> Result<BridgeDependencies, Vec<Diagnostic>>;
}

/// Validate a context-only package. Bridge artifacts require `validate_package`
/// with the flow domain's validator and are explicitly rejected here.
pub fn validate_context_package(package: &ContextPackage) -> PackageValidation {
    struct NoBridges;
    impl BridgeArtifactValidator for NoBridges {
        fn validate(&self, _: &str) -> Result<BridgeDependencies, Vec<Diagnostic>> {
            Err(vec![Diagnostic::new(
                "package_bridge_validator",
                "bridge",
                "Bridge artifacts require an explicit flow-domain validator",
            )])
        }
    }
    validate_package(package, &NoBridges)
}

/// Flow files are intentionally external prerequisites. A source bundle can be
/// valid without executing, locating or automatically installing those flows.
pub fn validate_package(
    package: &ContextPackage,
    bridge_validator: &dyn BridgeArtifactValidator,
) -> PackageValidation {
    let mut diagnostics = vec![];
    let mut prerequisites = BTreeSet::new();
    let mut strategies = BTreeMap::<String, ContextStrategy>::new();
    let mut bridges = BTreeMap::<String, BridgeDependencies>::new();
    let mut library = ContextLibrary::default();
    let mut types = TypeRegistry::new();
    if !matches!(package.version, 1 | 2) {
        diagnostics.push(Diagnostic::new(
            "package_version",
            "version",
            "Unsupported source package version",
        ));
    }
    if package.artifacts.is_empty() || package.artifacts.len() > MAX_FILES {
        diagnostics.push(Diagnostic::new(
            "package_limit",
            "artifacts",
            "Package requires 1–256 artifacts",
        ));
    }
    let mut total = 0usize;
    let mut seen = BTreeSet::new();
    for (index, artifact) in package.artifacts.iter().take(MAX_FILES + 1).enumerate() {
        let path = format!("artifacts[{index}]");
        total = total.saturating_add(artifact.source.len());
        if total > MAX_BYTES || artifact.source.len() > context_source::MAX_SOURCE_BYTES {
            diagnostics.push(Diagnostic::new(
                "package_limit",
                &path,
                "Source package exceeds its byte limit",
            ));
            break;
        }
        if !valid_id(&artifact.key) || !seen.insert((artifact.kind, artifact.key.clone())) {
            diagnostics.push(Diagnostic::new(
                "package_identity",
                &path,
                "Artifact keys must be valid and unique within their catalogue",
            ));
            continue;
        }
        if super::type_examples::hash(artifact.source.as_bytes()) != artifact.hash {
            diagnostics.push(Diagnostic::new(
                "package_hash",
                &path,
                "Artifact source does not match its declared hash",
            ));
            continue;
        }
        let result = match artifact.kind {
            ArtifactKind::Example => super::type_examples::parse(&artifact.source)
                .and_then(|example| {
                    ensure!(package.version >= 2, "Examples require package format v2");
                    ensure!(
                        example.id == artifact.key,
                        "Example identity must match its artifact key"
                    );
                    Ok(())
                })
                .map_err(|error| {
                    vec![Diagnostic::new(
                        "package_example",
                        &path,
                        format!("{error:#}"),
                    )]
                }),
            ArtifactKind::Strategy => {
                context_source::parse(&artifact.source).and_then(|strategy| {
                    if strategy.id != artifact.key {
                        return Err(vec![Diagnostic::new(
                            "package_identity",
                            &path,
                            "Strategy identity must match its key",
                        )]);
                    }
                    strategies.insert(artifact.key.clone(), strategy);
                    Ok(())
                })
            }
            ArtifactKind::Library => {
                context_source::parse_library(&artifact.source).and_then(|parsed| {
                    let mut errors = vec![];
                    for (target, values) in [
                        (&mut library.projections, parsed.projections),
                        (&mut library.subprograms, parsed.subprograms),
                    ] {
                        for (name, function) in values {
                            if let Some(old) = target.get(&name) {
                                if old != &function {
                                    errors.push(Diagnostic::new(
                                        "package_function_conflict",
                                        &path,
                                        format!("Conflicting function definition: {name}"),
                                    ));
                                }
                            } else {
                                target.insert(name, function);
                            }
                        }
                    }
                    if errors.is_empty() {
                        Ok(())
                    } else {
                        Err(errors)
                    }
                })
            }
            ArtifactKind::Types => {
                context_source::parse_types(&artifact.source).and_then(|parsed| {
                    let mut errors = vec![];
                    for (name, ty) in parsed {
                        if let Some(old) = types.get(&name) {
                            if old != &ty {
                                errors.push(Diagnostic::new(
                                    "package_type_conflict",
                                    &path,
                                    format!("Conflicting named type: {name}"),
                                ));
                            }
                        } else {
                            types.insert(name, ty);
                        }
                    }
                    if errors.is_empty() {
                        Ok(())
                    } else {
                        Err(errors)
                    }
                })
            }
            ArtifactKind::Bridge => bridge_validator.validate(&artifact.source).map(|bridge| {
                bridges.insert(artifact.key.clone(), bridge);
            }),
        };
        if let Err(errors) = result {
            diagnostics.extend(errors.into_iter().take(64).map(|mut d| {
                d.path = format!("{path}:{}", d.path);
                d
            }));
        }
        if diagnostics.len() >= 64 {
            break;
        }
    }
    if diagnostics.is_empty() {
        for (key, strategy) in &strategies {
            if let Err(errors) = context::validate_strategy_with_library(strategy, &types, &library)
            {
                diagnostics.extend(errors.into_iter().map(|mut d| {
                    d.path = format!("strategies.{key}:{}", d.path);
                    d
                }));
            }
        }
        if let Err(errors) = context::validate_library(&library, &types) {
            diagnostics.extend(errors);
        }
        fn visit(
            key: &str,
            bridges: &BTreeMap<String, BridgeDependencies>,
            active: &mut BTreeSet<String>,
            done: &mut BTreeSet<String>,
            prerequisites: &mut BTreeSet<PackagePrerequisite>,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            if done.contains(key) {
                return;
            }
            let Some(bridge) = bridges.get(key) else {
                prerequisites.insert(PackagePrerequisite {
                    kind: "bridge".into(),
                    key: key.into(),
                });
                return;
            };
            if active.len() >= 64 || !active.insert(key.into()) {
                diagnostics.push(Diagnostic::new(
                    "package_bridge_cycle",
                    format!("bridges.{key}"),
                    "Bridge dependency cycle or excessive nesting",
                ));
                return;
            }
            for dependency in &bridge.requires {
                visit(
                    dependency,
                    bridges,
                    active,
                    done,
                    prerequisites,
                    diagnostics,
                );
            }
            active.remove(key);
            done.insert(key.into());
        }
        let mut done = BTreeSet::new();
        for (key, bridge) in &bridges {
            visit(
                key,
                &bridges,
                &mut BTreeSet::new(),
                &mut done,
                &mut prerequisites,
                &mut diagnostics,
            );
            for flow in &bridge.flows {
                prerequisites.insert(PackagePrerequisite {
                    kind: "flow".into(),
                    key: flow.clone(),
                });
            }
        }
    }
    diagnostics.truncate(64);
    PackageValidation {
        valid: diagnostics.is_empty(),
        prerequisites: prerequisites.into_iter().collect(),
        diagnostics,
    }
}
