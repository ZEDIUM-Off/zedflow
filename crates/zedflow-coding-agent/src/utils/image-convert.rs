use base64::{Engine, engine::general_purpose::STANDARD};
pub fn convert_image_bytes_to_png(bytes: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(bytes).ok()?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}
pub fn convert_to_png(base64_data: &str, mime_type: &str) -> Option<(String, String)> {
    if mime_type == "image/png" {
        return Some((base64_data.to_owned(), mime_type.to_owned()));
    }
    let bytes = STANDARD.decode(base64_data).ok()?;
    Some((
        STANDARD.encode(convert_image_bytes_to_png(&bytes)?),
        "image/png".into(),
    ))
}
