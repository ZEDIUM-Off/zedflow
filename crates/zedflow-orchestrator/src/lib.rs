#![forbid(unsafe_code)]

//! Zedflow orchestrator crate.

#[path = "cli.rs"]
pub mod cli;
#[path = "config.rs"]
pub mod config;
#[path = "handler.rs"]
pub mod handler;
#[path = "index.rs"]
pub mod index;
#[path = "ipc/client.rs"]
pub mod ipc_client;
#[path = "ipc/protocol.rs"]
pub mod ipc_protocol;
#[path = "ipc/server.rs"]
pub mod ipc_server;
#[path = "radius.rs"]
pub mod radius;
#[path = "rpc-process.rs"]
pub mod rpc_process;
#[path = "serve.rs"]
pub mod serve;
#[path = "storage.rs"]
pub mod storage;
#[path = "supervisor.rs"]
pub mod supervisor;
#[path = "types.rs"]
pub mod types;

/// Crate identity.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
