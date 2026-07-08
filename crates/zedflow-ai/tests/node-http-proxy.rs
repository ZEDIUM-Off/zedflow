use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use zedflow_ai::types::ProviderEnv;
use zedflow_ai::utils::node_http_proxy::{
    UNSUPPORTED_PROXY_PROTOCOL_MESSAGE, resolve_http_proxy_url_for_target,
};

const PROXY_ENV_KEYS: &[&str] = &[
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "ALL_PROXY",
    "http_proxy",
    "https_proxy",
    "no_proxy",
    "all_proxy",
    "npm_config_http_proxy",
    "npm_config_https_proxy",
    "npm_config_proxy",
    "npm_config_no_proxy",
];

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct ProxyEnvGuard {
    _lock: MutexGuard<'static, ()>,
    original: Vec<(&'static str, Option<String>)>,
}

impl ProxyEnvGuard {
    fn new() -> Self {
        let lock = ENV_LOCK
            .lock()
            .expect("proxy env lock should not be poisoned");
        let original = PROXY_ENV_KEYS
            .iter()
            .map(|key| (*key, std::env::var(key).ok()))
            .collect();
        for key in PROXY_ENV_KEYS {
            remove_env(key);
        }
        Self {
            _lock: lock,
            original,
        }
    }

    fn set(&self, key: &str, value: &str) {
        set_env(key, value);
    }
}

impl Drop for ProxyEnvGuard {
    fn drop(&mut self) {
        for (key, value) in &self.original {
            match value {
                Some(value) => set_env(key, value),
                None => remove_env(key),
            }
        }
    }
}

fn set_env(key: &str, value: &str) {
    // SAFETY: these integration tests serialize all proxy environment mutation with ENV_LOCK and
    // do not spawn threads while the guard is alive.
    unsafe { std::env::set_var(key, value) }
}

fn remove_env(key: &str) {
    // SAFETY: these integration tests serialize all proxy environment mutation with ENV_LOCK and
    // do not spawn threads while the guard is alive.
    unsafe { std::env::remove_var(key) }
}

fn env(items: &[(&str, &str)]) -> ProviderEnv {
    items
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect::<HashMap<_, _>>()
}

#[test]
fn respects_no_proxy_exclusions() {
    let guard = ProxyEnvGuard::new();
    guard.set("HTTPS_PROXY", "http://proxy.example:8080");
    guard.set("NO_PROXY", "bedrock-runtime.us-east-1.amazonaws.com");

    assert_eq!(
        resolve_http_proxy_url_for_target("https://bedrock-runtime.us-east-1.amazonaws.com", None)
            .expect("proxy resolution should succeed"),
        None
    );
}

#[test]
fn resolves_http_and_https_proxy_urls() {
    let guard = ProxyEnvGuard::new();
    guard.set("HTTPS_PROXY", "http://proxy.example:8080");

    let proxy =
        resolve_http_proxy_url_for_target("https://bedrock-runtime.us-east-1.amazonaws.com", None)
            .expect("proxy resolution should succeed")
            .expect("proxy should be configured");

    assert_eq!(proxy.as_str(), "http://proxy.example:8080/");
}

#[test]
fn prefers_scoped_proxy_env_aliases_before_process_env_aliases() {
    let guard = ProxyEnvGuard::new();
    guard.set("https_proxy", "http://process-proxy.example:8080");
    let scoped_env = env(&[("HTTPS_PROXY", "http://scoped-proxy.example:8080")]);

    let proxy = resolve_http_proxy_url_for_target(
        "https://bedrock-runtime.us-east-1.amazonaws.com",
        Some(&scoped_env),
    )
    .expect("proxy resolution should succeed")
    .expect("proxy should be configured");

    assert_eq!(proxy.as_str(), "http://scoped-proxy.example:8080/");
}

#[test]
fn rejects_socks_and_pac_proxy_urls_explicitly() {
    let guard = ProxyEnvGuard::new();
    guard.set("HTTPS_PROXY", "socks5://proxy.example:1080");

    let error =
        resolve_http_proxy_url_for_target("https://bedrock-runtime.us-east-1.amazonaws.com", None)
            .expect_err("SOCKS proxy should be rejected");

    assert!(
        error
            .to_string()
            .contains(UNSUPPORTED_PROXY_PROTOCOL_MESSAGE),
        "expected unsupported protocol message, got {error}"
    );
}
