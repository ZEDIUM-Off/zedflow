use std::path::{Path, PathBuf};
use zedflow_coding_agent::session_cwd::{SessionCwdSource, assert_session_cwd_exists};
struct Source(PathBuf);
impl SessionCwdSource for Source {
    fn cwd(&self) -> &Path {
        &self.0
    }
    fn session_file(&self) -> Option<&Path> {
        Some(Path::new("session.jsonl"))
    }
}
#[test]
fn missing_session_cwd_reports_session_and_fallback() {
    let missing = std::env::temp_dir().join(format!("zedflow-missing-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&missing);
    let source = Source(missing);
    let error = assert_session_cwd_exists(&source, Path::new("/fallback"))
        .unwrap_err()
        .to_string();
    assert!(error.contains("/fallback") && error.contains("does not exist"));
}
