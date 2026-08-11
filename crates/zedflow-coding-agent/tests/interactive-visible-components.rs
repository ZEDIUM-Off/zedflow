use zedflow_coding_agent::{
    armin::{ArminComponent, Effect},
    daxnuts::DaxnutsComponent,
    earendil_announcement::{BLOG_URL, EarendilAnnouncementComponent},
    modes_interactive_theme_theme::{ColorMode, Theme},
};
use zedflow_tui::{Component, visible_width};

fn theme() -> Theme {
    Theme::builtin("dark", ColorMode::Truecolor).expect("embedded theme")
}

#[test]
fn armin_reveals_the_bitmap_with_the_active_accent() {
    let theme = theme();
    let mut armin = ArminComponent::with_effect(Effect::Scanline, 0);
    for _ in 0..19 {
        armin.tick();
    }
    let plain = armin.render(32);
    assert!(plain.iter().any(|line| line.contains('█')));
    assert_eq!(plain.last().unwrap(), " ARMIN SAYS HI                  ");

    let themed = ArminComponent::with_theme(&theme).render(32);
    assert!(
        themed
            .last()
            .unwrap()
            .contains(theme.fg_ansi("accent").unwrap())
    );
}

#[test]
fn daxnuts_scanline_reveals_image_then_copy() {
    let mut dax = DaxnutsComponent::new(&theme());
    let initial = dax.render(40);
    assert!(initial[1].contains("\x1b[38;2;100;200;255m"));
    assert_eq!(visible_width(&initial[1]), 36);
    assert!(!initial.join("\n").contains("Powered by daxnuts"));

    for _ in 0..16 {
        dax.tick();
    }
    assert!(dax.render(80).join("\n").contains("Powered by daxnuts"));
    dax.tick();
    dax.tick();
    assert!(
        dax.render(80)
            .join("\n")
            .contains("https://mistral.ai/news/mistral-vibe-2-0")
    );
    while dax.is_running() {
        dax.tick();
    }
    assert_eq!(dax.tick_count(), 25);
    assert_eq!(visible_width(&dax.render(32)[1]), 32);
}

#[test]
fn earendil_announcement_renders_copy_link_borders_and_asset() {
    let output = EarendilAnnouncementComponent::new(&theme()).render(64);
    let transcript = output.join("\n");
    assert!(transcript.contains("pi has joined Earendil"));
    assert!(transcript.contains("Read the blog post:"));
    assert!(transcript.contains(BLOG_URL));
    assert_eq!(visible_width(&output[0]), 64);
    assert_eq!(visible_width(output.last().unwrap()), 64);
    assert!(
        output.len() > 7,
        "the bundled image contributes transcript rows"
    );
}
