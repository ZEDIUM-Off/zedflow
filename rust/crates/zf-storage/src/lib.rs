//! Persistence adapters. No runtime, composition compiler or HTTP server lives here.

pub mod bridge_store;
pub mod content_store;
pub mod context_store;
pub mod contracts;
pub mod data;
pub mod data_archive;
pub mod flow_packages;
pub mod flow_store;
pub mod legacy_compositions;
pub mod live_files;
pub mod migration;
pub mod projection_summary;
pub mod revision_publications;
pub mod session_archive;
pub mod session_store;
pub mod session_sync;
pub mod source_acceptance;
pub mod source_catalog;
pub mod timeline;
pub mod workspaces;
