//! Build the first prompt from stdin, file text, and CLI messages.

use zedflow_ai::types::ImageContent;

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
    let mut message = String::new();
    if let Some(stdin) = stdin_content {
        message.push_str(stdin);
    }
    if let Some(text) = file_text {
        if !text.is_empty() {
            message.push_str(text);
        }
    }
    if !parsed.messages.is_empty() {
        message.push_str(&parsed.messages.remove(0));
    }
    InitialMessageResult {
        initial_message: (!message.is_empty()).then_some(message),
        initial_images: file_images.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::parse_args;
    #[test]
    fn combines_sources_and_consumes_one_message() {
        let mut args = parse_args(["first", "second"]);
        let result = build_initial_message(&mut args, Some("file\n"), &[], Some("stdin\n"));
        assert_eq!(
            result.initial_message.as_deref(),
            Some("stdin\nfile\nfirst")
        );
        assert_eq!(args.messages, vec!["second"]);
    }
}
