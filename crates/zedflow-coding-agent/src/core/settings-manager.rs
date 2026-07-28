//! Session settings shared by coding-agent frontends.
//!
//! Pi keeps settings as two JSON documents (global and project) and merges the
//! project document over the global one.  This small Rust equivalent keeps the
//! same useful boundary without pulling a file-locking dependency into the
//! runtime: writes are serialized by the manager mutex and use a temporary file.

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use zedflow_ai::Transport;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CompactionSettings {
    pub enabled: Option<bool>,
    pub reserve_tokens: Option<u64>,
    pub keep_recent_tokens: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RetrySettings {
    pub enabled: Option<bool>,
    pub max_retries: Option<u32>,
    pub base_delay_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DefaultProjectTrust {
    #[default]
    Ask,
    Always,
    Never,
}

impl<'de> Deserialize<'de> for DefaultProjectTrust {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(match value.as_str() {
            Some("always") => Self::Always,
            Some("never") => Self::Never,
            _ => Self::Ask,
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub default_thinking_level: Option<String>,
    pub transport: Option<Transport>,
    pub steering_mode: Option<String>,
    pub follow_up_mode: Option<String>,
    pub theme: Option<String>,
    pub session_dir: Option<String>,
    pub compaction: Option<CompactionSettings>,
    pub retry: Option<RetrySettings>,
    pub hide_thinking_block: Option<bool>,
    pub quiet_startup: Option<bool>,
    pub default_project_trust: Option<DefaultProjectTrust>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

fn merge_extra(
    base: &BTreeMap<String, serde_json::Value>,
    overlay: &BTreeMap<String, serde_json::Value>,
) -> BTreeMap<String, serde_json::Value> {
    let mut result = base.clone();
    for (key, value) in overlay {
        match (result.get_mut(key), value) {
            (Some(serde_json::Value::Object(base)), serde_json::Value::Object(overlay)) => {
                base.extend(overlay.clone());
            }
            _ => {
                result.insert(key.clone(), value.clone());
            }
        }
    }
    result
}

fn merge(base: &Settings, overlay: &Settings) -> Settings {
    Settings {
        default_provider: overlay
            .default_provider
            .clone()
            .or_else(|| base.default_provider.clone()),
        default_model: overlay
            .default_model
            .clone()
            .or_else(|| base.default_model.clone()),
        default_thinking_level: overlay
            .default_thinking_level
            .clone()
            .or_else(|| base.default_thinking_level.clone()),
        transport: overlay.transport.or(base.transport),
        steering_mode: overlay
            .steering_mode
            .clone()
            .or_else(|| base.steering_mode.clone()),
        follow_up_mode: overlay
            .follow_up_mode
            .clone()
            .or_else(|| base.follow_up_mode.clone()),
        theme: overlay.theme.clone().or_else(|| base.theme.clone()),
        session_dir: overlay
            .session_dir
            .clone()
            .or_else(|| base.session_dir.clone()),
        compaction: Some(CompactionSettings {
            enabled: overlay
                .compaction
                .as_ref()
                .and_then(|v| v.enabled)
                .or_else(|| base.compaction.as_ref().and_then(|v| v.enabled)),
            reserve_tokens: overlay
                .compaction
                .as_ref()
                .and_then(|v| v.reserve_tokens)
                .or_else(|| base.compaction.as_ref().and_then(|v| v.reserve_tokens)),
            keep_recent_tokens: overlay
                .compaction
                .as_ref()
                .and_then(|v| v.keep_recent_tokens)
                .or_else(|| base.compaction.as_ref().and_then(|v| v.keep_recent_tokens)),
        })
        .filter(|v| {
            v.enabled.is_some() || v.reserve_tokens.is_some() || v.keep_recent_tokens.is_some()
        }),
        retry: Some(RetrySettings {
            enabled: overlay
                .retry
                .as_ref()
                .and_then(|v| v.enabled)
                .or_else(|| base.retry.as_ref().and_then(|v| v.enabled)),
            max_retries: overlay
                .retry
                .as_ref()
                .and_then(|v| v.max_retries)
                .or_else(|| base.retry.as_ref().and_then(|v| v.max_retries)),
            base_delay_ms: overlay
                .retry
                .as_ref()
                .and_then(|v| v.base_delay_ms)
                .or_else(|| base.retry.as_ref().and_then(|v| v.base_delay_ms)),
        })
        .filter(|v| v.enabled.is_some() || v.max_retries.is_some() || v.base_delay_ms.is_some()),
        hide_thinking_block: overlay.hide_thinking_block.or(base.hide_thinking_block),
        quiet_startup: overlay.quiet_startup.or(base.quiet_startup),
        default_project_trust: overlay.default_project_trust.or(base.default_project_trust),
        extra: merge_extra(&base.extra, &overlay.extra),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsScope {
    Global,
    Project,
}

#[derive(Debug, Clone)]
pub struct SettingsManager {
    paths: Option<(PathBuf, PathBuf)>,
    state: Arc<Mutex<(Settings, Settings)>>,
    project_trusted: Arc<Mutex<bool>>,
    errors: Arc<Mutex<Vec<String>>>,
}

impl SettingsManager {
    pub fn create(cwd: impl AsRef<Path>, agent_dir: impl AsRef<Path>) -> Self {
        Self::create_with_project_trust(cwd, agent_dir, true)
    }

    pub fn create_with_project_trust(
        cwd: impl AsRef<Path>,
        agent_dir: impl AsRef<Path>,
        project_trusted: bool,
    ) -> Self {
        let global = agent_dir.as_ref().join("settings.json");
        let project = cwd.as_ref().join(".pi").join("settings.json");
        Self::from_paths_with_project_trust(global, project, project_trusted)
    }

    pub fn from_paths(global: impl Into<PathBuf>, project: impl Into<PathBuf>) -> Self {
        Self::from_paths_with_project_trust(global, project, true)
    }

    pub fn from_paths_with_project_trust(
        global: impl Into<PathBuf>,
        project: impl Into<PathBuf>,
        project_trusted: bool,
    ) -> Self {
        let paths = (global.into(), project.into());
        let mut errors = Vec::new();
        let global = read_settings_result(&paths.0).unwrap_or_else(|error| {
            errors.push(error);
            Settings::default()
        });
        let project = if project_trusted {
            read_settings_result(&paths.1).unwrap_or_else(|error| {
                errors.push(error);
                Settings::default()
            })
        } else {
            Settings::default()
        };
        Self {
            paths: Some(paths),
            state: Arc::new(Mutex::new((global, project))),
            project_trusted: Arc::new(Mutex::new(project_trusted)),
            errors: Arc::new(Mutex::new(errors)),
        }
    }

    pub fn in_memory(settings: Settings) -> Self {
        Self::with_settings(settings, Settings::default())
    }

    pub fn with_settings(global: Settings, project: Settings) -> Self {
        Self {
            paths: None,
            state: Arc::new(Mutex::new((global, project))),
            project_trusted: Arc::new(Mutex::new(true)),
            errors: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn settings(&self) -> Settings {
        let (global, project) = self.state.lock().expect("settings lock").clone();
        merge(&global, &project)
    }
    pub fn global_settings(&self) -> Settings {
        self.state.lock().expect("settings lock").0.clone()
    }
    pub fn project_settings(&self) -> Settings {
        self.state.lock().expect("settings lock").1.clone()
    }

    pub fn reload(&self) {
        let Some((global_path, project_path)) = &self.paths else {
            return;
        };
        let mut state = self.state.lock().expect("settings lock");
        match read_settings_result(global_path) {
            Ok(global) => state.0 = global,
            Err(error) => self
                .errors
                .lock()
                .expect("settings errors lock")
                .push(error),
        }
        if *self.project_trusted.lock().expect("settings trust lock") {
            match read_settings_result(project_path) {
                Ok(project) => state.1 = project,
                Err(error) => self
                    .errors
                    .lock()
                    .expect("settings errors lock")
                    .push(error),
            }
        } else {
            state.1 = Settings::default();
        }
    }

    pub fn drain_errors(&self) -> Vec<String> {
        std::mem::take(&mut *self.errors.lock().expect("settings errors lock"))
    }

    pub fn set_project_trusted(&self, trusted: bool) {
        let mut project_trusted = self.project_trusted.lock().expect("settings trust lock");
        if *project_trusted == trusted {
            return;
        }
        *project_trusted = trusted;
        drop(project_trusted);

        let project = if trusted {
            self.paths
                .as_ref()
                .and_then(|(_, path)| match read_settings_result(path) {
                    Ok(settings) => Some(settings),
                    Err(error) => {
                        self.errors
                            .lock()
                            .expect("settings errors lock")
                            .push(error);
                        None
                    }
                })
                .unwrap_or_default()
        } else {
            Settings::default()
        };
        self.state.lock().expect("settings lock").1 = project;
    }

    pub fn get_default_provider(&self) -> Option<String> {
        self.settings().default_provider
    }
    pub fn get_default_model(&self) -> Option<String> {
        self.settings().default_model
    }
    pub fn set_default_model_and_provider(
        &self,
        provider: impl Into<String>,
        model: impl Into<String>,
    ) -> io::Result<()> {
        self.update_global(|s| {
            s.default_provider = Some(provider.into());
            s.default_model = Some(model.into());
        });
        self.flush()
    }
    pub fn get_transport(&self) -> Transport {
        self.settings().transport.unwrap_or(Transport::Auto)
    }
    pub fn get_steering_mode(&self) -> String {
        if self.settings().steering_mode.as_deref() == Some("all") {
            "all".into()
        } else {
            "one-at-a-time".into()
        }
    }
    pub fn get_follow_up_mode(&self) -> String {
        if self.settings().follow_up_mode.as_deref() == Some("all") {
            "all".into()
        } else {
            "one-at-a-time".into()
        }
    }
    pub fn get_compaction_settings(&self) -> (bool, u64, u64) {
        let c = self.settings().compaction.unwrap_or_default();
        (
            c.enabled.unwrap_or(true),
            c.reserve_tokens.unwrap_or(16_384),
            c.keep_recent_tokens.unwrap_or(20_000),
        )
    }
    pub fn get_retry_settings(&self) -> (bool, u32, u64) {
        let r = self.settings().retry.unwrap_or_default();
        (
            r.enabled.unwrap_or(true),
            r.max_retries.unwrap_or(3),
            r.base_delay_ms.unwrap_or(2_000),
        )
    }
    /// Project trust is a global-only setting in Pi.
    pub fn get_default_project_trust(&self) -> DefaultProjectTrust {
        self.global_settings()
            .default_project_trust
            .unwrap_or_default()
    }
    pub fn set_default_project_trust(&self, value: DefaultProjectTrust) -> io::Result<()> {
        self.update_global(|settings| settings.default_project_trust = Some(value));
        self.flush()
    }
    pub fn get_session_dir(&self) -> Option<PathBuf> {
        self.settings().session_dir.map(|p| {
            crate::utils::paths::resolve_path(
                &p,
                std::env::current_dir().unwrap_or_default(),
                &crate::utils::paths::PathInputOptions::default(),
            )
        })
    }
    pub fn set_compaction_enabled(&self, enabled: bool) -> io::Result<()> {
        self.update_global(|s| {
            s.compaction.get_or_insert_with(Default::default).enabled = Some(enabled)
        });
        self.flush()
    }
    pub fn set_retry_enabled(&self, enabled: bool) -> io::Result<()> {
        self.update_global(|s| {
            s.retry.get_or_insert_with(Default::default).enabled = Some(enabled)
        });
        self.flush()
    }

    fn update_global(&self, f: impl FnOnce(&mut Settings)) {
        let disk = self.paths.as_ref().and_then(|(path, _)| {
            read_settings_result(path)
                .map_err(|error| {
                    self.errors
                        .lock()
                        .expect("settings errors lock")
                        .push(error);
                })
                .ok()
        });
        let mut state = self.state.lock().expect("settings lock");
        if let Some(disk) = disk {
            state.0 = disk;
        }
        f(&mut state.0);
    }
    fn flush(&self) -> io::Result<()> {
        let Some((global, _)) = &self.paths else {
            return Ok(());
        };
        let value = self.global_settings();
        let bytes = serde_json::to_vec_pretty(&value).map_err(io::Error::other)?;
        if let Some(parent) = global.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = global.with_extension("json.tmp");
        fs::write(&tmp, bytes)?;
        fs::rename(tmp, global)
    }
}

fn read_settings_result(path: &Path) -> Result<Settings, String> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Settings::default()),
        Err(error) => {
            return Err(format!(
                "Failed to read settings {}: {error}",
                path.display()
            ));
        }
    };
    serde_json::from_str(&content)
        .map_err(|error| format!("Failed to parse settings {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "zedflow-settings-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn untrusted_projects_do_not_load_project_settings() {
        let root = temp_dir();
        let global = root.join("agent/settings.json");
        let project = root.join("project/.pi/settings.json");
        fs::create_dir_all(global.parent().unwrap()).unwrap();
        fs::create_dir_all(project.parent().unwrap()).unwrap();
        fs::write(&global, r#"{"theme":"global"}"#).unwrap();
        fs::write(&project, r#"{"theme":"project"}"#).unwrap();

        let manager = SettingsManager::from_paths_with_project_trust(&global, &project, false);
        assert_eq!(manager.settings().theme.as_deref(), Some("global"));
        manager.set_project_trusted(true);
        assert_eq!(manager.settings().theme.as_deref(), Some("project"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_reload_preserves_settings_and_reports_error() {
        let root = temp_dir();
        let global = root.join("settings.json");
        fs::create_dir_all(&root).unwrap();
        fs::write(&global, r#"{"theme":"valid"}"#).unwrap();
        let manager = SettingsManager::from_paths(&global, root.join("project.json"));
        fs::write(&global, "{").unwrap();
        manager.reload();
        assert_eq!(manager.settings().theme.as_deref(), Some("valid"));
        assert_eq!(manager.drain_errors().len(), 1);
        assert!(manager.drain_errors().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn setters_preserve_external_and_unknown_settings() {
        let root = temp_dir();
        let global = root.join("settings.json");
        fs::create_dir_all(&root).unwrap();
        fs::write(&global, r#"{"theme":"old"}"#).unwrap();
        let manager = SettingsManager::from_paths(&global, root.join("project.json"));
        fs::write(&global, r#"{"theme":"new","custom":{"enabled":true}}"#).unwrap();
        manager.set_retry_enabled(false).unwrap();
        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&global).unwrap()).unwrap();
        assert_eq!(saved["theme"], "new");
        assert_eq!(saved["custom"]["enabled"], true);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn default_project_trust_comes_from_global_settings() {
        let manager = SettingsManager::with_settings(
            Settings {
                default_project_trust: Some(DefaultProjectTrust::Always),
                ..Settings::default()
            },
            Settings {
                default_project_trust: Some(DefaultProjectTrust::Never),
                ..Settings::default()
            },
        );
        assert_eq!(
            manager.get_default_project_trust(),
            DefaultProjectTrust::Always
        );
    }
}
