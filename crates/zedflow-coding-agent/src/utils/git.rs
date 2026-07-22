#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitSource {
    pub repo: String,
    pub host: String,
    pub path: String,
    pub ref_name: Option<String>,
    pub pinned: bool,
}

fn split_ref(url: &str) -> (&str, Option<&str>) {
    let marker = if url.starts_with("git@") {
        url.find('@')
            .and_then(|_| url[4..].find('@').map(|i| i + 4))
    } else {
        url.find('@')
    };
    marker
        .map(|i| (&url[..i], Some(&url[i + 1..])))
        .unwrap_or((url, None))
}
fn unsafe_part(value: &str, allow_slash: bool) -> bool {
    let Some(decoded) = percent_decode(value) else {
        return true;
    };
    [value, decoded.as_str()].iter().any(|v| {
        v.contains('\0')
            || v.contains('\\')
            || v.starts_with('/')
            || (!allow_slash && v.contains('/'))
            || v.split('/').any(|p| p == "..")
    })
}
fn percent_decode(s: &str) -> Option<String> {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return None;
            }
            let v = u8::from_str_radix(&s[i + 1..i + 3], 16).ok()?;
            out.push(v);
            i += 3;
        } else {
            let next = s[i..].chars().next()?.len_utf8();
            out.extend_from_slice(&bytes[i..i + next]);
            i += next;
        }
    }
    String::from_utf8(out).ok()
}
fn build(repo: String, host: String, path: String, ref_name: Option<String>) -> Option<GitSource> {
    let path = path
        .trim_start_matches('/')
        .trim_end_matches(".git")
        .to_string();
    if host.is_empty()
        || path.split('/').count() < 2
        || unsafe_part(&host, false)
        || unsafe_part(&path, true)
    {
        return None;
    }
    Some(GitSource {
        pinned: ref_name.is_some(),
        repo,
        host,
        path,
        ref_name,
    })
}
fn parse_generic(url: &str) -> Option<GitSource> {
    let (repo, reference) = split_ref(url);
    let reference = reference.map(str::to_owned);
    if let Some(rest) = repo.strip_prefix("git@") {
        let (host, path) = rest.split_once(':')?;
        return build(repo.to_owned(), host.to_owned(), path.to_owned(), reference);
    }
    if let Some(rest) = repo
        .strip_prefix("https://")
        .or_else(|| repo.strip_prefix("http://"))
        .or_else(|| repo.strip_prefix("ssh://"))
        .or_else(|| repo.strip_prefix("git://"))
    {
        let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
        return build(
            repo.to_owned(),
            host.split(':').next().unwrap_or(host).to_owned(),
            path.to_owned(),
            reference,
        );
    }
    let (host, path) = repo.split_once('/')?;
    if !host.contains('.') && host != "localhost" {
        return None;
    }
    build(
        format!("https://{repo}"),
        host.to_owned(),
        path.to_owned(),
        reference,
    )
}

pub fn parse_git_url(source: &str) -> Option<GitSource> {
    let source = source.trim();
    let explicit = source.strip_prefix("git:");
    let url = explicit.unwrap_or(source).trim();
    if explicit.is_none()
        && !url.starts_with("http://")
        && !url.starts_with("https://")
        && !url.starts_with("ssh://")
        && !url.starts_with("git://")
    {
        return None;
    }
    let (repo, reference) = split_ref(url);
    let reference = reference.map(str::to_owned);
    let (base, host_path) = if let Some(rest) = repo.strip_prefix("git@") {
        (repo.to_owned(), rest.replace(':', "/"))
    } else {
        (
            repo.to_owned(),
            repo.trim_start_matches("https://")
                .trim_start_matches("http://")
                .trim_start_matches("ssh://")
                .trim_start_matches("git://")
                .to_owned(),
        )
    };
    if let Some((host, path)) = host_path.split_once('/') {
        if (host == "github.com" || host == "gitlab.com" || host == "bitbucket.org")
            && path.split('/').count() >= 2
        {
            return build(base, host.to_owned(), path.to_owned(), reference);
        }
    }
    parse_generic(url)
}
