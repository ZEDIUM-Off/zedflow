//! Configuration-selection contract used by embedders.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsScope {
    Global,
    Project,
}

#[must_use]
pub fn toggle_scope(scope: SettingsScope) -> SettingsScope {
    match scope {
        SettingsScope::Global => SettingsScope::Project,
        SettingsScope::Project => SettingsScope::Global,
    }
}
