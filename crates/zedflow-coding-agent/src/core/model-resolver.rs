use globset::GlobBuilder;
use zedflow_agent::types::ThinkingLevel;
use zedflow_ai::types::Model;

use crate::model_registry::ModelRegistry;

#[derive(Debug, Clone, PartialEq)]
pub struct ScopedModel {
    pub model: Model,
    pub thinking_level: Option<ThinkingLevel>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedModelResult {
    pub model: Option<Model>,
    pub thinking_level: Option<ThinkingLevel>,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelScopeDiagnostic {
    pub message: String,
    pub pattern: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolveModelScopeResult {
    pub scoped_models: Vec<ScopedModel>,
    pub diagnostics: Vec<ModelScopeDiagnostic>,
}

fn is_alias(id: &str) -> bool {
    if id.ends_with("-latest") {
        return true;
    }
    let suffix = id.rsplit_once('-').map(|(_, suffix)| suffix);
    !matches!(suffix, Some(s) if s.len() == 8 && s.bytes().all(|b| b.is_ascii_digit()))
}

pub fn find_exact_model_reference_match(
    model_reference: &str,
    available_models: &[Model],
) -> Option<Model> {
    let reference = model_reference.trim();
    if reference.is_empty() {
        return None;
    }

    let canonical: Vec<_> = available_models
        .iter()
        .filter(|model| format!("{}/{}", model.provider, model.id).eq_ignore_ascii_case(reference))
        .collect();
    if canonical.len() == 1 {
        return Some(canonical[0].clone());
    }
    if canonical.len() > 1 {
        return None;
    }

    if let Some((provider, id)) = reference.split_once('/') {
        let provider = provider.trim();
        let id = id.trim();
        if !provider.is_empty() && !id.is_empty() {
            let matches: Vec<_> = available_models
                .iter()
                .filter(|model| {
                    model.provider.eq_ignore_ascii_case(provider)
                        && model.id.eq_ignore_ascii_case(id)
                })
                .collect();
            if matches.len() == 1 {
                return Some(matches[0].clone());
            }
            if matches.len() > 1 {
                return None;
            }
        }
    }

    let matches: Vec<_> = available_models
        .iter()
        .filter(|model| model.id.eq_ignore_ascii_case(reference))
        .collect();
    (matches.len() == 1).then(|| matches[0].clone())
}

fn try_match_model(pattern: &str, available_models: &[Model]) -> Option<Model> {
    if let Some(model) = find_exact_model_reference_match(pattern, available_models) {
        return Some(model);
    }
    let pattern = pattern.to_lowercase();
    let mut matches: Vec<_> = available_models
        .iter()
        .filter(|model| {
            model.id.to_lowercase().contains(&pattern)
                || model.name.to_lowercase().contains(&pattern)
        })
        .collect();
    matches.sort_by(|a, b| b.id.cmp(&a.id));
    matches
        .iter()
        .find(|model| is_alias(&model.id))
        .or_else(|| matches.first())
        .map(|model| (*model).clone())
}

fn parse_thinking_level(value: &str) -> Option<ThinkingLevel> {
    match value {
        "off" => Some(ThinkingLevel::Off),
        "minimal" => Some(ThinkingLevel::Minimal),
        "low" => Some(ThinkingLevel::Low),
        "medium" => Some(ThinkingLevel::Medium),
        "high" => Some(ThinkingLevel::High),
        "xhigh" => Some(ThinkingLevel::XHigh),
        _ => None,
    }
}

pub fn parse_model_pattern(
    pattern: &str,
    available_models: &[Model],
    allow_invalid_thinking_level_fallback: bool,
) -> ParsedModelResult {
    if let Some(model) = try_match_model(pattern, available_models) {
        return ParsedModelResult {
            model: Some(model),
            thinking_level: None,
            warning: None,
        };
    }

    let Some((prefix, suffix)) = pattern.rsplit_once(':') else {
        return ParsedModelResult {
            model: None,
            thinking_level: None,
            warning: None,
        };
    };

    if let Some(level) = parse_thinking_level(suffix) {
        let mut result = parse_model_pattern(
            prefix,
            available_models,
            allow_invalid_thinking_level_fallback,
        );
        if result.model.is_some() && result.warning.is_none() {
            result.thinking_level = Some(level);
        }
        return result;
    }
    if !allow_invalid_thinking_level_fallback {
        return ParsedModelResult {
            model: None,
            thinking_level: None,
            warning: None,
        };
    }

    let mut result = parse_model_pattern(
        prefix,
        available_models,
        allow_invalid_thinking_level_fallback,
    );
    if result.model.is_some() {
        result.thinking_level = None;
        result.warning = Some(format!(
            "Invalid thinking level \"{suffix}\" in pattern \"{pattern}\". Using default instead."
        ));
    }
    result
}

fn same_model(a: &Model, b: &Model) -> bool {
    a.id == b.id && a.provider == b.provider
}

pub fn resolve_model_scope_with_diagnostics(
    patterns: &[String],
    model_registry: &ModelRegistry,
) -> ResolveModelScopeResult {
    let available_models: Vec<_> = model_registry
        .get_available()
        .into_iter()
        .cloned()
        .collect();
    resolve_available_model_scope_with_diagnostics(patterns, &available_models)
}

pub fn resolve_available_model_scope_with_diagnostics(
    patterns: &[String],
    available_models: &[Model],
) -> ResolveModelScopeResult {
    let mut scoped_models: Vec<ScopedModel> = Vec::new();
    let mut diagnostics = Vec::new();

    for pattern in patterns {
        if pattern.contains(['*', '?', '[']) {
            let (glob_pattern, thinking_level) = match pattern.rsplit_once(':') {
                Some((prefix, suffix)) if parse_thinking_level(suffix).is_some() => {
                    (prefix, parse_thinking_level(suffix))
                }
                _ => (pattern.as_str(), None),
            };
            let matcher = GlobBuilder::new(glob_pattern)
                .case_insensitive(true)
                .literal_separator(true)
                .build()
                .ok()
                .map(|glob| glob.compile_matcher());
            let matching: Vec<_> = available_models
                .iter()
                .filter(|model| {
                    matcher.as_ref().is_some_and(|matcher| {
                        matcher.is_match(format!("{}/{}", model.provider, model.id))
                            || matcher.is_match(&model.id)
                    })
                })
                .collect();
            if matching.is_empty() {
                diagnostics.push(ModelScopeDiagnostic {
                    message: format!("No models match pattern \"{pattern}\""),
                    pattern: pattern.clone(),
                });
                continue;
            }
            for model in matching {
                if !scoped_models
                    .iter()
                    .any(|item| same_model(&item.model, model))
                {
                    scoped_models.push(ScopedModel {
                        model: model.clone(),
                        thinking_level,
                    });
                }
            }
            continue;
        }

        let result = parse_model_pattern(pattern, available_models, true);
        if let Some(message) = result.warning {
            diagnostics.push(ModelScopeDiagnostic {
                message,
                pattern: pattern.clone(),
            });
        }
        match result.model {
            Some(model)
                if !scoped_models
                    .iter()
                    .any(|item| same_model(&item.model, &model)) =>
            {
                scoped_models.push(ScopedModel {
                    model,
                    thinking_level: result.thinking_level,
                });
            }
            Some(_) => {}
            None => diagnostics.push(ModelScopeDiagnostic {
                message: format!("No models match pattern \"{pattern}\""),
                pattern: pattern.clone(),
            }),
        }
    }

    ResolveModelScopeResult {
        scoped_models,
        diagnostics,
    }
}

pub fn resolve_model_scope(
    patterns: &[String],
    model_registry: &ModelRegistry,
) -> Vec<ScopedModel> {
    let result = resolve_model_scope_with_diagnostics(patterns, model_registry);
    for diagnostic in &result.diagnostics {
        eprintln!("Warning: {}", diagnostic.message);
    }
    result.scoped_models
}
