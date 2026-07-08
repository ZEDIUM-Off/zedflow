//! OAuth public entrypoint ported from Pi's `packages/ai/src/oauth.ts`.

/// OAuth provider identifier.
pub type OAuthProviderId = String;

/// Minimal OAuth provider metadata re-exported by this entrypoint until utility rows land.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthProviderInfo {
    /// Provider id.
    pub id: OAuthProviderId,
    /// Display name.
    pub name: String,
}

/// Marker for the Pi OAuth entrypoint row.
pub const OAUTH_ENTRYPOINT: &str = "utils/oauth";
