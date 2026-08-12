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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reserve_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_recent_tokens: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RetrySettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_delay_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TerminalSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_images: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_width_cells: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clear_on_shrink: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_terminal_progress: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ImageSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_resize: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_images: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WarningSettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anthropic_extra_usage: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PackageSource {
    Source(String),
    Filtered {
        source: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        autoload: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        extensions: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        skills: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prompts: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        themes: Option<Vec<String>>,
    },
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_changelog_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_thinking_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport: Option<Transport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steering_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub follow_up_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compaction: Option<CompactionSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetrySettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide_thinking_block: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quiet_startup: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_project_trust: Option<DefaultProjectTrust>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_idle_timeout_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collapse_changelog: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_install_telemetry: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_skill_commands: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packages: Option<Vec<PackageSource>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub themes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal: Option<TerminalSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<ImageSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled_models: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub double_escape_action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tree_filter_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editor_padding_x: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_pad: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autocomplete_max_visible: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_hardware_cursor: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<WarningSettings>,
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
        last_changelog_version: overlay
            .last_changelog_version
            .clone()
            .or_else(|| base.last_changelog_version.clone()),
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
        http_idle_timeout_ms: overlay.http_idle_timeout_ms.or(base.http_idle_timeout_ms),
        collapse_changelog: overlay.collapse_changelog.or(base.collapse_changelog),
        enable_install_telemetry: overlay
            .enable_install_telemetry
            .or(base.enable_install_telemetry),
        enable_skill_commands: overlay.enable_skill_commands.or(base.enable_skill_commands),
        packages: overlay.packages.clone().or_else(|| base.packages.clone()),
        extensions: overlay
            .extensions
            .clone()
            .or_else(|| base.extensions.clone()),
        skills: overlay.skills.clone().or_else(|| base.skills.clone()),
        prompts: overlay.prompts.clone().or_else(|| base.prompts.clone()),
        themes: overlay.themes.clone().or_else(|| base.themes.clone()),
        terminal: Some(TerminalSettings {
            show_images: overlay
                .terminal
                .as_ref()
                .and_then(|v| v.show_images)
                .or_else(|| base.terminal.as_ref().and_then(|v| v.show_images)),
            image_width_cells: overlay
                .terminal
                .as_ref()
                .and_then(|v| v.image_width_cells)
                .or_else(|| base.terminal.as_ref().and_then(|v| v.image_width_cells)),
            clear_on_shrink: overlay
                .terminal
                .as_ref()
                .and_then(|v| v.clear_on_shrink)
                .or_else(|| base.terminal.as_ref().and_then(|v| v.clear_on_shrink)),
            show_terminal_progress: overlay
                .terminal
                .as_ref()
                .and_then(|v| v.show_terminal_progress)
                .or_else(|| {
                    base.terminal
                        .as_ref()
                        .and_then(|v| v.show_terminal_progress)
                }),
        })
        .filter(|v| {
            v.show_images.is_some()
                || v.image_width_cells.is_some()
                || v.clear_on_shrink.is_some()
                || v.show_terminal_progress.is_some()
        }),
        images: Some(ImageSettings {
            auto_resize: overlay
                .images
                .as_ref()
                .and_then(|v| v.auto_resize)
                .or_else(|| base.images.as_ref().and_then(|v| v.auto_resize)),
            block_images: overlay
                .images
                .as_ref()
                .and_then(|v| v.block_images)
                .or_else(|| base.images.as_ref().and_then(|v| v.block_images)),
        })
        .filter(|v| v.auto_resize.is_some() || v.block_images.is_some()),
        enabled_models: overlay
            .enabled_models
            .clone()
            .or_else(|| base.enabled_models.clone()),
        double_escape_action: overlay
            .double_escape_action
            .clone()
            .or_else(|| base.double_escape_action.clone()),
        tree_filter_mode: overlay
            .tree_filter_mode
            .clone()
            .or_else(|| base.tree_filter_mode.clone()),
        editor_padding_x: overlay.editor_padding_x.or(base.editor_padding_x),
        output_pad: overlay.output_pad.or(base.output_pad),
        autocomplete_max_visible: overlay
            .autocomplete_max_visible
            .or(base.autocomplete_max_visible),
        show_hardware_cursor: overlay.show_hardware_cursor.or(base.show_hardware_cursor),
        warnings: Some(WarningSettings {
            anthropic_extra_usage: overlay
                .warnings
                .as_ref()
                .and_then(|v| v.anthropic_extra_usage)
                .or_else(|| base.warnings.as_ref().and_then(|v| v.anthropic_extra_usage)),
        })
        .filter(|v| v.anthropic_extra_usage.is_some()),
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
    #[must_use]
    pub fn is_project_trusted(&self) -> bool {
        *self.project_trusted.lock().expect("settings trust lock")
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
    pub fn get_theme_setting(&self) -> Option<String> {
        self.settings().theme
    }
    pub fn get_theme(&self) -> Option<String> {
        self.get_theme_setting()
            .filter(|theme| !theme.contains('/'))
    }
    pub fn set_theme(&self, theme: impl Into<String>) -> io::Result<()> {
        self.update_global(|settings| settings.theme = Some(theme.into()));
        self.flush()
    }
    pub fn get_last_changelog_version(&self) -> Option<String> {
        self.settings().last_changelog_version
    }
    pub fn set_last_changelog_version(&self, version: impl Into<String>) -> io::Result<()> {
        self.update_global(|settings| settings.last_changelog_version = Some(version.into()));
        self.flush()
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
    pub fn set_transport(&self, transport: Transport) -> io::Result<()> {
        self.update_global(|settings| settings.transport = Some(transport));
        self.flush()
    }
    pub fn get_steering_mode(&self) -> String {
        if self.settings().steering_mode.as_deref() == Some("all") {
            "all".into()
        } else {
            "one-at-a-time".into()
        }
    }
    pub fn set_steering_mode(&self, value: impl Into<String>) -> io::Result<()> {
        let value = value.into();
        if !matches!(value.as_str(), "all" | "one-at-a-time") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid steering mode",
            ));
        }
        self.update_global(|settings| settings.steering_mode = Some(value));
        self.flush()
    }
    pub fn get_follow_up_mode(&self) -> String {
        if self.settings().follow_up_mode.as_deref() == Some("all") {
            "all".into()
        } else {
            "one-at-a-time".into()
        }
    }
    pub fn set_follow_up_mode(&self, value: impl Into<String>) -> io::Result<()> {
        let value = value.into();
        if !matches!(value.as_str(), "all" | "one-at-a-time") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid follow-up mode",
            ));
        }
        self.update_global(|settings| settings.follow_up_mode = Some(value));
        self.flush()
    }
    pub fn get_default_thinking_level(&self) -> String {
        self.settings()
            .default_thinking_level
            .unwrap_or_else(|| "off".into())
    }
    pub fn set_default_thinking_level(&self, value: impl Into<String>) -> io::Result<()> {
        self.update_global(|settings| settings.default_thinking_level = Some(value.into()));
        self.flush()
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

    pub fn get_http_idle_timeout_ms(&self) -> f64 {
        self.settings()
            .http_idle_timeout_ms
            .unwrap_or(crate::http_dispatcher::DEFAULT_HTTP_IDLE_TIMEOUT_MS)
    }
    pub fn set_http_idle_timeout_ms(&self, timeout_ms: f64) -> io::Result<()> {
        if !timeout_ms.is_finite() || timeout_ms < 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid HTTP idle timeout",
            ));
        }
        self.update_global(|settings| settings.http_idle_timeout_ms = Some(timeout_ms.floor()));
        self.flush()
    }
    pub fn get_hide_thinking_block(&self) -> bool {
        self.settings().hide_thinking_block.unwrap_or(false)
    }
    pub fn set_hide_thinking_block(&self, value: bool) -> io::Result<()> {
        self.update_global(|settings| settings.hide_thinking_block = Some(value));
        self.flush()
    }
    pub fn get_quiet_startup(&self) -> bool {
        self.settings().quiet_startup.unwrap_or(false)
    }
    pub fn set_quiet_startup(&self, value: bool) -> io::Result<()> {
        self.update_global(|settings| settings.quiet_startup = Some(value));
        self.flush()
    }
    pub fn get_collapse_changelog(&self) -> bool {
        self.settings().collapse_changelog.unwrap_or(false)
    }
    pub fn set_collapse_changelog(&self, value: bool) -> io::Result<()> {
        self.update_global(|settings| settings.collapse_changelog = Some(value));
        self.flush()
    }
    pub fn get_enable_install_telemetry(&self) -> bool {
        self.settings().enable_install_telemetry.unwrap_or(true)
    }
    pub fn set_enable_install_telemetry(&self, value: bool) -> io::Result<()> {
        self.update_global(|settings| settings.enable_install_telemetry = Some(value));
        self.flush()
    }
    pub fn get_enable_skill_commands(&self) -> bool {
        self.settings().enable_skill_commands.unwrap_or(true)
    }
    pub fn set_enable_skill_commands(&self, value: bool) -> io::Result<()> {
        self.update_global(|settings| settings.enable_skill_commands = Some(value));
        self.flush()
    }
    pub fn get_show_images(&self) -> bool {
        self.settings()
            .terminal
            .and_then(|value| value.show_images)
            .unwrap_or(true)
    }
    pub fn set_show_images(&self, value: bool) -> io::Result<()> {
        self.update_global(|settings| {
            settings
                .terminal
                .get_or_insert_with(Default::default)
                .show_images = Some(value)
        });
        self.flush()
    }
    pub fn get_image_width_cells(&self) -> u32 {
        self.settings()
            .terminal
            .and_then(|value| value.image_width_cells)
            .unwrap_or(60)
    }
    pub fn set_image_width_cells(&self, value: u32) -> io::Result<()> {
        self.update_global(|settings| {
            settings
                .terminal
                .get_or_insert_with(Default::default)
                .image_width_cells = Some(value)
        });
        self.flush()
    }
    pub fn get_clear_on_shrink(&self) -> bool {
        self.settings()
            .terminal
            .and_then(|value| value.clear_on_shrink)
            .unwrap_or(false)
    }
    pub fn set_clear_on_shrink(&self, value: bool) -> io::Result<()> {
        self.update_global(|settings| {
            settings
                .terminal
                .get_or_insert_with(Default::default)
                .clear_on_shrink = Some(value)
        });
        self.flush()
    }
    pub fn get_show_terminal_progress(&self) -> bool {
        self.settings()
            .terminal
            .and_then(|value| value.show_terminal_progress)
            .unwrap_or(false)
    }
    pub fn set_show_terminal_progress(&self, value: bool) -> io::Result<()> {
        self.update_global(|settings| {
            settings
                .terminal
                .get_or_insert_with(Default::default)
                .show_terminal_progress = Some(value)
        });
        self.flush()
    }
    pub fn get_image_auto_resize(&self) -> bool {
        self.settings()
            .images
            .and_then(|value| value.auto_resize)
            .unwrap_or(true)
    }
    pub fn set_image_auto_resize(&self, value: bool) -> io::Result<()> {
        self.update_global(|settings| {
            settings
                .images
                .get_or_insert_with(Default::default)
                .auto_resize = Some(value)
        });
        self.flush()
    }
    pub fn get_block_images(&self) -> bool {
        self.settings()
            .images
            .and_then(|value| value.block_images)
            .unwrap_or(false)
    }
    pub fn set_block_images(&self, value: bool) -> io::Result<()> {
        self.update_global(|settings| {
            settings
                .images
                .get_or_insert_with(Default::default)
                .block_images = Some(value)
        });
        self.flush()
    }
    pub fn get_enabled_models(&self) -> Option<Vec<String>> {
        self.settings().enabled_models
    }
    pub fn set_enabled_models(&self, value: Option<Vec<String>>) -> io::Result<()> {
        self.update_global(|settings| settings.enabled_models = value);
        self.flush()
    }
    pub fn get_double_escape_action(&self) -> String {
        self.settings()
            .double_escape_action
            .unwrap_or_else(|| "tree".into())
    }
    pub fn set_double_escape_action(&self, value: impl Into<String>) -> io::Result<()> {
        let value = value.into();
        if !matches!(value.as_str(), "fork" | "tree" | "none") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid double escape action",
            ));
        }
        self.update_global(|settings| settings.double_escape_action = Some(value));
        self.flush()
    }
    pub fn get_tree_filter_mode(&self) -> String {
        self.settings()
            .tree_filter_mode
            .unwrap_or_else(|| "default".into())
    }
    pub fn set_tree_filter_mode(&self, value: impl Into<String>) -> io::Result<()> {
        let value = value.into();
        if !matches!(
            value.as_str(),
            "default" | "no-tools" | "user-only" | "labeled-only" | "all"
        ) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid tree filter mode",
            ));
        }
        self.update_global(|settings| settings.tree_filter_mode = Some(value));
        self.flush()
    }
    pub fn get_show_hardware_cursor(&self) -> bool {
        self.settings().show_hardware_cursor.unwrap_or(false)
    }
    pub fn set_show_hardware_cursor(&self, value: bool) -> io::Result<()> {
        self.update_global(|settings| settings.show_hardware_cursor = Some(value));
        self.flush()
    }
    pub fn get_editor_padding_x(&self) -> u16 {
        self.settings().editor_padding_x.unwrap_or(0)
    }
    pub fn set_editor_padding_x(&self, value: u16) -> io::Result<()> {
        self.update_global(|settings| settings.editor_padding_x = Some(value));
        self.flush()
    }
    pub fn get_output_pad(&self) -> u8 {
        self.settings().output_pad.unwrap_or(1).min(1)
    }
    pub fn set_output_pad(&self, value: u8) -> io::Result<()> {
        if value > 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "output padding must be 0 or 1",
            ));
        }
        self.update_global(|settings| settings.output_pad = Some(value));
        self.flush()
    }
    pub fn get_autocomplete_max_visible(&self) -> usize {
        self.settings().autocomplete_max_visible.unwrap_or(5)
    }
    pub fn set_autocomplete_max_visible(&self, value: usize) -> io::Result<()> {
        self.update_global(|settings| settings.autocomplete_max_visible = Some(value));
        self.flush()
    }
    pub fn get_warnings(&self) -> WarningSettings {
        self.settings().warnings.unwrap_or_default()
    }
    pub fn set_warnings(&self, value: WarningSettings) -> io::Result<()> {
        self.update_global(|settings| settings.warnings = Some(value));
        self.flush()
    }
    pub fn get_packages(&self) -> Vec<PackageSource> {
        self.settings().packages.unwrap_or_default()
    }
    pub fn set_packages(&self, value: Vec<PackageSource>) -> io::Result<()> {
        self.update_global(|settings| settings.packages = Some(value));
        self.flush()
    }
    pub fn set_project_packages(&self, value: Vec<PackageSource>) -> io::Result<()> {
        self.update_project(|settings| settings.packages = Some(value))?;
        self.flush_project()
    }

    pub fn set_extension_paths(&self, value: Vec<String>) -> io::Result<()> {
        self.set_global_paths(value, |s, v| s.extensions = Some(v))
    }
    pub fn set_skill_paths(&self, value: Vec<String>) -> io::Result<()> {
        self.set_global_paths(value, |s, v| s.skills = Some(v))
    }
    pub fn set_prompt_template_paths(&self, value: Vec<String>) -> io::Result<()> {
        self.set_global_paths(value, |s, v| s.prompts = Some(v))
    }
    pub fn set_theme_paths(&self, value: Vec<String>) -> io::Result<()> {
        self.set_global_paths(value, |s, v| s.themes = Some(v))
    }
    pub fn set_project_extension_paths(&self, value: Vec<String>) -> io::Result<()> {
        self.set_project_paths(value, |s, v| s.extensions = Some(v))
    }
    pub fn set_project_skill_paths(&self, value: Vec<String>) -> io::Result<()> {
        self.set_project_paths(value, |s, v| s.skills = Some(v))
    }
    pub fn set_project_prompt_template_paths(&self, value: Vec<String>) -> io::Result<()> {
        self.set_project_paths(value, |s, v| s.prompts = Some(v))
    }
    pub fn set_project_theme_paths(&self, value: Vec<String>) -> io::Result<()> {
        self.set_project_paths(value, |s, v| s.themes = Some(v))
    }

    fn set_global_paths(
        &self,
        value: Vec<String>,
        set: impl FnOnce(&mut Settings, Vec<String>),
    ) -> io::Result<()> {
        self.update_global(|settings| set(settings, value));
        self.flush()
    }
    fn set_project_paths(
        &self,
        value: Vec<String>,
        set: impl FnOnce(&mut Settings, Vec<String>),
    ) -> io::Result<()> {
        self.update_project(|settings| set(settings, value))?;
        self.flush_project()
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
    fn update_project(&self, f: impl FnOnce(&mut Settings)) -> io::Result<()> {
        if !self.is_project_trusted() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "project settings are not trusted",
            ));
        }
        f(&mut self.state.lock().expect("settings lock").1);
        Ok(())
    }

    pub fn flush(&self) -> io::Result<()> {
        let Some((global, _)) = &self.paths else {
            return Ok(());
        };
        write_settings(global, &self.global_settings())
    }

    fn flush_project(&self) -> io::Result<()> {
        let Some((_, project)) = &self.paths else {
            return Ok(());
        };
        write_settings(project, &self.project_settings())
    }
}

fn write_settings(path: &Path, value: &Settings) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(tmp, path)
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
        fs::write(
            &global,
            r#"{"theme":"new","custom":{"enabled":true,"value":null}}"#,
        )
        .unwrap();
        manager.set_retry_enabled(false).unwrap();
        let saved: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&global).unwrap()).unwrap();
        assert_eq!(saved["theme"], "new");
        assert_eq!(saved["custom"]["enabled"], true);
        assert!(saved["custom"]["value"].is_null());
        assert!(saved.get("httpIdleTimeoutMs").is_none());
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
