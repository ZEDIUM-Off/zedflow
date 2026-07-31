use std::fs;

use zedflow_coding_agent::settings_manager::SettingsManager;

#[test]
fn changing_one_setting_preserves_external_unknown_settings() {
    let root = std::env::temp_dir().join(format!("zedflow-settings-bug-{}", std::process::id()));
    let global = root.join("settings.json");
    fs::create_dir_all(&root).unwrap();
    fs::write(&global, r#"{"theme":"dark","packages":["old"]}"#).unwrap();
    let manager = SettingsManager::from_paths(&global, root.join("project.json"));

    fs::write(&global, r#"{"theme":"dark","packages":[]}"#).unwrap();
    manager.set_retry_enabled(false).unwrap();

    let saved: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&global).unwrap()).unwrap();
    assert_eq!(saved["packages"], serde_json::json!([]));
    assert_eq!(saved["retry"]["enabled"], false);
    fs::remove_dir_all(root).unwrap();
}
