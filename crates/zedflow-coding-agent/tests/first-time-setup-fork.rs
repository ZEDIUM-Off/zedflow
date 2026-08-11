use zedflow_coding_agent::first_time_setup::{FirstTimeSetup, TerminalTheme};
#[test]
fn independent_setups_keep_independent_selection() {
    let left = FirstTimeSetup::new(TerminalTheme::Dark);
    let mut right = left;
    right.move_selection(1);
    assert_eq!(left.theme(), TerminalTheme::Dark);
    assert_eq!(right.theme(), TerminalTheme::Light);
}
