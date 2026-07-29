use std::{
    fs,
    path::{Path, PathBuf},
};

use serde_yaml::Value;

use crate::{
    source_info::{SourceInfo, SourceOrigin, SourceScope, create_synthetic_source_info},
    utils::frontmatter::parse_frontmatter,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptTemplate {
    pub name: String,
    pub description: String,
    pub argument_hint: Option<String>,
    pub content: String,
    pub source_info: SourceInfo,
    pub file_path: String,
}

#[derive(Debug, Clone)]
pub struct LoadPromptTemplatesOptions {
    pub cwd: String,
    pub agent_dir: String,
    pub prompt_paths: Vec<String>,
    pub include_defaults: bool,
}

pub fn parse_command_args(args: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    for character in args.chars() {
        if let Some(open) = quote {
            if character == open {
                quote = None;
            } else {
                current.push(character);
            }
        } else if matches!(character, '\'' | '"') {
            quote = Some(character);
        } else if character.is_whitespace() {
            if !current.is_empty() {
                result.push(std::mem::take(&mut current));
            }
        } else {
            current.push(character);
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

pub fn substitute_args(content: &str, args: &[String]) -> String {
    let mut out = String::new();
    let mut rest = content;
    while let Some(start) = rest.find('$') {
        out.push_str(&rest[..start]);
        let tail = &rest[start + 1..];
        if let Some(after) = tail.strip_prefix('@') {
            if let Some(slice) = after.strip_prefix(':') {
                if let Some((consumed, replacement)) = parse_slice(slice, args) {
                    out.push_str(&replacement);
                    rest = &slice[consumed..];
                    continue;
                }
            }
            out.push_str(&args.join(" "));
            rest = after;
        } else if let Some(after) = tail.strip_prefix("ARGUMENTS") {
            out.push_str(&args.join(" "));
            rest = after;
        } else if let Some(after) = tail.strip_prefix('{') {
            if let Some((consumed, replacement)) = parse_default(after, args) {
                out.push_str(&replacement);
                rest = &after[consumed..];
            } else {
                out.push('$');
                rest = tail;
            }
        } else {
            let count = tail.bytes().take_while(u8::is_ascii_digit).count();
            if count == 0 {
                out.push('$');
                rest = tail;
            } else {
                let index = tail[..count]
                    .parse::<usize>()
                    .unwrap_or(0)
                    .saturating_sub(1);
                out.push_str(args.get(index).map_or("", String::as_str));
                rest = &tail[count..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn parse_default(input: &str, args: &[String]) -> Option<(usize, String)> {
    let end = input.find('}')?;
    let value = &input[..end];
    let (number, fallback) = value.split_once(":-")?;
    let index = number.parse::<usize>().ok()?.saturating_sub(1);
    Some((
        end + 1,
        args.get(index)
            .filter(|value| !value.is_empty())
            .cloned()
            .unwrap_or_else(|| fallback.into()),
    ))
}

fn parse_slice(input: &str, args: &[String]) -> Option<(usize, String)> {
    let count = input.bytes().take_while(u8::is_ascii_digit).count();
    if count == 0 {
        return None;
    }
    let start = input[..count].parse::<usize>().ok()?.saturating_sub(1);
    let remaining = &input[count..];
    if let Some(length) = remaining.strip_prefix(':') {
        let length_count = length.bytes().take_while(u8::is_ascii_digit).count();
        if length_count == 0 {
            return None;
        }
        let size = length[..length_count].parse::<usize>().ok()?;
        return Some((
            count + 1 + length_count,
            args.iter()
                .skip(start)
                .take(size)
                .cloned()
                .collect::<Vec<_>>()
                .join(" "),
        ));
    }
    Some((
        count,
        args.iter()
            .skip(start)
            .cloned()
            .collect::<Vec<_>>()
            .join(" "),
    ))
}

pub fn expand_prompt_template(text: &str, templates: &[PromptTemplate]) -> String {
    let Some(command) = text.strip_prefix('/') else {
        return text.into();
    };
    let (name, args) = command
        .split_once(char::is_whitespace)
        .unwrap_or((command, ""));
    templates
        .iter()
        .find(|template| template.name == name)
        .map_or_else(
            || text.into(),
            |template| substitute_args(&template.content, &parse_command_args(args.trim_start())),
        )
}

pub fn load_prompt_templates(options: LoadPromptTemplatesOptions) -> Vec<PromptTemplate> {
    let cwd = fs::canonicalize(&options.cwd).unwrap_or_else(|_| PathBuf::from(options.cwd));
    let agent_dir =
        fs::canonicalize(&options.agent_dir).unwrap_or_else(|_| PathBuf::from(options.agent_dir));
    let user_dir = agent_dir.join("prompts");
    let project_dir = cwd.join(".pi/prompts");
    let mut templates = Vec::new();
    if options.include_defaults {
        load_dir(&user_dir, SourceScope::User, &mut templates);
        load_dir(&project_dir, SourceScope::Project, &mut templates);
    }
    for path in options.prompt_paths {
        let path = PathBuf::from(path);
        let path = if path.is_absolute() {
            path
        } else {
            cwd.join(path)
        };
        load_path(&path, SourceScope::Temporary, &mut templates);
    }
    templates
}

fn load_dir(dir: &Path, scope: SourceScope, templates: &mut Vec<PromptTemplate>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries = entries
        .flatten()
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if path.extension().is_some_and(|extension| extension == "md") {
            load_file(&path, scope, templates);
        }
    }
}

fn load_path(path: &Path, scope: SourceScope, templates: &mut Vec<PromptTemplate>) {
    if path.is_dir() {
        load_dir(path, scope, templates);
    } else if path.extension().is_some_and(|extension| extension == "md") {
        load_file(path, scope, templates);
    }
}

fn load_file(path: &Path, scope: SourceScope, templates: &mut Vec<PromptTemplate>) {
    let Ok(raw) = fs::read_to_string(path) else {
        return;
    };
    let Ok(parsed) = parse_frontmatter(&raw) else {
        return;
    };
    let field = |name: &str| {
        parsed
            .frontmatter
            .get(Value::String(name.into()))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let description = field("description").unwrap_or_else(|| {
        let first = parsed
            .body
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("");
        if first.chars().count() > 60 {
            format!("{}...", first.chars().take(60).collect::<String>())
        } else {
            first.into()
        }
    });
    let base = path.parent().map(|path| path.display().to_string());
    templates.push(PromptTemplate {
        name: path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .into(),
        description,
        argument_hint: field("argument-hint"),
        content: parsed.body,
        source_info: create_synthetic_source_info(
            path.display().to_string(),
            "local",
            Some(scope),
            Some(SourceOrigin::TopLevel),
            base,
        ),
        file_path: path.display().to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_once_and_preserves_unknown_dollars() {
        assert_eq!(
            substitute_args(
                "$1 ${2:-two} ${@:2:2} $UNKNOWN",
                &["one".into(), "".into(), "three".into()]
            ),
            "one two  three $UNKNOWN"
        );
    }
}
