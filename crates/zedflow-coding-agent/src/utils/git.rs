#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSource {
    pub repo: String,
    pub host: String,
    pub path: String,
    pub ref_name: Option<String>,
    pub pinned: bool,
}

fn split_ref(url: &str) -> (&str, Option<&str>) {
    let path = url
        .rsplit_once(':')
        .map(|(_, p)| p)
        .or_else(|| url.split_once("//").map(|(_, p)| p))
        .unwrap_or(url);
    if let Some(i) = path.find('@') {
        if i > 0 && i + 1 < path.len() {
            return (&url[..url.len() - path.len() + i], Some(&path[i + 1..]));
        }
    }
    (url, None)
}

fn unsafe_part(value: &str, allow_slash: bool) -> bool {
    let decoded = percent_decode(value);
    [value, decoded.as_deref().unwrap_or("")].iter().any(|v| {
        v.contains('\0')
            || v.contains('\\')
            || v.starts_with('/')
            || (!allow_slash && v.contains('/'))
            || v.split('/').any(|p| p == "..")
    })
}
fn percent_decode(s: &str) -> Option<String> {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            if i + 2 >= b.len() {
                return None;
            };
            out.push(u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).ok()?, 16).ok()?);
            i += 3
        } else {
            out.push(b[i]);
            i += 1
        }
    }
    String::from_utf8(out).ok()
}

#[must_use]
pub fn parse_git_url(source: &str) -> Option<GitSource> {
    let trimmed = source.trim();
    let prefixed = trimmed.strip_prefix("git:");
    let url = prefixed.unwrap_or(trimmed).trim();
    if prefixed.is_none()
        && !["http://", "https://", "ssh://", "git://"]
            .iter()
            .any(|p| url.starts_with(p))
    {
        return None;
    }
    let (repo, ref_name) = split_ref(url);
    let (host, path) = if let Some(rest) = repo.strip_prefix("git@").and_then(|s| s.split_once(':'))
    {
        (rest.0, rest.1)
    } else if let Some(rest) = repo.split_once("://").map(|(_, s)| s) {
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        (authority.rsplit('@').next().unwrap_or(authority), path)
    } else {
        repo.split_once('/').unwrap_or(("", ""))
    };
    if host.is_empty()
        || path.is_empty()
        || path.trim_matches('/').split('/').count() < 2
        || unsafe_part(host, false)
        || unsafe_part(path, true)
    {
        return None;
    }
    let path = path.trim_matches('/').trim_end_matches(".git").to_owned();
    let repo = if prefixed.is_some() && !repo.contains("://") && !repo.starts_with("git@") {
        format!("https://{repo}")
    } else {
        repo.to_owned()
    };
    Some(GitSource {
        repo,
        host: host.to_owned(),
        path,
        pinned: ref_name.is_some(),
        ref_name: ref_name.map(str::to_owned),
    })
}
