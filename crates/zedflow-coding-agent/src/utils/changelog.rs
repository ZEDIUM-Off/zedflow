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

    #[test]
    fn normalizes_entry_version_like_pi() {
        let entry = ChangelogEntry {
            major: 1,
            minor: 2,
            patch: 3,
            content: String::new(),
        };
        assert_eq!(
            normalize_changelog_links("[file](README.md)", &entry),
            "[file](https://github.com/earendil-works/pi/blob/v1.2.3/packages/README.md)"
        );
    }

    #[test]
    fn normalizes_local_changelog_links_like_pi() {
        let markdown = "[file](../README.md?raw=1#top) [dir](docs/) [web](https://example.com/a) [reserved](a&b;c=x+$,@:.md) [upper](HTTPS://example.com/a)";
        assert_eq!(
            normalize_changelog_links(markdown, "1.2.3"),
            "[file](https://github.com/earendil-works/pi/blob/v1.2.3/packages/README.md?raw=1#top) [dir](https://github.com/earendil-works/pi/tree/v1.2.3/packages/docs) [web](https://example.com/a) [reserved](https://github.com/earendil-works/pi/blob/v1.2.3/packages/a&b;c=x+$,@:.md) [upper](HTTPS://example.com/a)"
        );
    }
}

fn encode_uri_path(path: &str) -> String {
    const UNESCAPED: &[u8] = b"-_.!~*'();,/?:@&=+$#";
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || UNESCAPED.contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn resolve_repository_path(target: &str) -> Option<String> {
    let target = target.replace('\\', "/");
    let joined = if target.starts_with('/') {
        target.trim_start_matches('/').to_owned()
    } else {
        format!("packages/coding-agent/{target}")
    };
    let mut parts = Vec::new();
    for part in joined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    let repository_path = parts.join("/");
    let repository_path = repository_path
        .strip_prefix("packages/coding-agent/")
        .map(|path| format!("packages/{path}"))
        .unwrap_or(repository_path);
    (!repository_path.is_empty()).then_some(repository_path)
}

fn split_local_target(target: &str) -> (&str, String, String) {
    let (before_hash, fragment) = target.split_once('#').unwrap_or((target, ""));
    let (path, query) = before_hash.split_once('?').unwrap_or((before_hash, ""));
    (
        path,
        (!query.is_empty())
            .then(|| format!("?{query}"))
            .unwrap_or_default(),
        (!fragment.is_empty())
            .then(|| format!("#{fragment}"))
            .unwrap_or_default(),
    )
}

pub enum ChangelogVersion {
    Text(String),
    Entry(ChangelogEntry),
}

impl From<&str> for ChangelogVersion {
    fn from(version: &str) -> Self {
        Self::Text(version.to_owned())
    }
}

impl From<String> for ChangelogVersion {
    fn from(version: String) -> Self {
        Self::Text(version)
    }
}

impl From<&ChangelogEntry> for ChangelogVersion {
    fn from(entry: &ChangelogEntry) -> Self {
        Self::Entry(entry.clone())
    }
}

impl From<ChangelogEntry> for ChangelogVersion {
    fn from(entry: ChangelogEntry) -> Self {
        Self::Entry(entry)
    }
}

pub fn normalize_changelog_links<V: Into<ChangelogVersion>>(markdown: &str, version: V) -> String {
    let version = version.into();
    let version = match version {
        ChangelogVersion::Text(version) => version,
        ChangelogVersion::Entry(entry) => {
            format!("{}.{}.{}", entry.major, entry.minor, entry.patch)
        }
    };
    let tag = if version.starts_with('v') {
        version
    } else {
        format!("v{version}")
    };
    let repo = "https://github.com/earendil-works/pi";
    let links = regex::Regex::new(r#"(!?\[[^\]\n]+\]\()([^\s)]+)((?:\s+[^)]*)?\))"#).unwrap();
    links
        .replace_all(markdown, |caps: &regex::Captures<'_>| {
            let mut target = caps[2].to_owned();
            for prefix in [
                "https://github.com/badlogic/pi-mono",
                "https://github.com/earendil-works/pi-mono",
            ] {
                if let Some(rest) = target.strip_prefix(prefix) {
                    if rest.is_empty() || rest.starts_with('/') {
                        target = format!("{repo}{rest}");
                        break;
                    }
                }
            }
            for route in ["blob", "tree"] {
                for branch in ["main", "master"] {
                    let prefix = format!("{repo}/{route}/{branch}/");
                    if let Some(rest) = target.strip_prefix(&prefix) {
                        target = format!("{repo}/{route}/{tag}/{rest}");
                    }
                }
            }
            let external = target.starts_with('#')
                || target.starts_with("//")
                || regex::Regex::new(r"(?i)^[a-z][a-z0-9+.-]*:")
                    .unwrap()
                    .is_match(&target);
            if !external {
                let (path, query, fragment) = split_local_target(&target);
                if !path.is_empty() {
                    if let Some(repository_path) = resolve_repository_path(path) {
                        let route = if path.ends_with('/')
                            || !repository_path.rsplit('/').next().unwrap().contains('.')
                        {
                            "tree"
                        } else {
                            "blob"
                        };
                        target = format!(
                            "{repo}/{route}/{tag}/{}{query}{fragment}",
                            encode_uri_path(&repository_path)
                        );
                    }
                }
            }
            format!("{}{}{}", &caps[1], target, &caps[3])
        })
        .into_owned()
}
