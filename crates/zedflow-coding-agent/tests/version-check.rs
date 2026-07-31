#[test]
fn semantic_versions_sort_numerically() {
    assert_eq!(
        zedflow_coding_agent::utils::version_check::compare_package_versions("v1.10.0", "1.9.0"),
        Some(1)
    );
}
