use zedflow_tui::terminal_image::{
    CellDimensions, ImageDimensions, KittyOptions, calculate_image_cell_size,
    delete_all_kitty_images, delete_kitty_image, encode_kitty, hyperlink, is_image_line,
};

#[test]
fn detects_image_sequences_anywhere_in_a_line() {
    assert!(is_image_line("text \x1b]1337;File=inline=1:data\x07"));
    assert!(is_image_line("prefix \x1b_Ga=T;data\x1b\\ suffix"));
    assert!(!is_image_line("plain ]1337;File text"));
}

#[test]
fn kitty_sequences_match_the_frozen_protocol() {
    assert_eq!(delete_kitty_image(7), "\x1b_Ga=d,d=I,i=7,q=2\x1b\\");
    assert_eq!(delete_all_kitty_images(), "\x1b_Ga=d,d=A,q=2\x1b\\");
    let encoded = encode_kitty(
        "AAAA",
        &KittyOptions {
            columns: Some(2),
            rows: Some(2),
            move_cursor: Some(false),
            ..Default::default()
        },
    );
    assert_eq!(encoded, "\x1b_Ga=T,f=100,q=2,C=1,c=2,r=2;AAAA\x1b\\");
}

#[test]
fn image_size_respects_both_bounds() {
    let size = calculate_image_cell_size(
        ImageDimensions {
            width: 10,
            height: 100,
        },
        10,
        Some(5),
        CellDimensions {
            width: 10,
            height: 10,
        },
    );
    assert_eq!((size.columns, size.rows), (1, 5));
}

#[test]
fn osc8_links_use_string_terminators() {
    assert_eq!(
        hyperlink("label", "url"),
        "\x1b]8;;url\x1b\\label\x1b]8;;\x1b\\"
    );
}
