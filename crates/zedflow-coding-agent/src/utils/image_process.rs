use std::io::Cursor;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, imageops::FilterType};

const DEFAULT_MAX_BYTES: usize = 4_718_592;
const CONVERSION_FAILURE: &str =
    "[Image omitted: could not be converted to a supported inline image format.]";
const RESIZE_FAILURE: &str =
    "[Image omitted: could not be resized below the inline image size limit.]";

#[derive(Clone, Copy, Debug)]
pub struct ImageResizeOptions {
    pub max_width: u32,
    pub max_height: u32,
    pub max_bytes: usize,
    pub jpeg_quality: u8,
}

impl Default for ImageResizeOptions {
    fn default() -> Self {
        Self {
            max_width: 2_000,
            max_height: 2_000,
            max_bytes: DEFAULT_MAX_BYTES,
            jpeg_quality: 80,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ProcessImageOptions {
    pub auto_resize_images: bool,
    pub resize: ImageResizeOptions,
}

impl Default for ProcessImageOptions {
    fn default() -> Self {
        Self {
            auto_resize_images: true,
            resize: ImageResizeOptions::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessedImage {
    pub data: String,
    pub mime_type: String,
    pub hints: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResizedImage {
    pub data: String,
    pub mime_type: String,
    pub original_width: u32,
    pub original_height: u32,
    pub width: u32,
    pub height: u32,
    pub was_resized: bool,
}

pub fn process_image(
    bytes: &[u8],
    mime_type: &str,
    options: ProcessImageOptions,
) -> Result<ProcessedImage, &'static str> {
    let base_mime_type = mime_type
        .split(';')
        .next()
        .unwrap_or(mime_type)
        .trim()
        .to_ascii_lowercase();
    let normalized_mime_type = match base_mime_type.as_str() {
        "image/png" => Some("image/png"),
        "image/jpeg" | "image/jpg" => Some("image/jpeg"),
        "image/gif" => Some("image/gif"),
        "image/webp" => Some("image/webp"),
        _ => None,
    };

    let (normalized_bytes, normalized_mime_type, converted_from) =
        if let Some(normalized_mime_type) = normalized_mime_type {
            (bytes.to_vec(), normalized_mime_type, None)
        } else {
            let image = decode_image(bytes, &base_mime_type).ok_or(CONVERSION_FAILURE)?;
            (
                encode_png(&image).ok_or(CONVERSION_FAILURE)?,
                "image/png",
                Some(base_mime_type),
            )
        };

    if !options.auto_resize_images {
        return Ok(ProcessedImage {
            data: STANDARD.encode(&normalized_bytes),
            mime_type: normalized_mime_type.to_owned(),
            hints: conversion_hint(converted_from.as_deref(), normalized_mime_type)
                .into_iter()
                .collect(),
        });
    }

    let resized = resize_image(&normalized_bytes, normalized_mime_type, options.resize)
        .ok_or(RESIZE_FAILURE)?;
    let mut hints: Vec<_> = conversion_hint(converted_from.as_deref(), &resized.mime_type)
        .into_iter()
        .collect();
    if let Some(note) = format_dimension_note(&resized) {
        hints.push(note);
    }

    Ok(ProcessedImage {
        data: resized.data,
        mime_type: resized.mime_type,
        hints,
    })
}

pub fn resize_image(
    bytes: &[u8],
    mime_type: &str,
    options: ImageResizeOptions,
) -> Option<ResizedImage> {
    let image = decode_image(bytes, mime_type)?;
    let original_width = image.width();
    let original_height = image.height();
    let input_base64_size = bytes.len().div_ceil(3).checked_mul(4)?;

    if original_width <= options.max_width
        && original_height <= options.max_height
        && input_base64_size < options.max_bytes
    {
        return Some(ResizedImage {
            data: STANDARD.encode(bytes),
            mime_type: mime_type.to_owned(),
            original_width,
            original_height,
            width: original_width,
            height: original_height,
            was_resized: false,
        });
    }

    let (mut width, mut height) = fit_dimensions(
        original_width,
        original_height,
        options.max_width,
        options.max_height,
    )?;
    let qualities = [options.jpeg_quality, 85, 70, 55, 40];

    loop {
        let resized = image.resize_exact(width, height, FilterType::Lanczos3);
        let png = encode_png(&resized)?;
        if encoded_size(&png)? < options.max_bytes {
            return Some(resized_result(
                png,
                "image/png",
                original_width,
                original_height,
                width,
                height,
            ));
        }
        for (index, quality) in qualities.into_iter().enumerate() {
            if qualities[..index].contains(&quality) {
                continue;
            }
            let jpeg = encode_jpeg(&resized, quality)?;
            if encoded_size(&jpeg)? < options.max_bytes {
                return Some(resized_result(
                    jpeg,
                    "image/jpeg",
                    original_width,
                    original_height,
                    width,
                    height,
                ));
            }
        }

        if width == 1 && height == 1 {
            return None;
        }
        let next_width = if width == 1 {
            1
        } else {
            (f64::from(width) * 0.75).floor() as u32
        };
        let next_height = if height == 1 {
            1
        } else {
            (f64::from(height) * 0.75).floor() as u32
        };
        if (next_width, next_height) == (width, height) {
            return None;
        }
        (width, height) = (next_width, next_height);
    }
}

fn conversion_hint(from: Option<&str>, to: &str) -> Option<String> {
    from.filter(|from| *from != to)
        .map(|from| format!("[Image converted from {from} to {to}.]"))
}

pub fn format_dimension_note(image: &ResizedImage) -> Option<String> {
    image.was_resized.then(|| {
        let scale = f64::from(image.original_width) / f64::from(image.width);
        format!(
            "[Image: original {}x{}, displayed at {}x{}. Multiply coordinates by {scale:.2} to map to original image.]",
            image.original_width, image.original_height, image.width, image.height
        )
    })
}

fn fit_dimensions(width: u32, height: u32, max_width: u32, max_height: u32) -> Option<(u32, u32)> {
    if width == 0 || height == 0 || max_width == 0 || max_height == 0 {
        return None;
    }
    let mut width = width;
    let mut height = height;
    if width > max_width {
        height = (f64::from(height) * f64::from(max_width) / f64::from(width)).round() as u32;
        width = max_width;
    }
    if height > max_height {
        width = (f64::from(width) * f64::from(max_height) / f64::from(height)).round() as u32;
        height = max_height;
    }
    Some((width.max(1), height.max(1)))
}

fn decode_image(bytes: &[u8], mime_type: &str) -> Option<DynamicImage> {
    let format = match mime_type
        .split(';')
        .next()?
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "image/png" => ImageFormat::Png,
        "image/jpeg" | "image/jpg" => ImageFormat::Jpeg,
        "image/gif" => ImageFormat::Gif,
        "image/webp" => ImageFormat::WebP,
        "image/bmp" => ImageFormat::Bmp,
        _ => image::guess_format(bytes).ok()?,
    };
    let mut decoder = ImageReader::with_format(Cursor::new(bytes), format)
        .into_decoder()
        .ok()?;
    let orientation = decoder.orientation().ok()?;
    let mut image = DynamicImage::from_decoder(decoder).ok()?;
    image.apply_orientation(orientation);
    Some(image)
}

fn encode_png(image: &DynamicImage) -> Option<Vec<u8>> {
    let mut output = Cursor::new(Vec::new());
    image.write_to(&mut output, ImageFormat::Png).ok()?;
    Some(output.into_inner())
}

fn encode_jpeg(image: &DynamicImage, quality: u8) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, quality)
        .encode_image(image)
        .ok()?;
    Some(output)
}

fn encoded_size(bytes: &[u8]) -> Option<usize> {
    bytes.len().div_ceil(3).checked_mul(4)
}

fn resized_result(
    bytes: Vec<u8>,
    mime_type: &str,
    original_width: u32,
    original_height: u32,
    width: u32,
    height: u32,
) -> ResizedImage {
    ResizedImage {
        data: STANDARD.encode(bytes),
        mime_type: mime_type.to_owned(),
        original_width,
        original_height,
        width,
        height,
        was_resized: true,
    }
}
