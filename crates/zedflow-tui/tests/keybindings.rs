use std::collections::HashMap;
use zedflow_tui::{KeybindingsManager, matches_key, tui_keybindings};

#[test]
fn keybindings_match_decoded_input() {
    assert!(matches_key("\x1b[B", "down"));
    assert!(!matches_key("x", "down"));

    let mut user = HashMap::new();
    user.insert("tui.input.submit".into(), vec!["ctrl+x".into()]);
    user.insert("tui.select.confirm".into(), vec!["ctrl+x".into()]);
    let manager = KeybindingsManager::new(tui_keybindings(), user);
    assert!(manager.matches("\x18", "tui.input.submit"));
    assert_eq!(
        manager.get_keys("tui.input.newLine"),
        ["shift+enter", "ctrl+j"]
    );
    assert!(manager.matches("\n", "tui.input.newLine"));
    assert_eq!(manager.get_conflicts()[0].key, "ctrl+x");
    assert_eq!(
        manager.get_keys("tui.editor.cursorLeft"),
        ["left", "ctrl+b"]
    );
}
