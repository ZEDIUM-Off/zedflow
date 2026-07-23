//! Command-line parsing for the coding-agent entry point.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use zedflow_agent::types::ThinkingLevel;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Text,
    Json,
    Rpc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnknownFlagValue {
    Bool(bool),
    String(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticType {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub kind: DiagnosticType,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct Args {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub system_prompt: Option<String>,
    pub append_system_prompt: Vec<String>,
    pub thinking: Option<ThinkingLevel>,
    pub continue_session: bool,
    pub resume: bool,
    pub help: bool,
    pub version: bool,
    pub mode: Option<Mode>,
    pub name: Option<String>,
    pub no_session: bool,
    pub session: Option<String>,
    pub session_id: Option<String>,
    pub fork: Option<String>,
    pub session_dir: Option<String>,
    pub models: Vec<String>,
    pub tools: Vec<String>,
    pub exclude_tools: Vec<String>,
    pub no_tools: bool,
    pub no_builtin_tools: bool,
    pub extensions: Vec<String>,
    pub no_extensions: bool,
    pub print: bool,
    pub export: Option<String>,
    pub no_skills: bool,
    pub skills: Vec<String>,
    pub prompt_templates: Vec<String>,
    pub no_prompt_templates: bool,
    pub themes: Vec<String>,
    pub no_themes: bool,
    pub no_context_files: bool,
    pub list_models: Option<Option<String>>,
    pub offline: bool,
    pub verbose: bool,
    pub project_trust_override: Option<bool>,
    pub messages: Vec<String>,
    pub file_args: Vec<String>,
    pub unknown_flags: HashMap<String, UnknownFlagValue>,
    pub diagnostics: Vec<Diagnostic>,
}

const VALID_THINKING: &[(&str, ThinkingLevel)] = &[
    ("off", ThinkingLevel::Off),
    ("minimal", ThinkingLevel::Minimal),
    ("low", ThinkingLevel::Low),
    ("medium", ThinkingLevel::Medium),
    ("high", ThinkingLevel::High),
    ("xhigh", ThinkingLevel::XHigh),
];

pub fn is_valid_thinking_level(value: &str) -> bool {
    VALID_THINKING.iter().any(|(name, _)| *name == value)
}

fn take_value(args: &[String], index: &mut usize) -> Option<String> {
    if *index + 1 < args.len() {
        *index += 1;
        Some(args[*index].clone())
    } else {
        None
    }
}

fn csv(value: String) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Parse Pi's CLI without consuming ordinary messages as values for known flags.
pub fn parse_args<I, S>(args: I) -> Args
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let args: Vec<String> = args.into_iter().map(Into::into).collect();
    let mut result = Args::default();
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "--help" | "-h" => result.help = true,
            "--version" | "-v" => result.version = true,
            "--continue" | "-c" => result.continue_session = true,
            "--resume" | "-r" => result.resume = true,
            "--no-session" => result.no_session = true,
            "--no-tools" | "-nt" => result.no_tools = true,
            "--no-builtin-tools" | "-nbt" => result.no_builtin_tools = true,
            "--no-extensions" | "-ne" => result.no_extensions = true,
            "--no-skills" | "-ns" => result.no_skills = true,
            "--no-prompt-templates" | "-np" => result.no_prompt_templates = true,
            "--no-themes" => result.no_themes = true,
            "--no-context-files" | "-nc" => result.no_context_files = true,
            "--verbose" => result.verbose = true,
            "--offline" => result.offline = true,
            "--approve" | "-a" => result.project_trust_override = Some(true),
            "--no-approve" | "-na" => result.project_trust_override = Some(false),
            "--print" | "-p" => {
                result.print = true;
                if let Some(next) = args.get(i + 1) {
                    if !next.starts_with('@') && (!next.starts_with('-') || next.starts_with("---"))
                    {
                        result.messages.push(next.clone());
                        i += 1;
                    }
                }
            }
            "--list-models" => {
                if let Some(next) = args.get(i + 1) {
                    if !next.starts_with('-') && !next.starts_with('@') {
                        result.list_models = Some(Some(next.clone()));
                        i += 1;
                    } else {
                        result.list_models = Some(None);
                    }
                } else {
                    result.list_models = Some(None);
                }
            }
            "--mode" => {
                if let Some(value) = take_value(&args, &mut i) {
                    result.mode = match value.as_str() {
                        "text" => Some(Mode::Text),
                        "json" => Some(Mode::Json),
                        "rpc" => Some(Mode::Rpc),
                        _ => result.mode,
                    };
                }
            }
            "--provider" => result.provider = take_value(&args, &mut i),
            "--model" => result.model = take_value(&args, &mut i),
            "--api-key" => result.api_key = take_value(&args, &mut i),
            "--system-prompt" => result.system_prompt = take_value(&args, &mut i),
            "--session" => result.session = take_value(&args, &mut i),
            "--session-id" => result.session_id = take_value(&args, &mut i),
            "--fork" => result.fork = take_value(&args, &mut i),
            "--session-dir" => result.session_dir = take_value(&args, &mut i),
            "--export" => result.export = take_value(&args, &mut i),
            "--models" => {
                if let Some(value) = take_value(&args, &mut i) {
                    result.models = csv(value);
                }
            }
            "--tools" | "-t" => {
                if let Some(value) = take_value(&args, &mut i) {
                    result.tools = csv(value);
                }
            }
            "--exclude-tools" | "-xt" => {
                if let Some(value) = take_value(&args, &mut i) {
                    result.exclude_tools = csv(value);
                }
            }
            "--thinking" => {
                if let Some(value) = take_value(&args, &mut i) {
                    if let Some((_, level)) = VALID_THINKING.iter().find(|(name, _)| *name == value)
                    {
                        result.thinking = Some(*level);
                    } else {
                        result.diagnostics.push(Diagnostic { kind: DiagnosticType::Warning, message: format!("Invalid thinking level \"{value}\". Valid values: off, minimal, low, medium, high, xhigh") });
                    }
                }
            }
            "--name" | "-n" => match take_value(&args, &mut i) {
                Some(value) => result.name = Some(value),
                None => result.diagnostics.push(Diagnostic {
                    kind: DiagnosticType::Error,
                    message: "--name requires a value".into(),
                }),
            },
            "--append-system-prompt" => {
                if let Some(value) = take_value(&args, &mut i) {
                    result.append_system_prompt.push(value)
                }
            }
            "--extension" | "-e" => {
                if let Some(value) = take_value(&args, &mut i) {
                    result.extensions.push(value)
                }
            }
            "--skill" => {
                if let Some(value) = take_value(&args, &mut i) {
                    result.skills.push(value)
                }
            }
            "--prompt-template" => {
                if let Some(value) = take_value(&args, &mut i) {
                    result.prompt_templates.push(value)
                }
            }
            "--theme" => {
                if let Some(value) = take_value(&args, &mut i) {
                    result.themes.push(value)
                }
            }
            _ if arg.starts_with('@') => result.file_args.push(arg[1..].to_owned()),
            _ if arg.starts_with("--") => {
                if let Some((name, value)) = arg[2..].split_once('=') {
                    result
                        .unknown_flags
                        .insert(name.into(), UnknownFlagValue::String(value.into()));
                } else {
                    let name = &arg[2..];
                    if let Some(next) = args
                        .get(i + 1)
                        .filter(|v| !v.starts_with('-') && !v.starts_with('@'))
                    {
                        result
                            .unknown_flags
                            .insert(name.into(), UnknownFlagValue::String(next.clone()));
                        i += 1;
                    } else {
                        result
                            .unknown_flags
                            .insert(name.into(), UnknownFlagValue::Bool(true));
                    }
                }
            }
            _ if arg.starts_with('-') => result.diagnostics.push(Diagnostic {
                kind: DiagnosticType::Error,
                message: format!("Unknown option: {arg}"),
            }),
            _ => result.messages.push(arg.clone()),
        }
        i += 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_frontmatter_prompt_after_print() {
        let parsed = parse_args(["-p", "---\ntitle: x\n---\nSay hi."]);
        assert_eq!(parsed.messages, vec!["---\ntitle: x\n---\nSay hi."]);
    }
    #[test]
    fn parses_unknown_flags() {
        let parsed = parse_args(["--flag=value", "--boolean"]);
        assert_eq!(
            parsed.unknown_flags["flag"],
            UnknownFlagValue::String("value".into())
        );
        assert_eq!(
            parsed.unknown_flags["boolean"],
            UnknownFlagValue::Bool(true)
        );
    }
}
