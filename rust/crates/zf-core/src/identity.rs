//! Opaque identities: their type distinguishes ownership, their bytes stay intact.
//!
//! Parsing a stored identity does not establish existence or authorization. Those
//! checks belong to the owning domain. In particular, historical non-UUID IDs
//! remain readable without normalization.
//!
//! ```compile_fail
//! use zf_core::identity::{RunId, SessionId};
//! let session: SessionId = RunId::from("same-text");
//! ```
use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! identity {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
            pub fn into_string(self) -> String {
                self.0
            }
        }
        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }
        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

identity!(
    RunId,
    "Identity of an execution, interactive or autonomous."
);
identity!(SessionId, "Identity of an interactive session.");
identity!(VisitId, "Identity of one routed visit.");
identity!(OccurrenceId, "Identity of one passage through a node.");
identity!(InvocationId, "Identity of a captured model invocation.");
identity!(
    PackageId,
    "Domain identity of a package, independent of its Cargo name."
);
identity!(
    Revision,
    "Identity of one publication, not its content hash."
);
identity!(
    EntityId,
    "Identity of an entity across its immutable revisions."
);
identity!(NodePath, "Qualified path of a node within a runtime graph.");
identity!(
    ContentRef,
    "Immutable content reference; resolution verifies its hash."
);

/// An explicit namespace, not an authentication credential or fallback scope.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum Scope {
    Flow(String),
    Bridge(String),
    Runtime,
}

/// Rights conveyed by a granted alias. Enforcement belongs to the data store.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Permission {
    Read,
    Write,
}
