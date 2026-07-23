pub use super::image_resize_core::{
    ImageResizeOptions, ResizedImage, format_dimension_note, resize_image_in_process,
};
pub fn resize_image(
    bytes: &[u8],
    mime: &str,
    options: Option<ImageResizeOptions>,
) -> Option<ResizedImage> {
    resize_image_in_process(bytes, mime, options)
}
