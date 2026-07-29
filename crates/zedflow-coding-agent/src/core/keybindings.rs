//! Pi-compatible coding-agent keybindings and legacy configuration migration.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;
use zedflow_tui::{
    KeybindingDefinition, KeybindingDefinitions, KeybindingsConfig,
    KeybindingsManager as TuiKeybindingsManager, tui_keybindings,
};

pub const MODULE_PATH: &str = "core/keybindings.rs";

pub type Keybinding = String;
pub type KeyId = String;
pub type RawKeybindingsConfig = BTreeMap<String, Value>;

fn definition(keys: &[&str], description: &str) -> KeybindingDefinition {
    KeybindingDefinition {
        default_keys: keys.iter().map(|key| (*key).into()).collect(),
        description: Some(description.into()),
    }
}

/// The complete coding-agent additions to the TUI keybindings.
#[must_use]
pub fn keybindings() -> KeybindingDefinitions {
    let mut bindings = tui_keybindings();
    let suspend = if cfg!(windows) { &[][..] } else { &["ctrl+z"] };
    for (id, keys, description) in [
        ("app.interrupt", &["escape"][..], "Cancel or abort"),
        ("app.clear", &["ctrl+c"], "Clear editor"),
        ("app.exit", &["ctrl+d"], "Exit when editor is empty"),
        ("app.suspend", suspend, "Suspend to background"),
        ("app.thinking.cycle", &["shift+tab"], "Cycle thinking level"),
        ("app.model.cycleForward", &["ctrl+p"], "Cycle to next model"),
        (
            "app.model.cycleBackward",
            &["shift+ctrl+p"],
            "Cycle to previous model",
        ),
        ("app.model.select", &["ctrl+l"], "Open model selector"),
        ("app.tools.expand", &["ctrl+o"], "Toggle tool output"),
        ("app.thinking.toggle", &["ctrl+t"], "Toggle thinking blocks"),
        (
            "app.session.toggleNamedFilter",
            &["ctrl+n"],
            "Toggle named session filter",
        ),
        ("app.editor.external", &["ctrl+g"], "Open external editor"),
        (
            "app.message.followUp",
            &["alt+enter"],
            "Queue follow-up message",
        ),
        (
            "app.message.dequeue",
            &["alt+up"],
            "Restore queued messages",
        ),
        (
            "app.clipboard.pasteImage",
            if cfg!(windows) {
                &["alt+v"]
            } else {
                &["ctrl+v"]
            },
            "Paste image from clipboard",
        ),
        ("app.session.new", &[], "Start a new session"),
        ("app.session.tree", &[], "Open session tree"),
        ("app.session.fork", &[], "Fork current session"),
        ("app.session.resume", &[], "Resume a session"),
        (
            "app.tree.foldOrUp",
            &["ctrl+left", "alt+left"],
            "Fold tree branch or move up",
        ),
        (
            "app.tree.unfoldOrDown",
            &["ctrl+right", "alt+right"],
            "Unfold tree branch or move down",
        ),
        ("app.tree.editLabel", &["shift+l"], "Edit tree label"),
        (
            "app.tree.toggleLabelTimestamp",
            &["shift+t"],
            "Toggle tree label timestamps",
        ),
        (
            "app.session.togglePath",
            &["ctrl+p"],
            "Toggle session path display",
        ),
        (
            "app.session.toggleSort",
            &["ctrl+s"],
            "Toggle session sort mode",
        ),
        ("app.session.rename", &["ctrl+r"], "Rename session"),
        ("app.session.delete", &["ctrl+d"], "Delete session"),
        (
            "app.session.deleteNoninvasive",
            &["ctrl+backspace"],
            "Delete session when query is empty",
        ),
        ("app.models.save", &["ctrl+s"], "Save model selection"),
        ("app.models.enableAll", &["ctrl+a"], "Enable all models"),
        ("app.models.clearAll", &["ctrl+x"], "Clear all models"),
        (
            "app.models.toggleProvider",
            &["ctrl+p"],
            "Toggle all models for provider",
        ),
        (
            "app.models.reorderUp",
            &["alt+up"],
            "Move model up in order",
        ),
        (
            "app.models.reorderDown",
            &["alt+down"],
            "Move model down in order",
        ),
        (
            "app.tree.filter.default",
            &["ctrl+d"],
            "Tree filter: default view",
        ),
        (
            "app.tree.filter.noTools",
            &["ctrl+t"],
            "Tree filter: hide tool results",
        ),
        (
            "app.tree.filter.userOnly",
            &["ctrl+u"],
            "Tree filter: user messages only",
        ),
        (
            "app.tree.filter.labeledOnly",
            &["ctrl+l"],
            "Tree filter: labeled entries only",
        ),
        (
            "app.tree.filter.all",
            &["ctrl+a"],
            "Tree filter: show all entries",
        ),
        (
            "app.tree.filter.cycleForward",
            &["ctrl+o"],
            "Tree filter: cycle forward",
        ),
        (
            "app.tree.filter.cycleBackward",
            &["shift+ctrl+o"],
            "Tree filter: cycle backward",
        ),
    ] {
        bindings.insert(id.into(), definition(keys, description));
    }
    bindings
}

fn migrated_name(name: &str) -> Option<&'static str> {
    Some(match name {
        "cursorUp" => "tui.editor.cursorUp",
        "cursorDown" => "tui.editor.cursorDown",
        "cursorLeft" => "tui.editor.cursorLeft",
        "cursorRight" => "tui.editor.cursorRight",
        "cursorWordLeft" => "tui.editor.cursorWordLeft",
        "cursorWordRight" => "tui.editor.cursorWordRight",
        "cursorLineStart" => "tui.editor.cursorLineStart",
        "cursorLineEnd" => "tui.editor.cursorLineEnd",
        "jumpForward" => "tui.editor.jumpForward",
        "jumpBackward" => "tui.editor.jumpBackward",
        "pageUp" => "tui.editor.pageUp",
        "pageDown" => "tui.editor.pageDown",
        "deleteCharBackward" => "tui.editor.deleteCharBackward",
        "deleteCharForward" => "tui.editor.deleteCharForward",
        "deleteWordBackward" => "tui.editor.deleteWordBackward",
        "deleteWordForward" => "tui.editor.deleteWordForward",
        "deleteToLineStart" => "tui.editor.deleteToLineStart",
        "deleteToLineEnd" => "tui.editor.deleteToLineEnd",
        "yank" => "tui.editor.yank",
        "yankPop" => "tui.editor.yankPop",
        "undo" => "tui.editor.undo",
        "newLine" => "tui.input.newLine",
        "submit" => "tui.input.submit",
        "tab" => "tui.input.tab",
        "copy" => "tui.input.copy",
        "selectUp" => "tui.select.up",
        "selectDown" => "tui.select.down",
        "selectPageUp" => "tui.select.pageUp",
        "selectPageDown" => "tui.select.pageDown",
        "selectConfirm" => "tui.select.confirm",
        "selectCancel" => "tui.select.cancel",
        "interrupt" => "app.interrupt",
        "clear" => "app.clear",
        "exit" => "app.exit",
        "suspend" => "app.suspend",
        "cycleThinkingLevel" => "app.thinking.cycle",
        "cycleModelForward" => "app.model.cycleForward",
        "cycleModelBackward" => "app.model.cycleBackward",
        "selectModel" => "app.model.select",
        "expandTools" => "app.tools.expand",
        "toggleThinking" => "app.thinking.toggle",
        "toggleSessionNamedFilter" => "app.session.toggleNamedFilter",
        "externalEditor" => "app.editor.external",
        "followUp" => "app.message.followUp",
        "dequeue" => "app.message.dequeue",
        "pasteImage" => "app.clipboard.pasteImage",
        "newSession" => "app.session.new",
        "tree" => "app.session.tree",
        "fork" => "app.session.fork",
        "resume" => "app.session.resume",
        "treeFoldOrUp" => "app.tree.foldOrUp",
        "treeUnfoldOrDown" => "app.tree.unfoldOrDown",
        "treeEditLabel" => "app.tree.editLabel",
        "treeToggleLabelTimestamp" => "app.tree.toggleLabelTimestamp",
        "toggleSessionPath" => "app.session.togglePath",
        "toggleSessionSort" => "app.session.toggleSort",
        "renameSession" => "app.session.rename",
        "deleteSession" => "app.session.delete",
        "deleteSessionNoninvasive" => "app.session.deleteNoninvasive",
        _ => return None,
    })
}

/// Rename legacy keys, preserving an explicitly supplied modern key.
#[must_use]
pub fn migrate_keybindings_config(raw: &RawKeybindingsConfig) -> (RawKeybindingsConfig, bool) {
    let mut migrated = false;
    let mut config = RawKeybindingsConfig::new();
    for (key, value) in raw {
        let next = migrated_name(key).unwrap_or(key);
        migrated |= next != key;
        if next == key || !raw.contains_key(next) {
            config.insert(next.into(), value.clone());
        }
    }
    (config, migrated)
}

fn to_config(raw: RawKeybindingsConfig) -> KeybindingsConfig {
    raw.into_iter()
        .filter_map(|(key, value)| match value {
            Value::String(key_id) => Some((key, vec![key_id])),
            Value::Array(keys) if keys.iter().all(Value::is_string) => Some((
                key,
                keys.into_iter()
                    .filter_map(|key| key.as_str().map(str::to_owned))
                    .collect(),
            )),
            _ => None,
        })
        .collect()
}

fn load(path: &Path) -> KeybindingsConfig {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<RawKeybindingsConfig>(&text).ok())
        .map(|raw| to_config(migrate_keybindings_config(&raw).0))
        .unwrap_or_default()
}

pub struct KeybindingsManager {
    manager: TuiKeybindingsManager,
    config_path: Option<PathBuf>,
}

impl KeybindingsManager {
    #[must_use]
    pub fn new(user_bindings: KeybindingsConfig, config_path: Option<PathBuf>) -> Self {
        Self {
            manager: TuiKeybindingsManager::new(keybindings(), user_bindings),
            config_path,
        }
    }

    #[must_use]
    pub fn create(agent_dir: impl AsRef<Path>) -> Self {
        let path = agent_dir.as_ref().join("keybindings.json");
        Self::new(load(&path), Some(path))
    }

    pub fn reload(&mut self) {
        if let Some(path) = &self.config_path {
            self.manager.set_user_bindings(load(path));
        }
    }

    #[must_use]
    pub fn get_effective_config(&self) -> KeybindingsConfig {
        self.manager.get_resolved_bindings()
    }
    #[must_use]
    pub fn inner(&self) -> &TuiKeybindingsManager {
        &self.manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_keeps_the_explicit_modern_binding() {
        let raw =
            serde_json::from_str(r#"{"interrupt":"ctrl+c","app.interrupt":"escape"}"#).unwrap();
        let (migrated, changed) = migrate_keybindings_config(&raw);
        assert!(changed);
        assert_eq!(migrated["app.interrupt"], "escape");
    }
}
