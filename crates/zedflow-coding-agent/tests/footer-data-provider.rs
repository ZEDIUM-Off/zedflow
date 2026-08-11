use zedflow_coding_agent::footer_data_provider::FooterDataProvider;
#[test]
fn footer_provider_tracks_extension_statuses_and_provider_count() {
    let mut provider = FooterDataProvider::new(".");
    provider.set_extension_status("a", Some("ready".into()));
    provider.set_available_provider_count(2);
    assert_eq!(
        provider.get_extension_statuses().get("a"),
        Some(&"ready".into())
    );
    assert_eq!(provider.get_available_provider_count(), 2);
    provider.clear_extension_statuses();
    assert!(provider.get_extension_statuses().is_empty());
}
