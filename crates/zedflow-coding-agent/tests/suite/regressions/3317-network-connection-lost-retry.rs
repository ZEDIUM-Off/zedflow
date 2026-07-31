use zedflow_coding_agent::settings_manager::{CompactionSettings, Settings, SettingsManager};

#[test]
fn project_settings_merge_nested_compaction_values() {
    let manager = SettingsManager::with_settings(
        Settings {
            compaction: Some(CompactionSettings {
                enabled: Some(true),
                reserve_tokens: Some(4),
                keep_recent_tokens: None,
            }),
            ..Default::default()
        },
        Settings {
            compaction: Some(CompactionSettings {
                enabled: None,
                reserve_tokens: None,
                keep_recent_tokens: Some(9),
            }),
            ..Default::default()
        },
    );
    assert_eq!(manager.get_compaction_settings(), (true, 4, 9));
}
