use zedflow_coding_agent::utils::frontmatter::{parse_frontmatter, strip_frontmatter};
#[test]
fn frontmatter_is_parsed_and_stripped() {
    let text = "---\ntitle: hello\n---\nbody";
    assert_eq!(
        parse_frontmatter(text).unwrap().frontmatter["title"].as_str(),
        Some("hello")
    );
    assert_eq!(strip_frontmatter(text).unwrap(), "body");
}
