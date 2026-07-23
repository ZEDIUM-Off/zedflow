use zedflow_ai::Transport;
use zedflow_coding_agent::core::{
    session_manager::SessionInfo,
    settings_manager::{CompactionSettings, Settings, SettingsManager},
};

#[test]
fn project_settings_override_global_values_and_keep_defaults() {
    let manager = SettingsManager::with_settings(
        Settings {
            transport: Some(Transport::Sse),
            compaction: Some(CompactionSettings {
                enabled: Some(false),
                ..Default::default()
            }),
            ..Default::default()
        },
        Settings {
            transport: Some(Transport::Websocket),
            compaction: Some(CompactionSettings {
                reserve_tokens: Some(8_000),
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    assert_eq!(manager.get_transport(), Transport::Websocket);
    assert_eq!(manager.get_compaction_settings(), (false, 8_000, 20_000));
}

#[test]
fn session_info_distinguishes_memory_from_persisted_sessions() {
    assert!(!SessionInfo::in_memory("/tmp", "id").is_persisted());
    assert!(SessionInfo::persisted("/tmp", "/tmp/session.jsonl", "id").is_persisted());
}
