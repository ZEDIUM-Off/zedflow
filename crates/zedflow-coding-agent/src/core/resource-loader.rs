use std::{
    fs,
    path::{Path, PathBuf},
};

use super::{
    diagnostics::ResourceDiagnostic,
    extensions::{
        ExtensionError, LoadExtensionsResult, NativeExtensionArtifact, NativeExtensionInstall,
        discover_and_load_extensions,
    },
    prompt_templates::{LoadPromptTemplatesOptions, PromptTemplate, load_prompt_templates},
    skills::{LoadSkillsOptions, LoadSkillsResult, load_skills},
    system_prompt::build_system_prompt,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub name: String,
    pub file_path: String,
}
#[derive(Debug, Clone, Default)]
pub struct ResourceExtensionPaths {
    pub skill_paths: Vec<PathBuf>,
    pub prompt_paths: Vec<PathBuf>,
    pub theme_paths: Vec<PathBuf>,
    /// Native code is accepted only from a locally built source-install receipt.
    pub native_extensions: Vec<NativeExtensionInstall>,
}
#[derive(Debug, Clone, Default)]
pub struct ResourceLoaderReloadOptions;

pub trait ResourceLoader {
    fn get_extensions(&self) -> &LoadExtensionsResult;
    fn get_skills(&self) -> &LoadSkillsResult;
    fn get_prompts(&self) -> (&[PromptTemplate], &[ResourceDiagnostic]);
    fn get_themes(&self) -> (&[Theme], &[ResourceDiagnostic]);
    fn get_agents_files(&self) -> &[(String, String)];
    fn get_system_prompt(&self) -> Option<&str>;
    fn get_append_system_prompt(&self) -> &[String];
    fn extend_resources(&mut self, paths: ResourceExtensionPaths);
    fn reload(&mut self);
}

pub fn load_project_context_files(
    cwd: impl AsRef<Path>,
    agent_dir: impl AsRef<Path>,
) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut dirs = Vec::new();
    let cwd = cwd.as_ref();
    let mut current = cwd.to_path_buf();
    while let Some(parent) = current.parent() {
        dirs.push(current.clone());
        if parent == current {
            break;
        }
        current = parent.to_path_buf();
    }
    dirs.reverse();
    dirs.push(agent_dir.as_ref().to_path_buf());
    for dir in dirs {
        for name in ["AGENTS.md", "AGENTS.MD", "CLAUDE.md", "CLAUDE.MD"] {
            let path = dir.join(name);
            if let Ok(content) = fs::read_to_string(&path) {
                if !result
                    .iter()
                    .any(|(known, _): &(String, String)| known == &path.display().to_string())
                {
                    result.push((path.display().to_string(), content));
                }
                break;
            }
        }
    }
    result
}

pub struct DefaultResourceLoader {
    cwd: PathBuf,
    agent_dir: PathBuf,
    extensions: LoadExtensionsResult,
    skills: LoadSkillsResult,
    prompts: Vec<PromptTemplate>,
    prompt_diagnostics: Vec<ResourceDiagnostic>,
    themes: Vec<Theme>,
    theme_diagnostics: Vec<ResourceDiagnostic>,
    agents_files: Vec<(String, String)>,
    system_prompt: Option<String>,
    append_system_prompt: Vec<String>,
    extra: ResourceExtensionPaths,
    native_extension_artifacts: Vec<NativeExtensionArtifact>,
}
impl DefaultResourceLoader {
    #[must_use]
    pub fn new(cwd: impl Into<PathBuf>, agent_dir: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            agent_dir: agent_dir.into(),
            extensions: LoadExtensionsResult::default(),
            skills: LoadSkillsResult::default(),
            prompts: Vec::new(),
            prompt_diagnostics: Vec::new(),
            themes: Vec::new(),
            theme_diagnostics: Vec::new(),
            agents_files: Vec::new(),
            system_prompt: None,
            append_system_prompt: Vec::new(),
            extra: ResourceExtensionPaths::default(),
            native_extension_artifacts: Vec::new(),
        }
    }
    pub fn reload(&mut self) {
        // Build a complete candidate first. A bad extension must not leave a
        // partially replaced resource set active.
        let extension_dir = self.cwd.join(".pi/extensions");
        let extensions = if extension_dir.exists() && !extension_dir.is_dir() {
            LoadExtensionsResult {
                extensions: Vec::new(),
                errors: vec![ExtensionError {
                    message: format!(
                        "extension directory is not a directory: {}",
                        extension_dir.display()
                    ),
                    source: None,
                }],
            }
        } else {
            discover_and_load_extensions(&self.cwd)
        };
        if !extensions.errors.is_empty() {
            self.extensions.errors.extend(extensions.errors);
            return;
        }
        let persisted_native_extensions =
            match NativeExtensionInstall::load_persisted(&extension_dir) {
                Ok(installs) => installs,
                Err(message) => {
                    self.extensions.errors.push(ExtensionError {
                        message,
                        source: None,
                    });
                    return;
                }
            };
        let native_extension_artifacts = persisted_native_extensions
            .iter()
            .chain(&self.extra.native_extensions)
            .map(|install| {
                install
                    .resolve()
                    .map(|(path, sha256)| NativeExtensionArtifact {
                        path,
                        sha256,
                        trusted: true,
                    })
            })
            .collect::<Result<Vec<_>, _>>();
        let native_extension_artifacts = match native_extension_artifacts {
            Ok(artifacts) => artifacts,
            Err(message) => {
                self.extensions.errors.push(ExtensionError {
                    message,
                    source: None,
                });
                return;
            }
        };
        let skills = load_skills(LoadSkillsOptions {
            cwd: self.cwd.display().to_string(),
            agent_dir: self.agent_dir.display().to_string(),
            skill_paths: self
                .extra
                .skill_paths
                .iter()
                .map(|p| p.display().to_string())
                .collect(),
            include_defaults: true,
        });
        let prompts = load_prompt_templates(LoadPromptTemplatesOptions {
            cwd: self.cwd.display().to_string(),
            agent_dir: self.agent_dir.display().to_string(),
            prompt_paths: self
                .extra
                .prompt_paths
                .iter()
                .map(|p| p.display().to_string())
                .collect(),
            include_defaults: true,
        });
        let agents_files = load_project_context_files(&self.cwd, &self.agent_dir);
        self.extensions = extensions;
        self.native_extension_artifacts = native_extension_artifacts;
        self.skills = skills;
        self.prompts = prompts;
        self.prompt_diagnostics.clear();
        self.agents_files = agents_files;
    }
    #[must_use]
    pub fn get_extensions(&self) -> &LoadExtensionsResult {
        &self.extensions
    }
    #[must_use]
    pub fn native_extension_artifacts(&self) -> &[NativeExtensionArtifact] {
        &self.native_extension_artifacts
    }
    pub fn get_skills(&self) -> &LoadSkillsResult {
        &self.skills
    }
    #[must_use]
    pub fn get_prompts(&self) -> (&[PromptTemplate], &[ResourceDiagnostic]) {
        (&self.prompts, &self.prompt_diagnostics)
    }
    #[must_use]
    pub fn get_themes(&self) -> (&[Theme], &[ResourceDiagnostic]) {
        (&self.themes, &self.theme_diagnostics)
    }
    #[must_use]
    pub fn get_agents_files(&self) -> &[(String, String)] {
        &self.agents_files
    }
    #[must_use]
    pub fn get_system_prompt(&self) -> Option<&str> {
        self.system_prompt.as_deref()
    }
    #[must_use]
    pub fn get_append_system_prompt(&self) -> &[String] {
        &self.append_system_prompt
    }
    pub fn extend_resources(&mut self, paths: ResourceExtensionPaths) {
        self.extra.skill_paths.extend(paths.skill_paths);
        self.extra.prompt_paths.extend(paths.prompt_paths);
        self.extra.theme_paths.extend(paths.theme_paths);
        self.extra.native_extensions.extend(paths.native_extensions);
    }
}
impl ResourceLoader for DefaultResourceLoader {
    fn get_extensions(&self) -> &LoadExtensionsResult {
        self.get_extensions()
    }
    fn get_skills(&self) -> &LoadSkillsResult {
        self.get_skills()
    }
    fn get_prompts(&self) -> (&[PromptTemplate], &[ResourceDiagnostic]) {
        self.get_prompts()
    }
    fn get_themes(&self) -> (&[Theme], &[ResourceDiagnostic]) {
        self.get_themes()
    }
    fn get_agents_files(&self) -> &[(String, String)] {
        self.get_agents_files()
    }
    fn get_system_prompt(&self) -> Option<&str> {
        self.get_system_prompt()
    }
    fn get_append_system_prompt(&self) -> &[String] {
        self.get_append_system_prompt()
    }
    fn extend_resources(&mut self, paths: ResourceExtensionPaths) {
        self.extend_resources(paths);
    }
    fn reload(&mut self) {
        self.reload();
    }
}

#[allow(dead_code)]
fn _prompt_for(loader: &DefaultResourceLoader) -> String {
    build_system_prompt(&super::system_prompt::BuildSystemPromptOptions {
        cwd: loader.cwd.display().to_string(),
        skills: loader.skills.skills.clone(),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_extension_reload_keeps_the_active_set_and_reports_the_error() {
        let root =
            std::env::temp_dir().join(format!("zedflow-resource-loader-{}", std::process::id()));
        let extensions = root.join(".pi/extensions");
        fs::create_dir_all(&extensions).unwrap();
        fs::write(extensions.join("active.rs"), "active").unwrap();
        let mut loader = DefaultResourceLoader::new(&root, root.join("agent"));
        loader.reload();
        assert_eq!(loader.get_extensions().extensions[0].name, "active");

        fs::remove_dir_all(&extensions).unwrap();
        fs::write(&extensions, "not a directory").unwrap();
        loader.reload();
        assert_eq!(loader.get_extensions().extensions[0].name, "active");
        assert!(!loader.get_extensions().errors.is_empty());
        let _ = fs::remove_dir_all(&root);
    }
}
