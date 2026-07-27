use zedflow_tui::terminal_image::{ImageProtocol, hyperlink};
#[test]
fn terminal_image_protocols_and_links_are_available() {
    assert_eq!(ImageProtocol::Kitty, ImageProtocol::Kitty);
    assert!(hyperlink("label", "url").starts_with("\x1b]8;;"));
}
