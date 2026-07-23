//! Prompt assembly used by print and JSON modes.

use zedflow_ai::ImageContent;

use super::args::Args;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InitialMessageResult {
    pub initial_message: Option<String>,
    pub initial_images: Vec<ImageContent>,
}

pub fn build_initial_message(
    parsed: &mut Args,
    file_text: Option<&str>,
    file_images: &[ImageContent],
    stdin_content: Option<&str>,
) -> InitialMessageResult {
    let mut parts = Vec::new();
    if let Some(text) = stdin_content {
        parts.push(text.to_owned());
    }
    if let Some(text) = file_text.filter(|text| !text.is_empty()) {
        parts.push((*text).to_owned());
    }
    if let Some(message) = parsed.messages.first().cloned() {
        parts.push(message);
        parsed.messages.remove(0);
    }
    InitialMessageResult {
        initial_message: (!parts.is_empty()).then(|| parts.concat()),
        initial_images: file_images.to_vec(),
    }
}
