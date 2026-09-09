//! ADK experiments; see the flows directory. No Zedflow runtime contracts yet.

#[path = "../flows/mod.rs"]
pub mod flows;

#[cfg(feature = "web")]
pub mod web;
