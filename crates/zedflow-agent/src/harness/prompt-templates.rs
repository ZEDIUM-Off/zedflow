use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::harness::types::{ExecutionEnv, FileErrorCode, FileInfo, FileKind, PromptTemplate};

/// Stable prompt-template diagnostic codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptTemplateDiagnosticCode {
    /// Metadata lookup failed.
    FileInfoFailed,
    /// Directory listing failed.
    ListFailed,
    /// File read failed.
    ReadFailed,
    /// Frontmatter parse failed.
    ParseFailed,
}

/// Warning produced while loading prompt templates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptTemplateDiagnostic {
    /// Diagnostic severity. Currently only warnings are emitted.
    #[serde(rename = "type")]
    pub diagnostic_type: String,
    /// Stable diagnostic code.
    pub code: PromptTemplateDiagnosticCode,
    /// Human-readable message.
    pub message: String,
    /// Path associated with the diagnostic.
    pub path: String,
}

/// Result of loading prompt templates.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadPromptTemplatesResult {
    /// Loaded templates.
    pub prompt_templates: Vec<PromptTemplate>,
    /// Non-fatal diagnostics.
    pub diagnostics: Vec<PromptTemplateDiagnostic>,
}

/// A source-tagged loaded prompt template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcedPromptTemplate<TSource, TPromptTemplate = PromptTemplate> {
    /// Loaded prompt template.
    pub prompt_template: TPromptTemplate,
    /// Caller-provided source value.
    pub source: TSource,
}

/// Result of loading source-tagged prompt templates.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadSourcedPromptTemplatesResult<TSource, TPromptTemplate = PromptTemplate> {
    /// Loaded templates with sources.
    pub prompt_templates: Vec<SourcedPromptTemplate<TSource, TPromptTemplate>>,
    /// Diagnostics with sources.
    pub diagnostics: Vec<SourcedPromptTemplateDiagnostic<TSource>>,
}

/// Source-tagged prompt-template diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcedPromptTemplateDiagnostic<TSource> {
    /// Prompt diagnostic.
    #[serde(flatten)]
    pub diagnostic: PromptTemplateDiagnostic,
    /// Caller-provided source value.
    pub source: TSource,
}

#[derive(Debug, Default, Deserialize)]
struct PromptTemplateFrontmatter {
    description: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "argument-hint")]
    argument_hint: Option<Value>,
}

/// Load prompt templates from files or direct markdown children of directories.
///
/// # Errors
///
/// This function records recoverable filesystem and parse failures as diagnostics.
pub async fn load_prompt_templates(
    env: &dyn ExecutionEnv,
    paths: impl IntoIterator<Item = impl AsRef<str>>,
) -> LoadPromptTemplatesResult {
    let mut prompt_templates = Vec::new();
    let mut diagnostics = Vec::new();

    for path in paths {
        let path = path.as_ref();
        match env.file_info(path, None).await {
            Ok(info) => match resolve_kind(env, &info, &mut diagnostics).await {
                Some(FileKind::Directory) => {
                    let result = load_templates_from_dir(env, &info.path).await;
                    prompt_templates.extend(result.prompt_templates);
                    diagnostics.extend(result.diagnostics);
                }
                Some(FileKind::File) if info.name.ends_with(".md") => {
                    let result = load_template_from_file(env, &info.path).await;
                    if let Some(template) = result.0 {
                        prompt_templates.push(template);
                    }
                    diagnostics.extend(result.1);
                }
                _ => {}
            },
            Err(error) => {
                if error.code != FileErrorCode::NotFound {
                    diagnostics.push(diagnostic(
                        PromptTemplateDiagnosticCode::FileInfoFailed,
                        error.message,
                        path,
                    ));
                }
            }
        }
    }

    LoadPromptTemplatesResult {
        prompt_templates,
        diagnostics,
    }
}

/// Load source-tagged prompt templates.
pub async fn load_sourced_prompt_templates<TSource, TPromptTemplate, F>(
    env: &dyn ExecutionEnv,
    inputs: impl IntoIterator<Item = (String, TSource)>,
    map_prompt_template: Option<F>,
) -> LoadSourcedPromptTemplatesResult<TSource, TPromptTemplate>
where
    TSource: Clone,
    TPromptTemplate: From<PromptTemplate>,
    F: Fn(PromptTemplate, &TSource) -> TPromptTemplate,
{
    let mut prompt_templates = Vec::new();
    let mut diagnostics = Vec::new();

    for (path, source) in inputs {
        let result = load_prompt_templates(env, [path]).await;
        for prompt_template in result.prompt_templates {
            let prompt_template = match &map_prompt_template {
                Some(mapper) => mapper(prompt_template, &source),
                None => TPromptTemplate::from(prompt_template),
            };
            prompt_templates.push(SourcedPromptTemplate {
                prompt_template,
                source: source.clone(),
            });
        }
        for diagnostic in result.diagnostics {
            diagnostics.push(SourcedPromptTemplateDiagnostic {
                diagnostic,
                source: source.clone(),
            });
        }
    }

    LoadSourcedPromptTemplatesResult {
        prompt_templates,
        diagnostics,
    }
}

async fn load_templates_from_dir(env: &dyn ExecutionEnv, dir: &str) -> LoadPromptTemplatesResult {
    let entries = match env.list_dir(dir, None).await {
        Ok(entries) => entries,
        Err(error) => {
            return LoadPromptTemplatesResult {
                prompt_templates: Vec::new(),
                diagnostics: vec![diagnostic(
                    PromptTemplateDiagnosticCode::ListFailed,
                    error.message,
                    dir,
                )],
            };
        }
    };

    let mut diagnostics = Vec::new();
    let mut prompt_templates = Vec::new();
    let mut entries = entries;
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    for entry in entries {
        match resolve_kind(env, &entry, &mut diagnostics).await {
            Some(FileKind::File) if entry.name.ends_with(".md") => {
                let result = load_template_from_file(env, &entry.path).await;
                if let Some(template) = result.0 {
                    prompt_templates.push(template);
                }
                diagnostics.extend(result.1);
            }
            _ => {}
        }
    }

    LoadPromptTemplatesResult {
        prompt_templates,
        diagnostics,
    }
}

async fn load_template_from_file(
    env: &dyn ExecutionEnv,
    file_path: &str,
) -> (Option<PromptTemplate>, Vec<PromptTemplateDiagnostic>) {
    let raw_content = match env.read_text_file(file_path, None).await {
        Ok(content) => content,
        Err(error) => {
            return (
                None,
                vec![diagnostic(
                    PromptTemplateDiagnosticCode::ReadFailed,
                    error.message,
                    file_path,
                )],
            );
        }
    };

    let parsed = match parse_frontmatter::<PromptTemplateFrontmatter>(&raw_content) {
        Ok(parsed) => parsed,
        Err(message) => {
            return (
                None,
                vec![diagnostic(
                    PromptTemplateDiagnosticCode::ParseFailed,
                    message,
                    file_path,
                )],
            );
        }
    };

    let first_line = parsed.body.lines().find(|line| !line.trim().is_empty());
    let mut description = parsed.frontmatter.description.unwrap_or_default();
    if description.is_empty() {
        if let Some(first_line) = first_line {
            description = first_line.chars().take(60).collect();
            if first_line.chars().count() > 60 {
                description.push_str("...");
            }
        }
    }

    (
        Some(PromptTemplate {
            name: basename_env_path(file_path).trim_end_matches_ignore_case(".md"),
            description: Some(description),
            content: parsed.body,
        }),
        Vec::new(),
    )
}

async fn resolve_kind(
    env: &dyn ExecutionEnv,
    info: &FileInfo,
    diagnostics: &mut Vec<PromptTemplateDiagnostic>,
) -> Option<FileKind> {
    if matches!(info.kind, FileKind::File | FileKind::Directory) {
        return Some(info.kind);
    }
    let canonical_path = match env.canonical_path(&info.path, None).await {
        Ok(path) => path,
        Err(error) => {
            if error.code != FileErrorCode::NotFound {
                diagnostics.push(diagnostic(
                    PromptTemplateDiagnosticCode::FileInfoFailed,
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
                PromptTemplateDiagnosticCode::FileInfoFailed,
                error.message,
                &info.path,
            ));
            None
        }
        _ => None,
    }
}

/// Parsed YAML frontmatter and body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFrontmatter<T> {
    /// YAML metadata.
    pub frontmatter: T,
    /// Markdown body.
    pub body: String,
}

/// Parse Pi-style markdown frontmatter using `yaml_serde`.
///
/// # Errors
///
/// Returns YAML parse errors from `yaml_serde` as strings.
pub fn parse_frontmatter<T>(content: &str) -> Result<ParsedFrontmatter<T>, String>
where
    T: DeserializeOwned + Default,
{
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
    if !normalized.starts_with("---") {
        return Ok(ParsedFrontmatter {
            frontmatter: T::default(),
            body: normalized,
        });
    }
    let Some(end_index) = normalized[3..].find("\n---").map(|index| index + 3) else {
        return Ok(ParsedFrontmatter {
            frontmatter: T::default(),
            body: normalized,
        });
    };
    let yaml = &normalized[4..end_index];
    let body = normalized[end_index + 4..].trim().to_string();
    let frontmatter = yaml_serde::from_str(yaml).map_err(|error| error.to_string())?;
    Ok(ParsedFrontmatter { frontmatter, body })
}

/// Parse a shell-style argument string with simple single and double quotes.
#[must_use]
pub fn parse_command_args(args_string: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quote = None;

    for character in args_string.chars() {
        if let Some(quote) = in_quote {
            if character == quote {
                in_quote = None;
            } else {
                current.push(character);
            }
        } else if character == '"' || character == '\'' {
            in_quote = Some(character);
        } else if character == ' ' || character == '\t' {
            if !current.is_empty() {
                args.push(std::mem::take(&mut current));
            }
        } else {
            current.push(character);
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

/// Substitute prompt template placeholders with command arguments.
#[must_use]
pub fn substitute_args(content: &str, args: &[String]) -> String {
    let chars: Vec<char> = content.chars().collect();
    let mut output = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] != '$' {
            output.push(chars[index]);
            index += 1;
            continue;
        }
        if let Some((replacement, consumed)) = placeholder_replacement(&chars[index..], args) {
            output.push_str(&replacement);
            index += consumed;
        } else {
            output.push('$');
            index += 1;
        }
    }
    output
}

/// Format a prompt template invocation with positional arguments.
#[must_use]
pub fn format_prompt_template_invocation(template: &PromptTemplate, args: &[String]) -> String {
    substitute_args(&template.content, args)
}

fn placeholder_replacement(chars: &[char], args: &[String]) -> Option<(String, usize)> {
    if chars.len() >= 10 && chars[0..10].iter().collect::<String>() == "$ARGUMENTS" {
        return Some((args.join(" "), 10));
    }
    if chars.len() >= 2 && chars[1] == '@' {
        return Some((args.join(" "), 2));
    }
    if chars.len() >= 2 && chars[1].is_ascii_digit() {
        let mut end = 1;
        while end < chars.len() && chars[end].is_ascii_digit() {
            end += 1;
        }
        let number = chars[1..end]
            .iter()
            .collect::<String>()
            .parse::<usize>()
            .ok()?;
        return Some((
            args.get(number.saturating_sub(1))
                .cloned()
                .unwrap_or_default(),
            end,
        ));
    }
    if chars.len() >= 6 && chars[1] == '{' && chars[2] == '@' && chars[3] == ':' {
        let mut index = 4;
        while index < chars.len() && chars[index].is_ascii_digit() {
            index += 1;
        }
        let start = chars[4..index]
            .iter()
            .collect::<String>()
            .parse::<usize>()
            .ok()?;
        let mut length = None;
        if index < chars.len() && chars[index] == ':' {
            index += 1;
            let length_start = index;
            while index < chars.len() && chars[index].is_ascii_digit() {
                index += 1;
            }
            length = chars[length_start..index]
                .iter()
                .collect::<String>()
                .parse::<usize>()
                .ok();
        }
        if index >= chars.len() || chars[index] != '}' {
            return None;
        }
        let start = start.saturating_sub(1);
        let selected = match length {
            Some(length) => args.iter().skip(start).take(length),
            None => args.iter().skip(start).take(usize::MAX),
        };
        return Some((selected.cloned().collect::<Vec<_>>().join(" "), index + 1));
    }
    None
}

fn diagnostic(
    code: PromptTemplateDiagnosticCode,
    message: impl Into<String>,
    path: impl Into<String>,
) -> PromptTemplateDiagnostic {
    PromptTemplateDiagnostic {
        diagnostic_type: "warning".to_string(),
        code,
        message: message.into(),
        path: path.into(),
    }
}

trait TrimEndMatchesIgnoreCase {
    fn trim_end_matches_ignore_case(&self, suffix: &str) -> String;
}

impl TrimEndMatchesIgnoreCase for str {
    fn trim_end_matches_ignore_case(&self, suffix: &str) -> String {
        if self.len() >= suffix.len()
            && self[self.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
        {
            self[..self.len() - suffix.len()].to_string()
        } else {
            self.to_string()
        }
    }
}

fn basename_env_path(path: &str) -> String {
    let normalized = path.trim_end_matches('/');
    normalized
        .rsplit_once('/')
        .map_or(normalized, |(_, basename)| basename)
        .to_string()
}
