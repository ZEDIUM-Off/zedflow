use std::fs;

use zedflow_coding_agent::settings_manager::{CompactionSettings, Settings, SettingsManager};

#[test]
fn project_settings_override_global_and_nested_settings_merge() {
    let manager = SettingsManager::with_settings(
        Settings {
            theme: Some("dark".into()),
            compaction: Some(CompactionSettings {
                enabled: Some(true),
                reserve_tokens: Some(4),
                keep_recent_tokens: None,
            }),
            ..Default::default()
        },
        Settings {
            theme: Some("light".into()),
            compaction: Some(CompactionSettings {
                enabled: None,
                reserve_tokens: None,
                keep_recent_tokens: Some(9),
            }),
            ..Default::default()
        },
    );

    assert_eq!(manager.settings().theme.as_deref(), Some("light"));
    assert_eq!(manager.get_compaction_settings(), (true, 4, 9));
}

#[test]
fn selector_updates_persist_and_untrusted_project_writes_fail() {
    let root =
        std::env::temp_dir().join(format!("zedflow-settings-selector-{}", std::process::id()));
    let global = root.join("settings.json");
    let project = root.join("project.json");
    let manager = SettingsManager::from_paths(&global, &project);

    manager.set_theme("light").unwrap();
    manager.set_show_images(false).unwrap();
    manager
        .set_project_theme_paths(vec!["theme.json".into()])
        .unwrap();
    let reopened = SettingsManager::from_paths(&global, &project);
    assert_eq!(reopened.get_theme_setting().as_deref(), Some("light"));
    assert!(!reopened.get_show_images());
    assert_eq!(reopened.project_settings().themes.unwrap(), ["theme.json"]);

    reopened.set_project_trusted(false);
    assert!(reopened.set_project_theme_paths(Vec::new()).is_err());
    assert!(reopened.set_http_idle_timeout_ms(f64::NAN).is_err());
    fs::remove_dir_all(root).unwrap();
}
