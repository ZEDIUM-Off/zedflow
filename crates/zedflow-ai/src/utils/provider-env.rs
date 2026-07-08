//! Provider environment lookup ported from Pi's `packages/ai/src/utils/provider-env.ts`.

use std::collections::HashMap;
use std::fs;
use std::sync::OnceLock;

use crate::types::ProviderEnv;

static PROC_ENV_CACHE: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Resolves a provider environment value from scoped overrides, process environment,
/// then Pi's Bun sandbox `/proc/self/environ` fallback.
#[must_use]
pub fn get_provider_env_value(name: &str, env: Option<&ProviderEnv>) -> Option<String> {
    env.and_then(|env| env.get(name))
        .filter(|value| !value.is_empty())
        .cloned()
        .or_else(|| {
            std::env::var_os(name)
                .map(|value| value.to_string_lossy().into_owned())
                .filter(|value| !value.is_empty())
        })
        .or_else(|| get_bun_sandbox_env_value(name))
}

fn get_bun_sandbox_env_value(name: &str) -> Option<String> {
    if std::env::vars_os().next().is_some() {
        return None;
    }

    PROC_ENV_CACHE
        .get_or_init(read_proc_env)
        .get(name)
        .filter(|value| !value.is_empty())
        .cloned()
}

fn read_proc_env() -> HashMap<String, String> {
    fs::read("/proc/self/environ")
        .ok()
        .map(|data| {
            String::from_utf8_lossy(&data)
                .split('\0')
                .filter_map(|entry| {
                    let (key, value) = entry.split_once('=')?;
                    (!key.is_empty()).then(|| (key.to_owned(), value.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_wins_and_empty_override_falls_through() {
        let key = format!("ZEDFLOW_PROVIDER_ENV_TEST_{}", std::process::id());
        let mut env = ProviderEnv::new();
        env.insert(key.clone(), "override".to_owned());

        assert_eq!(
            get_provider_env_value(&key, Some(&env)).as_deref(),
            Some("override")
        );

        env.insert(key.clone(), String::new());
        assert_eq!(
            get_provider_env_value(&key, Some(&env)),
            std::env::var_os(&key)
                .map(|value| value.to_string_lossy().into_owned())
                .filter(|value| !value.is_empty())
        );
    }
}
