//! Validation of frozen context artifacts, shared by daemon and Cargo exports.
use super::{context, context_source};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sha2::Digest;

fn diagnostics(errors: Vec<zf_core::diagnostics::Diagnostic>) -> anyhow::Error {
    anyhow::anyhow!(
        "{}",
        errors
            .into_iter()
            .map(|e| format!("{}: {} [{}]", e.path, e.message, e.code))
            .collect::<Vec<_>>()
            .join("; ")
    )
}

pub fn validate_frozen(program: &Value) -> Result<()> {
    let source = program["source"]
        .as_str()
        .context("Frozen context source is required")?;
    ensure!(
        program["hash"].as_str()
            == Some(format!("{:x}", sha2::Sha256::digest(source.as_bytes())).as_str()),
        "Frozen context source hash mismatch"
    );
    let parsed = context_source::parse(source).map_err(diagnostics)?;
    ensure!(
        serde_json::to_value(&parsed)? == program["strategy"],
        "Frozen context source and strategy disagree"
    );
    let types = serde_json::from_value(program.get("types").cloned().unwrap_or(json!({})))?;
    let library = serde_json::from_value(program.get("library").cloned().unwrap_or(json!({})))?;
    if let Some(sources) = program
        .get("typeSources")
        .and_then(Value::as_array)
        .filter(|v| !v.is_empty())
    {
        let mut restored = zf_core::types::TypeRegistry::new();
        for file in sources {
            let source = file["source"].as_str().context("Type source missing")?;
            ensure!(
                file["hash"].as_str()
                    == Some(format!("{:x}", sha2::Sha256::digest(source.as_bytes())).as_str()),
                "Type source hash mismatch"
            );
            let decoded = context_source::parse_types(source).map_err(diagnostics)?;
            for (name, ty) in decoded {
                ensure!(
                    restored.insert(name, ty).is_none(),
                    "Duplicate named type in frozen sources"
                );
            }
        }
        ensure!(
            context::resolved_types(&parsed, &restored).map_err(diagnostics)? == types,
            "Frozen type sources and registry disagree"
        );
    }
    if let Some(sources) = program
        .get("librarySources")
        .and_then(Value::as_array)
        .filter(|v| !v.is_empty())
    {
        let mut restored = context::ContextLibrary::default();
        for file in sources {
            let source = file["source"].as_str().context("Library source missing")?;
            ensure!(
                file["hash"].as_str()
                    == Some(format!("{:x}", sha2::Sha256::digest(source.as_bytes())).as_str()),
                "Library source hash mismatch"
            );
            let decoded = context_source::parse_library(source).map_err(diagnostics)?;
            for (name, function) in decoded.projections {
                ensure!(
                    restored.projections.insert(name, function).is_none(),
                    "Duplicate projection in frozen libraries"
                );
            }
            for (name, function) in decoded.subprograms {
                ensure!(
                    restored.subprograms.insert(name, function).is_none(),
                    "Duplicate subprogram in frozen libraries"
                );
            }
        }
        ensure!(
            restored == library,
            "Frozen library sources and functions disagree"
        );
    }
    context::validate_strategy_with_library(&parsed, &types, &library).map_err(diagnostics)
}
