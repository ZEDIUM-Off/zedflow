//! Provider-selector state independent of the terminal renderer.

use crate::auth_storage::{AuthCredential, AuthSource, AuthStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthSelectorMode {
    Login,
    Logout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthSelectorProviderType {
    OAuth,
    ApiKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSelectorProvider {
    pub id: String,
    pub name: String,
    pub auth_type: AuthSelectorProviderType,
}

#[derive(Debug, Clone)]
pub struct OAuthSelector {
    pub mode: AuthSelectorMode,
    providers: Vec<AuthSelectorProvider>,
    filtered: Vec<usize>,
    selected: usize,
}

impl OAuthSelector {
    #[must_use]
    pub fn new(providers: Vec<AuthSelectorProvider>) -> Self {
        Self::with_mode(AuthSelectorMode::Login, providers)
    }

    #[must_use]
    pub fn with_mode(mode: AuthSelectorMode, providers: Vec<AuthSelectorProvider>) -> Self {
        let filtered = (0..providers.len()).collect();
        Self {
            mode,
            providers,
            filtered,
            selected: 0,
        }
    }

    /// Filter like Pi's selector: query characters must occur in order in provider metadata.
    pub fn filter(&mut self, query: &str) {
        let query = query.to_ascii_lowercase();
        self.filtered = self
            .providers
            .iter()
            .enumerate()
            .filter_map(|(index, provider)| {
                let auth_type = match provider.auth_type {
                    AuthSelectorProviderType::OAuth => "oauth",
                    AuthSelectorProviderType::ApiKey => "api_key",
                };
                let haystack =
                    format!("{} {} {auth_type}", provider.name, provider.id).to_ascii_lowercase();
                fuzzy_matches(&haystack, &query).then_some(index)
            })
            .collect();
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
    }

    pub fn move_selection(&mut self, delta: isize) {
        self.selected = self
            .selected
            .saturating_add_signed(delta)
            .min(self.filtered.len().saturating_sub(1));
    }

    #[must_use]
    pub fn selected_provider(&self) -> Option<&AuthSelectorProvider> {
        self.filtered
            .get(self.selected)
            .map(|&index| &self.providers[index])
    }

    #[must_use]
    pub fn filtered_count(&self) -> usize {
        self.filtered.len()
    }

    #[must_use]
    pub fn visible_providers(
        &self,
        max_visible: usize,
    ) -> impl Iterator<Item = &AuthSelectorProvider> {
        let start = self
            .selected
            .saturating_sub(max_visible / 2)
            .min(self.filtered.len().saturating_sub(max_visible));
        self.filtered[start..(start + max_visible).min(self.filtered.len())]
            .iter()
            .map(|&index| &self.providers[index])
    }

    #[must_use]
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    #[must_use]
    pub fn needs_scroll_info(&self, max_visible: usize) -> bool {
        self.filtered.len() > max_visible
    }

    #[must_use]
    pub fn empty_message(&self) -> &'static str {
        if !self.providers.is_empty() {
            return "No matching providers";
        }
        match self.mode {
            AuthSelectorMode::Login => "No providers available",
            AuthSelectorMode::Logout => "No providers logged in. Use /login first.",
        }
    }
}

#[must_use]
pub fn status_indicator(
    provider_type: AuthSelectorProviderType,
    credential: Option<&AuthCredential>,
    status: Option<&AuthStatus>,
) -> String {
    if credential.is_some_and(|credential| {
        matches!(
            (provider_type, credential),
            (
                AuthSelectorProviderType::OAuth,
                AuthCredential::OAuth { .. }
            ) | (
                AuthSelectorProviderType::ApiKey,
                AuthCredential::ApiKey { .. }
            )
        )
    }) {
        return "✓ configured".into();
    }
    if let Some(credential) = credential {
        return match credential {
            AuthCredential::OAuth { .. } => "subscription configured".into(),
            AuthCredential::ApiKey { .. } => "API key configured".into(),
        };
    }
    if provider_type == AuthSelectorProviderType::OAuth {
        return "unconfigured".into();
    }
    match status.and_then(|status| status.source.as_ref()) {
        Some(AuthSource::Environment) => format!(
            "✓ env: {}",
            status
                .and_then(|status| status.label.as_deref())
                .unwrap_or("API key")
        ),
        Some(AuthSource::Runtime) => "✓ runtime API key".into(),
        Some(AuthSource::Fallback) => "✓ custom API key".into(),
        Some(AuthSource::ModelsJsonKey) => "✓ key in models.json".into(),
        Some(AuthSource::ModelsJsonCommand) => "✓ command in models.json".into(),
        _ => "unconfigured".into(),
    }
}

fn fuzzy_matches(haystack: &str, needle: &str) -> bool {
    let mut needle = needle.chars();
    let mut next = needle.next();
    for character in haystack.chars() {
        if Some(character) == next {
            next = needle.next();
        }
    }
    next.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_filters_and_clamps_selection() {
        let mut selector = OAuthSelector::new(vec![
            AuthSelectorProvider {
                id: "openai".into(),
                name: "OpenAI".into(),
                auth_type: AuthSelectorProviderType::ApiKey,
            },
            AuthSelectorProvider {
                id: "github".into(),
                name: "GitHub".into(),
                auth_type: AuthSelectorProviderType::OAuth,
            },
        ]);
        selector.move_selection(1);
        selector.filter("gh");
        assert_eq!(selector.filtered_count(), 1);
        assert_eq!(selector.selected_provider().unwrap().id, "github");
    }
}
