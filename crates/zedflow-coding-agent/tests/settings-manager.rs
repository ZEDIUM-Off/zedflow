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
