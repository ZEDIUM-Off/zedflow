use std::path::Path;
use zedflow_coding_agent::utils::{
    git::parse_git_url,
    paths::{
        PathInputOptions, format_path_relative_to_cwd_or_absolute, get_cwd_relative_path,
        normalize_path,
    },
    shell::sanitize_binary_output,
};

#[test]
fn paths_and_git_cover_common_inputs() {
    let options = PathInputOptions {
        trim: true,
        home_dir: Some("/home/test".into()),
        ..Default::default()
    };
    assert_eq!(
        normalize_path(" ~/src ", &options),
        Path::new("/home/test/src")
    );
    assert_eq!(
        get_cwd_relative_path(Path::new("/tmp/project/src"), Path::new("/tmp/project")),
        Some(Path::new("src").to_path_buf())
    );
    assert_eq!(
        format_path_relative_to_cwd_or_absolute(
            Path::new("/tmp/project/src"),
            Path::new("/tmp/project")
        ),
        "src"
    );
    let git = parse_git_url("https://github.com/user/repo.git@main").unwrap();
    assert_eq!(
        (git.host, git.path, git.ref_name, git.pinned),
        (
            "github.com".into(),
            "user/repo".into(),
            Some("main".into()),
            true
        )
    );
    assert!(parse_git_url("user/repo").is_none());
    assert!(parse_git_url("https://github.com/user/%ZZ").is_none());
}

#[test]
fn shell_output_removes_unsafe_controls() {
    assert_eq!(sanitize_binary_output("ok\0\x1b[31m\n"), "ok[31m\n");
}
