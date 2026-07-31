#[test]
fn explicit_provider_versions_remain_comparable() {
    assert!(zedflow_coding_agent::utils::version_check::is_newer_package_version("1.2.0", "1.1.9"));
}
