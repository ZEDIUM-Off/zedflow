use std::{cell::RefCell, path::Path, rc::Rc};

use zedflow_coding_agent::modes_interactive_components_index::{
    bordered_loader::BorderedLoader, countdown_timer::CountdownTimer,
};
use zedflow_coding_agent::{
    dynamic_border::DynamicBorder,
    footer::{FooterSnapshot, format_cwd_for_footer},
    keybinding_hints::format_key_text_for_platform,
    modes_interactive_theme_theme::{
        CLANKOLAS_PNG, ColorMode, TerminalTheme, Theme, parse_auto_theme_setting,
    },
    theme_controller::InteractiveThemeController,
};
use zedflow_tui::Component;

#[test]
fn built_in_chrome_is_renderable_without_source_tree_assets() {
    let dark = Theme::builtin("dark", ColorMode::Truecolor).unwrap();
    assert_eq!(
        dark.fg("accent", "pi").unwrap(),
        "\x1b[38;2;138;190;183mpi\x1b[39m"
    );
    assert_eq!(&CLANKOLAS_PNG[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(
        parse_auto_theme_setting(Some(" light / dark ")),
        Some(("light".into(), "dark".into()))
    );

    let mut controller = InteractiveThemeController::new(
        Some("light/dark".into()),
        TerminalTheme::Dark,
        ColorMode::Color256,
    );
    assert!(controller.apply_from_settings().success);
    assert_eq!(controller.active_theme_name(), Some("dark"));
    assert!(
        controller
            .apply_terminal_theme(TerminalTheme::Light)
            .success
    );
    assert_eq!(controller.active_theme_name(), Some("light"));

    assert_eq!(DynamicBorder::default().render(0), ["─"]);
    assert_eq!(
        format_key_text_for_platform("ctrl+alt/x", true, true),
        "Ctrl+Option/X"
    );
    assert_eq!(
        format_cwd_for_footer(Path::new("/tmp/../var"), Some(Path::new("/home/me"))),
        "/tmp/../var"
    );

    let footer = FooterSnapshot {
        cwd: "~/src".into(),
        stats: vec!["↑1.0k".into()],
        model: "model".into(),
        ..Default::default()
    };
    let lines = footer.render(20);
    assert_eq!(lines.len(), 2);
    assert!(lines[1].ends_with("model"));

    let loader = BorderedLoader::new(&dark, "Loading", true);
    let rendered = loader.render(12);
    assert!(rendered.first().unwrap().contains("────────────"));
    assert!(rendered.iter().any(|line| line.contains("Loading")));
}

#[test]
fn countdown_ticks_immediately_and_expires_once() {
    let ticks = Rc::new(RefCell::new(Vec::new()));
    let expired = Rc::new(RefCell::new(0));
    let tick_sink = Rc::clone(&ticks);
    let expire_sink = Rc::clone(&expired);
    let mut timer = CountdownTimer::new(
        1_001,
        move |seconds| tick_sink.borrow_mut().push(seconds),
        move || *expire_sink.borrow_mut() += 1,
    );
    timer.tick();
    timer.tick();
    timer.tick();
    assert_eq!(*ticks.borrow(), [2, 1, 0]);
    assert_eq!(*expired.borrow(), 1);
    assert!(!timer.is_active());
}
