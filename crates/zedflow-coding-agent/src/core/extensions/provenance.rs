use std::{
    fs, io,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExtensionSource {
    Crate {
        name: String,
        version: String,
    },
    Github {
        owner: String,
        repo: String,
        commit: String,
        package: Option<String>,
    },
    Path(PathBuf),
}

impl ExtensionSource {
    pub fn parse(value: &str) -> Result<Self, String> {
        if let Some(value) = value.strip_prefix("crate:") {
            let (name, version) = value
                .rsplit_once('@')
                .ok_or("crate source must pin an exact version")?;
            if !is_crate_name(name) || !is_exact_version(version) {
                return Err("crate source must pin an exact version".into());
            }
            return Ok(Self::Crate {
                name: name.into(),
                version: version.into(),
            });
        }
        if let Some(value) = value.strip_prefix("github:") {
            let (repo, tail) = value
                .rsplit_once('@')
                .ok_or("github source must pin a resolved commit")?;
            let (commit, package) = tail
                .split_once('#')
                .map_or((tail, None), |(commit, package)| (commit, Some(package)));
            let (owner, repo) = repo
                .split_once('/')
                .ok_or("github source must be owner/repo")?;
            if !is_repository_part(owner)
                || !is_repository_part(repo)
                || !is_commit(commit)
                || package.is_some_and(|path| !is_safe_relative_path(path))
            {
                return Err("github source must be owner/repo@<40-hex-commit>[#package]".into());
            }
            return Ok(Self::Github {
                owner: owner.into(),
                repo: repo.into(),
                commit: commit.into(),
                package: package.map(str::to_owned),
            });
        }
        if let Some(path) = value.strip_prefix("path:") {
            if path.is_empty() {
                return Err("path source must not be empty".into());
            }
            return Ok(Self::Path(PathBuf::from(path)));
        }
        Err("extension source must begin crate:, github:, or path:".into())
    }

    #[must_use]
    pub fn canonical(&self) -> String {
        match self {
            Self::Crate { name, version } => format!("crate:{name}@{version}"),
            Self::Github {
                owner,
                repo,
                commit,
                package,
            } => format!(
                "github:{owner}/{repo}@{commit}{}",
                package.as_ref().map_or(String::new(), |p| format!("#{p}"))
            ),
            Self::Path(path) => format!("path:{}", path.display()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvenanceReceipt {
    pub source: String,
    pub source_sha256: String,
    pub artifact_sha256: String,
    pub previous_artifact_sha256: Option<String>,
}

pub fn receipt(
    source: &ExtensionSource,
    source_dir: &Path,
    artifact: &Path,
    previous: Option<String>,
) -> io::Result<ProvenanceReceipt> {
    Ok(ProvenanceReceipt {
        source: source.canonical(),
        source_sha256: digest_tree(source_dir)?,
        artifact_sha256: digest_file(artifact)?,
        previous_artifact_sha256: previous,
    })
}

pub fn digest_file(path: &Path) -> io::Result<String> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}

pub fn digest_tree(root: &Path) -> io::Result<String> {
    let mut files = Vec::new();
    collect(root, root, &mut files)?;
    files.sort();
    let mut hash = Sha256::new();
    for file in files {
        hash.update(file.to_string_lossy().as_bytes());
        hash.update([0]);
        hash.update(fs::read(root.join(file))?);
        hash.update([0]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn collect(root: &Path, path: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "source tree contains symlink",
            ));
        }
        if ty.is_dir() {
            collect(root, &entry.path(), files)?;
        } else if ty.is_file() {
            files.push(
                entry
                    .path()
                    .strip_prefix(root)
                    .expect("descendant")
                    .to_path_buf(),
            );
        }
    }
    Ok(())
}

fn is_crate_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn is_repository_part(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn is_safe_relative_path(value: &str) -> bool {
    !value.is_empty()
        && Path::new(value).is_relative()
        && Path::new(value)
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)))
}

fn is_commit(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn is_exact_version(value: &str) -> bool {
    let core = value.split(['-', '+']).next().unwrap_or_default();
    core.split('.').count() == 3
        && core
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        && !value.contains(['*', '^', '~', '<', '>', '=', ' '])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_pinned_sources_are_accepted() {
        assert!(ExtensionSource::parse("crate:demo@1.2.3").is_ok());
        assert!(
            ExtensionSource::parse(
                "github:acme/demo@0123456789abcdef0123456789abcdef01234567#plugin"
            )
            .is_ok()
        );
        assert!(ExtensionSource::parse("crate:demo@^1").is_err());
        assert!(ExtensionSource::parse("github:acme/demo@main").is_err());
        assert!(
            ExtensionSource::parse(
                "github:acme/demo@0123456789abcdef0123456789abcdef01234567#../plugin"
            )
            .is_err()
        );
    }
}
