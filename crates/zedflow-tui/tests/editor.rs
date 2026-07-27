use zedflow_tui::utils::slice_with_width;
#[test]
fn editor_slicing_reports_consumed_columns() {
    assert_eq!(slice_with_width("abcdef", 2, 3, false), ("cde".into(), 3));
}
