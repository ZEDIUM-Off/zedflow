//! Pure checks for trial context. Provider config, request adaptation and raw
//! serialization remain in runtime; this view never replaces its full profile.
use crate::{
    context::{ContextEvaluation, ContextStrategy},
    context_library::ContextLibrary,
    resources::ContextProgram,
};
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::collections::BTreeMap;
use zf_core::types::TypeRegistry;

pub struct PreviewTarget<'a> {
    pub provider: &'a str,
    pub model: &'a str,
    pub tools: &'a BTreeMap<String, Value>,
}

pub fn validate(
    profile: &PreviewTarget<'_>,
    program: &ContextProgram,
    evaluation: &ContextEvaluation,
) -> Result<()> {
    ensure!(
        evaluation.complete && evaluation.diagnostics.is_empty() && evaluation.needs.is_empty(),
        "Le contexte est incomplet : fournissez les données d’essai requises avant de préparer la requête"
    );
    ensure!(
        matches!(profile.provider, "codex" | "gemini" | "fixture"),
        "Choisissez une frontière prise en charge : Codex, Gemini ou fixture"
    );
    ensure!(
        !profile.model.trim().is_empty(),
        "Le modèle d’aperçu doit être précisé"
    );
    ensure!(
        program.strategy.version >= 2,
        "L’aperçu autonome exige une stratégie v2 : une stratégie historique peut dépendre d’un encodage déclaré par son flow"
    );
    for capability in &evaluation.capabilities {
        let declaration = profile
            .tools
            .get(&capability.id)
            .with_context(|| format!("Déclaration d’outil absente : {}", capability.id))?;
        ensure!(
            declaration["description"].is_string() && declaration["parameters"].is_object(),
            "L’outil {} exige une description et un schéma parameters explicites",
            capability.id
        );
    }
    Ok(())
}

pub fn program(
    strategy: ContextStrategy,
    source: String,
    hash: String,
    types: TypeRegistry,
    library: ContextLibrary,
) -> ContextProgram {
    ContextProgram {
        strategy,
        source,
        hash,
        types,
        library,
        bindings: BTreeMap::new(),
        library_sources: vec![],
        type_sources: vec![],
        window: None,
    }
}
