//! UUID helper for session identifiers.

/// Create a time-ordered UUIDv7 session identifier.
#[must_use]
pub fn uuidv7() -> String {
    uuid::Uuid::now_v7().to_string()
}
