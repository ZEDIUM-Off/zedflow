//! Provider-selector state independent of the terminal renderer.

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
    providers: Vec<AuthSelectorProvider>,
    filtered: Vec<usize>,
    selected: usize,
}

impl OAuthSelector {
    #[must_use]
    pub fn new(providers: Vec<AuthSelectorProvider>) -> Self {
        let filtered = (0..providers.len()).collect();
        Self {
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
                let haystack =
                    format!("{} {} {:?}", provider.name, provider.id, provider.auth_type)
                        .to_ascii_lowercase();
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
