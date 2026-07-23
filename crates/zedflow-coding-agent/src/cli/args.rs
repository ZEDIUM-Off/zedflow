//! Command-line argument parsing for the coding-agent entry points.

use zedflow_agent::ThinkingLevel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Text,
    Json,
    Rpc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticKind {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub unknown_flags: Vec<(String, Option<String>)>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            provider: None,
            model: None,
            api_key: None,
            system_prompt: None,
            append_system_prompt: Vec::new(),
            thinking: None,
            continue_session: false,
            resume: false,
            help: false,
            version: false,
            mode: None,
            name: None,
            no_session: false,
            session: None,
            session_id: None,
            fork: None,
            session_dir: None,
            models: Vec::new(),
            tools: Vec::new(),
            exclude_tools: Vec::new(),
            no_tools: false,
            no_builtin_tools: false,
            extensions: Vec::new(),
            no_extensions: false,
            print: false,
            export: None,
            no_skills: false,
            skills: Vec::new(),
            prompt_templates: Vec::new(),
            no_prompt_templates: false,
            themes: Vec::new(),
            no_themes: false,
            no_context_files: false,
            list_models: None,
            offline: false,
            verbose: false,
            project_trust_override: None,
            messages: Vec::new(),
            file_args: Vec::new(),
            unknown_flags: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

const THINKING_LEVELS: &[(&str, ThinkingLevel)] = &[
    ("off", ThinkingLevel::Off),
    ("minimal", ThinkingLevel::Minimal),
    ("low", ThinkingLevel::Low),
    ("medium", ThinkingLevel::Medium),
    ("high", ThinkingLevel::High),
    ("xhigh", ThinkingLevel::XHigh),
];

pub fn is_valid_thinking_level(level: &str) -> bool {
    THINKING_LEVELS.iter().any(|(name, _)| *name == level)
}

fn split_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

pub fn parse_args(argv: &[String]) -> Args {
    let mut result = Args::default();
    let mut i = 0;
    let value = |i: &mut usize| -> Option<String> {
        (*i + 1 < argv.len()).then(|| {
            *i += 1;
            argv[*i].clone()
        })
    };
    while i < argv.len() {
        let arg = &argv[i];
        match arg.as_str() {
            "--help" | "-h" => result.help = true,
            "--version" | "-v" => result.version = true,
            "--mode" => {
                if let Some(v) = value(&mut i) {
                    result.mode = match v.as_str() {
                        "text" => Some(Mode::Text),
                        "json" => Some(Mode::Json),
                        "rpc" => Some(Mode::Rpc),
                        _ => None,
                    }
                }
            }
            "--continue" | "-c" => result.continue_session = true,
            "--resume" | "-r" => result.resume = true,
            "--provider" => result.provider = value(&mut i),
            "--model" => result.model = value(&mut i),
            "--api-key" => result.api_key = value(&mut i),
            "--system-prompt" => result.system_prompt = value(&mut i),
            "--append-system-prompt" => {
                if let Some(v) = value(&mut i) {
                    result.append_system_prompt.push(v)
                }
            }
            "--name" | "-n" => match value(&mut i) {
                Some(v) => result.name = Some(v),
                None => result.diagnostics.push(Diagnostic {
                    kind: DiagnosticKind::Error,
                    message: "--name requires a value".into(),
                }),
            },
            "--no-session" => result.no_session = true,
            "--session" => result.session = value(&mut i),
            "--session-id" => result.session_id = value(&mut i),
            "--fork" => result.fork = value(&mut i),
            "--session-dir" => result.session_dir = value(&mut i),
            "--models" => {
                if let Some(v) = value(&mut i) {
                    result.models = split_list(&v)
                }
            }
            "--no-tools" | "-nt" => result.no_tools = true,
            "--no-builtin-tools" | "-nbt" => result.no_builtin_tools = true,
            "--tools" | "-t" => {
                if let Some(v) = value(&mut i) {
                    result.tools = split_list(&v)
                }
            }
            "--exclude-tools" | "-xt" => {
                if let Some(v) = value(&mut i) {
                    result.exclude_tools = split_list(&v)
                }
            }
            "--thinking" => {
                if let Some(v) = value(&mut i) {
                    if let Some((_, level)) = THINKING_LEVELS.iter().find(|(name, _)| *name == v) {
                        result.thinking = Some(*level)
                    } else {
                        result.diagnostics.push(Diagnostic { kind: DiagnosticKind::Warning, message: format!("Invalid thinking level \"{v}\". Valid values: off, minimal, low, medium, high, xhigh") })
                    }
                }
            }
            "--print" | "-p" => {
                result.print = true;
                if let Some(next) = argv.get(i + 1) {
                    if !next.starts_with('@') && (!next.starts_with('-') || next.starts_with("---"))
                    {
                        result.messages.push(next.clone());
                        i += 1;
                    }
                }
            }
            "--export" => result.export = value(&mut i),
            "--extension" | "-e" => {
                if let Some(v) = value(&mut i) {
                    result.extensions.push(v)
                }
            }
            "--no-extensions" | "-ne" => result.no_extensions = true,
            "--skill" => {
                if let Some(v) = value(&mut i) {
                    result.skills.push(v)
                }
            }
            "--prompt-template" => {
                if let Some(v) = value(&mut i) {
                    result.prompt_templates.push(v)
                }
            }
            "--theme" => {
                if let Some(v) = value(&mut i) {
                    result.themes.push(v)
                }
            }
            "--no-skills" | "-ns" => result.no_skills = true,
            "--no-prompt-templates" | "-np" => result.no_prompt_templates = true,
            "--no-themes" => result.no_themes = true,
            "--no-context-files" | "-nc" => result.no_context_files = true,
            "--list-models" => {
                let next = argv.get(i + 1);
                if let Some(v) = next.filter(|v| !v.starts_with('-') && !v.starts_with('@')) {
                    result.list_models = Some(Some(v.clone()));
                    i += 1
                } else {
                    result.list_models = Some(None)
                }
            }
            "--verbose" => result.verbose = true,
            "--approve" | "-a" => result.project_trust_override = Some(true),
            "--no-approve" | "-na" => result.project_trust_override = Some(false),
            "--offline" => result.offline = true,
            _ if arg.starts_with('@') => result.file_args.push(arg[1..].to_owned()),
            _ if arg.starts_with("--") => {
                let flag = &arg[2..];
                if let Some((name, val)) = flag.split_once('=') {
                    result.unknown_flags.push((name.into(), Some(val.into())))
                } else if argv
                    .get(i + 1)
                    .is_some_and(|v| !v.starts_with('-') && !v.starts_with('@'))
                {
                    i += 1;
                    result
                        .unknown_flags
                        .push((flag.into(), Some(argv[i].clone())))
                } else {
                    result.unknown_flags.push((flag.into(), None))
                }
            }
            _ if arg.starts_with('-') => result.diagnostics.push(Diagnostic {
                kind: DiagnosticKind::Error,
                message: format!("Unknown option: {arg}"),
            }),
            _ => result.messages.push(arg.clone()),
        }
        i += 1;
    }
    result
}

pub fn help_text() -> String {
    "pi - AI coding assistant\n\nUsage: pi [options] [@files...] [messages...]\n\nOptions:\n  --mode <text|json|rpc>\n  --print, -p\n  --continue, -c\n  --resume, -r\n  --model <pattern>\n  --thinking <off|minimal|low|medium|high|xhigh>\n  --help, -h\n  --version, -v\n".into()
}
