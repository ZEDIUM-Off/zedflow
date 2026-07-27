use std::sync::atomic::{AtomicU32, Ordering};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageProtocol {
    Kitty,
    ITerm2,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellDimensions {
    pub width: u32,
    pub height: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageDimensions {
    pub width: u32,
    pub height: u32,
}
static NEXT_IMAGE_ID: AtomicU32 = AtomicU32::new(1);
pub fn allocate_image_id() -> u32 {
    NEXT_IMAGE_ID.fetch_add(1, Ordering::Relaxed)
}
pub fn delete_kitty_image(id: u32) -> String {
    format!("\x1b_Ga=d,d={id};\x1b\\")
}
pub fn delete_all_kitty_images() -> String {
    "\x1b_Ga=d,d=A;\x1b\\".into()
}
pub fn hyperlink(text: &str, url: &str) -> String {
    format!("\x1b]8;;{url}\x07{text}\x1b]8;;\x07")
}
