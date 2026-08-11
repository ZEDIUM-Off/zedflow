use std::{io, path::Path};

use crate::{
    config::CONFIG_DIR_NAME,
    extensions::{
        LoadExtensionsResult, ProjectTrustEvent, ProjectTrustEventDecision,
        emit_project_trust_event,
    },
    settings_manager::DefaultProjectTrust,
    trust_manager::{
        ProjectTrustOption, ProjectTrustStore, get_project_trust_options,
        has_trust_requiring_project_resources,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Interactive,
    Print,
    Json,
    Rpc,
}

pub struct ResolveProjectTrustedOptions<'a> {
    pub cwd: &'a Path,
    pub trust_store: &'a ProjectTrustStore,
    pub trust_override: Option<bool>,
    pub default_project_trust: DefaultProjectTrust,
    pub extensions_result: Option<&'a LoadExtensionsResult>,
    pub has_ui: bool,
    /// Result selected by the UI. `None` represents cancellation or no UI.
    pub selected_option: Option<&'a ProjectTrustOption>,
}

#[must_use]
pub fn format_project_trust_prompt(cwd: &Path) -> String {
    format!(
        "Trust project folder?\n{}\n\nThis allows pi to load {CONFIG_DIR_NAME} settings and resources, install missing project packages, and execute project extensions.",
        cwd.display()
    )
}

pub fn project_trust_options(cwd: &Path) -> io::Result<Vec<ProjectTrustOption>> {
    get_project_trust_options(cwd, true)
}

/// Resolve trust in the same precedence order as Pi: CLI override, absence of
/// gated resources, extension decision, saved decision, global default, UI.
pub fn resolve_project_trusted(options: ResolveProjectTrustedOptions<'_>) -> io::Result<bool> {
    if let Some(trusted) = options.trust_override {
        return Ok(trusted);
    }
    if !has_trust_requiring_project_resources(options.cwd) {
        return Ok(true);
    }

    if let Some(extensions) = options.extensions_result {
        let event = ProjectTrustEvent {
            cwd: options.cwd.to_string_lossy().into_owned(),
            extensions: extensions.extensions.clone(),
        };
        match emit_project_trust_event(event, &extensions.extensions).decision {
            ProjectTrustEventDecision::Yes => return Ok(true),
            ProjectTrustEventDecision::No => return Ok(false),
            ProjectTrustEventDecision::Undecided => {}
        }
    }

    if let Some(decision) = options.trust_store.get(options.cwd)? {
        return Ok(decision);
    }
    match options.default_project_trust {
        DefaultProjectTrust::Always => return Ok(true),
        DefaultProjectTrust::Never => return Ok(false),
        DefaultProjectTrust::Ask => {}
    }
    if !options.has_ui {
        return Ok(false);
    }
    let Some(selected) = options.selected_option else {
        return Ok(false);
    };
    if !selected.updates.is_empty() {
        options.trust_store.set_many(&selected.updates)?;
    }
    Ok(selected.trusted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "zedflow-project-trust-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn no_ui_defaults_to_untrusted_when_project_resources_exist() {
        let root = temp_dir();
        let project = root.join("project");
        fs::create_dir_all(project.join(".pi")).unwrap();
        fs::write(project.join(".pi/settings.json"), "{}").unwrap();
        let store = ProjectTrustStore::new(root.join("agent"));
        let trusted = resolve_project_trusted(ResolveProjectTrustedOptions {
            cwd: &project,
            trust_store: &store,
            trust_override: None,
            default_project_trust: DefaultProjectTrust::Ask,
            extensions_result: None,
            has_ui: false,
            selected_option: None,
        })
        .unwrap();
        assert!(!trusted);
        fs::remove_dir_all(root).unwrap();
    }
}
