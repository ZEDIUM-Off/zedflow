//! Non-interactive project trust decisions.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectTrustDecision {
    Trusted,
    Untrusted,
    Cancelled,
}

#[must_use]
pub fn resolve_project_trust(
    override_value: Option<bool>,
    stored: Option<bool>,
    requires_trust: bool,
) -> ProjectTrustDecision {
    if let Some(value) = override_value {
        return if value {
            ProjectTrustDecision::Trusted
        } else {
            ProjectTrustDecision::Untrusted
        };
    }
    if !requires_trust || stored == Some(true) {
        ProjectTrustDecision::Trusted
    } else {
        ProjectTrustDecision::Untrusted
    }
}
