use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

pub trait SessionCwdSource {
    fn cwd(&self) -> &Path;
    fn session_file(&self) -> Option<&Path>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionCwdIssue {
    pub session_file: Option<PathBuf>,
    pub session_cwd: PathBuf,
    pub fallback_cwd: PathBuf,
}

pub fn get_missing_session_cwd_issue(
    source: &impl SessionCwdSource,
    fallback_cwd: &Path,
) -> Option<SessionCwdIssue> {
    let session_file = source.session_file()?;
    let session_cwd = source.cwd();
    if session_cwd.as_os_str().is_empty() || session_cwd.exists() {
        return None;
    }
    Some(SessionCwdIssue {
        session_file: Some(session_file.to_owned()),
        session_cwd: session_cwd.to_owned(),
        fallback_cwd: fallback_cwd.to_owned(),
    })
}

pub fn format_missing_session_cwd_error(issue: &SessionCwdIssue) -> String {
    let session_file = issue
        .session_file
        .as_ref()
        .map(|path| format!("\nSession file: {}", path.display()))
        .unwrap_or_default();
    format!(
        "Stored session working directory does not exist: {}{session_file}\nCurrent working directory: {}",
        issue.session_cwd.display(),
        issue.fallback_cwd.display()
    )
}

pub fn format_missing_session_cwd_prompt(issue: &SessionCwdIssue) -> String {
    format!(
        "cwd from session file does not exist\n{}\n\ncontinue in current cwd\n{}",
        issue.session_cwd.display(),
        issue.fallback_cwd.display()
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissingSessionCwdError(pub SessionCwdIssue);
impl fmt::Display for MissingSessionCwdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&format_missing_session_cwd_error(&self.0))
    }
}
impl Error for MissingSessionCwdError {}

pub fn assert_session_cwd_exists(
    source: &impl SessionCwdSource,
    fallback_cwd: &Path,
) -> Result<(), MissingSessionCwdError> {
    get_missing_session_cwd_issue(source, fallback_cwd)
        .map_or(Ok(()), |issue| Err(MissingSessionCwdError(issue)))
}
