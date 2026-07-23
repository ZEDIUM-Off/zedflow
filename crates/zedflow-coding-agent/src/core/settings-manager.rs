//! Session settings shared by coding-agent frontends.
//!
//! Pi keeps settings as two JSON documents (global and project) and merges the
//! project document over the global one.  This small Rust equivalent keeps the
//! same useful boundary without pulling a file-locking dependency into the
//! runtime: writes are serialized by the manager mutex and use a temporary file.

use std::{
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
}

impl SettingsManager {
    pub fn create(cwd: impl AsRef<Path>, agent_dir: impl AsRef<Path>) -> Self {
        let global = agent_dir.as_ref().join("settings.json");
        let project = cwd.as_ref().join(".pi").join("settings.json");
        Self::from_paths(global, project)
    }

    pub fn from_paths(global: impl Into<PathBuf>, project: impl Into<PathBuf>) -> Self {
        let paths = (global.into(), project.into());
        let state = (read_settings(&paths.0), read_settings(&paths.1));
        Self {
            paths: Some(paths),
            state: Arc::new(Mutex::new(state)),
        }
    }

    pub fn in_memory(settings: Settings) -> Self {
        Self::with_settings(settings, Settings::default())
    }

    pub fn with_settings(global: Settings, project: Settings) -> Self {
        Self {
            paths: None,
            state: Arc::new(Mutex::new((global, project))),
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
        if let Some((global, project)) = &self.paths {
            *self.state.lock().expect("settings lock") =
                (read_settings(global), read_settings(project));
        }
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
        f(&mut self.state.lock().expect("settings lock").0);
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

fn read_settings(path: &Path) -> Settings {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}
