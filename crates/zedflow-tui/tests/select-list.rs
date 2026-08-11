use std::{collections::HashMap, sync::Arc};

use zedflow_tui::{
    Component, KeybindingsManager, SelectItem, SelectList, SelectListLayoutOptions,
    SelectListStyle, SelectListTheme, SettingItem, SettingsList, set_keybindings, tui_keybindings,
    visible_width,
};

fn style(prefix: &'static str) -> SelectListStyle {
    Arc::new(move |text| format!("{prefix}{text}"))
}

fn theme() -> SelectListTheme {
    SelectListTheme {
        selected_prefix: style(""),
        selected_text: style(""),
        description: style(""),
        scroll_info: style(""),
        no_match: style(""),
    }
}

fn item(label: &str, description: &str) -> SelectItem {
    SelectItem {
        value: label.into(),
        label: label.into(),
        description: Some(description.into()),
    }
}

fn visible_index_of(line: &str, text: &str) -> usize {
    visible_width(&line[..line.find(text).unwrap()])
}

#[test]
fn select_list_applies_layout_and_normalizes_descriptions() {
    let list = SelectList::with_layout(
        vec![
            item(
                "very-long-command-name-that-needs-truncation",
                "Line one\nLine two",
            ),
            item("short", "second"),
        ],
        5,
        theme(),
        SelectListLayoutOptions {
            min_primary_column_width: Some(12),
            max_primary_column_width: Some(20),
            truncate_primary: Some(Arc::new(|context| {
                if visible_width(context.text) <= context.max_width {
                    context.text.into()
                } else {
                    format!("{}…", &context.text[..context.max_width - 1])
                }
            })),
        },
    );

    let rendered = list.render(80);
    assert!(rendered[0].contains('…'));
    assert!(rendered[0].contains("Line one Line two"));
    assert_eq!(
        visible_index_of(&rendered[0], "Line one"),
        visible_index_of(&rendered[1], "second")
    );
    assert_eq!(visible_index_of(&rendered[1], "second"), 22);
}

#[test]
fn settings_list_filters_navigates_and_cycles_values() {
    let mut list = SettingsList::new(
        vec![
            SettingItem {
                id: "theme".into(),
                label: "Theme".into(),
                description: Some("Color theme".into()),
                current_value: "dark".into(),
                values: vec!["dark".into(), "light".into()],
            },
            SettingItem {
                id: "model".into(),
                label: "Model".into(),
                description: None,
                current_value: "fast".into(),
                values: vec!["fast".into(), "smart".into()],
            },
        ],
        5,
    );
    list.handle_input("\r");
    assert!(list.render(80).iter().any(|line| line.contains("light")));
    list.set_filter("mdl");
    let rendered = list.render(80).join("\n");
    assert!(rendered.contains("Model"));
    assert!(!rendered.contains("Theme"));
}

#[test]
fn select_list_uses_theme_and_global_keybindings() {
    let themed = SelectListTheme {
        selected_prefix: style("prefix:"),
        selected_text: style("selected:"),
        description: style("description:"),
        scroll_info: style("scroll:"),
        no_match: style("empty:"),
    };
    let mut list = SelectList::new(
        vec![item("one", "first"), item("two", "second")],
        1,
        themed.clone(),
    );
    assert!(list.render(80)[0].starts_with("selected:"));
    assert!(list.render(80)[1].starts_with("scroll:"));

    let mut user = HashMap::new();
    user.insert("tui.select.down".into(), vec!["ctrl+x".into()]);
    set_keybindings(KeybindingsManager::new(tui_keybindings(), user));
    list.handle_input("\x18");
    set_keybindings(KeybindingsManager::new(tui_keybindings(), HashMap::new()));
    assert_eq!(list.selected().unwrap().value, "two");
    let full_list = SelectList::new(vec![item("one", "first"), item("two", "second")], 2, themed);
    assert!(full_list.render(80)[1].contains("description:"));

    list.set_filter("missing");
    assert_eq!(list.render(80), ["empty:  No matching commands"]);
}
