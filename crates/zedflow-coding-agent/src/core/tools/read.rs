use std::path::{Path, PathBuf};
use std::sync::Arc;

use zedflow_agent::types::{
    AgentCallbackError, AgentTool, AgentToolResult, AgentToolResultContent, ToolSchema,
};
use zedflow_ai::{ImageContent, ImageContentType, TextContent, TextContentType, Tool};

use crate::path_utils::resolve_read_path_async;
use crate::truncate::{
    DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, TruncationOptions, TruncationResult, format_size,
    truncate_head,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadToolInput {
    pub path: String,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

pub fn create_read_tool(cwd: impl Into<PathBuf>) -> AgentTool {
    let cwd = cwd.into();
    AgentTool {
        tool: Tool {
            name: "read".into(),
            description: format!(
                "Read the contents of a file. Supports text files and images (jpg, png, gif, webp, bmp). Images are sent as attachments. For text files, output is truncated to {DEFAULT_MAX_LINES} lines or {}KB (whichever is hit first). Use offset/limit for large files. When you need the full file, continue with offset until complete.",
                DEFAULT_MAX_BYTES / 1024
            ),
            parameters: schema(
                r#"{
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Path to the file to read (relative or absolute)" },
                        "offset": { "type": "number", "description": "Line number to start reading from (1-indexed)" },
                        "limit": { "type": "number", "description": "Maximum number of lines to read" }
                    },
                    "required": ["path"]
                }"#,
            ),
        },
        label: "read".into(),
        prepare_arguments: None,
        execution_mode: None,
        execute: Arc::new(move |_tool_call_id, arguments, signal, _on_update| {
            let cwd = cwd.clone();
            Box::pin(async move {
                let input = ReadToolInput {
                    path: required_string(&arguments, "path")?,
                    offset: optional_usize(&arguments, "offset")?,
                    limit: optional_usize(&arguments, "limit")?,
                };
                execute_read(&cwd, input, signal).await
            })
        }),
    }
}

pub async fn execute_read(
    cwd: &Path,
    input: ReadToolInput,
    signal: Option<zedflow_ai::AbortSignal>,
) -> Result<AgentToolResult, AgentCallbackError> {
    check_aborted(signal.as_ref())?;
    let absolute_path = resolve_read_path_async(&input.path, cwd).await?;
    let bytes = tokio::fs::read(&absolute_path).await?;
    check_aborted(signal.as_ref())?;

    if let Some(mime_type) = crate::utils::mime::detect_supported_image_mime_type(&bytes) {
        let (image, output_mime_type, note) = if mime_type == "image/bmp" {
            let Some(png) = bmp_to_png(&bytes) else {
                return Ok(AgentToolResult {
                    content: vec![text_content(
                        "Read image file [image/bmp]\nFailed to convert BMP image to PNG",
                    )],
                    details: ToolSchema::Null,
                    terminate: None,
                });
            };
            (
                png,
                "image/png",
                "\n[Image converted from image/bmp to image/png.]",
            )
        } else {
            (bytes, mime_type, "")
        };
        return Ok(AgentToolResult {
            content: vec![
                text_content(format!("Read image file [{output_mime_type}]{note}")),
                AgentToolResultContent::Image(ImageContent {
                    content_type: ImageContentType::Image,
                    data: encode_base64(&image),
                    mime_type: output_mime_type.into(),
                }),
            ],
            details: ToolSchema::Null,
            terminate: None,
        });
    }

    let text = String::from_utf8_lossy(&bytes);
    let all_lines: Vec<&str> = text.split('\n').collect();
    let start_line = input.offset.map_or(0, |offset| offset.saturating_sub(1));
    if start_line >= all_lines.len() {
        return Err(format!(
            "Offset {} is beyond end of file ({} lines total)",
            input.offset.unwrap_or(1),
            all_lines.len()
        )
        .into());
    }

    let end_line = input.limit.map_or(all_lines.len(), |limit| {
        start_line.saturating_add(limit).min(all_lines.len())
    });
    let selected = all_lines[start_line..end_line].join("\n");
    let truncation = truncate_head(&selected, TruncationOptions::default());
    let start_display = start_line + 1;
    let total_lines = all_lines.len();
    let mut details = ToolSchema::Null;

    let output = if truncation.first_line_exceeds_limit {
        details = object([("truncation", truncation_value(&truncation))]);
        format!(
            "[Line {start_display} is {}, exceeds {} limit. Use bash: sed -n '{start_display}p' {} | head -c {DEFAULT_MAX_BYTES}]",
            format_size(all_lines[start_line].len()),
            format_size(DEFAULT_MAX_BYTES),
            input.path
        )
    } else if truncation.truncated {
        let end_display = start_display + truncation.output_lines - 1;
        let byte_notice = if truncation.truncated_by == Some(crate::truncate::TruncatedBy::Bytes) {
            format!(" ({} limit)", format_size(DEFAULT_MAX_BYTES))
        } else {
            String::new()
        };
        details = object([("truncation", truncation_value(&truncation))]);
        format!(
            "{}\n\n[Showing lines {start_display}-{end_display} of {total_lines}{byte_notice}. Use offset={} to continue.]",
            truncation.content,
            end_display + 1
        )
    } else if input.limit.is_some() && end_line < all_lines.len() {
        format!(
            "{}\n\n[{} more lines in file. Use offset={} to continue.]",
            truncation.content,
            all_lines.len() - end_line,
            end_line + 1
        )
    } else {
        truncation.content.clone()
    };

    Ok(AgentToolResult {
        content: vec![text_content(output)],
        details,
        terminate: None,
    })
}

pub(crate) fn text_content(text: impl Into<String>) -> AgentToolResultContent {
    AgentToolResultContent::Text(TextContent {
        content_type: TextContentType::Text,
        text: text.into(),
        text_signature: None,
    })
}

pub(crate) fn schema(value: &str) -> ToolSchema {
    serde_yaml::from_str(value).expect("valid tool schema")
}

pub(crate) fn object<const N: usize>(entries: [(&str, ToolSchema); N]) -> ToolSchema {
    let mut value = ToolSchema::Object(Default::default());
    let ToolSchema::Object(map) = &mut value else {
        unreachable!()
    };
    for (key, entry) in entries {
        map.insert(key.into(), entry);
    }
    value
}

pub(crate) fn required_string(value: &ToolSchema, key: &str) -> Result<String, AgentCallbackError> {
    value
        .get(key)
        .and_then(ToolSchema::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("{key} must be a string").into())
}

pub(crate) fn optional_usize(
    value: &ToolSchema,
    key: &str,
) -> Result<Option<usize>, AgentCallbackError> {
    match value.get(key) {
        None | Some(ToolSchema::Null) => Ok(None),
        Some(number) => number
            .as_u64()
            .and_then(|number| usize::try_from(number).ok())
            .map(Some)
            .ok_or_else(|| format!("{key} must be a non-negative integer").into()),
    }
}

pub(crate) fn truncation_value(result: &TruncationResult) -> ToolSchema {
    object([
        ("content", result.content.clone().into()),
        ("truncated", result.truncated.into()),
        (
            "truncatedBy",
            result.truncated_by.map_or(ToolSchema::Null, |kind| {
                match kind {
                    crate::truncate::TruncatedBy::Lines => "lines",
                    crate::truncate::TruncatedBy::Bytes => "bytes",
                }
                .into()
            }),
        ),
        ("totalLines", result.total_lines.into()),
        ("totalBytes", result.total_bytes.into()),
        ("outputLines", result.output_lines.into()),
        ("outputBytes", result.output_bytes.into()),
        ("lastLinePartial", result.last_line_partial.into()),
        (
            "firstLineExceedsLimit",
            result.first_line_exceeds_limit.into(),
        ),
        ("maxLines", result.max_lines.into()),
        ("maxBytes", result.max_bytes.into()),
    ])
}

pub(crate) fn check_aborted(
    signal: Option<&zedflow_ai::AbortSignal>,
) -> Result<(), AgentCallbackError> {
    if signal.is_some_and(zedflow_ai::AbortSignal::aborted) {
        Err("Operation aborted".into())
    } else {
        Ok(())
    }
}

fn bmp_to_png(bytes: &[u8]) -> Option<Vec<u8>> {
    let data_offset =
        usize::try_from(u32::from_le_bytes(bytes.get(10..14)?.try_into().ok()?)).ok()?;
    let width = i32::from_le_bytes(bytes.get(18..22)?.try_into().ok()?);
    let height = i32::from_le_bytes(bytes.get(22..26)?.try_into().ok()?);
    let planes = u16::from_le_bytes(bytes.get(26..28)?.try_into().ok()?);
    let bits = u16::from_le_bytes(bytes.get(28..30)?.try_into().ok()?);
    let compression = u32::from_le_bytes(bytes.get(30..34)?.try_into().ok()?);
    if width <= 0 || height == 0 || planes != 1 || !matches!(bits, 24 | 32) || compression != 0 {
        return None;
    }
    let width = usize::try_from(width).ok()?;
    let rows = usize::try_from(height.unsigned_abs()).ok()?;
    let bytes_per_pixel = usize::from(bits / 8);
    let row_bytes = width.checked_mul(bytes_per_pixel)?;
    let stride = row_bytes.checked_add(3)? & !3;
    let pixel_bytes = stride.checked_mul(rows)?;
    bytes.get(data_offset..data_offset.checked_add(pixel_bytes)?)?;

    let channels = if bits == 24 { 3 } else { 4 };
    let mut scanlines =
        Vec::with_capacity(rows.checked_mul(width.checked_mul(channels)?.checked_add(1)?)?);
    for output_row in 0..rows {
        scanlines.push(0);
        let source_row = if height > 0 {
            rows - output_row - 1
        } else {
            output_row
        };
        let row_start = data_offset.checked_add(source_row.checked_mul(stride)?)?;
        for column in 0..width {
            let pixel = row_start.checked_add(column.checked_mul(bytes_per_pixel)?)?;
            scanlines.extend_from_slice(&[
                *bytes.get(pixel + 2)?,
                *bytes.get(pixel + 1)?,
                *bytes.get(pixel)?,
            ]);
            if channels == 4 {
                scanlines.push(*bytes.get(pixel + 3)?);
            }
        }
    }

    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&u32::try_from(width).ok()?.to_be_bytes());
    header.extend_from_slice(&u32::try_from(rows).ok()?.to_be_bytes());
    header.extend_from_slice(&[8, if channels == 3 { 2 } else { 6 }, 0, 0, 0]);
    push_png_chunk(&mut png, b"IHDR", &header);
    push_png_chunk(&mut png, b"IDAT", &zlib_store(&scanlines));
    push_png_chunk(&mut png, b"IEND", &[]);
    Some(png)
}

fn zlib_store(data: &[u8]) -> Vec<u8> {
    let mut output = vec![0x78, 0x01];
    for (index, block) in data.chunks(u16::MAX as usize).enumerate() {
        output.push(u8::from((index + 1) * u16::MAX as usize >= data.len()));
        let length = block.len() as u16;
        output.extend_from_slice(&length.to_le_bytes());
        output.extend_from_slice(&(!length).to_le_bytes());
        output.extend_from_slice(block);
    }
    let mut first = 1_u32;
    let mut second = 0_u32;
    for byte in data {
        first = (first + u32::from(*byte)) % 65_521;
        second = (second + first) % 65_521;
    }
    output.extend_from_slice(&((second << 16) | first).to_be_bytes());
    output
}

fn push_png_chunk(output: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    output.extend_from_slice(&(data.len() as u32).to_be_bytes());
    output.extend_from_slice(kind);
    output.extend_from_slice(data);
    let mut crc = u32::MAX;
    for byte in kind.iter().chain(data) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    output.extend_from_slice(&(!crc).to_be_bytes());
}

fn encode_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        output.push(ALPHABET[((value >> 18) & 63) as usize] as char);
        output.push(ALPHABET[((value >> 12) & 63) as usize] as char);
        output.push(if chunk.len() > 1 {
            ALPHABET[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            ALPHABET[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    output
}
