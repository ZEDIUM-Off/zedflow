use ignore::gitignore::GitignoreBuilder;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::harness::prompt_templates::{ParsedFrontmatter, parse_frontmatter};
use crate::harness::types::{ExecutionEnv, FileErrorCode, FileInfo, FileKind, Skill};

const MAX_NAME_LENGTH: usize = 64;
const MAX_DESCRIPTION_LENGTH: usize = 1024;
const IGNORE_FILE_NAMES: [&str; 3] = [".gitignore", ".ignore", ".fdignore"];

/// Stable skill diagnostic codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillDiagnosticCode {
    /// Metadata lookup failed.
    FileInfoFailed,
    /// Directory listing failed.
    ListFailed,
    /// File read failed.
    ReadFailed,
    /// Frontmatter parse failed.
    ParseFailed,
    /// Skill metadata is invalid.
    InvalidMetadata,
}

/// Warning produced while loading skills.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillDiagnostic {
    /// Diagnostic severity. Currently only warnings are emitted.
    #[serde(rename = "type")]
    pub diagnostic_type: String,
    /// Stable diagnostic code.
    pub code: SkillDiagnosticCode,
    /// Human-readable message.
    pub message: String,
    /// Path associated with the diagnostic.
    pub path: String,
}

/// Result of loading skills.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadSkillsResult {
    /// Loaded skills.
    pub skills: Vec<Skill>,
    /// Non-fatal diagnostics.
    pub diagnostics: Vec<SkillDiagnostic>,
}

/// A source-tagged loaded skill.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcedSkill<TSource, TSkill = Skill> {
    /// Loaded skill.
    pub skill: TSkill,
    /// Caller-provided source value.
    pub source: TSource,
}

/// Source-tagged skill diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcedSkillDiagnostic<TSource> {
    /// Skill diagnostic.
    #[serde(flatten)]
    pub diagnostic: SkillDiagnostic,
    /// Caller-provided source value.
    pub source: TSource,
}

/// Result of loading source-tagged skills.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadSourcedSkillsResult<TSource, TSkill = Skill> {
    /// Loaded skills with sources.
    pub skills: Vec<SourcedSkill<TSource, TSkill>>,
    /// Diagnostics with sources.
    pub diagnostics: Vec<SourcedSkillDiagnostic<TSource>>,
}

#[derive(Debug, Default, Deserialize)]
struct SkillFrontmatter {
    name: Option<String>,
    description: Option<String>,
    #[serde(rename = "disable-model-invocation")]
    disable_model_invocation: Option<bool>,
    #[allow(dead_code)]
    #[serde(flatten)]
    extra: std::collections::HashMap<String, Value>,
}

/// Format a skill invocation prompt.
#[must_use]
pub fn format_skill_invocation(skill: &Skill, additional_instructions: Option<&str>) -> String {
    let skill_block = format!(
        "<skill name=\"{}\" location=\"{}\">\nReferences are relative to {}.\n\n{}\n</skill>",
        skill.name,
        skill.file_path,
        dirname_env_path(&skill.file_path),
        skill.content
    );
    match additional_instructions {
        Some(instructions) if !instructions.is_empty() => {
            format!("{skill_block}\n\n{instructions}")
        }
        _ => skill_block,
    }
}

/// Load skills from one or more directories.
pub async fn load_skills(
    env: &dyn ExecutionEnv,
    dirs: impl IntoIterator<Item = impl AsRef<str>>,
) -> LoadSkillsResult {
    let mut skills = Vec::new();
    let mut diagnostics = Vec::new();

    for dir in dirs {
        let dir = dir.as_ref();
        let root_info = match env.file_info(dir, None).await {
            Ok(info) => info,
            Err(error) => {
                if error.code != FileErrorCode::NotFound {
                    diagnostics.push(diagnostic(
                        SkillDiagnosticCode::FileInfoFailed,
                        error.message,
                        dir,
                    ));
                }
                continue;
            }
        };
        if resolve_kind(env, &root_info, &mut diagnostics).await != Some(FileKind::Directory) {
            continue;
        }
        let mut ignore = SkillIgnore::new(&root_info.path);
        let result =
            load_skills_from_dir_internal(env, &root_info.path, true, &mut ignore, &root_info.path)
                .await;
        skills.extend(result.skills);
        diagnostics.extend(result.diagnostics);
    }

    LoadSkillsResult {
        skills,
        diagnostics,
    }
}

/// Load source-tagged skills.
pub async fn load_sourced_skills<TSource, TSkill, F>(
    env: &dyn ExecutionEnv,
    inputs: impl IntoIterator<Item = (String, TSource)>,
    map_skill: Option<F>,
) -> LoadSourcedSkillsResult<TSource, TSkill>
where
    TSource: Clone,
    TSkill: From<Skill>,
    F: Fn(Skill, &TSource) -> TSkill,
{
    let mut skills = Vec::new();
    let mut diagnostics = Vec::new();

    for (path, source) in inputs {
        let result = load_skills(env, [path]).await;
        for skill in result.skills {
            let skill = match &map_skill {
                Some(mapper) => mapper(skill, &source),
                None => TSkill::from(skill),
            };
            skills.push(SourcedSkill {
                skill,
                source: source.clone(),
            });
        }
        for diagnostic in result.diagnostics {
            diagnostics.push(SourcedSkillDiagnostic {
                diagnostic,
                source: source.clone(),
            });
        }
    }

    LoadSourcedSkillsResult {
        skills,
        diagnostics,
    }
}

async fn load_skills_from_dir_internal(
    env: &dyn ExecutionEnv,
    dir: &str,
    include_root_files: bool,
    ignore: &mut SkillIgnore,
    root_dir: &str,
) -> LoadSkillsResult {
    let dir_info = match env.file_info(dir, None).await {
        Ok(info) => info,
        Err(error) => {
            let diagnostics = if error.code == FileErrorCode::NotFound {
                Vec::new()
            } else {
                vec![diagnostic(
                    SkillDiagnosticCode::FileInfoFailed,
                    error.message,
                    dir,
                )]
            };
            return LoadSkillsResult {
                skills: Vec::new(),
                diagnostics,
            };
        }
    };

    let mut diagnostics = Vec::new();
    if resolve_kind(env, &dir_info, &mut diagnostics).await != Some(FileKind::Directory) {
        return LoadSkillsResult {
            skills: Vec::new(),
            diagnostics,
        };
    }

    add_ignore_rules(env, ignore, dir, root_dir, &mut diagnostics).await;

    let entries = match env.list_dir(dir, None).await {
        Ok(entries) => entries,
        Err(error) => {
            diagnostics.push(diagnostic(
                SkillDiagnosticCode::ListFailed,
                error.message,
                dir,
            ));
            return LoadSkillsResult {
                skills: Vec::new(),
                diagnostics,
            };
        }
    };

    for entry in &entries {
        if entry.name != "SKILL.md" {
            continue;
        }
        if resolve_kind(env, entry, &mut diagnostics).await != Some(FileKind::File) {
            continue;
        }
        let rel_path = relative_env_path(root_dir, &entry.path);
        if ignore.ignores(&rel_path, false) {
            continue;
        }
        let (skill, skill_diagnostics) = load_skill_from_file(env, &entry.path).await;
        diagnostics.extend(skill_diagnostics);
        return LoadSkillsResult {
            skills: skill.into_iter().collect(),
            diagnostics,
        };
    }

    let mut entries = entries;
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    let mut skills = Vec::new();

    for entry in entries {
        if entry.name.starts_with('.') || entry.name == "node_modules" {
            continue;
        }
        let Some(kind) = resolve_kind(env, &entry, &mut diagnostics).await else {
            continue;
        };
        let rel_path = relative_env_path(root_dir, &entry.path);
        if ignore.ignores(&rel_path, kind == FileKind::Directory) {
            continue;
        }

        if kind == FileKind::Directory {
            let result = Box::pin(load_skills_from_dir_internal(
                env,
                &entry.path,
                false,
                ignore,
                root_dir,
            ))
            .await;
            skills.extend(result.skills);
            diagnostics.extend(result.diagnostics);
            continue;
        }

        if kind != FileKind::File || !include_root_files || !entry.name.ends_with(".md") {
            continue;
        }
        let (skill, skill_diagnostics) = load_skill_from_file(env, &entry.path).await;
        if let Some(skill) = skill {
            skills.push(skill);
        }
        diagnostics.extend(skill_diagnostics);
    }

    LoadSkillsResult {
        skills,
        diagnostics,
    }
}

async fn add_ignore_rules(
    env: &dyn ExecutionEnv,
    ignore: &mut SkillIgnore,
    dir: &str,
    root_dir: &str,
    diagnostics: &mut Vec<SkillDiagnostic>,
) {
    let relative_dir = relative_env_path(root_dir, dir);
    let prefix = if relative_dir.is_empty() {
        String::new()
    } else {
        format!("{relative_dir}/")
    };

    for filename in IGNORE_FILE_NAMES {
        let ignore_path = join_env_path(dir, filename);
        let info = match env.file_info(&ignore_path, None).await {
            Ok(info) => info,
            Err(error) => {
                if error.code != FileErrorCode::NotFound {
                    diagnostics.push(diagnostic(
                        SkillDiagnosticCode::FileInfoFailed,
                        error.message,
                        &ignore_path,
                    ));
                }
                continue;
            }
        };
        if info.kind != FileKind::File {
            continue;
        }
        let content = match env.read_text_file(&ignore_path, None).await {
            Ok(content) => content,
            Err(error) => {
                diagnostics.push(diagnostic(
                    SkillDiagnosticCode::ReadFailed,
                    error.message,
                    &ignore_path,
                ));
                continue;
            }
        };
        for line in content.lines() {
            if let Some(pattern) = prefix_ignore_pattern(line, &prefix) {
                ignore.add_pattern(&pattern);
            }
        }
    }
}

async fn load_skill_from_file(
    env: &dyn ExecutionEnv,
    file_path: &str,
) -> (Option<Skill>, Vec<SkillDiagnostic>) {
    let raw_content = match env.read_text_file(file_path, None).await {
        Ok(content) => content,
        Err(error) => {
            return (
                None,
                vec![diagnostic(
                    SkillDiagnosticCode::ReadFailed,
                    error.message,
                    file_path,
                )],
            );
        }
    };

    let ParsedFrontmatter { frontmatter, body } =
        match parse_frontmatter::<SkillFrontmatter>(&raw_content) {
            Ok(parsed) => parsed,
            Err(message) => {
                return (
                    None,
                    vec![diagnostic(
                        SkillDiagnosticCode::ParseFailed,
                        message,
                        file_path,
                    )],
                );
            }
        };

    let skill_dir = dirname_env_path(file_path);
    let parent_dir_name = basename_env_path(&skill_dir);
    let description = frontmatter.description;
    let mut diagnostics = Vec::new();

    for error in validate_description(description.as_deref()) {
        diagnostics.push(diagnostic(
            SkillDiagnosticCode::InvalidMetadata,
            error,
            file_path,
        ));
    }

    let name = frontmatter.name.unwrap_or(parent_dir_name.clone());
    for error in validate_name(&name, &parent_dir_name) {
        diagnostics.push(diagnostic(
            SkillDiagnosticCode::InvalidMetadata,
            error,
            file_path,
        ));
    }

    let Some(description) = description else {
        return (None, diagnostics);
    };
    if description.trim().is_empty() {
        return (None, diagnostics);
    }

    (
        Some(Skill {
            name,
            description,
            content: body,
            file_path: file_path.to_string(),
            disable_model_invocation: Some(frontmatter.disable_model_invocation.unwrap_or(false)),
        }),
        diagnostics,
    )
}

async fn resolve_kind(
    env: &dyn ExecutionEnv,
    info: &FileInfo,
    diagnostics: &mut Vec<SkillDiagnostic>,
) -> Option<FileKind> {
    if matches!(info.kind, FileKind::File | FileKind::Directory) {
        return Some(info.kind);
    }
    let canonical_path = match env.canonical_path(&info.path, None).await {
        Ok(path) => path,
        Err(error) => {
            if error.code != FileErrorCode::NotFound {
                diagnostics.push(diagnostic(
                    SkillDiagnosticCode::FileInfoFailed,
                    error.message,
                    &info.path,
                ));
            }
            return None;
        }
    };
    match env.file_info(&canonical_path, None).await {
        Ok(target) if matches!(target.kind, FileKind::File | FileKind::Directory) => {
            Some(target.kind)
        }
        Err(error) if error.code != FileErrorCode::NotFound => {
            diagnostics.push(diagnostic(
                SkillDiagnosticCode::FileInfoFailed,
                error.message,
                &info.path,
            ));
            None
        }
        _ => None,
    }
}

fn validate_name(name: &str, parent_dir_name: &str) -> Vec<String> {
    let mut errors = Vec::new();
    if name != parent_dir_name {
        errors.push(format!(
            "name \"{name}\" does not match parent directory \"{parent_dir_name}\""
        ));
    }
    if name.len() > MAX_NAME_LENGTH {
        errors.push(format!(
            "name exceeds {MAX_NAME_LENGTH} characters ({})",
            name.len()
        ));
    }
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        errors.push(
            "name contains invalid characters (must be lowercase a-z, 0-9, hyphens only)"
                .to_string(),
        );
    }
    if name.starts_with('-') || name.ends_with('-') {
        errors.push("name must not start or end with a hyphen".to_string());
    }
    if name.contains("--") {
        errors.push("name must not contain consecutive hyphens".to_string());
    }
    errors
}

fn validate_description(description: Option<&str>) -> Vec<String> {
    match description {
        None => vec!["description is required".to_string()],
        Some(description) if description.trim().is_empty() => {
            vec!["description is required".to_string()]
        }
        Some(description) if description.len() > MAX_DESCRIPTION_LENGTH => vec![format!(
            "description exceeds {MAX_DESCRIPTION_LENGTH} characters ({})",
            description.len()
        )],
        Some(_) => Vec::new(),
    }
}

fn prefix_ignore_pattern(line: &str, prefix: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() || (trimmed.starts_with('#') && !trimmed.starts_with("\\#")) {
        return None;
    }

    let mut pattern = line;
    let mut negated = false;
    if let Some(rest) = pattern.strip_prefix('!') {
        negated = true;
        pattern = rest;
    } else if let Some(rest) = pattern.strip_prefix("\\!") {
        pattern = rest;
    }
    if let Some(rest) = pattern.strip_prefix('/') {
        pattern = rest;
    }
    let prefixed = if prefix.is_empty() {
        pattern.to_string()
    } else {
        format!("{prefix}{pattern}")
    };
    Some(if negated {
        format!("!{prefixed}")
    } else {
        prefixed
    })
}

struct SkillIgnore {
    builder: GitignoreBuilder,
}

impl SkillIgnore {
    fn new(root: &str) -> Self {
        Self {
            builder: GitignoreBuilder::new(root),
        }
    }

    fn add_pattern(&mut self, pattern: &str) {
        let _ = self.builder.add_line(None, pattern);
    }

    fn ignores(&self, rel_path: &str, is_dir: bool) -> bool {
        self.builder
            .build()
            .map(|ignore| ignore.matched(rel_path, is_dir).is_ignore())
            .unwrap_or(false)
    }
}

fn diagnostic(
    code: SkillDiagnosticCode,
    message: impl Into<String>,
    path: impl Into<String>,
) -> SkillDiagnostic {
    SkillDiagnostic {
        diagnostic_type: "warning".to_string(),
        code,
        message: message.into(),
        path: path.into(),
    }
}

fn join_env_path(base: &str, child: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        child.trim_start_matches('/')
    )
}

fn dirname_env_path(path: &str) -> String {
    let normalized = path.trim_end_matches('/');
    let Some((dir, _)) = normalized.rsplit_once('/') else {
        return "/".to_string();
    };
    if dir.is_empty() {
        "/".to_string()
    } else {
        dir.to_string()
    }
}

fn basename_env_path(path: &str) -> String {
    let normalized = path.trim_end_matches('/');
    normalized
        .rsplit_once('/')
        .map_or(normalized, |(_, basename)| basename)
        .to_string()
}

fn relative_env_path(root: &str, path: &str) -> String {
    let normalized_root = root.trim_end_matches('/');
    let normalized_path = path.trim_end_matches('/');
    if normalized_path == normalized_root {
        return String::new();
    }
    normalized_path
        .strip_prefix(&format!("{normalized_root}/"))
        .unwrap_or_else(|| normalized_path.trim_start_matches('/'))
        .to_string()
}
