//! Default auth context ported from Pi.

use std::ffi::OsString;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

/// Boxed future returned by [`AuthContext`] methods.
pub type AuthFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// Environment and filesystem access used while resolving provider auth.
pub trait AuthContext {
    /// Returns a non-empty environment variable value after trimming whitespace.
    fn env<'a>(&'a self, name: &'a str) -> AuthFuture<'a, Option<String>>;

    /// Returns whether a path exists, expanding a leading `~` to the home directory.
    fn file_exists<'a>(&'a self, path: &'a str) -> AuthFuture<'a, bool>;
}

/// Default provider auth context backed by process environment and local filesystem.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DefaultProviderAuthContext;

/// Creates the default provider auth context.
#[must_use]
pub const fn default_provider_auth_context() -> DefaultProviderAuthContext {
    DefaultProviderAuthContext
}

impl AuthContext for DefaultProviderAuthContext {
    fn env<'a>(&'a self, name: &'a str) -> AuthFuture<'a, Option<String>> {
        Box::pin(async move { env_value(name) })
    }

    fn file_exists<'a>(&'a self, path: &'a str) -> AuthFuture<'a, bool> {
        Box::pin(async move { path_exists(path) })
    }
}

fn env_value(name: &str) -> Option<String> {
    let value = std::env::var(name).ok()?;
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn path_exists(path: &str) -> bool {
    resolve_home(path).exists()
}

fn resolve_home(path: &str) -> PathBuf {
    let Some(rest) = path.strip_prefix('~') else {
        return PathBuf::from(path);
    };

    home_dir()
        .map(|mut home| {
            home.push(rest);
            PathBuf::from(home)
        })
        .unwrap_or_else(|| PathBuf::from(path))
}

fn home_dir() -> Option<OsString> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_value_filters_missing_or_blank_values() {
        assert_eq!(env_value("ZEDFLOW_AI_PORT_TEST_ENV_SHOULD_NOT_EXIST"), None);
    }

    #[test]
    fn path_exists_matches_local_filesystem() {
        let path =
            std::env::temp_dir().join(format!("zedflow-ai-auth-context-{}", std::process::id()));
        let path = path.to_string_lossy();
        std::fs::write(path.as_ref(), b"ok").expect("write temp auth context probe");
        assert!(path_exists(path.as_ref()));
        std::fs::remove_file(path.as_ref()).expect("remove temp auth context probe");
        assert!(!path_exists(path.as_ref()));
    }

    #[test]
    fn resolve_home_leaves_non_home_paths_unchanged() {
        assert_eq!(resolve_home("/tmp/pi"), PathBuf::from("/tmp/pi"));
    }

    #[test]
    fn resolve_home_matches_pi_string_prefix_semantics() {
        let Some(home) = home_dir() else {
            return;
        };

        let mut expected = home;
        expected.push("pi");
        assert_eq!(resolve_home("~pi"), PathBuf::from(expected));
    }
}
