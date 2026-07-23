use super::skills::{Skill, format_skills_for_prompt};

#[derive(Debug, Clone, Default)]
pub struct BuildSystemPromptOptions {
    pub custom_prompt: Option<String>,
    pub selected_tools: Option<Vec<String>>,
    pub tool_snippets: Option<std::collections::HashMap<String, String>>,
    pub prompt_guidelines: Vec<String>,
    pub append_system_prompt: Option<String>,
    pub cwd: String,
    pub context_files: Vec<(String, String)>,
    pub skills: Vec<Skill>,
}

#[must_use]
pub fn build_system_prompt(options: &BuildSystemPromptOptions) -> String {
    let tools = options.selected_tools.clone().unwrap_or_else(|| {
        vec!["read", "bash", "edit", "write"]
            .into_iter()
            .map(String::from)
            .collect()
    });
    let visible = tools
        .iter()
        .filter_map(|name| {
            options
                .tool_snippets
                .as_ref()?
                .get(name)
                .map(|snippet| format!("- {name}: {snippet}"))
        })
        .collect::<Vec<_>>();
    let tools_list = if visible.is_empty() {
        "(none)".to_owned()
    } else {
        visible.join("\n")
    };
    let mut guidelines = Vec::<String>::new();
    if tools.contains(&"bash".into())
        && !tools
            .iter()
            .any(|tool| ["grep", "find", "ls"].contains(&tool.as_str()))
    {
        guidelines.push("Use bash for file operations like ls, rg, find".into());
    }
    for guideline in &options.prompt_guidelines {
        let trimmed = guideline.trim();
        if !trimmed.is_empty() && !guidelines.iter().any(|item| item == trimmed) {
            guidelines.push(trimmed.into());
        }
    }
    for guideline in [
        "Be concise in your responses",
        "Show file paths clearly when working with files",
    ] {
        if !guidelines.iter().any(|item| item == guideline) {
            guidelines.push(guideline.into());
        }
    }
    let guidelines = guidelines
        .iter()
        .map(|g| format!("- {g}"))
        .collect::<Vec<_>>()
        .join("\n");
    let append = options
        .append_system_prompt
        .as_deref()
        .map(|v| format!("\n\n{v}"))
        .unwrap_or_default();
    let context = if options.context_files.is_empty() {
        String::new()
    } else {
        let body = options
            .context_files
            .iter()
            .map(|(path, content)| {
                format!(
                    "<project_instructions path=\"{path}\">\n{content}\n</project_instructions>\n\n"
                )
            })
            .collect::<String>();
        format!(
            "\n\n<project_context>\n\nProject-specific instructions and guidelines:\n\n{body}</project_context>\n"
        )
    };
    let skills = if tools.iter().any(|tool| tool == "read") {
        format_skills_for_prompt(&options.skills)
    } else {
        String::new()
    };
    let cwd = options.cwd.replace('\\', "/");
    let prompt = options.custom_prompt.clone().unwrap_or_else(|| format!("You are an expert coding assistant operating inside pi, a coding agent harness. You help users by reading files, executing commands, editing code, and writing new files.\n\nAvailable tools:\n{tools_list}\n\nGuidelines:\n{guidelines}"));
    format!(
        "{prompt}{append}{context}{skills}\nCurrent date: {}\nCurrent working directory: {cwd}",
        chrono_date()
    )
}

fn chrono_date() -> String {
    // UTC date without adding a dependency.
    use std::time::{SystemTime, UNIX_EPOCH};
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 86_400;
    // Civil date conversion, proleptic Gregorian (Howard Hinnant's compact algorithm).
    let z = days as i64 + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let y = y + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}
