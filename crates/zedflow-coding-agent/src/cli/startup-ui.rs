//! Startup UI decisions shared by interactive and headless callers.

#[must_use]
pub fn should_run_first_time_setup(already_configured: bool, interactive: bool) -> bool {
    interactive && !already_configured
}

#[must_use]
pub fn select_or_default<T: Clone>(choices: &[(String, T)], selected: Option<usize>) -> Option<T> {
    selected
        .and_then(|index| choices.get(index).map(|(_, value)| value.clone()))
        .or_else(|| choices.first().map(|(_, value)| value.clone()))
}
