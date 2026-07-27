use zedflow_tui::{
    Component, Image, ImageOptions,
    terminal_image::{
        CellDimensions, ImageDimensions, ImageProtocol, TerminalCapabilities,
        reset_capabilities_cache, set_capabilities, set_cell_dimensions,
    },
};

#[test]
fn kitty_image_occupies_its_rendered_rows() {
    set_capabilities(TerminalCapabilities {
        images: Some(ImageProtocol::Kitty),
        true_color: true,
        hyperlinks: true,
    });
    set_cell_dimensions(CellDimensions {
        width: 10,
        height: 10,
    });
    let image = Image::with_options(
        "AAAA",
        "image/png",
        ImageOptions {
            max_width_cells: Some(2),
            ..Default::default()
        },
        ImageDimensions {
            width: 20,
            height: 20,
        },
    );
    let lines = image.render(4);
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("C=1"));
    assert_eq!(lines[1], "");
    reset_capabilities_cache();
    set_cell_dimensions(CellDimensions {
        width: 9,
        height: 18,
    });
}
