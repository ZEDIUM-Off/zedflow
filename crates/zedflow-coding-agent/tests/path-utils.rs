use std::path::PathBuf;
use zedflow_coding_agent::path_utils::{expand_path, resolve_to_cwd};

#[test]
fn path_utils_expand_file_urls_and_relative_paths() {
    assert_eq!(
        expand_path("file:///root/a%20b").unwrap(),
        PathBuf::from("/root/a b")
    );
    assert_eq!(
        resolve_to_cwd("file", "/root").unwrap(),
        PathBuf::from("/root/file")
    );
}
