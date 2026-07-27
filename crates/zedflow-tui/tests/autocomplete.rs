use zedflow_tui::utils::{slice_by_column, visible_width};
use zedflow_tui::{AutocompleteProvider, CombinedAutocompleteProvider, SlashCommand};

#[test]
fn autocomplete_candidates_preserve_terminal_columns() {
    assert_eq!(visible_width("hello"), 5);
    assert_eq!(slice_by_column("hello", 1, 3, false), "ell");

    let provider = CombinedAutocompleteProvider::new(
        vec![SlashCommand {
            name: "model".into(),
            description: Some("Select model".into()),
            argument_hint: None,
        }],
        ".",
    );
    let lines = vec!["/mo".into()];
    let suggestions = provider.get_suggestions(&lines, 0, 3, false).unwrap();
    assert_eq!(suggestions.items[0].value, "model");
    let completion =
        provider.apply_completion(&lines, 0, 3, &suggestions.items[0], &suggestions.prefix);
    assert_eq!(completion.lines, ["/model "]);
    assert_eq!(completion.cursor_col, 7);
}
