use zedflow_coding_agent::utils::git::parse_git_url;
#[test]
fn ssh_git_urls_are_normalized() {
    let source = parse_git_url("ssh://git@github.com/owner/repo.git@main").unwrap();
    assert_eq!(source.host, "github.com");
    assert_eq!(source.repo, "ssh://git@github.com/owner/repo.git");
    assert_eq!(source.ref_name.as_deref(), Some("main"));
}
