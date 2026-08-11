use zedflow_coding_agent::sdk::ExtensionApi;

mod session_manager {
    pub use zedflow_coding_agent::session_manager::*;
}

#[path = "session-manager/build-context.rs"]
mod build_context;
#[path = "session-manager/custom-session-id.rs"]
mod custom_session_id;
#[path = "session-manager/file-operations.rs"]
mod file_operations;
#[path = "session-manager/labels.rs"]
mod labels;
#[path = "session-manager/migration.rs"]
mod migration;
#[path = "session-manager/save-entry.rs"]
mod save_entry;
#[path = "session-manager/tree-traversal.rs"]
mod tree_traversal;

#[test]
fn extension_api_records_host_registrations() {
    let mut api = ExtensionApi::default();
    api.register_tool("read");
    api.register_command("go");
    api.on_event("message");
    let snapshot = api.snapshot();
    assert_eq!(snapshot["tools"][0], "read");
    assert_eq!(snapshot["commands"][0], "go");
    assert_eq!(snapshot["events"][0], "message");
}
