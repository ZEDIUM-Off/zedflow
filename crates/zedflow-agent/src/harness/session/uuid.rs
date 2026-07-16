//! UUID helper for session identifiers.

// PORT PLACEHOLDER: Pi's `uuidv7` is time-ordered UUIDv7. A2 followed the approved
// `uuid::Uuid::new_v4()` replacement; implement local UUIDv7 here only if AT1 keeps
// the Pi UUID version/order assertions.
/// Create a new session UUID using the approved `uuid` crate replacement.
#[must_use]
pub fn uuidv7() -> String {
    uuid::Uuid::new_v4().to_string()
}
