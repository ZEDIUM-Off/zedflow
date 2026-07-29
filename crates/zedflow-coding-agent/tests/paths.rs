use std::path::Path;
use zedflow_coding_agent::utils::paths::{get_cwd_relative_path, is_local_path};
#[test]
fn paths_detect_local_values_and_relative_files() {
    assert!(is_local_path("./file"));
    assert_eq!(
        get_cwd_relative_path(Path::new("/work/a"), Path::new("/work")),
        Some("a".into())
    );
}
