//! Read `@file` arguments into prompt text and image attachments.

use crate::{
    path_utils::resolve_read_path, utils::mime::detect_supported_image_mime_type_from_file,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use zedflow_ai::types::{ImageContent, ImageContentType};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessedFiles {
    pub text: String,
    pub images: Vec<ImageContent>,
}

pub fn process_file_arguments<I, S>(file_args: I) -> io::Result<ProcessedFiles>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut result = ProcessedFiles::default();
    for argument in file_args {
        let path = resolve_read_path(argument.as_ref(), std::env::current_dir()?)?;
        let path = path.canonicalize().unwrap_or(path);
        let metadata = fs::metadata(&path).map_err(|e| {
            io::Error::new(e.kind(), format!("File not found: {}: {e}", path.display()))
        })?;
        if metadata.len() == 0 {
            continue;
        }
        if let Some(mime_type) = detect_supported_image_mime_type_from_file(&path)? {
            let data = STANDARD.encode(fs::read(&path)?);
            result.images.push(ImageContent {
                content_type: ImageContentType::Image,
                data,
                mime_type: mime_type.into(),
            });
            result
                .text
                .push_str(&format!("<file name=\"{}\"></file>\n", path.display()));
        } else {
            let content = fs::read_to_string(&path)?;
            result.text.push_str(&format!(
                "<file name=\"{}\">\n{content}\n</file>\n",
                path.display()
            ));
        }
    }
    Ok(result)
}

#[allow(dead_code)]
fn _path(_: &Path) -> PathBuf {
    PathBuf::new()
}
