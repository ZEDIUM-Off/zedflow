//! Project-trust picker state, backed by the core trust-manager contract.

use std::{io, path::Path};

use crate::trust_manager::{
    ProjectTrustOption, ProjectTrustStoreEntry, ProjectTrustUpdate, get_project_trust_options,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustSelection {
    pub trusted: bool,
    pub updates: Vec<ProjectTrustUpdate>,
}

#[derive(Debug, Clone)]
pub struct TrustSelectorState {
    options: Vec<ProjectTrustOption>,
    selected: usize,
    saved_decision: Option<ProjectTrustStoreEntry>,
}

impl TrustSelectorState {
    pub fn new(
        cwd: impl AsRef<Path>,
        saved_decision: Option<ProjectTrustStoreEntry>,
    ) -> io::Result<Self> {
        let options = get_project_trust_options(cwd, false)?;
        let selected = options
            .iter()
            .position(|option| {
                option.saved_path.as_ref().is_some_and(|path| {
                    saved_decision.as_ref().is_some_and(|saved| {
                        saved.path == *path && saved.decision == option.trusted
                    })
                })
            })
            .unwrap_or(0);
        Ok(Self {
            options,
            selected,
            saved_decision,
        })
    }

    #[must_use]
    pub fn options(&self) -> &[ProjectTrustOption] {
        &self.options
    }

    #[must_use]
    pub const fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        self.selected = (self.selected + 1).min(self.options.len().saturating_sub(1));
    }

    #[must_use]
    pub fn select(&self) -> Option<TrustSelection> {
        self.options
            .get(self.selected)
            .map(|option| TrustSelection {
                trusted: option.trusted,
                updates: option.updates.clone(),
            })
    }

    #[must_use]
    pub fn is_saved_option(&self, index: usize) -> bool {
        self.options.get(index).is_some_and(|option| {
            option.saved_path.as_ref().is_some_and(|path| {
                self.saved_decision
                    .as_ref()
                    .is_some_and(|saved| saved.path == *path && saved.decision == option.trusted)
            })
        })
    }
}

#[must_use]
pub fn format_saved_decision(
    trust_path: Option<&Path>,
    decision: Option<&ProjectTrustStoreEntry>,
) -> String {
    let Some(decision) = decision else {
        return "none".into();
    };
    let label = if decision.decision {
        "trusted"
    } else {
        "untrusted"
    };
    if trust_path.is_some_and(|path| path != decision.path) {
        format!("{label} (inherited from {})", decision.path.display())
    } else {
        format!("{label} ({})", decision.path.display())
    }
}
