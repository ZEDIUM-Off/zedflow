use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangelogEntry {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub content: String,
}

fn version(entry: &ChangelogEntry) -> String {
    format!("{}.{}.{}", entry.major, entry.minor, entry.patch)
}
fn tag(v: &str) -> String {
    if v.starts_with('v') {
        v.to_owned()
    } else {
        format!("v{v}")
    }
}

pub fn parse_changelog(path: impl AsRef<Path>) -> Vec<ChangelogEntry> {
    let Ok(text) = fs::read_to_string(path) else {
        return vec![];
    };
    let mut out = vec![];
    let mut current: Option<(u64, u64, u64, Vec<String>)> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            if let Some((major, minor, patch, lines)) = current.take() {
                out.push(ChangelogEntry {
                    major,
                    minor,
                    patch,
                    content: lines.join("\n").trim().to_owned(),
                });
            }
            let nums: Vec<_> = rest
                .trim_start_matches('[')
                .split(|c: char| !c.is_ascii_digit() && c != '.')
                .next()
                .unwrap_or("")
                .split('.')
                .collect();
            if nums.len() == 3 {
                if let (Ok(a), Ok(b), Ok(c)) = (nums[0].parse(), nums[1].parse(), nums[2].parse()) {
                    current = Some((a, b, c, vec![line.to_owned()]));
                }
            }
        } else if let Some((_, _, _, lines)) = current.as_mut() {
            lines.push(line.to_owned());
        }
    }
    if let Some((major, minor, patch, lines)) = current {
        out.push(ChangelogEntry {
            major,
            minor,
            patch,
            content: lines.join("\n").trim().to_owned(),
        });
    }
    out
}

pub fn compare_versions(a: &ChangelogEntry, b: &ChangelogEntry) -> i32 {
    for (left, right) in [(a.major, b.major), (a.minor, b.minor), (a.patch, b.patch)] {
        if left != right {
            return left.cmp(&right) as i32;
        }
    }
    0
}
pub fn get_new_entries(entries: &[ChangelogEntry], last: &str) -> Vec<ChangelogEntry> {
    let p: Vec<u64> = last
        .trim_start_matches('v')
        .split('.')
        .map(|x| x.parse().unwrap_or(0))
        .collect();
    let old = ChangelogEntry {
        major: p.first().copied().unwrap_or(0),
        minor: p.get(1).copied().unwrap_or(0),
        patch: p.get(2).copied().unwrap_or(0),
        content: String::new(),
    };
    entries
        .iter()
        .filter(|e| compare_versions(e, &old) > 0)
        .cloned()
        .collect()
}

fn local_target(target: &str) -> Option<(String, String, String)> {
    let (path_query, fragment) = target.split_once('#').map_or((target, ""), |(a, b)| (a, b));
    let (path, query) = path_query
        .split_once('?')
        .map_or((path_query, ""), |(a, b)| (a, b));
    Some((
        path.to_owned(),
        if query.is_empty() {
            String::new()
        } else {
            format!("?{query}")
        },
        if fragment.is_empty() {
            String::new()
        } else {
            format!("#{fragment}")
        },
    ))
}
fn normalize_target(target: &str, version: &str) -> String {
    let mut target = target
        .replace(
            "https://github.com/earendil-works/pi-mono",
            "https://github.com/earendil-works/pi",
        )
        .replace(
            "https://github.com/badlogic/pi-mono",
            "https://github.com/earendil-works/pi",
        );
    let repo = "https://github.com/earendil-works/pi";
    for route in ["blob", "tree"] {
        for branch in ["main", "master"] {
            let p = format!("{repo}/{route}/{branch}/");
            if let Some(rest) = target.strip_prefix(&p) {
                target = format!("{repo}/{route}/{}/{rest}", tag(version));
            }
        }
    }
    if target.starts_with('#') || target.starts_with("//") || target.contains(":") {
        return target;
    }
    let (path, query, fragment) = local_target(&target).unwrap();
    if path.is_empty() {
        return target;
    }
    let mut parts = PathBuf::from("packages/coding-agent");
    for part in path.replace('\\', "/").split('/') {
        if part == ".." {
            parts.pop();
        } else if part != "." && !part.is_empty() {
            parts.push(part);
        }
    }
    let p = parts.to_string_lossy().replace('\\', "/");
    if !p.starts_with("packages/") {
        return target;
    }
    let route = if path.ends_with('/')
        || !Path::new(&p)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .contains('.')
    {
        "tree"
    } else {
        "blob"
    };
    format!("{repo}/{route}/{}/{p}{query}{fragment}", tag(version))
}
pub fn normalize_changelog_links(markdown: &str, v: impl AsRef<str>) -> String {
    let v = v.as_ref();
    let mut out = String::with_capacity(markdown.len());
    let mut rest = markdown;
    while let Some(start) = rest.find("]( ").or_else(|| rest.find("](")) {
        let (before, after) = rest.split_at(start + 2);
        out.push_str(before);
        let Some(end) = after.find(')') else {
            out.push_str(after);
            break;
        };
        let target = &after[..end];
        out.push_str(&normalize_target(target, v));
        out.push(')');
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}
pub fn get_changelog_path() -> PathBuf {
    PathBuf::from("CHANGELOG.md")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn links_are_pinned() {
        assert_eq!(
            normalize_changelog_links("[x](README.md)", "1.2.3"),
            "[x](https://github.com/earendil-works/pi/blob/v1.2.3/packages/coding-agent/README.md)"
        );
    }
}
