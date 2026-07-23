use std::{fs, path::Path};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChangelogEntry {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub content: String,
}

pub fn compare_versions(a: &ChangelogEntry, b: &ChangelogEntry) -> i32 {
    match (a.major, a.minor, a.patch).cmp(&(b.major, b.minor, b.patch)) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

pub fn parse_changelog(path: impl AsRef<Path>) -> Vec<ChangelogEntry> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut current: Option<(u64, u64, u64, Vec<String>)> = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            if let Some((a, b, c, lines)) = current.take() {
                out.push(ChangelogEntry {
                    major: a,
                    minor: b,
                    patch: c,
                    content: lines.join("\n").trim().to_owned(),
                });
            }
            let nums: Vec<_> = rest
                .trim_start_matches('[')
                .split(|c: char| !c.is_ascii_digit())
                .filter_map(|s| s.parse().ok())
                .collect();
            if nums.len() >= 3 {
                current = Some((nums[0], nums[1], nums[2], vec![line.to_owned()]));
            }
        } else if let Some((_, _, _, lines)) = &mut current {
            lines.push(line.to_owned());
        }
    }
    if let Some((a, b, c, lines)) = current {
        out.push(ChangelogEntry {
            major: a,
            minor: b,
            patch: c,
            content: lines.join("\n").trim().to_owned(),
        });
    }
    out
}

pub fn get_new_entries(entries: &[ChangelogEntry], last_version: &str) -> Vec<ChangelogEntry> {
    let n: Vec<u64> = last_version
        .split('.')
        .filter_map(|x| x.parse().ok())
        .collect();
    let last = (
        n.first().copied().unwrap_or(0),
        n.get(1).copied().unwrap_or(0),
        n.get(2).copied().unwrap_or(0),
    );
    entries
        .iter()
        .filter(|e| (e.major, e.minor, e.patch) > last)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_and_filters_versions() {
        let dir = std::env::temp_dir().join(format!("zedflow-changelog-{}", std::process::id()));
        std::fs::write(&dir, "## [1.2.3]\nfirst\n## 1.0.0\nold\n").unwrap();
        let entries = parse_changelog(&dir);
        assert_eq!(entries.len(), 2);
        assert_eq!(get_new_entries(&entries, "1.1.0")[0].major, 1);
        let _ = std::fs::remove_file(dir);
    }
}

pub fn normalize_changelog_links(markdown: &str, version: &str) -> String {
    let tag = if version.starts_with('v') {
        version.to_owned()
    } else {
        format!("v{version}")
    };
    markdown.split_inclusive(')').map(|part| {
        if let Some(start) = part.find("](") { let (head, tail) = part.split_at(start+2); let end = tail.find(')').unwrap_or(tail.len()); let target=&tail[..end]; if !target.contains(":") && !target.starts_with('#') { return format!("{head}https://github.com/earendil-works/pi/blob/{tag}/packages/coding-agent/{}{}", target.trim_start_matches('/'), &tail[end..]); } }
        part.to_owned()
    }).collect()
}
