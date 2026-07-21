use std::io;
use std::path::Path;

use super::path_utils::resolve_to_cwd;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Edit {
    pub old_text: String,
    pub new_text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppliedEditsResult {
    pub base_content: String,
    pub new_content: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EditDiffResult {
    pub diff: String,
    pub first_changed_line: Option<usize>,
}

#[derive(Clone, Debug)]
struct MatchedEdit {
    edit_index: usize,
    match_index: usize,
    match_length: usize,
    new_text: String,
}

pub fn detect_line_ending(content: &str) -> &'static str {
    match (content.find("\r\n"), content.find('\n')) {
        (Some(crlf), Some(lf)) if crlf < lf => "\r\n",
        _ => "\n",
    }
}

pub fn normalize_to_lf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn restore_line_endings(text: &str, ending: &str) -> String {
    if ending == "\r\n" {
        text.replace('\n', "\r\n")
    } else {
        text.to_owned()
    }
}

pub fn normalize_for_fuzzy_match(text: &str) -> String {
    let compatibility = text
        .chars()
        .map(|character| match character {
            '\u{ff01}'..='\u{ff5e}' => {
                char::from_u32(character as u32 - 0xfee0).expect("fullwidth ASCII mapping")
            }
            '\u{3000}' => ' ',
            other => other,
        })
        .collect::<String>();
    let mut composed = String::with_capacity(compatibility.len());
    for character in compatibility.chars() {
        if character == '\u{0301}' {
            let replacement = composed
                .chars()
                .next_back()
                .and_then(|previous| match previous {
                    'a' => Some('á'),
                    'e' => Some('é'),
                    'i' => Some('í'),
                    'o' => Some('ó'),
                    'u' => Some('ú'),
                    'A' => Some('Á'),
                    'E' => Some('É'),
                    'I' => Some('Í'),
                    'O' => Some('Ó'),
                    'U' => Some('Ú'),
                    _ => None,
                });
            if let Some(replacement) = replacement {
                composed.pop();
                composed.push(replacement);
                continue;
            }
        }
        composed.push(character);
    }
    composed
        .split('\n')
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .map(|character| match character {
            '\u{2018}' | '\u{2019}' | '\u{201a}' | '\u{201b}' => '\'',
            '\u{201c}' | '\u{201d}' | '\u{201e}' | '\u{201f}' => '"',
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}'
            | '\u{2212}' => '-',
            '\u{00a0}' | '\u{2002}'..='\u{200a}' | '\u{202f}' | '\u{205f}' => ' ',
            other => other,
        })
        .collect()
}

pub fn strip_bom(content: &str) -> (&str, &str) {
    content
        .strip_prefix('\u{feff}')
        .map_or(("", content), |text| ("\u{feff}", text))
}

fn find_text(content: &str, old_text: &str) -> Option<(usize, usize, bool)> {
    if let Some(index) = content.find(old_text) {
        return Some((index, old_text.len(), false));
    }
    let fuzzy_content = normalize_for_fuzzy_match(content);
    let fuzzy_old_text = normalize_for_fuzzy_match(old_text);
    fuzzy_content
        .find(&fuzzy_old_text)
        .map(|index| (index, fuzzy_old_text.len(), true))
}

fn count_occurrences(content: &str, old_text: &str) -> usize {
    let content = normalize_for_fuzzy_match(content);
    let old_text = normalize_for_fuzzy_match(old_text);
    content.match_indices(&old_text).count()
}

fn edit_error(
    path: &str,
    edit_index: usize,
    total: usize,
    kind: &str,
    occurrences: usize,
) -> io::Error {
    let message = match (kind, total) {
        ("empty", 1) => format!("oldText must not be empty in {path}."),
        ("empty", _) => format!("edits[{edit_index}].oldText must not be empty in {path}."),
        ("missing", 1) => format!(
            "Could not find the exact text in {path}. The old text must match exactly including all whitespace and newlines."
        ),
        ("missing", _) => format!(
            "Could not find edits[{edit_index}] in {path}. The oldText must match exactly including all whitespace and newlines."
        ),
        ("duplicate", 1) => format!(
            "Found {occurrences} occurrences of the text in {path}. The text must be unique. Please provide more context to make it unique."
        ),
        ("duplicate", _) => format!(
            "Found {occurrences} occurrences of edits[{edit_index}] in {path}. Each oldText must be unique. Please provide more context to make it unique."
        ),
        _ => unreachable!(),
    };
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn apply_replacements(content: &str, replacements: &[MatchedEdit], offset: usize) -> String {
    let mut result = content.to_owned();
    for replacement in replacements.iter().rev() {
        let start = replacement.match_index - offset;
        result.replace_range(
            start..start + replacement.match_length,
            &replacement.new_text,
        );
    }
    result
}

fn line_spans(content: &str) -> Vec<(usize, usize)> {
    let mut offset = 0;
    content
        .split_inclusive('\n')
        .map(|line| {
            let span = (offset, offset + line.len());
            offset = span.1;
            span
        })
        .collect()
}

fn replacement_line_range(
    lines: &[(usize, usize)],
    replacement: &MatchedEdit,
) -> io::Result<(usize, usize)> {
    let start = lines
        .iter()
        .position(|&(line_start, line_end)| {
            replacement.match_index >= line_start && replacement.match_index < line_end
        })
        .ok_or_else(|| io::Error::other("Replacement range is outside the base content."))?;
    let replacement_end = replacement.match_index + replacement.match_length;
    let mut end = start;
    while end < lines.len() && lines[end].1 < replacement_end {
        end += 1;
    }
    if end >= lines.len() {
        return Err(io::Error::other(
            "Replacement range is outside the base content.",
        ));
    }
    Ok((start, end + 1))
}

fn apply_preserving_unchanged_lines(
    original: &str,
    base: &str,
    replacements: &[MatchedEdit],
) -> io::Result<String> {
    let original_lines: Vec<_> = original.split_inclusive('\n').collect();
    let base_lines = line_spans(base);
    if original_lines.len() != base_lines.len() {
        return Err(io::Error::other(
            "Cannot preserve unchanged lines because the base content has a different line count.",
        ));
    }

    let mut sorted = replacements.to_vec();
    sorted.sort_by_key(|replacement| replacement.match_index);
    let mut groups: Vec<(usize, usize, Vec<MatchedEdit>)> = Vec::new();
    for replacement in sorted {
        let (start, end) = replacement_line_range(&base_lines, &replacement)?;
        if let Some(group) = groups.last_mut()
            && start < group.1
        {
            group.1 = group.1.max(end);
            group.2.push(replacement);
        } else {
            groups.push((start, end, vec![replacement]));
        }
    }

    let mut result = String::new();
    let mut original_line_index = 0;
    for (start, end, replacements) in groups {
        result.extend(original_lines[original_line_index..start].iter().copied());
        let group_start = base_lines[start].0;
        let group_end = base_lines[end - 1].1;
        result.push_str(&apply_replacements(
            &base[group_start..group_end],
            &replacements,
            group_start,
        ));
        original_line_index = end;
    }
    result.extend(original_lines[original_line_index..].iter().copied());
    Ok(result)
}

pub fn apply_edits_to_normalized_content(
    normalized_content: &str,
    edits: &[Edit],
    path: &str,
) -> io::Result<AppliedEditsResult> {
    let edits: Vec<_> = edits
        .iter()
        .map(|edit| Edit {
            old_text: normalize_to_lf(&edit.old_text),
            new_text: normalize_to_lf(&edit.new_text),
        })
        .collect();
    for (index, edit) in edits.iter().enumerate() {
        if edit.old_text.is_empty() {
            return Err(edit_error(path, index, edits.len(), "empty", 0));
        }
    }

    let use_fuzzy = edits.iter().any(|edit| {
        find_text(normalized_content, &edit.old_text).is_some_and(|(_, _, fuzzy)| fuzzy)
    });
    let replacement_base = if use_fuzzy {
        normalize_for_fuzzy_match(normalized_content)
    } else {
        normalized_content.to_owned()
    };

    let mut matched = Vec::with_capacity(edits.len());
    for (index, edit) in edits.iter().enumerate() {
        let Some((match_index, match_length, _)) = find_text(&replacement_base, &edit.old_text)
        else {
            return Err(edit_error(path, index, edits.len(), "missing", 0));
        };
        let occurrences = count_occurrences(&replacement_base, &edit.old_text);
        if occurrences > 1 {
            return Err(edit_error(
                path,
                index,
                edits.len(),
                "duplicate",
                occurrences,
            ));
        }
        matched.push(MatchedEdit {
            edit_index: index,
            match_index,
            match_length,
            new_text: edit.new_text.clone(),
        });
    }

    matched.sort_by_key(|edit| edit.match_index);
    for pair in matched.windows(2) {
        if pair[0].match_index + pair[0].match_length > pair[1].match_index {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "edits[{}] and edits[{}] overlap in {path}. Merge them into one edit or target disjoint regions.",
                    pair[0].edit_index, pair[1].edit_index
                ),
            ));
        }
    }

    let new_content = if use_fuzzy {
        apply_preserving_unchanged_lines(normalized_content, &replacement_base, &matched)?
    } else {
        apply_replacements(&replacement_base, &matched, 0)
    };
    if normalized_content == new_content {
        let message = if edits.len() == 1 {
            format!(
                "No changes made to {path}. The replacement produced identical content. This might indicate an issue with special characters or the text not existing as expected."
            )
        } else {
            format!("No changes made to {path}. The replacements produced identical content.")
        };
        return Err(io::Error::new(io::ErrorKind::InvalidInput, message));
    }

    Ok(AppliedEditsResult {
        base_content: normalized_content.to_owned(),
        new_content,
    })
}

#[derive(Clone, Debug)]
struct DiffRow {
    tag: char,
    old_before: usize,
    new_before: usize,
    text: String,
}

fn diff_rows(old_content: &str, new_content: &str) -> Vec<DiffRow> {
    let old: Vec<_> = old_content.split_inclusive('\n').collect();
    let new: Vec<_> = new_content.split_inclusive('\n').collect();
    // ponytail: quadratic LCS is sufficient for edit previews; use Myers if multi-megabyte diffs matter.
    let mut lcs = vec![vec![0_u32; new.len() + 1]; old.len() + 1];
    for old_index in (0..old.len()).rev() {
        for new_index in (0..new.len()).rev() {
            lcs[old_index][new_index] = if old[old_index] == new[new_index] {
                lcs[old_index + 1][new_index + 1] + 1
            } else {
                lcs[old_index + 1][new_index].max(lcs[old_index][new_index + 1])
            };
        }
    }

    let mut rows = Vec::new();
    let (mut old_index, mut new_index) = (0, 0);
    let (mut old_line, mut new_line) = (1, 1);
    while old_index < old.len() || new_index < new.len() {
        if old_index < old.len() && new_index < new.len() && old[old_index] == new[new_index] {
            rows.push(DiffRow {
                tag: ' ',
                old_before: old_line,
                new_before: new_line,
                text: old[old_index].to_owned(),
            });
            old_index += 1;
            new_index += 1;
            old_line += 1;
            new_line += 1;
        } else if old_index < old.len()
            && (new_index == new.len()
                || lcs[old_index + 1][new_index] >= lcs[old_index][new_index + 1])
        {
            rows.push(DiffRow {
                tag: '-',
                old_before: old_line,
                new_before: new_line,
                text: old[old_index].to_owned(),
            });
            old_index += 1;
            old_line += 1;
        } else {
            rows.push(DiffRow {
                tag: '+',
                old_before: old_line,
                new_before: new_line,
                text: new[new_index].to_owned(),
            });
            new_index += 1;
            new_line += 1;
        }
    }
    rows
}

pub fn generate_diff_string(
    old_content: &str,
    new_content: &str,
    context: usize,
) -> EditDiffResult {
    let rows = diff_rows(old_content, new_content);
    let changed: Vec<_> = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| (row.tag != ' ').then_some(index))
        .collect();
    let first_changed_line = changed.first().map(|index| rows[*index].new_before);
    let width = old_content
        .split('\n')
        .count()
        .max(new_content.split('\n').count())
        .to_string()
        .len();
    let mut output = Vec::new();
    let mut skipped = false;
    for (index, row) in rows.iter().enumerate() {
        let show = row.tag != ' '
            || changed
                .iter()
                .any(|changed| index.abs_diff(*changed) <= context);
        if show {
            if skipped {
                output.push(format!(" {:width$} ...", ""));
                skipped = false;
            }
            let line = if row.tag == '+' {
                row.new_before
            } else {
                row.old_before
            };
            output.push(format!(
                "{}{:>width$} {}",
                row.tag,
                line,
                row.text.strip_suffix('\n').unwrap_or(&row.text)
            ));
        } else {
            skipped = true;
        }
    }
    if skipped {
        output.push(format!(" {:width$} ...", ""));
    }

    EditDiffResult {
        diff: output.join("\n"),
        first_changed_line,
    }
}

pub fn generate_unified_patch(
    path: &str,
    old_content: &str,
    new_content: &str,
    context: usize,
) -> String {
    let rows = diff_rows(old_content, new_content);
    let changes: Vec<_> = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| (row.tag != ' ').then_some(index))
        .collect();
    let mut patch = format!("--- {path}\n+++ {path}\n");
    let mut change_index = 0;
    while change_index < changes.len() {
        let start = changes[change_index].saturating_sub(context);
        let mut end = (changes[change_index] + context + 1).min(rows.len());
        change_index += 1;
        while change_index < changes.len() && changes[change_index].saturating_sub(context) <= end {
            end = (changes[change_index] + context + 1).min(rows.len());
            change_index += 1;
        }
        let hunk = &rows[start..end];
        let old_count = hunk.iter().filter(|row| row.tag != '+').count();
        let new_count = hunk.iter().filter(|row| row.tag != '-').count();
        let old_start = if old_count == 0 {
            hunk[0].old_before.saturating_sub(1)
        } else {
            hunk.iter()
                .find(|row| row.tag != '+')
                .map_or(0, |row| row.old_before)
        };
        let new_start = if new_count == 0 {
            hunk[0].new_before.saturating_sub(1)
        } else {
            hunk.iter()
                .find(|row| row.tag != '-')
                .map_or(0, |row| row.new_before)
        };
        patch.push_str(&format!(
            "@@ -{} +{} @@\n",
            unified_range(old_start, old_count),
            unified_range(new_start, new_count)
        ));
        for row in hunk {
            patch.push(row.tag);
            patch.push_str(&row.text);
            if !row.text.ends_with('\n') {
                patch.push_str("\n\\ No newline at end of file\n");
            }
        }
    }
    patch
}

fn unified_range(start: usize, count: usize) -> String {
    if count == 1 {
        start.to_string()
    } else {
        format!("{start},{count}")
    }
}

pub async fn compute_edits_diff(
    path: &str,
    edits: &[Edit],
    cwd: impl AsRef<Path>,
) -> Result<EditDiffResult, String> {
    let absolute_path = resolve_to_cwd(path, cwd).map_err(|error| error.to_string())?;
    let bytes = tokio::fs::read(&absolute_path)
        .await
        .map_err(|error| format!("Could not edit file: {path}. {}.", error_code(&error)))?;
    let raw = String::from_utf8_lossy(&bytes);
    let (_, content) = strip_bom(&raw);
    let content = normalize_to_lf(content);
    let applied = apply_edits_to_normalized_content(&content, edits, path)
        .map_err(|error| error.to_string())?;
    Ok(generate_diff_string(
        &applied.base_content,
        &applied.new_content,
        4,
    ))
}

pub async fn compute_edit_diff(
    path: &str,
    old_text: &str,
    new_text: &str,
    cwd: impl AsRef<Path>,
) -> Result<EditDiffResult, String> {
    compute_edits_diff(
        path,
        &[Edit {
            old_text: old_text.to_owned(),
            new_text: new_text.to_owned(),
        }],
        cwd,
    )
    .await
}

pub(crate) fn error_code(error: &io::Error) -> String {
    let code = match error.kind() {
        io::ErrorKind::NotFound => Some("ENOENT"),
        io::ErrorKind::PermissionDenied => Some("EACCES"),
        io::ErrorKind::AlreadyExists => Some("EEXIST"),
        io::ErrorKind::InvalidInput => Some("EINVAL"),
        _ => None,
    };
    code.map_or_else(|| error.to_string(), |code| format!("Error code: {code}"))
}
