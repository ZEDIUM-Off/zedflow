use base64::{Engine, engine::general_purpose::STANDARD};
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
            max_width: 2000,
            max_height: 2000,
            max_bytes: 4_718_592,
            jpeg_quality: 80,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResizedImage {
    pub data: String,
    pub mime_type: String,
    pub original_width: u32,
    pub original_height: u32,
    pub width: u32,
    pub height: u32,
    pub was_resized: bool,
}
pub fn resize_image_in_process(
    bytes: &[u8],
    mime: &str,
    o: Option<ImageResizeOptions>,
) -> Option<ResizedImage> {
    let o = o.unwrap_or_default();
    let im = image::load_from_memory(bytes).ok()?;
    let (ow, oh) = (im.width(), im.height());
    let scale = (o.max_width as f64 / ow as f64)
        .min(o.max_height as f64 / oh as f64)
        .min(1.0);
    let (w, h) = (
        (ow as f64 * scale).round() as u32,
        (oh as f64 * scale).round() as u32,
    );
    let out = if (w, h) == (ow, oh) {
        bytes.to_vec()
    } else {
        let mut c = std::io::Cursor::new(Vec::new());
        im.resize_exact(w, h, image::imageops::FilterType::Lanczos3)
            .write_to(&mut c, image::ImageFormat::Png)
            .ok()?;
        c.into_inner()
    };
    let data = STANDARD.encode(out);
    if data.len() > o.max_bytes {
        return None;
    }
    Some(ResizedImage {
        data,
        mime_type: if w == ow && h == oh {
            mime.into()
        } else {
            "image/png".into()
        },
        original_width: ow,
        original_height: oh,
        width: w,
        height: h,
        was_resized: w != ow || h != oh,
    })
}
pub fn format_dimension_note(x: &ResizedImage) -> Option<String> {
    x.was_resized.then(||format!("[Image: original {}x{}, displayed at {}x{}. Multiply coordinates by {:.2} to map to original image.]",x.original_width,x.original_height,x.width,x.height,x.original_width as f64/x.width as f64))
}
