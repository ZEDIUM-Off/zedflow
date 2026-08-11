use std::path::PathBuf;
use zedflow_coding_agent::{
    first_time_setup::{FirstTimeSetup, FirstTimeSetupAction, TerminalTheme},
    model_search::{ModelSearchItem, model_search_text, model_selector_search_text},
    model_selector::{ModelItem, ModelScope, ModelSelector},
    modes_interactive_components_index::config_selector::{
        ConfigAction, ConfigScope, ConfigSelector,
    },
    package_manager::{PathMetadata, ResolvedPaths, ResolvedResource, ResourceOrigin, SourceScope},
    scoped_models_selector::{ScopedModel, ScopedModelsSelector},
    settings_selector::{SettingChoice, SettingsAction, SettingsSelector},
    show_images_selector::ShowImagesSelector,
    theme_selector::{ThemeAction, ThemeSelector},
    thinking_selector::{ThinkingLevel, ThinkingSelector},
};

#[test]
fn model_search_and_scope_preserve_pi_ranking_and_wrap() {
    let item = ModelSearchItem {
        id: "openai/gpt-5",
        provider: "openrouter",
        name: Some("GPT 5"),
    };
    assert!(model_search_text(item).starts_with("openai/gpt-5 openrouter"));
    assert!(model_selector_search_text(item).starts_with("openrouter openrouter/openai/gpt-5"));

    let models = vec![
        ModelItem {
            provider: "openrouter".into(),
            id: "openai/gpt-5".into(),
            name: "Proxy".into(),
        },
        ModelItem {
            provider: "openai".into(),
            id: "gpt-5".into(),
            name: "GPT 5".into(),
        },
    ];
    let mut selector = ModelSelector::new(models, &["openai/gpt-5".into()], None);
    assert_eq!(selector.scope, ModelScope::Scoped);
    selector.toggle_scope("");
    selector.filter("openai/gpt-5");
    assert_eq!(selector.selected_model().unwrap().provider, "openai");
    selector.move_selection(-1);
    assert_eq!(selector.selected, selector.filtered_count() - 1);
}

#[test]
fn scoped_models_toggle_order_and_clear() {
    let mut selector = ScopedModelsSelector::new(
        vec![
            ScopedModel {
                full_id: "a/one".into(),
                name: "One".into(),
            },
            ScopedModel {
                full_id: "b/two".into(),
                name: "Two".into(),
            },
        ],
        None,
    );
    selector.toggle_selected();
    assert_eq!(selector.enabled_ids().unwrap(), ["a/one"]);
    selector.clear_all();
    assert_eq!(selector.enabled_ids().unwrap(), &[] as &[String]);
    selector.enable_all();
    assert!(selector.enabled_ids().is_none());
}

#[test]
fn config_selector_groups_and_toggles_resources() {
    let mut paths = ResolvedPaths::default();
    paths.skills.push(ResolvedResource {
        path: PathBuf::from("/project/.pi/skills/demo/SKILL.md"),
        enabled: true,
        metadata: PathMetadata {
            source: "auto".into(),
            scope: SourceScope::Project,
            origin: ResourceOrigin::TopLevel,
            base_dir: None,
        },
    });
    let mut selector = ConfigSelector::new(&paths);
    assert_eq!(
        selector.toggle_selected(),
        Some(ConfigAction::Toggle {
            scope: ConfigScope::Project,
            path: PathBuf::from("/project/.pi/skills/demo/SKILL.md"),
            enabled: false,
        })
    );
}

#[test]
fn settings_and_small_selectors_emit_deterministic_actions() {
    let mut settings = SettingsSelector::new(vec![SettingChoice {
        id: "autoCompact".into(),
        label: "Auto compact".into(),
        description: "Compact context".into(),
        value: "yes".into(),
        values: vec!["yes".into(), "no".into()],
    }]);
    assert_eq!(
        settings.activate(),
        Some(SettingsAction::Change {
            id: "autoCompact".into(),
            value: "no".into()
        })
    );
    assert_eq!(settings.cancel(), SettingsAction::Cancel);

    let mut images = ShowImagesSelector::new(false);
    images.move_selection(-1);
    assert!(images.selected_value());

    let thinking = ThinkingSelector::new(
        ThinkingLevel::High,
        vec![ThinkingLevel::Off, ThinkingLevel::High],
    );
    assert_eq!(thinking.selected(), Some(ThinkingLevel::High));

    let mut themes = ThemeSelector::new("dark", vec!["dark".into(), "light".into()]);
    assert_eq!(
        themes.move_selection(1),
        Some(ThemeAction::Preview("light".into()))
    );
    assert_eq!(themes.confirm(), Some(ThemeAction::Select("light".into())));
}

#[test]
fn first_time_setup_previews_submits_and_cancels() {
    let mut setup = FirstTimeSetup::new(TerminalTheme::Light);
    assert_eq!(
        setup.move_selection(-1),
        Some(FirstTimeSetupAction::Preview(TerminalTheme::Dark))
    );
    assert_eq!(setup.action_confirm(), FirstTimeSetupAction::Continue);
    setup.move_selection(1);
    assert!(!setup.confirm().unwrap().share_analytics);
    assert_eq!(setup.cancel(), FirstTimeSetupAction::Cancel);
}
