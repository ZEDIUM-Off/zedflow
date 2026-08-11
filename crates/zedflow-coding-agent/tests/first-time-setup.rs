use zedflow_coding_agent::first_time_setup::{FirstTimeSetup, TerminalTheme};
#[test]
fn setup_confirms_theme_then_analytics() {
    let mut setup = FirstTimeSetup::new(TerminalTheme::Light);
    setup.move_selection(-1);
    assert_eq!(setup.confirm(), None);
    setup.move_selection(1);
    assert_eq!(setup.confirm().unwrap().share_analytics, false);
}
