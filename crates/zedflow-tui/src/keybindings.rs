use std::{
    collections::{HashMap, HashSet},
    sync::{Mutex, OnceLock},
};

pub type KeybindingDefinitions = HashMap<String, KeybindingDefinition>;
pub type KeybindingsConfig = HashMap<String, Vec<String>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeybindingDefinition {
    pub default_keys: Vec<String>,
    pub description: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeybindingConflict {
    pub key: String,
    pub keybindings: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct KeybindingsManager {
    definitions: KeybindingDefinitions,
    user_bindings: KeybindingsConfig,
    keys_by_id: KeybindingsConfig,
    conflicts: Vec<KeybindingConflict>,
}

impl KeybindingsManager {
    pub fn new(definitions: KeybindingDefinitions, user_bindings: KeybindingsConfig) -> Self {
        let mut manager = Self {
            definitions,
            user_bindings,
            keys_by_id: HashMap::new(),
            conflicts: Vec::new(),
        };
        manager.rebuild();
        manager
    }
    fn rebuild(&mut self) {
        self.conflicts.clear();
        let mut claims: HashMap<&str, Vec<&str>> = HashMap::new();
        for (binding, keys) in &self.user_bindings {
            if self.definitions.contains_key(binding) {
                for key in keys {
                    claims.entry(key).or_default().push(binding);
                }
            }
        }
        for (key, bindings) in claims {
            let unique: HashSet<_> = bindings.iter().copied().collect();
            if unique.len() > 1 {
                self.conflicts.push(KeybindingConflict {
                    key: key.into(),
                    keybindings: bindings.into_iter().map(str::to_owned).collect(),
                });
            }
        }
        self.keys_by_id = self
            .definitions
            .iter()
            .map(|(id, definition)| {
                let keys = self
                    .user_bindings
                    .get(id)
                    .unwrap_or(&definition.default_keys);
                let mut seen = HashSet::new();
                (
                    id.clone(),
                    keys.iter()
                        .filter(|key| seen.insert((*key).clone()))
                        .cloned()
                        .collect(),
                )
            })
            .collect();
    }
    pub fn matches(&self, data: &str, keybinding: &str) -> bool {
        self.keys_by_id
            .get(keybinding)
            .is_some_and(|keys| keys.iter().any(|key| crate::matches_key(data, key)))
    }
    pub fn get_keys(&self, keybinding: &str) -> Vec<String> {
        self.keys_by_id.get(keybinding).cloned().unwrap_or_default()
    }
    pub fn get_definition(&self, keybinding: &str) -> Option<&KeybindingDefinition> {
        self.definitions.get(keybinding)
    }
    pub fn get_conflicts(&self) -> Vec<KeybindingConflict> {
        self.conflicts.clone()
    }
    pub fn set_user_bindings(&mut self, bindings: KeybindingsConfig) {
        self.user_bindings = bindings;
        self.rebuild();
    }
    pub fn get_user_bindings(&self) -> KeybindingsConfig {
        self.user_bindings.clone()
    }
    pub fn get_resolved_bindings(&self) -> KeybindingsConfig {
        self.keys_by_id.clone()
    }
}

fn definition(keys: &[&str], description: &str) -> KeybindingDefinition {
    KeybindingDefinition {
        default_keys: keys.iter().map(|key| (*key).into()).collect(),
        description: Some(description.into()),
    }
}

pub fn tui_keybindings() -> KeybindingDefinitions {
    [
        ("tui.editor.cursorUp", &["up"][..], "Move cursor up"),
        ("tui.editor.cursorDown", &["down"], "Move cursor down"),
        (
            "tui.editor.cursorLeft",
            &["left", "ctrl+b"],
            "Move cursor left",
        ),
        (
            "tui.editor.cursorRight",
            &["right", "ctrl+f"],
            "Move cursor right",
        ),
        (
            "tui.editor.cursorWordLeft",
            &["alt+left", "ctrl+left", "alt+b"],
            "Move cursor word left",
        ),
        (
            "tui.editor.cursorWordRight",
            &["alt+right", "ctrl+right", "alt+f"],
            "Move cursor word right",
        ),
        (
            "tui.editor.cursorLineStart",
            &["home", "ctrl+a"],
            "Move to line start",
        ),
        (
            "tui.editor.cursorLineEnd",
            &["end", "ctrl+e"],
            "Move to line end",
        ),
        (
            "tui.editor.jumpForward",
            &["ctrl+]"],
            "Jump forward to character",
        ),
        (
            "tui.editor.jumpBackward",
            &["ctrl+alt+]"],
            "Jump backward to character",
        ),
        ("tui.editor.pageUp", &["pageUp"], "Page up"),
        ("tui.editor.pageDown", &["pageDown"], "Page down"),
        (
            "tui.editor.deleteCharBackward",
            &["backspace"],
            "Delete character backward",
        ),
        (
            "tui.editor.deleteCharForward",
            &["delete", "ctrl+d"],
            "Delete character forward",
        ),
        (
            "tui.editor.deleteWordBackward",
            &["ctrl+w", "alt+backspace"],
            "Delete word backward",
        ),
        (
            "tui.editor.deleteWordForward",
            &["alt+d", "alt+delete"],
            "Delete word forward",
        ),
        (
            "tui.editor.deleteToLineStart",
            &["ctrl+u"],
            "Delete to line start",
        ),
        (
            "tui.editor.deleteToLineEnd",
            &["ctrl+k"],
            "Delete to line end",
        ),
        ("tui.editor.yank", &["ctrl+y"], "Yank"),
        ("tui.editor.yankPop", &["alt+y"], "Yank pop"),
        ("tui.editor.undo", &["ctrl+-"], "Undo"),
        (
            "tui.input.newLine",
            &["shift+enter", "ctrl+j"],
            "Insert newline",
        ),
        ("tui.input.submit", &["enter"], "Submit input"),
        ("tui.input.tab", &["tab"], "Tab / autocomplete"),
        ("tui.input.copy", &["ctrl+c"], "Copy selection"),
        ("tui.select.up", &["up"], "Move selection up"),
        ("tui.select.down", &["down"], "Move selection down"),
        ("tui.select.pageUp", &["pageUp"], "Selection page up"),
        ("tui.select.pageDown", &["pageDown"], "Selection page down"),
        ("tui.select.confirm", &["enter"], "Confirm selection"),
        (
            "tui.select.cancel",
            &["escape", "ctrl+c"],
            "Cancel selection",
        ),
    ]
    .into_iter()
    .map(|(id, keys, description)| (id.into(), definition(keys, description)))
    .collect()
}

static GLOBAL: OnceLock<Mutex<KeybindingsManager>> = OnceLock::new();
pub fn get_keybindings() -> &'static Mutex<KeybindingsManager> {
    GLOBAL.get_or_init(|| Mutex::new(KeybindingsManager::new(tui_keybindings(), HashMap::new())))
}
pub fn set_keybindings(keybindings: KeybindingsManager) {
    *get_keybindings().lock().unwrap() = keybindings;
}
