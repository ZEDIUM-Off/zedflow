use zedflow_coding_agent::utils::{
    git::parse_git_url,
    paths::{get_cwd_relative_path, normalize_path, resolve_path},
    shell::sanitize_binary_output,
};

#[test]
fn utility_substrate_preserves_pi_basics() {
    assert_eq!(normalize_path("~/x").ends_with("/x"), true);
    assert_eq!(
        resolve_path("child", "/tmp/base"),
        std::path::PathBuf::from("/tmp/base/child")
    );
    assert_eq!(
        get_cwd_relative_path("/tmp/base/a", "/tmp/base"),
        Some("a".into())
    );
    assert_eq!(sanitize_binary_output("a\0\u{fff9}\tb"), "a\tb");
    let git = parse_git_url("https://github.com/user/repo@v1").unwrap();
    assert_eq!(
        (git.host, git.path, git.ref_name),
        ("github.com".into(), "user/repo".into(), Some("v1".into()))
    );
    assert!(parse_git_url("github.com/user/repo").is_none());
}
