//! One-time coding-agent migrations run during startup.

use serde_json::{Map, Value};
use std::{
    fs,
    io::{self, BufRead, Read},
    path::Path,
};

use crate::config::{CONFIG_DIR_NAME, get_agent_dir, get_bin_dir};

pub const MIGRATION_GUIDE_URL: &str = "https://github.com/earendil-works/pi-mono/blob/main/packages/coding-agent/CHANGELOG.md#extensions-migration";
pub const EXTENSIONS_DOC_URL: &str =
    "https://github.com/earendil-works/pi-mono/blob/main/packages/coding-agent/docs/extensions.md";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationResult {
    pub migrated_auth_providers: Vec<String>,
    pub deprecation_warnings: Vec<String>,
}

/// Migrate legacy `oauth.json` and `settings.json.apiKeys` into `auth.json`.
pub fn migrate_auth_to_auth_json() -> Vec<String> {
    migrate_auth_in(&get_agent_dir())
}

fn migrate_auth_in(agent_dir: &Path) -> Vec<String> {
    let auth_path = agent_dir.join("auth.json");
    if auth_path.exists() {
        return Vec::new();
    }
    let oauth_path = agent_dir.join("oauth.json");
    let settings_path = agent_dir.join("settings.json");
    let mut migrated = Map::new();
    let mut providers = Vec::new();

    if let Ok(content) = fs::read_to_string(&oauth_path)
        && let Ok(Value::Object(credentials)) = serde_json::from_str::<Value>(&content)
    {
        for (provider, credential) in credentials {
            if let Value::Object(mut credential) = credential {
                credential.insert("type".into(), Value::String("oauth".into()));
                migrated.insert(provider.clone(), Value::Object(credential));
                providers.push(provider);
            }
        }
        let _ = fs::rename(&oauth_path, oauth_path.with_extension("json.migrated"));
    }

    if let Ok(content) = fs::read_to_string(&settings_path)
        && let Ok(Value::Object(mut settings)) = serde_json::from_str::<Value>(&content)
        && let Some(Value::Object(api_keys)) = settings.remove("apiKeys")
    {
        for (provider, key) in api_keys {
            if !migrated.contains_key(&provider)
                && let Value::String(key) = key
            {
                migrated.insert(
                    provider.clone(),
                    serde_json::json!({"type": "api_key", "key": key}),
                );
                providers.push(provider);
            }
        }
        if let Ok(content) = serde_json::to_string_pretty(&settings) {
            let _ = fs::write(&settings_path, content);
        }
    }

    if !migrated.is_empty() {
        let _ = fs::create_dir_all(agent_dir);
        if let Ok(content) = serde_json::to_vec_pretty(&migrated) {
            let _ = fs::write(&auth_path, content);
            set_private_file_mode(&auth_path);
        }
    }
    providers
}

#[cfg(unix)]
fn set_private_file_mode(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}
#[cfg(not(unix))]
fn set_private_file_mode(_: &Path) {}

/// Move sessions accidentally created directly under the agent directory.
pub fn migrate_sessions_from_agent_root() {
    migrate_sessions_in(&get_agent_dir());
}

fn migrate_sessions_in(agent_dir: &Path) {
    let Ok(entries) = fs::read_dir(agent_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
            continue;
        }
        let Ok(file) = fs::File::open(&path) else {
            continue;
        };
        let Some(Ok(first_line)) = io::BufReader::new(file).lines().next() else {
            continue;
        };
        let Ok(Value::Object(header)) = serde_json::from_str::<Value>(&first_line) else {
            continue;
        };
        let Some("session") = header.get("type").and_then(Value::as_str) else {
            continue;
        };
        let Some(cwd) = header.get("cwd").and_then(Value::as_str) else {
            continue;
        };
        let cwd = cwd
            .strip_prefix('/')
            .or_else(|| cwd.strip_prefix('\\'))
            .unwrap_or(cwd);
        let safe_cwd = cwd
            .chars()
            .map(|character| {
                if matches!(character, '/' | '\\' | ':') {
                    '-'
                } else {
                    character
                }
            })
            .collect::<String>();
        let target_dir = agent_dir.join("sessions").join(format!("--{safe_cwd}--"));
        let Some(file_name) = path.file_name() else {
            continue;
        };
        let target = target_dir.join(file_name);
        if target.exists() || fs::create_dir_all(&target_dir).is_err() {
            continue;
        }
        let _ = fs::rename(path, target);
    }
}

fn migrate_commands_to_prompts(base_dir: &Path, label: &str) -> bool {
    let commands = base_dir.join("commands");
    let prompts = base_dir.join("prompts");
    if commands.exists() && !prompts.exists() {
        match fs::rename(commands, prompts) {
            Ok(()) => {
                println!("Migrated {label} commands/ → prompts/");
                return true;
            }
            Err(error) => {
                eprintln!("Warning: Could not migrate {label} commands/ to prompts/: {error}")
            }
        }
    }
    false
}

fn legacy_keybinding_name(name: &str) -> Option<&'static str> {
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

fn migrate_keybindings_config_file(agent_dir: &Path) {
    let path = agent_dir.join("keybindings.json");
    let Ok(content) = fs::read_to_string(&path) else {
        return;
    };
    let Ok(Value::Object(raw)) = serde_json::from_str::<Value>(&content) else {
        return;
    };
    let mut migrated = false;
    let mut config = Map::new();
    for (name, value) in &raw {
        let Some(next_name) = legacy_keybinding_name(name) else {
            config.insert(name.clone(), value.clone());
            continue;
        };
        migrated = true;
        if !raw.contains_key(next_name) {
            config.insert(next_name.into(), value.clone());
        }
    }
    if migrated && let Ok(mut content) = serde_json::to_string_pretty(&config) {
        content.push('\n');
        let _ = fs::write(path, content);
    }
}

fn migrate_tools_to_bin(agent_dir: &Path, bin_dir: &Path) {
    let tools_dir = agent_dir.join("tools");
    if !tools_dir.exists() {
        return;
    }
    let mut moved_any = false;
    for binary in ["fd", "rg", "fd.exe", "rg.exe"] {
        let old_path = tools_dir.join(binary);
        if !old_path.exists() {
            continue;
        }
        let new_path = bin_dir.join(binary);
        if new_path.exists() {
            let _ = fs::remove_file(old_path);
        } else if fs::create_dir_all(bin_dir).is_ok() && fs::rename(old_path, new_path).is_ok() {
            moved_any = true;
        }
    }
    if moved_any {
        println!("Migrated managed binaries tools/ → bin/");
    }
}

fn check_deprecated_extension_dirs(base_dir: &Path, label: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    if base_dir.join("hooks").exists() {
        warnings.push(format!(
            "{label} hooks/ directory found. Hooks have been renamed to extensions."
        ));
    }
    if let Ok(entries) = fs::read_dir(base_dir.join("tools")) {
        let has_custom_tools = entries.flatten().any(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            !name.starts_with('.')
                && !matches!(
                    name.to_ascii_lowercase().as_str(),
                    "fd" | "rg" | "fd.exe" | "rg.exe"
                )
        });
        if has_custom_tools {
            warnings.push(format!(
                "{label} tools/ directory contains custom tools. Custom tools have been merged into extensions."
            ));
        }
    }
    warnings
}

fn migrate_extension_system(agent_dir: &Path, cwd: &Path) -> Vec<String> {
    let project_dir = cwd.join(CONFIG_DIR_NAME);
    migrate_commands_to_prompts(agent_dir, "Global");
    migrate_commands_to_prompts(&project_dir, "Project");
    let mut warnings = check_deprecated_extension_dirs(agent_dir, "Global");
    warnings.extend(check_deprecated_extension_dirs(&project_dir, "Project"));
    warnings
}

pub async fn show_deprecation_warnings(warnings: &[String]) {
    if warnings.is_empty() {
        return;
    }
    for warning in warnings {
        eprintln!("Warning: {warning}");
    }
    eprintln!("\nMove your extensions to the extensions/ directory.");
    eprintln!("Migration guide: {MIGRATION_GUIDE_URL}");
    eprintln!("Documentation: {EXTENSIONS_DOC_URL}");
    eprintln!("\nPress any key to continue...");
    let mut byte = [0];
    let _ = io::stdin().read(&mut byte);
}

#[must_use]
pub fn run_migrations(cwd: impl AsRef<Path>) -> MigrationResult {
    let agent_dir = get_agent_dir();
    let migrated_auth_providers = migrate_auth_in(&agent_dir);
    migrate_sessions_in(&agent_dir);
    migrate_tools_to_bin(&agent_dir, &get_bin_dir());
    migrate_keybindings_config_file(&agent_dir);
    let deprecation_warnings = migrate_extension_system(&agent_dir, cwd.as_ref());
    MigrationResult {
        migrated_auth_providers,
        deprecation_warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "zedflow-migration-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn migrates_auth_and_removes_legacy_api_keys() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();
        fs::write(&root.join("oauth.json"), r#"{"oauth":{"refresh":"r"}}"#).unwrap();
        fs::write(
            root.join("settings.json"),
            r#"{"theme":"dark","apiKeys":{"keyed":"secret"}}"#,
        )
        .unwrap();

        let providers = migrate_auth_in(&root);
        assert_eq!(providers, ["oauth", "keyed"]);
        let auth: Value =
            serde_json::from_str(&fs::read_to_string(root.join("auth.json")).unwrap()).unwrap();
        assert_eq!(auth["oauth"]["type"], "oauth");
        assert_eq!(auth["keyed"]["key"], "secret");
        assert!(
            !fs::read_to_string(root.join("settings.json"))
                .unwrap()
                .contains("apiKeys")
        );
        assert!(root.join("oauth.json.migrated").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn migrates_legacy_keybinding_names_without_overwriting_new_names() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("keybindings.json"),
            r#"{"submit":"enter","app.exit":"ctrl+x","exit":"ctrl+c"}"#,
        )
        .unwrap();
        migrate_keybindings_config_file(&root);
        let config: Value =
            serde_json::from_str(&fs::read_to_string(root.join("keybindings.json")).unwrap())
                .unwrap();
        assert_eq!(config["tui.input.submit"], "enter");
        assert_eq!(config["app.exit"], "ctrl+x");
        assert!(config.get("exit").is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn migrates_root_session_using_header_cwd() {
        let root = temp_dir();
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("session.jsonl"),
            "{\"type\":\"session\",\"cwd\":\"/tmp/project\"}\n",
        )
        .unwrap();
        migrate_sessions_in(&root);
        assert!(root.join("sessions/--tmp-project--/session.jsonl").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
