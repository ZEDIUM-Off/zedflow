use zedflow_coding_agent::utils::git::parse_git_url;
#[test]
fn git_sources_are_parsed_for_extension_installation() {
    let source = parse_git_url("https://github.com/owner/repo.git@main").unwrap();
    assert_eq!(source.host, "github.com");
    assert_eq!(source.repo, "https://github.com/owner/repo.git");
    assert_eq!(source.ref_name.as_deref(), Some("main"));
}
