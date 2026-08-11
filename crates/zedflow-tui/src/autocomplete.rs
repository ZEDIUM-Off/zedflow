use std::{
    fs,
    path::{Path, PathBuf},
};

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
        crate::fuzzy::fuzzy_filter(&self.commands, prefix, |c| c.name.as_str())
            .into_iter()
            .map(|c| {
                let description = match (c.argument_hint, c.description) {
                    (Some(h), Some(d)) => Some(format!("{h} — {d}")),
                    (Some(h), None) => Some(h),
                    (_, d) => d,
                };
                AutocompleteItem {
                    value: c.name.clone(),
                    label: c.name,
                    description,
                }
            })
            .collect()
    }

    fn parse_prefix(prefix: &str) -> (&str, bool, bool) {
        if let Some(v) = prefix.strip_prefix("@\"") {
            (v, true, true)
        } else if let Some(v) = prefix.strip_prefix('"') {
            (v, false, true)
        } else if let Some(v) = prefix.strip_prefix('@') {
            (v, true, false)
        } else {
            (prefix, false, false)
        }
    }
    fn quote_value(path: &str, at: bool, quoted: bool) -> String {
        let prefix = if at { "@" } else { "" };
        if quoted || path.contains(' ') {
            format!("{prefix}\"{path}\"")
        } else {
            format!("{prefix}{path}")
        }
    }
    fn path_suggestions(&self, prefix: &str) -> Vec<AutocompleteItem> {
        let (raw, at, quoted) = Self::parse_prefix(prefix);
        if at {
            return self.fuzzy_paths(raw, quoted);
        }
        let normalized = raw.replace('\\', "/");
        let path = Path::new(&normalized);
        let (directory, search) = if normalized.ends_with('/') {
            (path, "")
        } else {
            (
                path.parent().unwrap_or(Path::new("")),
                path.file_name().and_then(|v| v.to_str()).unwrap_or(""),
            )
        };
        let search_dir = expand_home(directory, &self.base_path);
        let Ok(entries) = fs::read_dir(search_dir) else {
            return vec![];
        };
        let mut items = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                if !name.to_lowercase().starts_with(&search.to_lowercase()) {
                    return None;
                }
                let is_dir = entry.metadata().ok()?.is_dir();
                let mut display = if directory.as_os_str().is_empty() {
                    name.clone()
                } else {
                    format!(
                        "{}/{}",
                        directory
                            .to_string_lossy()
                            .replace('\\', "/")
                            .trim_end_matches('/'),
                        name
                    )
                };
                if normalized.starts_with("./") && !display.starts_with("./") {
                    display = format!("./{display}");
                }
                if is_dir {
                    display.push('/');
                }
                Some(AutocompleteItem {
                    value: Self::quote_value(&display, false, quoted),
                    label: format!("{name}{}", if is_dir { "/" } else { "" }),
                    description: None,
                })
            })
            .collect::<Vec<_>>();
        items.sort_by_key(|i| (!i.label.ends_with('/'), i.label.to_lowercase()));
        items
    }
    fn fuzzy_paths(&self, query: &str, quoted: bool) -> Vec<AutocompleteItem> {
        let normalized = query.replace('\\', "/");
        let slash = normalized.rfind('/');
        let (display_base, needle, root) = if let Some(i) = slash {
            let display = &normalized[..=i];
            let root = expand_home(Path::new(display), &self.base_path);
            if !root.is_dir() {
                return vec![];
            }
            (display.to_owned(), &normalized[i + 1..], root)
        } else {
            (String::new(), normalized.as_str(), self.base_path.clone())
        };
        let mut found = Vec::new();
        walk(&root, &root, &mut found, 100);
        let lower = needle.to_lowercase();
        let mut scored = found
            .into_iter()
            .filter_map(|(relative, is_dir)| {
                let display_relative = relative.to_string_lossy().replace('\\', "/");
                let name = relative.file_name()?.to_string_lossy().to_string();
                let lname = name.to_lowercase();
                let lpath = display_relative.to_lowercase();
                let score = (if lower.is_empty() {
                    1
                } else if lname == lower {
                    100
                } else if lname.starts_with(&lower) {
                    80
                } else if lname.contains(&lower) {
                    50
                } else if lpath.contains(&lower) {
                    30
                } else {
                    0
                }) + usize::from(is_dir) * 10;
                (score > 0).then_some((score, display_relative, name, is_dir))
            })
            .collect::<Vec<_>>();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        scored.truncate(20);
        scored
            .into_iter()
            .map(|(_, relative, name, is_dir)| {
                let mut display = format!("{display_base}{relative}");
                if is_dir {
                    display.push('/');
                }
                AutocompleteItem {
                    value: Self::quote_value(&display, true, quoted),
                    label: format!("{name}{}", if is_dir { "/" } else { "" }),
                    description: Some(display.trim_end_matches('/').into()),
                }
            })
            .collect()
    }

    fn extract_quoted(text: &str) -> Option<&str> {
        let mut open = None;
        for (i, c) in text.char_indices() {
            if c == '"' {
                open = if open.is_some() { None } else { Some(i) }
            }
        }
        let i = open?;
        if i > 0 && text.as_bytes()[i - 1] == b'@' {
            let start = i - 1;
            if start == 0 || is_delimiter(text[..start].chars().next_back()?) {
                return Some(&text[start..]);
            }
            return None;
        }
        if i == 0 || is_delimiter(text[..i].chars().next_back()?) {
            Some(&text[i..])
        } else {
            None
        }
    }
    fn extract_at(text: &str) -> Option<&str> {
        if let Some(q) = Self::extract_quoted(text).filter(|q| q.starts_with("@\"")) {
            return Some(q);
        }
        let start = text
            .char_indices()
            .rev()
            .find(|(_, c)| is_delimiter(*c))
            .map_or(0, |(i, c)| i + c.len_utf8());
        text.get(start..).filter(|s| s.starts_with('@'))
    }
    fn extract_path(text: &str, force: bool) -> Option<&str> {
        if let Some(q) = Self::extract_quoted(text) {
            return Some(q);
        }
        let start = text
            .char_indices()
            .rev()
            .find(|(_, c)| is_delimiter(*c))
            .map_or(0, |(i, c)| i + c.len_utf8());
        let prefix = &text[start..];
        (force
            || prefix.contains('/')
            || prefix.starts_with('.')
            || prefix.starts_with("~/")
            || (prefix.is_empty() && text.ends_with(' ')))
        .then_some(prefix)
    }
}
fn is_delimiter(c: char) -> bool {
    matches!(c, ' ' | '\t' | '"' | '\'' | '=')
}
fn expand_home(path: &Path, base: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if text == "~" || text.starts_with("~/") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(text.trim_start_matches("~/"))
    } else if path.is_absolute() {
        path.into()
    } else {
        base.join(path)
    }
}
fn walk(root: &Path, dir: &Path, out: &mut Vec<(PathBuf, bool)>, max: usize) {
    if out.len() >= max {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= max {
            return;
        }
        let name = entry.file_name();
        if name == ".git" {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        out.push((relative, meta.is_dir()));
        if meta.is_dir() {
            walk(root, &path, out, max)
        }
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
        if let Some(prefix) = Self::extract_at(before) {
            let items = self.path_suggestions(prefix);
            return (!items.is_empty()).then(|| AutocompleteSuggestions {
                items,
                prefix: prefix.into(),
            });
        }
        if !force && before.starts_with('/') {
            if !before.contains(' ') {
                let items = self.command_suggestions(&before[1..]);
                return (!items.is_empty()).then(|| AutocompleteSuggestions {
                    items,
                    prefix: before.into(),
                });
            }
            return None;
        }
        let prefix = Self::extract_path(before, force)?;
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
        let before = &line[..start];
        let after = &line[cursor_col.min(line.len())..];
        let quoted = prefix.starts_with('"') || prefix.starts_with("@\"");
        let adjusted_after = if quoted && item.value.ends_with('"') && after.starts_with('"') {
            &after[1..]
        } else {
            after
        };
        let command =
            prefix.starts_with('/') && before.trim().is_empty() && !prefix[1..].contains('/');
        let attachment = prefix.starts_with('@');
        let directory = item.label.ends_with('/');
        let value = if command {
            format!("/{}", item.value)
        } else {
            item.value.clone()
        };
        let suffix = if command || (attachment && !directory) {
            " "
        } else {
            ""
        };
        output[cursor_line] = format!("{before}{value}{suffix}{adjusted_after}");
        let mut offset = value.len();
        if directory && value.ends_with('"') {
            offset -= 1
        }
        CompletionResult {
            lines: output,
            cursor_line,
            cursor_col: start + offset + suffix.len(),
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
