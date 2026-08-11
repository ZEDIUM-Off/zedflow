use zedflow_tui::terminal_image::{
    CellDimensions, ImageDimensions, KittyOptions, TerminalCapabilities, calculate_image_cell_size,
    delete_all_kitty_images, delete_kitty_image, encode_kitty, get_gif_dimensions, hyperlink,
    image_fallback, is_image_line, set_capabilities,
};
use zedflow_tui::{Component, Image, ImageOptions};

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

#[test]
fn chunks_large_kitty_payloads_like_pi() {
    let encoded = encode_kitty(&"A".repeat(8193), &KittyOptions::default());
    assert!(encoded.starts_with("\x1b_Ga=T,f=100,q=2,m=1;"));
    assert!(encoded.contains("\x1b_Gm=1;"));
    assert!(encoded.ends_with("\x1b_Gm=0;A\x1b\\"));
}

#[test]
fn reads_dimensions_and_normalizes_fallback_frames() {
    let gif = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        b"GIF89a\x02\x00\x03\x00",
    );
    assert_eq!(
        get_gif_dimensions(&gif),
        Some(ImageDimensions {
            width: 2,
            height: 3
        })
    );
    assert_eq!(
        image_fallback(
            "image/gif",
            Some(ImageDimensions {
                width: 2,
                height: 3
            }),
            Some("dot.gif")
        ),
        "[Image: dot.gif [image/gif] 2x3]"
    );

    set_capabilities(TerminalCapabilities {
        images: None,
        true_color: false,
        hyperlinks: false,
    });
    let image = Image::with_options(
        gif,
        "image/gif",
        ImageOptions {
            filename: Some("dot.gif".into()),
            ..Default::default()
        },
        ImageDimensions {
            width: 2,
            height: 3,
        },
    );
    assert_eq!(image.render(20), ["[Image: dot.gif [image/gif] 2x3]"]);
    assert_eq!(image.image_id(), None);
}
