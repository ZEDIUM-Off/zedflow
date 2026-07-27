use std::collections::HashMap;
pub type KeybindingDefinitions = HashMap<String, String>;
pub fn get_keybindings() -> KeybindingDefinitions {
    HashMap::new()
}
pub fn set_keybindings(_: KeybindingDefinitions) {}
pub struct KeybindingsManager;
