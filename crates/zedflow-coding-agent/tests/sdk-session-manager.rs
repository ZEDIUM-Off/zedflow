use zedflow_coding_agent::sdk::ExtensionApi;
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
