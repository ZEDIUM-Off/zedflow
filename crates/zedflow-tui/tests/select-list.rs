use zedflow_tui::fuzzy_filter;

#[test]
fn select_list_filtering_preserves_matching_order() {
    let items = ["Alpha", "Beta", "Alpine"];
    assert_eq!(
        fuzzy_filter(&items, "al", |item| item),
        vec!["Alpha", "Alpine"]
    );
}
