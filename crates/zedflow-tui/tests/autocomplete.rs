use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};
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

#[test]
fn autocomplete_ports_quoted_dot_slash_and_recursive_at_paths() {
    let root = std::env::temp_dir().join(format!(
        "zedflow-autocomplete-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(root.join("my folder/nested")).unwrap();
    fs::write(root.join("my folder/nested/file.txt"), "").unwrap();
    fs::write(root.join("update.sh"), "").unwrap();
    let provider = CombinedAutocompleteProvider::new(vec![], &root);

    let direct = provider
        .get_suggestions(&["./up".into()], 0, 4, true)
        .unwrap();
    assert!(direct.items.iter().any(|i| i.value == "./update.sh"));

    let quoted = "\"my folder/\"";
    let suggestions = provider
        .get_suggestions(&[quoted.into()], 0, quoted.len() - 1, true)
        .unwrap();
    let directory = suggestions
        .items
        .iter()
        .find(|i| i.label == "nested/")
        .unwrap();
    assert_eq!(directory.value, "\"my folder/nested/\"");
    let applied = provider.apply_completion(
        &[quoted.into()],
        0,
        quoted.len() - 1,
        directory,
        &suggestions.prefix,
    );
    assert_eq!(applied.lines[0], "\"my folder/nested/\"");
    assert_eq!(applied.cursor_col, "\"my folder/nested/".len());

    let fuzzy = provider
        .get_suggestions(&["@file".into()], 0, 5, false)
        .unwrap();
    assert!(
        fuzzy
            .items
            .iter()
            .any(|i| i.value == "@\"my folder/nested/file.txt\"")
    );
    fs::remove_dir_all(root).unwrap();
}
