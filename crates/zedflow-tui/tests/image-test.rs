use zedflow_tui::terminal_image::{delete_all_kitty_images, delete_kitty_image, hyperlink};
#[test]
fn image_control_sequences_are_well_formed() {
    assert_eq!(delete_kitty_image(7), "\x1b_Ga=d,d=7;\x1b\\");
    assert_eq!(delete_all_kitty_images(), "\x1b_Ga=d,d=A;\x1b\\");
    assert!(hyperlink("x", "https://example.test").contains("x"));
}
