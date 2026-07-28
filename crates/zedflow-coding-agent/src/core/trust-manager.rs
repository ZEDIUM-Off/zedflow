use fs2::FileExt;
use std::{
    collections::BTreeMap,
    env, fs,
    fs::OpenOptions,
    io,
    path::{Component, Path, PathBuf},
    thread,
    time::Duration,
};

use crate::config::CONFIG_DIR_NAME;

pub type ProjectTrustDecision = Option<bool>;

type TrustFile = BTreeMap<String, Option<bool>>;

const TRUST_REQUIRING_PROJECT_CONFIG_RESOURCES: &[&str] = &[
    "settings.json",
    "extensions",
    "skills",
    "prompts",
    "themes",
    "SYSTEM.md",
    "APPEND_SYSTEM.md",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectTrustStoreEntry {
    pub path: PathBuf,
    pub decision: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectTrustUpdate {
    pub path: PathBuf,
    pub decision: ProjectTrustDecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectTrustOption {
    pub label: String,
    pub trusted: bool,
    pub updates: Vec<ProjectTrustUpdate>,
    pub saved_path: Option<PathBuf>,
}

fn lexical_normalize(path: impl AsRef<Path>) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.as_ref().components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir if result.file_name().is_some() => {
                result.pop();
            }
            Component::ParentDir => result.push(component),
            _ => result.push(component),
        }
    }
    result
}

fn normalize_cwd(cwd: impl AsRef<Path>) -> io::Result<PathBuf> {
    let path = cwd.as_ref();
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()?.join(path)
    };
    Ok(fs::canonicalize(&absolute).unwrap_or_else(|_| lexical_normalize(absolute)))
}

fn find_nearest_trust_entry(
    data: &TrustFile,
    cwd: impl AsRef<Path>,
) -> io::Result<Option<ProjectTrustStoreEntry>> {
    let mut current = normalize_cwd(cwd)?;
    loop {
        if let Some(decision) = data
            .get(&current.to_string_lossy().into_owned())
            .copied()
            .flatten()
        {
            return Ok(Some(ProjectTrustStoreEntry {
                path: current,
                decision,
            }));
        }
        if !current.pop() {
            return Ok(None);
        }
    }
}

pub fn get_project_trust_parent_path(cwd: impl AsRef<Path>) -> io::Result<Option<PathBuf>> {
    let path = normalize_cwd(cwd)?;
    Ok(path.parent().map(Path::to_path_buf))
}

pub fn get_project_trust_options(
    cwd: impl AsRef<Path>,
    include_session_only: bool,
) -> io::Result<Vec<ProjectTrustOption>> {
    let trust_path = normalize_cwd(cwd)?;
    let mut options = vec![ProjectTrustOption {
        label: "Trust".into(),
        trusted: true,
        updates: vec![ProjectTrustUpdate {
            path: trust_path.clone(),
            decision: Some(true),
        }],
        saved_path: Some(trust_path.clone()),
    }];
    if let Some(parent) = trust_path.parent().map(Path::to_path_buf) {
        options.push(ProjectTrustOption {
            label: format!("Trust parent folder ({})", parent.display()),
            trusted: true,
            updates: vec![
                ProjectTrustUpdate {
                    path: parent.clone(),
                    decision: Some(true),
                },
                ProjectTrustUpdate {
                    path: trust_path.clone(),
                    decision: None,
                },
            ],
            saved_path: Some(parent),
        });
    }
    if include_session_only {
        options.push(ProjectTrustOption {
            label: "Trust (this session only)".into(),
            trusted: true,
            updates: Vec::new(),
            saved_path: None,
        });
    }
    options.push(ProjectTrustOption {
        label: "Do not trust".into(),
        trusted: false,
        updates: vec![ProjectTrustUpdate {
            path: trust_path.clone(),
            decision: Some(false),
        }],
        saved_path: Some(trust_path),
    });
    if include_session_only {
        options.push(ProjectTrustOption {
            label: "Do not trust (this session only)".into(),
            trusted: false,
            updates: Vec::new(),
            saved_path: None,
        });
    }
    Ok(options)
}

fn read_trust_file(path: &Path) -> io::Result<TrustFile> {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to read trust store {}: {error}", path.display()),
            )
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(error) => Err(error),
    }
}

fn write_trust_file(path: &Path, data: &TrustFile) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut content = serde_json::to_string_pretty(data).map_err(io::Error::other)?;
    content.push('\n');
    fs::write(path, content)
}

fn with_trust_file_lock<T>(path: &Path, f: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let lock_path = PathBuf::from(format!("{}.lock", path.display()));
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(lock_path)?;
    let mut last_error = None;
    for _ in 0..10 {
        match lock.try_lock_exclusive() {
            Ok(()) => {
                let result = f();
                lock.unlock()?;
                return result;
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                last_error = Some(error);
                thread::sleep(Duration::from_millis(20));
            }
            Err(error) => return Err(error),
        }
    }
    Err(last_error.unwrap_or_else(|| io::Error::other("Failed to acquire trust store lock")))
}

/// Returns whether project-local resources exist that must be gated by trust.
pub fn has_trust_requiring_project_resources(cwd: impl AsRef<Path>) -> bool {
    let Ok(mut current) = normalize_cwd(cwd) else {
        return false;
    };
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .and_then(|path| normalize_cwd(path).ok());
    let user_skills = home.map(|path| path.join(".agents").join("skills"));

    let config_dir = current.join(CONFIG_DIR_NAME);
    if TRUST_REQUIRING_PROJECT_CONFIG_RESOURCES
        .iter()
        .any(|entry| config_dir.join(entry).exists())
    {
        return true;
    }

    loop {
        let skills = current.join(".agents").join("skills");
        if user_skills.as_ref() != Some(&skills) && skills.exists() {
            return true;
        }
        if !current.pop() {
            return false;
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectTrustStore {
    trust_path: PathBuf,
}

impl ProjectTrustStore {
    #[must_use]
    pub fn new(agent_dir: impl AsRef<Path>) -> Self {
        Self {
            trust_path: agent_dir.as_ref().join("trust.json"),
        }
    }

    pub fn get(&self, cwd: impl AsRef<Path>) -> io::Result<ProjectTrustDecision> {
        Ok(self.get_entry(cwd)?.map(|entry| entry.decision))
    }

    pub fn get_entry(&self, cwd: impl AsRef<Path>) -> io::Result<Option<ProjectTrustStoreEntry>> {
        with_trust_file_lock(&self.trust_path, || {
            find_nearest_trust_entry(&read_trust_file(&self.trust_path)?, cwd)
        })
    }

    pub fn set(&self, cwd: impl AsRef<Path>, decision: ProjectTrustDecision) -> io::Result<()> {
        self.set_many(&[ProjectTrustUpdate {
            path: cwd.as_ref().to_path_buf(),
            decision,
        }])
    }

    pub fn set_many(&self, decisions: &[ProjectTrustUpdate]) -> io::Result<()> {
        with_trust_file_lock(&self.trust_path, || {
            let mut data = read_trust_file(&self.trust_path)?;
            for update in decisions {
                let key = normalize_cwd(&update.path)?.to_string_lossy().into_owned();
                match update.decision {
                    Some(decision) => {
                        data.insert(key, Some(decision));
                    }
                    None => {
                        data.remove(&key);
                    }
                }
            }
            write_trust_file(&self.trust_path, &data)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        env::temp_dir().join(format!(
            "zedflow-trust-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn decisions_inherit_and_child_overrides_can_be_removed() {
        let root = temp_dir();
        let agent = root.join("agent");
        let parent = root.join("trusted-parent");
        let child = parent.join("project");
        fs::create_dir_all(&child).unwrap();
        let store = ProjectTrustStore::new(agent);

        assert_eq!(store.get(&child).unwrap(), None);
        store.set(&parent, Some(true)).unwrap();
        assert_eq!(store.get(&child).unwrap(), Some(true));
        store.set(&child, Some(false)).unwrap();
        assert_eq!(store.get(&child).unwrap(), Some(false));
        store.set(&child, None).unwrap();
        assert_eq!(store.get(&child).unwrap(), Some(true));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn detects_pi_resources() {
        let root = temp_dir();
        let project = root.join("project");
        fs::create_dir_all(project.join(".pi")).unwrap();
        assert!(!has_trust_requiring_project_resources(&project));
        fs::write(project.join(".pi/settings.json"), "{}").unwrap();
        assert!(has_trust_requiring_project_resources(&project));
        fs::remove_dir_all(root).unwrap();
    }
}
