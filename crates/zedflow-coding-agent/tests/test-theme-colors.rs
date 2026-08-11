#[test]
fn html_highlight_rendering_strips_tags_but_keeps_text() {
    assert_eq!(
        zedflow_coding_agent::utils::syntax_highlight::render_highlighted_html(
            "<span>blue</span>",
            &Default::default()
        ),
        "blue"
    );
}
