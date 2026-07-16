//! Command helpers ported from Pi's `packages/ai/src/cli.ts`.

use std::collections::BTreeMap;
use std::error::Error as StdError;
use std::fmt;
use std::fs;
use std::path::Path;

use crate::auth::types::{
    AuthEvent, AuthFuture, AuthLoginCallbacks, AuthPrompt, AuthResult, BoxError,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Pi CLI auth file name.
pub const AUTH_FILE: &str = "auth.json";

/// Built-in OAuth providers listed by Pi's CLI.
pub const BUILT_IN_OAUTH_PROVIDERS: &[OAuthProviderInfo] = &[
    OAuthProviderInfo {
        id: "anthropic",
        name: "Anthropic (Claude Pro/Max)",
    },
    OAuthProviderInfo {
        id: "github-copilot",
        name: "GitHub Copilot",
    },
    OAuthProviderInfo {
        id: "openai-codex",
        name: "ChatGPT Plus/Pro (Codex Subscription)",
    },
];

/// OAuth provider identity displayed and selected by the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OAuthProviderInfo {
    /// Provider id used on the command line.
    pub id: &'static str,
    /// Human-readable provider name.
    pub name: &'static str,
}

/// Stored OAuth credential fields in `auth.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OAuthCredentials {
    /// Refresh token.
    pub refresh: String,
    /// Access token.
    pub access: String,
    /// Expiration timestamp in milliseconds, matching Pi's `Date.now()` values.
    pub expires: i64,
    /// Provider-specific OAuth fields preserved from Pi's open credential shape.
    #[serde(default, flatten, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, Value>,
}

/// Type-tagged OAuth credential entry saved in `auth.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OAuthAuthEntry {
    /// Credential type tag; Pi writes `"oauth"`.
    #[serde(rename = "type")]
    pub credential_type: OAuthCredentialType,
    /// OAuth credential payload.
    #[serde(flatten)]
    pub credentials: OAuthCredentials,
}

impl OAuthAuthEntry {
    /// Creates an OAuth auth entry with Pi's `type: "oauth"` tag.
    #[must_use]
    pub const fn new(credentials: OAuthCredentials) -> Self {
        Self {
            credential_type: OAuthCredentialType::Oauth,
            credentials,
        }
    }
}

/// Credential type tag used by `auth.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OAuthCredentialType {
    /// OAuth credential entry.
    Oauth,
}

/// Parsed CLI command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    /// Print usage/help and exit successfully.
    Help,
    /// List available OAuth providers and exit successfully.
    List,
    /// Login to a selected provider, or prompt if no provider is present.
    Login {
        /// Provider id supplied after `login`.
        provider: Option<String>,
    },
}

/// Error returned by CLI parsing and persistence helpers.
#[derive(Debug)]
#[non_exhaustive]
pub enum CliError {
    /// Command is not recognized.
    UnknownCommand(String),
    /// Provider id is not registered.
    UnknownProvider(String),
    /// Interactive provider selection was outside the provider list.
    InvalidSelection(String),
    /// Filesystem access failed while saving credentials.
    Io(std::io::Error),
    /// JSON encoding failed while saving credentials.
    Json(serde_json::Error),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCommand(command) => write!(f, "Unknown command: {command}"),
            Self::UnknownProvider(provider) => write!(f, "Unknown provider: {provider}"),
            Self::InvalidSelection(selection) => write!(f, "Invalid selection: {selection}"),
            Self::Io(error) => error.fmt(f),
            Self::Json(error) => error.fmt(f),
        }
    }
}

impl StdError for CliError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::UnknownCommand(_) | Self::UnknownProvider(_) | Self::InvalidSelection(_) => None,
        }
    }
}

impl From<std::io::Error> for CliError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for CliError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

/// Parses Pi CLI arguments, excluding the executable name.
///
/// # Errors
///
/// Returns [`CliError::UnknownCommand`] for unrecognized commands.
pub fn parse_command<I, S>(args: I) -> std::result::Result<CliCommand, CliError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args = args
        .into_iter()
        .map(|arg| arg.as_ref().to_string())
        .collect::<Vec<_>>();
    let command = args.first().map(String::as_str);

    match command {
        None | Some("help" | "--help" | "-h") => Ok(CliCommand::Help),
        Some("list") => Ok(CliCommand::List),
        Some("login") => Ok(CliCommand::Login {
            provider: args.get(1).cloned(),
        }),
        Some(command) => Err(CliError::UnknownCommand(command.to_string())),
    }
}

/// Builds Pi's usage text for the supplied providers.
#[must_use]
pub fn usage_text(providers: &[OAuthProviderInfo]) -> String {
    let provider_list = providers
        .iter()
        .map(|provider| format!("  {:<20} {}", provider.id, provider.name))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "Usage: npx @earendil-works/pi-ai <command> [provider]\n\n\
Commands:\n  login [provider]  Login to an OAuth provider\n  list              List available providers\n\n\
Providers:\n{provider_list}\n\n\
Examples:\n  npx @earendil-works/pi-ai login              # interactive provider selection\n  npx @earendil-works/pi-ai login anthropic    # login to specific provider\n  npx @earendil-works/pi-ai list               # list providers\n"
    )
}

/// Builds Pi's provider listing text.
#[must_use]
pub fn provider_list_text(providers: &[OAuthProviderInfo]) -> String {
    let rows = providers
        .iter()
        .map(|provider| format!("  {:<20} {}", provider.id, provider.name))
        .collect::<Vec<_>>()
        .join("\n");
    format!("Available OAuth providers:\n\n{rows}")
}

/// Returns a provider by id.
#[must_use]
pub fn get_oauth_provider<'a>(
    providers: &'a [OAuthProviderInfo],
    provider_id: &str,
) -> Option<&'a OAuthProviderInfo> {
    providers.iter().find(|provider| provider.id == provider_id)
}

/// Validates a provider id against the supplied provider list.
///
/// # Errors
///
/// Returns [`CliError::UnknownProvider`] when the provider id is absent.
pub fn validate_provider(
    providers: &[OAuthProviderInfo],
    provider_id: &str,
) -> std::result::Result<(), CliError> {
    if get_oauth_provider(providers, provider_id).is_some() {
        return Ok(());
    }

    Err(CliError::UnknownProvider(provider_id.to_string()))
}

/// Converts Pi's interactive numeric selection into a provider.
///
/// # Errors
///
/// Returns [`CliError::InvalidSelection`] when the selection does not select an
/// existing provider.
pub fn provider_from_selection<'a>(
    providers: &'a [OAuthProviderInfo],
    selection: &str,
) -> std::result::Result<&'a OAuthProviderInfo, CliError> {
    let Some(index) = parse_js_integer(selection).and_then(|value| value.checked_sub(1)) else {
        return Err(CliError::InvalidSelection(selection.to_string()));
    };

    usize::try_from(index)
        .ok()
        .and_then(|index| providers.get(index))
        .ok_or_else(|| CliError::InvalidSelection(selection.to_string()))
}

/// Loads `auth.json`, returning an empty map when the file is missing or invalid.
#[must_use]
pub fn load_auth_from_path(path: impl AsRef<Path>) -> BTreeMap<String, OAuthAuthEntry> {
    let path = path.as_ref();
    if !path.exists() {
        return BTreeMap::new();
    }

    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

/// Saves `auth.json` with Pi's two-space pretty JSON formatting.
///
/// # Errors
///
/// Returns an error if JSON encoding or filesystem writing fails.
pub fn save_auth_to_path(
    path: impl AsRef<Path>,
    auth: &BTreeMap<String, OAuthAuthEntry>,
) -> std::result::Result<(), CliError> {
    let content = serde_json::to_string_pretty(auth)?;
    fs::write(path, content)?;
    Ok(())
}

/// Runs the provider OAuth login flow with caller-owned interaction callbacks.
///
/// # Errors
///
/// Returns unknown-provider, prompt, cancellation, or provider login failures.
pub async fn login_with_callbacks(
    provider_id: &str,
    callbacks: &dyn AuthLoginCallbacks,
) -> AuthResult<OAuthCredentials> {
    let provider = crate::utils::oauth::index::get_oauth_provider(provider_id)
        .ok_or_else(|| Box::new(CliError::UnknownProvider(provider_id.to_owned())) as BoxError)?;
    let credential = provider.login(callbacks).await?;
    Ok(OAuthCredentials {
        refresh: credential.refresh,
        access: credential.access,
        expires: credential.expires,
        extra: credential.extra,
    })
}

/// Runs the provider OAuth login flow using non-interactive callbacks.
///
/// This helper is useful for tests and callers that only need provider lookup. Real CLI callers
/// should use [`login_with_callbacks`] so prompts and auth/device-code notifications reach the UI.
///
/// # Errors
///
/// Returns an error when the selected provider needs interactive input.
pub async fn login(provider_id: &str) -> AuthResult<OAuthCredentials> {
    login_with_callbacks(provider_id, &NonInteractiveLoginCallbacks).await
}

#[derive(Debug, Clone, Copy)]
struct NonInteractiveLoginCallbacks;

impl AuthLoginCallbacks for NonInteractiveLoginCallbacks {
    fn prompt<'a>(&'a self, _prompt: AuthPrompt) -> AuthFuture<'a, AuthResult<String>> {
        Box::pin(async {
            Err(Box::new(CliError::InvalidSelection(
                "interactive OAuth login requires callbacks".to_owned(),
            )) as BoxError)
        })
    }

    fn notify(&self, _event: AuthEvent) {}
}

fn parse_js_integer(input: &str) -> Option<isize> {
    let trimmed = input.trim_start();
    let mut chars = trimmed.char_indices().peekable();
    let sign = match chars.peek().map(|(_, ch)| *ch) {
        Some('-') => {
            chars.next();
            -1
        }
        Some('+') => {
            chars.next();
            1
        }
        _ => 1,
    };

    let start = chars.peek().map(|(index, _)| *index)?;
    let mut end = start;
    let mut saw_digit = false;
    for (index, ch) in chars {
        if ch.is_ascii_digit() {
            saw_digit = true;
            end = index + ch.len_utf8();
        } else {
            break;
        }
    }

    if !saw_digit {
        return None;
    }

    trimmed[start..end]
        .parse::<isize>()
        .ok()
        .and_then(|value| value.checked_mul(sign))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn parses_commands_like_pi_cli() {
        assert_eq!(
            parse_command(std::iter::empty::<&str>()).unwrap(),
            CliCommand::Help
        );
        assert_eq!(parse_command(["--help"]).unwrap(), CliCommand::Help);
        assert_eq!(parse_command(["list"]).unwrap(), CliCommand::List);
        assert_eq!(
            parse_command(["login", "anthropic"]).unwrap(),
            CliCommand::Login {
                provider: Some("anthropic".to_string())
            }
        );
        assert!(matches!(
            parse_command(["wat"]),
            Err(CliError::UnknownCommand(command)) if command == "wat"
        ));
    }

    #[test]
    fn formats_usage_and_list_with_provider_padding() {
        let providers = [OAuthProviderInfo {
            id: "anthropic",
            name: "Anthropic (Claude Pro/Max)",
        }];

        assert!(
            usage_text(&providers).contains("  anthropic            Anthropic (Claude Pro/Max)")
        );
        assert_eq!(
            provider_list_text(&providers),
            "Available OAuth providers:\n\n  anthropic            Anthropic (Claude Pro/Max)"
        );
    }

    #[test]
    fn selects_provider_with_javascript_parse_int_behavior() {
        let provider = provider_from_selection(BUILT_IN_OAUTH_PROVIDERS, " 2abc").unwrap();
        assert_eq!(provider.id, "github-copilot");
        assert!(matches!(
            provider_from_selection(BUILT_IN_OAUTH_PROVIDERS, "0"),
            Err(CliError::InvalidSelection(selection)) if selection == "0"
        ));
    }

    #[test]
    fn auth_file_roundtrips_and_invalid_json_loads_empty() {
        let path = unique_temp_path();
        fs::write(&path, "not json").unwrap();
        assert!(load_auth_from_path(&path).is_empty());

        let mut auth = BTreeMap::new();
        auth.insert(
            "anthropic".to_string(),
            OAuthAuthEntry::new(OAuthCredentials {
                refresh: "refresh".to_string(),
                access: "access".to_string(),
                expires: 123,
                extra: BTreeMap::new(),
            }),
        );

        save_auth_to_path(&path, &auth).unwrap();
        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.contains("\"type\": \"oauth\""));
        assert_eq!(load_auth_from_path(&path), auth);

        fs::remove_file(path).unwrap();
    }

    fn unique_temp_path() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("zedflow-ai-cli-{nanos}.json"))
    }
}
