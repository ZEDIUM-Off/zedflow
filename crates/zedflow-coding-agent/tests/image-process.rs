use std::io::Cursor;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use zedflow_coding_agent::utils::image_process::{
    ImageResizeOptions, ProcessImageOptions, process_image,
};

fn tiny_bmp_1x1_red() -> Vec<u8> {
    let mut bytes = vec![0; 58];
    bytes[0..2].copy_from_slice(b"BM");
    bytes[2..6].copy_from_slice(&58_u32.to_le_bytes());
    bytes[10..14].copy_from_slice(&54_u32.to_le_bytes());
    bytes[14..18].copy_from_slice(&40_u32.to_le_bytes());
    bytes[18..22].copy_from_slice(&1_i32.to_le_bytes());
    bytes[22..26].copy_from_slice(&1_i32.to_le_bytes());
    bytes[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bytes[28..30].copy_from_slice(&24_u16.to_le_bytes());
    bytes[34..38].copy_from_slice(&4_u32.to_le_bytes());
    bytes[56] = 0xff;
    bytes
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let image = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(
        width,
        height,
        Rgba([255, 0, 0, 255]),
    ));
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, ImageFormat::Png).unwrap();
    bytes.into_inner()
}

#[test]
fn converts_bmp_to_png_before_attachment() {
    let result = process_image(
        &tiny_bmp_1x1_red(),
        "image/bmp",
        ProcessImageOptions::default(),
    )
    .unwrap();

    assert_eq!(result.mime_type, "image/png");
    let converted = image::load_from_memory_with_format(
        &STANDARD.decode(&result.data).unwrap(),
        ImageFormat::Png,
    )
    .unwrap()
    .to_rgb8();
    assert_eq!(converted.dimensions(), (1, 1));
    assert_eq!(converted.get_pixel(0, 0).0, [255, 0, 0]);
    assert_eq!(
        result.hints,
        ["[Image converted from image/bmp to image/png.]".to_owned()]
    );
}

#[test]
fn resizes_to_inline_dimensions_and_reports_coordinate_hint() {
    let result = process_image(
        &png(4, 2),
        "image/png",
        ProcessImageOptions {
            resize: ImageResizeOptions {
                max_width: 2,
                max_height: 2,
                max_bytes: 1_000_000,
                ..ImageResizeOptions::default()
            },
            ..ProcessImageOptions::default()
        },
    )
    .unwrap();

    assert_eq!(result.mime_type, "image/png");
    let displayed = image::load_from_memory(&STANDARD.decode(&result.data).unwrap()).unwrap();
    assert_eq!((displayed.width(), displayed.height()), (2, 1));
    assert_eq!(
        result.hints,
        ["[Image: original 4x2, displayed at 2x1. Multiply coordinates by 2.00 to map to original image.]".to_owned()]
    );
}

#[test]
fn preserves_supported_bytes_when_auto_resize_is_disabled() {
    let bytes = png(1, 1);
    let result = process_image(
        &bytes,
        "IMAGE/JPG; ignored=parameter",
        ProcessImageOptions {
            auto_resize_images: false,
            ..ProcessImageOptions::default()
        },
    )
    .unwrap();

    assert_eq!(result.mime_type, "image/jpeg");
    assert_eq!(result.data, STANDARD.encode(bytes));
    assert!(result.hints.is_empty());
}

#[test]
fn reports_resize_failure_instead_of_attaching_invalid_or_oversized_data() {
    let malformed_png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
    assert_eq!(
        process_image(malformed_png, "image/png", ProcessImageOptions::default()),
        Err("[Image omitted: could not be resized below the inline image size limit.]")
    );

    assert_eq!(
        process_image(
            &png(1, 1),
            "image/png",
            ProcessImageOptions {
                resize: ImageResizeOptions {
                    max_bytes: 1,
                    ..ImageResizeOptions::default()
                },
                ..ProcessImageOptions::default()
            }
        ),
        Err("[Image omitted: could not be resized below the inline image size limit.]")
    );
}
