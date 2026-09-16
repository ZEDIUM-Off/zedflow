//! Provider capabilities exposed to clients, without credentials or account data.
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::path::PathBuf;

pub async fn catalog(compositions: &[Value]) -> Value {
    let mut models = vec![
        json!({"provider":"fixture","id":"fixture","label":"Fixture · hors réseau","reasoningLevels":[]}),
    ];
    let cache = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
        .map(|home| home.join("models_cache.json"));
    if let Some(cache) = cache
        && let Ok(raw) = tokio::fs::read(cache).await
        && let Ok(document) = serde_json::from_slice::<Value>(&raw)
    {
        for model in document["models"].as_array().into_iter().flatten() {
            if model["visibility"] != "list" {
                continue;
            }
            if let Some(id) = model["slug"].as_str() {
                let levels: Vec<_> = model["supported_reasoning_levels"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|level| level["effort"].as_str())
                    .collect();
                models.push(json!({"provider":"codex","id":id,"label":model["display_name"].as_str().unwrap_or(id),"reasoningLevels":levels,"reasoningDefault":model["default_reasoning_level"]}));
            }
        }
    }
    fn collect(composition: &Value, models: &mut Vec<Value>) {
        for node in composition["nodes"].as_array().into_iter().flatten() {
            let config = &node["data"]["config"];
            if node["data"]["kind"] == "subgraph" {
                collect(&config["composition"], models);
            }
            if !matches!(node["data"]["kind"].as_str(), Some("agent" | "model")) {
                continue;
            }
            if let (Some(provider), Some(id)) =
                (config["provider"].as_str(), config["model"].as_str())
                && ["fixture", "codex", "gemini"].contains(&provider)
                && !id.trim().is_empty()
                && !models
                    .iter()
                    .any(|m| m["provider"] == provider && m["id"] == id)
            {
                models.push(json!({"provider":provider,"id":id,"label":id,"reasoningLevels":[]}));
            }
        }
    }
    for composition in compositions {
        collect(composition, &mut models);
    }
    json!({"models":models,"providers":[{"id":"fixture","label":"Démonstration"},{"id":"codex","label":"Abonnement Codex"},{"id":"gemini","label":"Gemini"}]})
}

pub fn validate_selection(value: &Value) -> Result<()> {
    let fields = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("La sélection du modèle doit être un objet"))?;
    for field in fields.keys() {
        ensure!(
            [
                "provider",
                "model",
                "reasoningEffort",
                "reasoningSummary",
                "textVerbosity"
            ]
            .contains(&field.as_str()),
            "Option de modèle non prise en charge : {field}"
        );
    }
    let provider = value["provider"].as_str().unwrap_or_default();
    ensure!(
        ["fixture", "codex", "gemini"].contains(&provider),
        "Fournisseur non disponible"
    );
    ensure!(
        value["model"]
            .as_str()
            .is_some_and(|id| !id.trim().is_empty()),
        "Un modèle est requis"
    );
    if provider == "codex" {
        crate::codex::validate(value)?;
    }
    if provider != "codex" {
        for field in ["reasoningEffort", "reasoningSummary", "textVerbosity"] {
            ensure!(
                value.get(field).is_none_or(|v| v.is_null()),
                "Option {field} non intégrée au fournisseur {provider}"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selections_reject_options_the_provider_cannot_apply() {
        assert!(validate_selection(&json!({"provider":"codex","model":"example","reasoningEffort":"high","textVerbosity":"low"})).is_ok());
        for provider in ["fixture", "gemini"] {
            assert!(validate_selection(&json!({"provider":provider,"model":"example"})).is_ok());
            for option in [
                "reasoningEffort",
                "reasoningSummary",
                "textVerbosity",
                "thinkingBudget",
            ] {
                let mut selection = json!({"provider":provider,"model":"example"});
                selection[option] = json!("high");
                assert!(
                    validate_selection(&selection).is_err(),
                    "accepted {option} for {provider}"
                );
            }
        }
        assert!(
            validate_selection(&json!({"provider":"codex","model":"example","tools":["exec"]}))
                .is_err()
        );
        assert!(
            validate_selection(&json!({"provider":"codex","model":"example","temperature":0.5}))
                .is_err()
        );
        assert!(
            validate_selection(
                &json!({"provider":"codex","model":"example","reasoningEffort":"invented"})
            )
            .is_err()
        );
        assert!(validate_selection(&json!({"provider":"unavailable","model":"example"})).is_err());
        assert!(validate_selection(&json!({"provider":"gemini","model":" "})).is_err());
    }
}
