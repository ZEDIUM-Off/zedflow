#[test]
fn highlighter_preserves_code_and_knows_rust() {
    let theme = Default::default();
    assert_eq!(
        zedflow_coding_agent::utils::syntax_highlight::highlight(
            "let x = 1;",
            Some("rust"),
            &theme
        ),
        "let x = 1;"
    );
    assert!(zedflow_coding_agent::utils::syntax_highlight::supports_language("rust"));
}
