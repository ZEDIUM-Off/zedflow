//! Node-style HTTP proxy environment resolution ported from Pi's `packages/ai/src/utils/node-http-proxy.ts`.

use std::error::Error;
use std::fmt;

use url::Url;

use crate::types::ProviderEnv;

const DEFAULT_PROXY_PORTS: &[(&str, u16)] = &[
    ("ftp", 21),
    ("gopher", 70),
    ("http", 80),
    ("https", 443),
    ("ws", 80),
    ("wss", 443),
];

/// Message returned when a proxy URL uses a protocol Pi does not support.
pub const UNSUPPORTED_PROXY_PROTOCOL_MESSAGE: &str = "Unsupported proxy protocol. SOCKS and PAC proxy URLs are not supported; use an HTTP or HTTPS proxy URL.";

/// Error returned while resolving a target URL's HTTP proxy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyUrlError {
    /// The selected proxy environment value could not be parsed as an absolute URL.
    InvalidProxyUrl {
        /// Raw proxy value after Pi's scheme-prefix normalization.
        proxy: String,
        /// URL parser error.
        source: url::ParseError,
    },
    /// The proxy URL parsed, but reqwest rejected it while building a proxy.
    ReqwestProxy {
        /// Parsed proxy URL string.
        proxy: String,
        /// Reqwest proxy builder error message.
        message: String,
    },
    /// The proxy URL parsed, but used a non-HTTP(S) protocol.
    UnsupportedProxyProtocol {
        /// Protocol string including the trailing colon, matching JavaScript `URL.protocol`.
        protocol: String,
    },
}

impl fmt::Display for ProxyUrlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProxyUrl { proxy, source } => {
                let quoted =
                    serde_json::to_string(proxy).unwrap_or_else(|_| format!("\"{proxy}\""));
                write!(formatter, "Invalid proxy URL {quoted}: {source}")
            }
            Self::ReqwestProxy { proxy, message } => {
                write!(formatter, "Invalid proxy URL {proxy:?}: {message}")
            }
            Self::UnsupportedProxyProtocol { protocol } => {
                write!(
                    formatter,
                    "{UNSUPPORTED_PROXY_PROTOCOL_MESSAGE} Got {protocol}"
                )
            }
        }
    }
}

impl Error for ProxyUrlError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidProxyUrl { source, .. } => Some(source),
            Self::ReqwestProxy { .. } => None,
            Self::UnsupportedProxyProtocol { .. } => None,
        }
    }
}

/// Resolves the HTTP(S) proxy URL Pi would use for `target_url`.
///
/// This matches Pi's `resolveHttpProxyUrlForTarget`: invalid target URLs and targets excluded by
/// `NO_PROXY` return `Ok(None)`, invalid proxy environment values return an error, and only HTTP
/// or HTTPS proxy URLs are accepted.
///
/// # Errors
///
/// Returns [`ProxyUrlError::InvalidProxyUrl`] when the selected proxy value is not a valid URL.
/// Returns [`ProxyUrlError::UnsupportedProxyProtocol`] when the proxy URL uses a protocol other
/// than `http:` or `https:`.
pub fn resolve_http_proxy_url_for_target(
    target_url: &str,
    env: Option<&ProviderEnv>,
) -> Result<Option<Url>, ProxyUrlError> {
    let Some(proxy) = proxy_for_url(target_url, env) else {
        return Ok(None);
    };

    let proxy_url = Url::parse(&proxy).map_err(|source| ProxyUrlError::InvalidProxyUrl {
        proxy: proxy.clone(),
        source,
    })?;

    match proxy_url.scheme() {
        "http" | "https" => Ok(Some(proxy_url)),
        scheme => Err(ProxyUrlError::UnsupportedProxyProtocol {
            protocol: format!("{scheme}:"),
        }),
    }
}

/// Resolves Pi's proxy environment rules and builds a reqwest proxy for the target.
///
/// # Errors
///
/// Returns [`ProxyUrlError`] for invalid, unsupported, or reqwest-rejected proxy URLs.
pub(crate) fn resolve_reqwest_proxy_for_target(
    target_url: &str,
    env: Option<&ProviderEnv>,
) -> Result<Option<reqwest::Proxy>, ProxyUrlError> {
    let Some(proxy_url) = resolve_http_proxy_url_for_target(target_url, env)? else {
        return Ok(None);
    };
    let proxy = proxy_url.as_str().to_string();
    reqwest::Proxy::all(&proxy)
        .map(Some)
        .map_err(|source| ProxyUrlError::ReqwestProxy {
            proxy,
            message: source.to_string(),
        })
}

fn proxy_for_url(target_url: &str, env: Option<&ProviderEnv>) -> Option<String> {
    let parsed_url = Url::parse(target_url).ok()?;
    let protocol = parsed_url.scheme();
    let hostname = parsed_url.host_str()?;
    let port = parsed_url
        .port()
        .or_else(|| default_proxy_port(protocol))
        .unwrap_or(0);

    if !should_proxy_hostname(hostname, port, env) {
        return None;
    }

    let mut proxy =
        proxy_env(&format!("{protocol}_proxy"), env).or_else(|| proxy_env("all_proxy", env))?;
    if !proxy.contains("://") {
        proxy = format!("{protocol}://{proxy}");
    }
    Some(proxy)
}

fn proxy_env(key: &str, env: Option<&ProviderEnv>) -> Option<String> {
    let lowercase_key = key.to_lowercase();
    let uppercase_key = key.to_uppercase();

    env_value(env, &lowercase_key)
        .or_else(|| env_value(env, &uppercase_key))
        .or_else(|| process_env_value(&lowercase_key))
        .or_else(|| process_env_value(&uppercase_key))
}

fn env_value(env: Option<&ProviderEnv>, key: &str) -> Option<String> {
    env.and_then(|env| env.get(key))
        .filter(|value| !value.is_empty())
        .cloned()
}

fn process_env_value(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|value| !value.is_empty())
}

fn default_proxy_port(protocol: &str) -> Option<u16> {
    DEFAULT_PROXY_PORTS
        .iter()
        .find_map(|(candidate, port)| (*candidate == protocol).then_some(*port))
}

fn should_proxy_hostname(hostname: &str, port: u16, env: Option<&ProviderEnv>) -> bool {
    let no_proxy = proxy_env("no_proxy", env)
        .unwrap_or_default()
        .to_lowercase();
    if no_proxy.is_empty() {
        return true;
    }
    if no_proxy == "*" {
        return false;
    }

    no_proxy
        .split(|char: char| char == ',' || char.is_whitespace())
        .all(|proxy| {
            if proxy.is_empty() {
                return true;
            }

            let (mut proxy_hostname, proxy_port) = parse_no_proxy_host_port(proxy);
            if proxy_port.is_some_and(|proxy_port| proxy_port != port) {
                return true;
            }

            if !(proxy_hostname.starts_with('.') || proxy_hostname.starts_with('*')) {
                return hostname != proxy_hostname;
            }

            if let Some(stripped) = proxy_hostname.strip_prefix('*') {
                proxy_hostname = stripped;
            }
            !hostname.ends_with(proxy_hostname)
        })
}

fn parse_no_proxy_host_port(proxy: &str) -> (&str, Option<u16>) {
    let Some((hostname, port)) = proxy.rsplit_once(':') else {
        return (proxy, None);
    };
    if hostname.is_empty() || port.is_empty() || !port.chars().all(|char| char.is_ascii_digit()) {
        return (proxy, None);
    }

    match port.parse::<u16>() {
        Ok(port) => (hostname, Some(port)),
        Err(_) => (proxy, None),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn env(items: &[(&str, &str)]) -> ProviderEnv {
        items
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect::<HashMap<_, _>>()
    }

    #[test]
    fn resolves_protocol_proxy_and_adds_missing_scheme() {
        let env = env(&[("https_proxy", "proxy.example:8443")]);
        let proxy = resolve_http_proxy_url_for_target("https://api.example/v1", Some(&env))
            .expect("proxy should parse")
            .expect("proxy should be configured");

        assert_eq!(proxy.as_str(), "https://proxy.example:8443/");
    }

    #[test]
    fn no_proxy_star_disables_proxy() {
        let env = env(&[("HTTPS_PROXY", "https://proxy.example"), ("NO_PROXY", "*")]);

        assert_eq!(
            resolve_http_proxy_url_for_target("https://api.example", Some(&env))
                .expect("proxy resolution should succeed"),
            None
        );
    }

    #[test]
    fn no_proxy_honors_ports_and_suffixes() {
        let env = env(&[
            ("HTTPS_PROXY", "https://proxy.example"),
            ("NO_PROXY", "internal.example:444, .example.test"),
        ]);

        assert!(
            resolve_http_proxy_url_for_target("https://internal.example:443", Some(&env))
                .expect("proxy resolution should succeed")
                .is_some()
        );
        assert_eq!(
            resolve_http_proxy_url_for_target("https://service.example.test", Some(&env))
                .expect("proxy resolution should succeed"),
            None
        );
    }

    #[test]
    fn rejects_unsupported_proxy_protocol() {
        let env = env(&[("all_proxy", "socks5://proxy.example:1080")]);

        assert_eq!(
            resolve_http_proxy_url_for_target("https://api.example", Some(&env)),
            Err(ProxyUrlError::UnsupportedProxyProtocol {
                protocol: "socks5:".to_string(),
            })
        );
    }

    #[test]
    fn invalid_target_url_has_no_proxy() {
        let env = env(&[("all_proxy", "https://proxy.example")]);

        assert_eq!(
            resolve_http_proxy_url_for_target("not a url", Some(&env))
                .expect("invalid targets should not fail proxy resolution"),
            None
        );
    }
}
