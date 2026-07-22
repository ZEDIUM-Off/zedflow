use zedflow_agent::types::ThinkingLevel;
use zedflow_ai::types::Model;
use zedflow_coding_agent::model_resolver::{
    find_exact_model_reference_match, parse_model_pattern,
    resolve_available_model_scope_with_diagnostics,
};

fn model(provider: &str, id: &str, name: &str) -> Model {
    Model {
        provider: provider.into(),
        id: id.into(),
        name: name.into(),
        ..Model::default()
    }
}

#[test]
fn resolves_exact_patterns_colon_suffixes_globs_and_diagnostics() {
    let models = vec![
        model("anthropic", "claude-sonnet-4-5", "Claude Sonnet 4.5"),
        model(
            "anthropic",
            "claude-sonnet-4-5-20250929",
            "Claude Sonnet dated",
        ),
        model("openrouter", "qwen/qwen3-coder:exacto", "Qwen Exacto"),
        model("other", "claude-sonnet-4-5", "Duplicate ID"),
    ];

    assert!(find_exact_model_reference_match("claude-sonnet-4-5", &models).is_none());
    assert_eq!(
        find_exact_model_reference_match(" ANTHROPIC/claude-sonnet-4-5 ", &models)
            .unwrap()
            .provider,
        "anthropic"
    );

    let exact_colon = parse_model_pattern("qwen/qwen3-coder:exacto", &models, true);
    assert_eq!(exact_colon.model.unwrap().id, "qwen/qwen3-coder:exacto");
    assert_eq!(exact_colon.thinking_level, None);

    let parsed = parse_model_pattern("sonnet:high", &models, true);
    assert_eq!(parsed.model.unwrap().id, "claude-sonnet-4-5");
    assert_eq!(parsed.thinking_level, Some(ThinkingLevel::High));

    let result = resolve_available_model_scope_with_diagnostics(
        &["anthropic/*sonnet*:low".into(), "missing".into()],
        &models,
    );
    assert_eq!(result.scoped_models.len(), 2);
    assert!(
        result
            .scoped_models
            .iter()
            .all(|item| item.thinking_level == Some(ThinkingLevel::Low))
    );
    assert_eq!(
        result.diagnostics[0].message,
        "No models match pattern \"missing\""
    );
}

#[test]
fn glob_wildcards_do_not_match_path_separators() {
    let models = vec![
        model("anthropic", "claude-sonnet", "Sonnet"),
        model("openrouter", "qwen/qwen3-coder", "Qwen Coder"),
    ];

    let single = resolve_available_model_scope_with_diagnostics(&["*".into()], &models);
    assert_eq!(single.scoped_models.len(), 1);
    assert_eq!(single.scoped_models[0].model.id, "claude-sonnet");

    let recursive = resolve_available_model_scope_with_diagnostics(&["**".into()], &models);
    assert_eq!(recursive.scoped_models.len(), 2);
}

#[test]
fn invalid_thinking_suffix_is_warned_or_rejected_in_strict_mode() {
    let models = vec![model("anthropic", "claude-sonnet", "Sonnet")];
    let fallback = parse_model_pattern("sonnet:turbo", &models, true);
    assert!(fallback.model.is_some());
    assert!(fallback.warning.unwrap().contains("turbo"));

    let strict = parse_model_pattern("sonnet:turbo", &models, false);
    assert!(strict.model.is_none());
    assert!(strict.warning.is_none());
}
