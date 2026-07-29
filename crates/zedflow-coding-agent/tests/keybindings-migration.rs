use serde_json::json;
use std::collections::BTreeMap;
use zedflow_coding_agent::keybindings::migrate_keybindings_config;
#[test]
fn legacy_keybinding_names_are_migrated() {
    let mut raw = BTreeMap::new();
    raw.insert("cursorUp".into(), json!("up"));
    let (migrated, changed) = migrate_keybindings_config(&raw);
    assert!(changed);
    assert_eq!(migrated.get("tui.editor.cursorUp"), Some(&json!("up")));
}
