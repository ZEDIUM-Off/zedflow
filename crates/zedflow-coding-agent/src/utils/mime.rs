use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};

const SNIFF_BYTES: usize = 4100;
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Detects image formats supported by Pi, rejecting JPEG XL and animated PNG.
#[must_use]
pub fn detect_supported_image_mime_type(buffer: &[u8]) -> Option<&'static str> {
    if buffer.starts_with(&[0xff, 0xd8, 0xff]) {
        return (buffer.get(3) != Some(&0xf7)).then_some("image/jpeg");
    }
    if buffer.starts_with(PNG) {
        return (is_png(buffer) && !is_animated_png(buffer)).then_some("image/png");
    }
    if buffer.starts_with(b"GIF") {
        return Some("image/gif");
    }
    if buffer.starts_with(b"RIFF") && buffer.get(8..12) == Some(b"WEBP") {
        return Some("image/webp");
    }
    if buffer.starts_with(b"BM") && is_bmp(buffer) {
        return Some("image/bmp");
    }
    None
}

pub fn detect_supported_image_mime_type_from_file(
    path: impl AsRef<Path>,
) -> io::Result<Option<&'static str>> {
    let mut file = File::open(path)?;
    let mut buffer = vec![0; SNIFF_BYTES];
    let read = file.read(&mut buffer)?;
    Ok(detect_supported_image_mime_type(&buffer[..read]))
}

fn is_png(buffer: &[u8]) -> bool {
    buffer.len() >= 16 && read_u32_be(buffer, 8) == 13 && buffer.get(12..16) == Some(b"IHDR")
}
fn is_animated_png(buffer: &[u8]) -> bool {
    let mut offset = PNG.len();
    while offset + 8 <= buffer.len() {
        let length = read_u32_be(buffer, offset) as usize;
        match buffer.get(offset + 4..offset + 8) {
            Some(b"acTL") => return true,
            Some(b"IDAT") => return false,
            _ => {}
        }
        let Some(next) = offset.checked_add(12).and_then(|n| n.checked_add(length)) else {
            return false;
        };
        if next <= offset || next > buffer.len() {
            return false;
        }
        offset = next;
    }
    false
}
fn is_bmp(buffer: &[u8]) -> bool {
    if buffer.len() < 26 {
        return false;
    }
    let size = read_u32_le(buffer, 2);
    let pixels = read_u32_le(buffer, 10);
    let dib = read_u32_le(buffer, 14);
    if (size != 0 && size < 26) || pixels < 14 + dib || (size != 0 && pixels >= size) {
        return false;
    }
    let (planes, bits) = if dib == 12 {
        (read_u16_le(buffer, 22), read_u16_le(buffer, 24))
    } else if (40..=124).contains(&dib) && buffer.len() >= 30 {
        (read_u16_le(buffer, 26), read_u16_le(buffer, 28))
    } else {
        return false;
    };
    planes == 1 && matches!(bits, 1 | 4 | 8 | 16 | 24 | 32)
}
fn read_u16_le(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([
        *bytes.get(offset).unwrap_or(&0),
        *bytes.get(offset + 1).unwrap_or(&0),
    ])
}
fn read_u32_le(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(std::array::from_fn(|i| {
        *bytes.get(offset + i).unwrap_or(&0)
    }))
}
fn read_u32_be(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(std::array::from_fn(|i| {
        *bytes.get(offset + i).unwrap_or(&0)
    }))
}
