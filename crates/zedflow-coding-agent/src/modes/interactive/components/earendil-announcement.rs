//! Pi's Earendil announcement transcript component.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use zedflow_tui::{Component, Container, Image, ImageTheme, Spacer, Text};

use crate::{
    dynamic_border::DynamicBorder,
    modes_interactive_theme_theme::{CLANKOLAS_PNG, Theme},
};

pub const BLOG_URL: &str = "https://mariozechner.at/posts/2026-04-08-ive-sold-out/";
const IMAGE_FILENAME: &str = "clankolas.png";

pub struct EarendilAnnouncementComponent {
    content: Container,
}

impl EarendilAnnouncementComponent {
    #[must_use]
    pub fn new(theme: &Theme) -> Self {
        let accent = theme.fg_ansi("accent").unwrap_or("").to_owned();
        let muted = theme.fg_ansi("muted").unwrap_or("").to_owned();
        let link = theme.fg_ansi("mdLink").unwrap_or("").to_owned();
        let paint = |ansi: &str, text: &str| {
            if ansi.is_empty() {
                text.to_owned()
            } else {
                format!("{ansi}{text}\x1b[39m")
            }
        };

        let mut content = Container::new();
        let border_accent = accent.clone();
        content.add_child(DynamicBorder::new(move |text| paint(&border_accent, text)));
        content.add_child(Text::new(
            format!(
                "\x1b[1m{}\x1b[22m",
                paint(&accent, "pi has joined Earendil")
            ),
            1,
            0,
        ));
        content.add_child(Spacer::new(1));
        content.add_child(Text::new(paint(&muted, "Read the blog post:"), 1, 0));
        content.add_child(Text::new(paint(&link, BLOG_URL), 1, 0));
        content.add_child(Spacer::new(1));

        let mut image = Image::new(STANDARD.encode(CLANKOLAS_PNG), "image/png");
        image.options.max_width_cells = Some(56);
        image.options.filename = Some(IMAGE_FILENAME.into());
        let fallback_muted = muted.clone();
        image.set_theme(ImageTheme {
            fallback_color: std::sync::Arc::new(move |text| paint(&fallback_muted, text)),
        });
        content.add_child(image);
        content.add_child(Spacer::new(1));

        content.add_child(DynamicBorder::new(move |text| paint(&accent, text)));
        Self { content }
    }
}

impl Component for EarendilAnnouncementComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.content.render(width)
    }

    fn invalidate(&mut self) {
        self.content.invalidate();
    }
}
