use zedflow_coding_agent::export_html::export_session_to_html;

#[test]
fn html_export_escapes_session_text() {
    let html = export_session_to_html("<script>alert('xss')</script>");
    assert!(html.contains("&lt;script&gt;"));
    assert!(!html.contains("<script>"));
}
