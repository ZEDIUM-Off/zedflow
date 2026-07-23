//! Session picker data contracts. Interactive rendering belongs to the TUI crate.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionChoice {
    pub id: String,
    pub path: String,
    pub cwd: String,
}

#[must_use]
pub fn choose_session<'a>(
    sessions: &'a [SessionChoice],
    prefix: &str,
) -> Option<&'a SessionChoice> {
    sessions
        .iter()
        .find(|session| session.id == prefix)
        .or_else(|| {
            sessions
                .iter()
                .find(|session| session.id.starts_with(prefix))
        })
}
