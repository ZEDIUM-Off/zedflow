#[test]
fn markdown_dependency_preserves_list_structure() {
    let html = markdown::to_html("- beep\n- boop");
    assert!(html.contains("<li>beep</li>"));
    assert!(html.contains("<li>boop</li>"));
}
