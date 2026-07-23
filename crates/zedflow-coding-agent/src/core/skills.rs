use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use super::{
    diagnostics::{ResourceDiagnostic, ResourceDiagnosticType},
    source_info::{SourceInfo, SourceOrigin, SourceScope, create_synthetic_source_info},
};

const MAX_NAME_LENGTH: usize = 64;
const MAX_DESCRIPTION_LENGTH: usize = 1024;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SkillFrontmatter {
    pub name: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "disable-model-invocation")]
    pub disable_model_invocation: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub file_path: String,
    pub base_dir: String,
    pub source_info: SourceInfo,
    pub disable_model_invocation: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadSkillsResult {
    pub skills: Vec<Skill>,
    pub diagnostics: Vec<ResourceDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct LoadSkillsFromDirOptions {
    pub dir: String,
    pub source: String,
}
#[derive(Debug, Clone)]
pub struct LoadSkillsOptions {
    pub cwd: String,
    pub agent_dir: String,
    pub skill_paths: Vec<String>,
    pub include_defaults: bool,
}

fn diagnostic(message: impl Into<String>, path: impl Into<String>) -> ResourceDiagnostic {
    ResourceDiagnostic {
        r#type: ResourceDiagnosticType::Warning,
        message: message.into(),
        path: Some(path.into()),
        collision: None,
    }
}

fn source_info(path: &Path, base: &Path, source: &str) -> SourceInfo {
    let (source_name, scope) = match source {
        "user" => ("local", SourceScope::User),
        "project" => ("local", SourceScope::Project),
        _ => (source, SourceScope::Temporary),
    };
    create_synthetic_source_info(
        path.display().to_string(),
        source_name,
        Some(scope),
        Some(SourceOrigin::TopLevel),
        Some(base.display().to_string()),
    )
}

fn parse_skill(path: &Path, source: &str) -> (Option<Skill>, Vec<ResourceDiagnostic>) {
    let mut diagnostics = Vec::new();
    let raw = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(error) => {
            diagnostics.push(diagnostic(error.to_string(), path.display().to_string()));
            return (None, diagnostics);
        }
    };
    let (frontmatter, has_frontmatter) = if let Some(rest) = raw.strip_prefix("---") {
        if let Some(end) = rest.find("\n---") {
            let yaml = &rest[1..end];
            match serde_yaml::from_str::<SkillFrontmatter>(yaml) {
                Ok(value) => (value, true),
                Err(error) => {
                    diagnostics.push(diagnostic(error.to_string(), path.display().to_string()));
                    return (None, diagnostics);
                }
            }
        } else {
            (SkillFrontmatter::default(), false)
        }
    } else {
        (SkillFrontmatter::default(), false)
    };
    let description = frontmatter.description.unwrap_or_default();
    if description.trim().is_empty() {
        diagnostics.push(diagnostic(
            "description is required",
            path.display().to_string(),
        ));
        return (None, diagnostics);
    }
    if description.len() > MAX_DESCRIPTION_LENGTH {
        diagnostics.push(diagnostic(
            format!(
                "description exceeds {MAX_DESCRIPTION_LENGTH} characters ({})",
                description.len()
            ),
            path.display().to_string(),
        ));
    }
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    let name = frontmatter.name.unwrap_or_else(|| {
        base.file_name()
            .and_then(|v| v.to_str())
            .unwrap_or_default()
            .to_owned()
    });
    let valid = !name.is_empty()
        && name.len() <= MAX_NAME_LENGTH
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--");
    if !valid {
        diagnostics.push(diagnostic(
            if name.len() > MAX_NAME_LENGTH {
                format!("name exceeds {MAX_NAME_LENGTH} characters ({})", name.len())
            } else {
                "name contains invalid characters (must be lowercase a-z, 0-9, hyphens only)"
                    .to_owned()
            },
            path.display().to_string(),
        ));
    }
    let _ = has_frontmatter;
    (
        Some(Skill {
            name,
            description,
            file_path: path.display().to_string(),
            base_dir: base.display().to_string(),
            source_info: source_info(path, base, source),
            disable_model_invocation: frontmatter.disable_model_invocation == Some(true),
        }),
        diagnostics,
    )
}

fn load_dir(dir: &Path, source: &str, root_files: bool, out: &mut LoadSkillsResult) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let entries: Vec<_> = entries.flatten().collect();
    if let Some(entry) = entries
        .iter()
        .find(|entry| entry.file_name() == "SKILL.md" && entry.path().is_file())
    {
        let (skill, diagnostics) = parse_skill(&entry.path(), source);
        out.diagnostics.extend(diagnostics);
        if let Some(skill) = skill {
            out.skills.push(skill);
        }
        return;
    }
    for entry in entries {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || name == "node_modules" {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            load_dir(&path, source, false, out);
        } else if root_files && name.ends_with(".md") {
            let (skill, diagnostics) = parse_skill(&path, source);
            out.diagnostics.extend(diagnostics);
            if let Some(skill) = skill {
                out.skills.push(skill);
            }
        }
    }
}

pub fn load_skills_from_dir(options: LoadSkillsFromDirOptions) -> LoadSkillsResult {
    let mut out = LoadSkillsResult::default();
    load_dir(Path::new(&options.dir), &options.source, true, &mut out);
    out
}

pub fn format_skills_for_prompt(skills: &[Skill]) -> String {
    let visible: Vec<_> = skills
        .iter()
        .filter(|skill| !skill.disable_model_invocation)
        .collect();
    if visible.is_empty() {
        return String::new();
    }
    let mut out = vec!["\n\nThe following skills provide specialized instructions for specific tasks.".to_owned(), "Use the read tool to load a skill's file when the task matches its description.".to_owned(), "When a skill file references a relative path, resolve it against the skill directory (parent of SKILL.md / dirname of the path) and use that absolute path in tool commands.".to_owned(), String::new(), "<available_skills>".to_owned()];
    for skill in visible {
        out.extend([
            "  <skill>".to_owned(),
            format!("    <name>{}</name>", escape_xml(&skill.name)),
            format!(
                "    <description>{}</description>",
                escape_xml(&skill.description)
            ),
            format!("    <location>{}</location>", escape_xml(&skill.file_path)),
            "  </skill>".to_owned(),
        ]);
    }
    out.push("</available_skills>".to_owned());
    out.join("\n")
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn load_skills(options: LoadSkillsOptions) -> LoadSkillsResult {
    let cwd = fs::canonicalize(&options.cwd).unwrap_or_else(|_| PathBuf::from(&options.cwd));
    let agent = PathBuf::from(&options.agent_dir);
    let mut out = LoadSkillsResult::default();
    let mut names = HashMap::<String, String>::new();
    let mut add = |result: LoadSkillsResult, out: &mut LoadSkillsResult| {
        out.diagnostics.extend(result.diagnostics);
        for skill in result.skills {
            if names.contains_key(&skill.name) {
                out.diagnostics.push(ResourceDiagnostic {
                    r#type: ResourceDiagnosticType::Collision,
                    message: format!("name \"{}\" collision", skill.name),
                    path: Some(skill.file_path.clone()),
                    collision: None,
                });
            } else {
                names.insert(skill.name.clone(), skill.file_path.clone());
                out.skills.push(skill);
            }
        }
    };
    if options.include_defaults {
        add(
            load_skills_from_dir(LoadSkillsFromDirOptions {
                dir: agent.join("skills").display().to_string(),
                source: "user".into(),
            }),
            &mut out,
        );
        add(
            load_skills_from_dir(LoadSkillsFromDirOptions {
                dir: cwd.join(".pi/skills").display().to_string(),
                source: "project".into(),
            }),
            &mut out,
        );
    }
    for raw in options.skill_paths {
        let path = PathBuf::from(raw);
        if !path.exists() {
            out.diagnostics.push(diagnostic(
                "skill path does not exist",
                path.display().to_string(),
            ));
            continue;
        }
        if path.is_dir() {
            add(
                load_skills_from_dir(LoadSkillsFromDirOptions {
                    dir: path.display().to_string(),
                    source: "path".into(),
                }),
                &mut out,
            );
        } else if path.extension().and_then(|v| v.to_str()) == Some("md") {
            let (skill, diagnostics) = parse_skill(&path, "path");
            let result = LoadSkillsResult {
                skills: skill.into_iter().collect(),
                diagnostics,
            };
            add(result, &mut out);
        } else {
            out.diagnostics.push(diagnostic(
                "skill path is not a markdown file",
                path.display().to_string(),
            ));
        }
    }
    out
}
