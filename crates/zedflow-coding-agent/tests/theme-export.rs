#[test]
fn rendered_highlight_text_has_no_html_markup() {
    assert_eq!(
        zedflow_coding_agent::utils::syntax_highlight::render_highlighted_html(
            "<b>export</b>",
            &Default::default()
        ),
        "export"
    );
}
