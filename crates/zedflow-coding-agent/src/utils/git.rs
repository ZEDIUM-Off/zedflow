#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitSource {
    pub repo: String,
    pub host: String,
    pub path: String,
    pub ref_name: Option<String>,
    pub pinned: bool,
}

fn split_ref(url: &str) -> (String, Option<String>) {
    if let Some(rest) = url.strip_prefix("git@") {
        let Some((host, path)) = rest.split_once(':') else {
            return (url.to_owned(), None);
        };
        let Some((repo_path, reference)) = path.split_once('@') else {
            return (url.to_owned(), None);
        };
        if repo_path.is_empty() || reference.is_empty() {
            return (url.to_owned(), None);
        }
        return (
            format!("git@{host}:{repo_path}"),
            Some(reference.to_owned()),
        );
    }

    if url.contains("://") {
        let Ok(mut parsed) = reqwest::Url::parse(url) else {
            return (url.to_owned(), None);
        };
        let path = parsed.path().trim_start_matches('/').to_owned();
        let Some((repo_path, reference)) = path.split_once('@') else {
            return (url.to_owned(), None);
        };
        if repo_path.is_empty() || reference.is_empty() {
            return (url.to_owned(), None);
        }
        let repo_path = repo_path.to_owned();
        let reference = reference.to_owned();
        parsed.set_path(&format!("/{repo_path}"));
        return (
            parsed.to_string().trim_end_matches('/').to_owned(),
            Some(reference),
        );
    }

    let Some((host, path)) = url.split_once('/') else {
        return (url.to_owned(), None);
    };
    let Some((repo_path, reference)) = path.split_once('@') else {
        return (url.to_owned(), None);
    };
    if repo_path.is_empty() || reference.is_empty() {
        return (url.to_owned(), None);
    }
    (format!("{host}/{repo_path}"), Some(reference.to_owned()))
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
    if let Some(rest) = repo.strip_prefix("git@") {
        let (host, path) = rest.split_once(':')?;
        return build(repo.clone(), host.to_owned(), path.to_owned(), reference);
    }
    if repo.starts_with("https://")
        || repo.starts_with("http://")
        || repo.starts_with("ssh://")
        || repo.starts_with("git://")
    {
        let parsed = reqwest::Url::parse(&repo).ok()?;
        return build(
            repo,
            parsed.host_str()?.to_owned(),
            parsed.path().trim_start_matches('/').to_owned(),
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
    let reference = reference;
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
