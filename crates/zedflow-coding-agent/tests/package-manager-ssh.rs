use zedflow_coding_agent::extensions::ExtensionSource;

#[test]
fn only_pinned_native_sources_are_accepted() {
    assert!(matches!(
        ExtensionSource::parse("github:owner/repo@0123456789012345678901234567890123456789"),
        Ok(ExtensionSource::Github { owner, repo, .. }) if owner == "owner" && repo == "repo"
    ));
    assert!(ExtensionSource::parse("git:git@github.com:owner/repo").is_err());
    assert!(ExtensionSource::parse("github:owner/repo@main").is_err());
}
