//! Data used by the interactive footer.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// Branch/status data not otherwise exposed to interactive extensions.
pub struct FooterDataProvider {
    cwd: PathBuf,
    cached_branch: Option<Option<String>>,
    extension_statuses: BTreeMap<String, String>,
    available_provider_count: usize,
}

impl FooterDataProvider {
    #[must_use]
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            cached_branch: None,
            extension_statuses: BTreeMap::new(),
            available_provider_count: 0,
        }
    }

    /// Current branch, or `detached` for a detached HEAD and `None` outside a repository.
    pub fn get_git_branch(&mut self) -> Option<&str> {
        if self.cached_branch.is_none() {
            self.cached_branch = Some(resolve_git_branch(&self.cwd));
        }
        self.cached_branch.as_ref().and_then(Option::as_deref)
    }

    #[must_use]
    pub fn get_extension_statuses(&self) -> &BTreeMap<String, String> {
        &self.extension_statuses
    }

    pub fn set_extension_status(&mut self, key: impl Into<String>, text: Option<String>) {
        if let Some(text) = text {
            self.extension_statuses.insert(key.into(), text);
        } else {
            self.extension_statuses.remove(&key.into());
        }
    }

    pub fn clear_extension_statuses(&mut self) {
        self.extension_statuses.clear();
    }

    #[must_use]
    pub const fn get_available_provider_count(&self) -> usize {
        self.available_provider_count
    }

    pub fn set_available_provider_count(&mut self, count: usize) {
        self.available_provider_count = count;
    }

    pub fn set_cwd(&mut self, cwd: impl Into<PathBuf>) {
        let cwd = cwd.into();
        if self.cwd != cwd {
            self.cwd = cwd;
            self.cached_branch = None;
        }
    }

    pub fn dispose(&mut self) {
        self.cached_branch = None;
    }
}

fn resolve_git_branch(cwd: &Path) -> Option<String> {
    let (repo, git_dir) = find_git_dir(cwd)?;
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    if let Some(branch) = head.strip_prefix("ref: refs/heads/") {
        // During an unborn/ref transition, git is the authoritative fallback.
        if branch != ".invalid" {
            return Some(branch.to_owned());
        }
        return std::process::Command::new("git")
            .args([
                "--no-optional-locks",
                "symbolic-ref",
                "--quiet",
                "--short",
                "HEAD",
            ])
            .current_dir(repo)
            .output()
            .ok()
            .filter(|result| result.status.success())
            .and_then(|result| String::from_utf8(result.stdout).ok())
            .map(|branch| branch.trim().to_owned())
            .filter(|branch| !branch.is_empty())
            .or_else(|| Some("detached".into()));
    }
    Some("detached".into())
}

fn find_git_dir(cwd: &Path) -> Option<(PathBuf, PathBuf)> {
    for directory in cwd.ancestors() {
        let git = directory.join(".git");
        if git.is_dir() && git.join("HEAD").is_file() {
            return Some((directory.to_path_buf(), git));
        }
        if git.is_file() {
            let git_file = fs::read_to_string(&git).ok()?;
            let target = git_file.trim().strip_prefix("gitdir: ")?.trim();
            let target = Path::new(target);
            let git_dir = if target.is_absolute() {
                target.into()
            } else {
                directory.join(target)
            };
            if git_dir.join("HEAD").is_file() {
                return Some((directory.to_path_buf(), git_dir));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_regular_and_detached_heads() {
        let root = std::env::temp_dir().join(format!("zedflow-footer-{}", std::process::id()));
        let git = root.join(".git");
        fs::create_dir_all(&git).unwrap();
        fs::write(git.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        let mut provider = FooterDataProvider::new(&root);
        assert_eq!(provider.get_git_branch(), Some("main"));
        fs::write(git.join("HEAD"), "012345\n").unwrap();
        provider.set_cwd(root.clone());
        provider.cached_branch = None;
        assert_eq!(provider.get_git_branch(), Some("detached"));
        let _ = fs::remove_dir_all(root);
    }
}
