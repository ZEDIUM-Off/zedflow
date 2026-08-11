use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::process::Command;
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicU32, Ordering},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageProtocol {
    Kitty,
    ITerm2,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalCapabilities {
    pub images: Option<ImageProtocol>,
    pub true_color: bool,
    pub hyperlinks: bool,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageCellSize {
    pub columns: u32,
    pub rows: u32,
}
#[derive(Debug, Clone, Default)]
pub struct KittyOptions {
    pub columns: Option<u32>,
    pub rows: Option<u32>,
    pub image_id: Option<u32>,
    pub move_cursor: Option<bool>,
}
#[derive(Debug, Clone, Default)]
pub struct ITerm2Options {
    pub width: Option<String>,
    pub height: Option<String>,
    pub name: Option<String>,
    pub preserve_aspect_ratio: Option<bool>,
    pub inline: Option<bool>,
}
#[derive(Debug, Clone, Default)]
pub struct ImageRenderOptions {
    pub max_width_cells: Option<u32>,
    pub max_height_cells: Option<u32>,
    pub preserve_aspect_ratio: Option<bool>,
    pub image_id: Option<u32>,
    pub move_cursor: Option<bool>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedImage {
    pub sequence: String,
    pub rows: u32,
    pub image_id: Option<u32>,
}

static NEXT_IMAGE_ID: AtomicU32 = AtomicU32::new(1);
static CAPABILITIES: OnceLock<Mutex<Option<TerminalCapabilities>>> = OnceLock::new();
static CELL_DIMENSIONS: OnceLock<Mutex<CellDimensions>> = OnceLock::new();
fn caps_cache() -> &'static Mutex<Option<TerminalCapabilities>> {
    CAPABILITIES.get_or_init(|| Mutex::new(None))
}
fn cell_cache() -> &'static Mutex<CellDimensions> {
    CELL_DIMENSIONS.get_or_init(|| {
        Mutex::new(CellDimensions {
            width: 9,
            height: 18,
        })
    })
}

pub fn get_cell_dimensions() -> CellDimensions {
    *cell_cache().lock().unwrap()
}
pub fn set_cell_dimensions(dimensions: CellDimensions) {
    *cell_cache().lock().unwrap() = dimensions;
}
fn env_lower(key: &str) -> String {
    std::env::var(key).unwrap_or_default().to_lowercase()
}
fn env_set(key: &str) -> bool {
    std::env::var_os(key).is_some()
}
fn probe_tmux_hyperlinks() -> bool {
    Command::new("tmux")
        .args(["display-message", "-p", "#{client_termfeatures}"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .is_some_and(|s| s.split(',').any(|f| f.trim() == "hyperlinks"))
}
pub fn detect_capabilities() -> TerminalCapabilities {
    detect_capabilities_with(probe_tmux_hyperlinks)
}
pub fn detect_capabilities_with(probe_tmux: impl FnOnce() -> bool) -> TerminalCapabilities {
    let program = env_lower("TERM_PROGRAM");
    let emulator = env_lower("TERMINAL_EMULATOR");
    let term = env_lower("TERM");
    let color = env_lower("COLORTERM");
    let color_hint = color == "truecolor" || color == "24bit";
    if env_set("TMUX") || term.starts_with("tmux") {
        return TerminalCapabilities {
            images: None,
            true_color: color_hint,
            hyperlinks: probe_tmux(),
        };
    }
    if term.starts_with("screen") {
        return TerminalCapabilities {
            images: None,
            true_color: color_hint,
            hyperlinks: false,
        };
    }
    if env_set("KITTY_WINDOW_ID")
        || program == "kitty"
        || program == "ghostty"
        || term.contains("ghostty")
        || env_set("GHOSTTY_RESOURCES_DIR")
        || env_set("WEZTERM_PANE")
        || program == "wezterm"
        || program == "warpterminal"
        || env_set("WARP_SESSION_ID")
        || env_set("WARP_TERMINAL_SESSION_UUID")
    {
        return TerminalCapabilities {
            images: Some(ImageProtocol::Kitty),
            true_color: true,
            hyperlinks: true,
        };
    }
    if env_set("ITERM_SESSION_ID") || program == "iterm.app" {
        return TerminalCapabilities {
            images: Some(ImageProtocol::ITerm2),
            true_color: true,
            hyperlinks: true,
        };
    }
    if env_set("WT_SESSION") || program == "vscode" || program == "alacritty" {
        return TerminalCapabilities {
            images: None,
            true_color: true,
            hyperlinks: true,
        };
    }
    if emulator == "jetbrains-jediterm" {
        return TerminalCapabilities {
            images: None,
            true_color: true,
            hyperlinks: false,
        };
    }
    TerminalCapabilities {
        images: None,
        true_color: color_hint,
        hyperlinks: false,
    }
}
pub fn get_capabilities() -> TerminalCapabilities {
    let mut cache = caps_cache().lock().unwrap();
    *cache.get_or_insert_with(detect_capabilities)
}
pub fn set_capabilities(caps: TerminalCapabilities) {
    *caps_cache().lock().unwrap() = Some(caps);
}
pub fn reset_capabilities_cache() {
    *caps_cache().lock().unwrap() = None;
}

pub fn is_image_line(line: &str) -> bool {
    line.contains("\x1b_G") || line.contains("\x1b]1337;File=")
}
pub fn allocate_image_id() -> u32 {
    NEXT_IMAGE_ID.fetch_add(1, Ordering::Relaxed).max(1)
}
pub fn encode_kitty(data: &str, options: &KittyOptions) -> String {
    const CHUNK: usize = 4096;
    let mut params = vec!["a=T".to_string(), "f=100".into(), "q=2".into()];
    if options.move_cursor == Some(false) {
        params.push("C=1".into());
    }
    if let Some(v) = options.columns {
        params.push(format!("c={v}"));
    }
    if let Some(v) = options.rows {
        params.push(format!("r={v}"));
    }
    if let Some(v) = options.image_id {
        params.push(format!("i={v}"));
    }
    if data.len() <= CHUNK {
        return format!("\x1b_G{};{}\x1b\\", params.join(","), data);
    }
    let mut out = String::new();
    for (i, chunk) in data.as_bytes().chunks(CHUNK).enumerate() {
        let chunk = std::str::from_utf8(chunk).expect("base64 is ASCII");
        let more = i + 1 < data.len().div_ceil(CHUNK);
        if i == 0 {
            out.push_str(&format!("\x1b_G{},m=1;{}\x1b\\", params.join(","), chunk));
        } else {
            out.push_str(&format!("\x1b_Gm={};{}\x1b\\", u8::from(more), chunk));
        }
    }
    out
}
pub fn delete_kitty_image(id: u32) -> String {
    format!("\x1b_Ga=d,d=I,i={id},q=2\x1b\\")
}
pub fn delete_all_kitty_images() -> String {
    "\x1b_Ga=d,d=A,q=2\x1b\\".into()
}
pub fn encode_iterm2(data: &str, options: &ITerm2Options) -> String {
    let mut params = vec![format!(
        "inline={}",
        u8::from(options.inline != Some(false))
    )];
    if let Some(v) = &options.width {
        params.push(format!("width={v}"));
    }
    if let Some(v) = &options.height {
        params.push(format!("height={v}"));
    }
    if let Some(v) = &options.name {
        params.push(format!("name={}", STANDARD.encode(v)));
    }
    if options.preserve_aspect_ratio == Some(false) {
        params.push("preserveAspectRatio=0".into());
    }
    format!("\x1b]1337;File={}:{}\x07", params.join(";"), data)
}
pub fn calculate_image_cell_size(
    image: ImageDimensions,
    max_width: u32,
    max_height: Option<u32>,
    cell: CellDimensions,
) -> ImageCellSize {
    let mw = max_width.max(1);
    let mh = max_height.map(|v| v.max(1));
    let iw = image.width.max(1) as f64;
    let ih = image.height.max(1) as f64;
    let ws = mw as f64 * cell.width.max(1) as f64 / iw;
    let hs = mh.map_or(ws, |h| h as f64 * cell.height.max(1) as f64 / ih);
    let scale = ws.min(hs);
    let columns = (iw * scale / cell.width.max(1) as f64).ceil() as u32;
    let rows = (ih * scale / cell.height.max(1) as f64).ceil() as u32;
    ImageCellSize {
        columns: columns.clamp(1, mw),
        rows: rows.clamp(1, mh.unwrap_or(rows.max(1))),
    }
}
pub fn calculate_image_rows(image: ImageDimensions, width: u32, cell: CellDimensions) -> u32 {
    calculate_image_cell_size(image, width, None, cell).rows
}

pub fn get_png_dimensions(data: &str) -> Option<ImageDimensions> {
    let b = STANDARD.decode(data).ok()?;
    (b.len() >= 24 && b.starts_with(&[0x89, b'P', b'N', b'G'])).then(|| ImageDimensions {
        width: u32::from_be_bytes(b[16..20].try_into().unwrap()),
        height: u32::from_be_bytes(b[20..24].try_into().unwrap()),
    })
}
pub fn get_gif_dimensions(data: &str) -> Option<ImageDimensions> {
    let b = STANDARD.decode(data).ok()?;
    (b.len() >= 10 && (&b[..6] == b"GIF87a" || &b[..6] == b"GIF89a")).then(|| ImageDimensions {
        width: u16::from_le_bytes([b[6], b[7]]) as u32,
        height: u16::from_le_bytes([b[8], b[9]]) as u32,
    })
}
pub fn get_jpeg_dimensions(data: &str) -> Option<ImageDimensions> {
    let b = STANDARD.decode(data).ok()?;
    if !b.starts_with(&[0xff, 0xd8]) {
        return None;
    }
    let mut p = 2;
    while p + 9 < b.len() {
        if b[p] != 0xff {
            p += 1;
            continue;
        }
        let m = b[p + 1];
        if (0xc0..=0xc2).contains(&m) {
            return Some(ImageDimensions {
                width: u16::from_be_bytes([b[p + 7], b[p + 8]]) as u32,
                height: u16::from_be_bytes([b[p + 5], b[p + 6]]) as u32,
            });
        }
        if p + 3 >= b.len() {
            return None;
        }
        let n = u16::from_be_bytes([b[p + 2], b[p + 3]]) as usize;
        if n < 2 {
            return None;
        }
        p += 2 + n;
    }
    None
}
pub fn get_webp_dimensions(data: &str) -> Option<ImageDimensions> {
    let b = STANDARD.decode(data).ok()?;
    if b.len() < 25 || &b[..4] != b"RIFF" || &b[8..12] != b"WEBP" {
        return None;
    }
    match &b[12..16] {
        b"VP8 " if b.len() >= 30 => Some(ImageDimensions {
            width: (u16::from_le_bytes([b[26], b[27]]) & 0x3fff) as u32,
            height: (u16::from_le_bytes([b[28], b[29]]) & 0x3fff) as u32,
        }),
        b"VP8L" => {
            let x = u32::from_le_bytes(b[21..25].try_into().ok()?);
            Some(ImageDimensions {
                width: (x & 0x3fff) + 1,
                height: ((x >> 14) & 0x3fff) + 1,
            })
        }
        b"VP8X" if b.len() >= 30 => Some(ImageDimensions {
            width: u32::from_le_bytes([b[24], b[25], b[26], 0]) + 1,
            height: u32::from_le_bytes([b[27], b[28], b[29], 0]) + 1,
        }),
        _ => None,
    }
}
pub fn get_image_dimensions(data: &str, mime: &str) -> Option<ImageDimensions> {
    match mime {
        "image/png" => get_png_dimensions(data),
        "image/jpeg" => get_jpeg_dimensions(data),
        "image/gif" => get_gif_dimensions(data),
        "image/webp" => get_webp_dimensions(data),
        _ => None,
    }
}
pub fn render_image(
    data: &str,
    dimensions: ImageDimensions,
    options: &ImageRenderOptions,
) -> Option<RenderedImage> {
    let caps = get_capabilities();
    let protocol = caps.images?;
    let size = calculate_image_cell_size(
        dimensions,
        options.max_width_cells.unwrap_or(80),
        options.max_height_cells,
        get_cell_dimensions(),
    );
    let sequence = match protocol {
        ImageProtocol::Kitty => encode_kitty(
            data,
            &KittyOptions {
                columns: Some(size.columns),
                rows: Some(size.rows),
                image_id: options.image_id,
                move_cursor: options.move_cursor,
            },
        ),
        ImageProtocol::ITerm2 => encode_iterm2(
            data,
            &ITerm2Options {
                width: Some(size.columns.to_string()),
                height: Some("auto".into()),
                preserve_aspect_ratio: Some(options.preserve_aspect_ratio.unwrap_or(true)),
                ..Default::default()
            },
        ),
    };
    Some(RenderedImage {
        sequence,
        rows: size.rows,
        image_id: (protocol == ImageProtocol::Kitty)
            .then_some(options.image_id)
            .flatten(),
    })
}
pub fn hyperlink(text: &str, url: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
}
pub fn image_fallback(
    mime: &str,
    dimensions: Option<ImageDimensions>,
    filename: Option<&str>,
) -> String {
    let mut p = Vec::new();
    if let Some(f) = filename {
        p.push(f.to_string())
    }
    p.push(format!("[{mime}]"));
    if let Some(d) = dimensions {
        p.push(format!("{}x{}", d.width, d.height))
    }
    format!("[Image: {}]", p.join(" "))
}
