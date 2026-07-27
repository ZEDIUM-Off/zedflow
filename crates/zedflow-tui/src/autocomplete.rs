use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutocompleteItem {
    pub value: String,
    pub label: String,
    pub description: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutocompleteSuggestions {
    pub items: Vec<AutocompleteItem>,
    pub prefix: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionResult {
    pub lines: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
}

pub trait AutocompleteProvider {
    fn get_suggestions(
        &self,
        lines: &[String],
        cursor_line: usize,
        cursor_col: usize,
        force: bool,
    ) -> Option<AutocompleteSuggestions>;
    fn apply_completion(
        &self,
        lines: &[String],
        cursor_line: usize,
        cursor_col: usize,
        item: &AutocompleteItem,
        prefix: &str,
    ) -> CompletionResult;
    fn should_trigger_file_completion(
        &self,
        lines: &[String],
        cursor_line: usize,
        cursor_col: usize,
    ) -> bool;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlashCommand {
    pub name: String,
    pub description: Option<String>,
    pub argument_hint: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CombinedAutocompleteProvider {
    commands: Vec<SlashCommand>,
    base_path: PathBuf,
}
impl CombinedAutocompleteProvider {
    pub fn new(commands: Vec<SlashCommand>, base_path: impl Into<PathBuf>) -> Self {
        Self {
            commands,
            base_path: base_path.into(),
        }
    }

    fn command_suggestions(&self, prefix: &str) -> Vec<AutocompleteItem> {
        crate::fuzzy::fuzzy_filter(&self.commands, prefix, |command| command.name.as_str())
            .into_iter()
            .map(|command| {
                let description = match (&command.argument_hint, &command.description) {
                    (Some(hint), Some(description)) => Some(format!("{hint} — {description}")),
                    (Some(hint), None) => Some(hint.clone()),
                    (_, description) => description.clone(),
                };
                AutocompleteItem {
                    value: command.name.clone(),
                    label: command.name,
                    description,
                }
            })
            .collect()
    }

    fn path_suggestions(&self, prefix: &str) -> Vec<AutocompleteItem> {
        let raw = prefix.trim_start_matches('@').trim_start_matches('"');
        let path = Path::new(raw);
        let (directory, search) = if raw.ends_with(['/', '\\']) {
            (path, "")
        } else {
            (
                path.parent().unwrap_or(Path::new("")),
                path.file_name().and_then(|v| v.to_str()).unwrap_or(""),
            )
        };
        let search_dir = if directory.is_absolute() {
            directory.to_path_buf()
        } else {
            self.base_path.join(directory)
        };
        let Ok(entries) = std::fs::read_dir(search_dir) else {
            return Vec::new();
        };
        let mut items: Vec<_> = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                if !name.to_lowercase().starts_with(&search.to_lowercase()) {
                    return None;
                }
                let directory_entry = entry.file_type().ok()?.is_dir();
                let mut value = if directory.as_os_str().is_empty() {
                    name.clone()
                } else {
                    directory.join(&name).to_string_lossy().replace('\\', "/")
                };
                if directory_entry {
                    value.push('/');
                }
                if prefix.starts_with('@') {
                    value.insert(0, '@');
                }
                Some(AutocompleteItem {
                    value,
                    label: format!("{name}{}", if directory_entry { "/" } else { "" }),
                    description: None,
                })
            })
            .collect();
        items.sort_by_key(|item| (!item.label.ends_with('/'), item.label.to_lowercase()));
        items
    }
}

impl AutocompleteProvider for CombinedAutocompleteProvider {
    fn get_suggestions(
        &self,
        lines: &[String],
        cursor_line: usize,
        cursor_col: usize,
        force: bool,
    ) -> Option<AutocompleteSuggestions> {
        let line = lines.get(cursor_line).map(String::as_str).unwrap_or("");
        let before = line.get(..cursor_col.min(line.len()))?;
        if !force && before.starts_with('/') && !before.contains(' ') {
            let items = self.command_suggestions(&before[1..]);
            return (!items.is_empty()).then(|| AutocompleteSuggestions {
                items,
                prefix: before.into(),
            });
        }
        let start = before
            .char_indices()
            .rev()
            .find(|(_, c)| matches!(c, ' ' | '\t' | '\'' | '='))
            .map_or(0, |(i, c)| i + c.len_utf8());
        let prefix = &before[start..];
        if !force && !prefix.starts_with('@') && !prefix.contains(['/', '\\']) {
            return None;
        }
        let items = self.path_suggestions(prefix);
        (!items.is_empty()).then(|| AutocompleteSuggestions {
            items,
            prefix: prefix.into(),
        })
    }

    fn apply_completion(
        &self,
        lines: &[String],
        cursor_line: usize,
        cursor_col: usize,
        item: &AutocompleteItem,
        prefix: &str,
    ) -> CompletionResult {
        let mut output = lines.to_vec();
        let line = output.get(cursor_line).cloned().unwrap_or_default();
        let start = cursor_col.saturating_sub(prefix.len()).min(line.len());
        let command = prefix.starts_with('/')
            && line[..start].trim().is_empty()
            && !prefix[1..].contains('/');
        let suffix = if command || (prefix.starts_with('@') && !item.label.ends_with('/')) {
            " "
        } else {
            ""
        };
        let value = if command {
            format!("/{}", item.value)
        } else {
            item.value.clone()
        };
        output[cursor_line] = format!(
            "{}{}{}{}",
            &line[..start],
            value,
            suffix,
            &line[cursor_col.min(line.len())..]
        );
        CompletionResult {
            lines: output,
            cursor_line,
            cursor_col: start + value.len() + suffix.len(),
        }
    }

    fn should_trigger_file_completion(
        &self,
        lines: &[String],
        cursor_line: usize,
        cursor_col: usize,
    ) -> bool {
        let line = lines.get(cursor_line).map(String::as_str).unwrap_or("");
        let before = &line[..cursor_col.min(line.len())];
        !(before.trim().starts_with('/') && !before.trim().contains(' '))
    }
}
