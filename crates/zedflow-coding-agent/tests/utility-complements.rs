use zedflow_coding_agent::utils::{changelog, syntax_highlight, version_check};
#[test]
fn utility_complements_cover_core_behavior() {
    assert_eq!(
        changelog::normalize_changelog_links("[x](README.md)", "1.2.3"),
        "[x](https://github.com/earendil-works/pi/blob/v1.2.3/packages/coding-agent/README.md)"
    );
    assert!(version_check::is_newer_package_version(
        "5.0.0-beta.20",
        "5.0.0-beta.9"
    ));
    assert!(syntax_highlight::supports_language("rust"));
}
