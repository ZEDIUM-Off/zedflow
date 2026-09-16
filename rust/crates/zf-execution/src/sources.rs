//! Acquire authenticated source bytes for the pure compiler. Catalogue paths and
//! accepted ancestry stay on the host side; linking stays in `zf-compiler`.
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeSet, path::Path};
use zf_compiler::programs::{self, ProgramSources, SourceKind, SourceOverride, SourceSnapshot};
use zf_flows::schema::Composition;
use zf_storage::{
    context_store::{Conflict, SourceStore},
    source_acceptance::FilePrecondition,
};

fn segments(kind: SourceKind) -> &'static [&'static str] {
    match kind {
        SourceKind::Strategy => &["context"],
        SourceKind::Library => &["context", "libraries"],
        SourceKind::Types => &["types"],
    }
}

/// Capture only the references which linking will consume. Each requested pin
/// is authenticated by storage against its accepted head, including ancestors.
/// Multiple references to one source must observe identical bytes. The caller
/// serializes authoring and rechecks captured conditions before publication.
pub async fn program_sources(
    doc: &Composition,
    workspace: &Path,
    overrides: &[SourceOverride],
) -> Result<ProgramSources> {
    // Libraries/types are inactive without a strategy. Preserve that historical
    // boundary while letting the compiler own reference syntax and traversal.
    fn active_references(doc: &mut Composition) -> Result<()> {
        for node in &mut doc.nodes {
            if node.data.kind == "subgraph" {
                let mut child = serde_json::from_value(node.data.config["composition"].clone())?;
                active_references(&mut child)?;
                node.data.config["composition"] = serde_json::to_value(child)?;
            }
            if matches!(node.data.kind.as_str(), "agent" | "context")
                && node
                    .data
                    .config
                    .get("contextStrategy")
                    .is_none_or(|v| v.is_null())
                && let Some(config) = node.data.config.as_object_mut()
            {
                config.remove("contextLibraryRef");
                config.remove("contextTypesRef");
            }
        }
        Ok(())
    }
    let mut references = doc.clone();
    active_references(&mut references)?;
    let mut result = ProgramSources::default();
    for reference in programs::references(&references)? {
        let candidate = overrides
            .iter()
            .find(|candidate| candidate.kind == reference.kind && candidate.key == reference.key);
        // Creation has no previous accepted head. The pure compiler checks the
        // proposed bytes and any reference pin against the override itself.
        if candidate.is_some_and(|candidate| candidate.expected_hash.is_none()) {
            continue;
        }
        let store = SourceStore::new(workspace.into(), segments(reference.kind))?;
        let file = if let Some(expected) = candidate.and_then(|c| c.expected_hash.as_deref()) {
            store
                .preflight(&reference.key, reference.hash.as_deref(), expected)
                .await?
        } else {
            store
                .resolve(&reference.key, reference.hash.as_deref())
                .await?
        };
        let source = file.source.context("Resolved source bytes absent")?;
        let captured = SourceSnapshot {
            source,
            hash: file.hash,
            accepted_references: reference.hash.into_iter().collect::<BTreeSet<_>>(),
        };
        captured.validate()?;
        let catalog = match reference.kind {
            SourceKind::Strategy => &mut result.strategies,
            SourceKind::Library => &mut result.libraries,
            SourceKind::Types => &mut result.types,
        };
        if let Some(previous) = catalog.get_mut(&reference.key) {
            ensure!(
                previous.hash == captured.hash && previous.source == captured.source,
                Conflict("A source changed while capturing affected definitions")
            );
            previous
                .accepted_references
                .extend(captured.accepted_references);
        } else {
            catalog.insert(reference.key, captured);
        }
    }
    Ok(result)
}

/// Locate the compiler's exact captured dependencies for storage's optimistic
/// acceptance transaction. No source is re-read or semantically reinterpreted.
pub fn captured_sources(doc: &Composition, workspace: &Path) -> Result<Vec<FilePrecondition>> {
    programs::captured_sources(doc)?
        .into_iter()
        .map(|source| {
            let directory = segments(source.kind)
                .iter()
                .fold(workspace.join(".zedflow"), |directory, segment| {
                    directory.join(segment)
                });
            Ok(FilePrecondition {
                path: directory.join(format!("{}.rs", source.key)),
                hash: source.hash,
            })
        })
        .collect()
}
