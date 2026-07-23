//! Processing of `@file` arguments for non-interactive modes.

use crate::utils::mime::detect_supported_image_mime_type_from_file;
use base64::{Engine, engine::general_purpose::STANDARD};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use zedflow_ai::{ImageContent, ImageContentType};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessedFiles {
    pub text: String,
    pub images: Vec<ImageContent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessFileOptions {
    pub auto_resize_images: bool,
}
impl Default for ProcessFileOptions {
    fn default() -> Self {
        Self {
            auto_resize_images: true,
        }
    }
}

pub fn process_file_arguments(
    file_args: &[String],
    _options: ProcessFileOptions,
) -> io::Result<ProcessedFiles> {
    let mut output = ProcessedFiles::default();
    for argument in file_args {
        let path = expand_path(argument);
        let metadata = fs::metadata(&path)
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))?;
        if metadata.len() == 0 {
            continue;
        }
        if let Some(mime) = detect_supported_image_mime_type_from_file(&path)? {
            let data = STANDARD.encode(fs::read(&path)?);
            output.images.push(ImageContent {
                content_type: ImageContentType::Image,
                data,
                mime_type: mime.into(),
            });
            output
                .text
                .push_str(&format!("<file name=\"{}\"></file>\n", path.display()));
        } else {
            let text = fs::read_to_string(&path)?;
            output.text.push_str(&format!(
                "<file name=\"{}\">\n{text}\n</file>\n",
                path.display()
            ));
        }
    }
    Ok(output)
}

fn expand_path(path: &str) -> PathBuf {
    let expanded = if let Some(rest) = path.strip_prefix("~/") {
        std::env::var_os("HOME")
            .map_or_else(|| PathBuf::from(path), |home| Path::new(&home).join(rest))
    } else {
        PathBuf::from(path)
    };
    if expanded.is_absolute() {
        expanded
    } else {
        std::env::current_dir().map_or(expanded.clone(), |cwd| cwd.join(expanded))
    }
}
