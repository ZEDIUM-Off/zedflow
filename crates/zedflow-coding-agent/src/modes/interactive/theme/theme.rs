//! Pi-compatible themes, embedded so installed binaries do not depend on source files.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;

pub const DARK_THEME_JSON: &str = include_str!("dark.json");
pub const LIGHT_THEME_JSON: &str = include_str!("light.json");
pub const THEME_SCHEMA_JSON: &str = include_str!("theme-schema.json");
pub const CLANKOLAS_PNG: &[u8] = include_bytes!("../assets/clankolas.png");

const BACKGROUNDS: &[&str] = &[
    "selectedBg",
    "userMessageBg",
    "customMessageBg",
    "toolPendingBg",
    "toolSuccessBg",
    "toolErrorBg",
];
const REQUIRED_COLORS: &[&str] = &[
    "accent",
    "border",
    "borderAccent",
    "borderMuted",
    "success",
    "error",
    "warning",
    "muted",
    "dim",
    "text",
    "thinkingText",
    "selectedBg",
    "userMessageBg",
    "userMessageText",
    "customMessageBg",
    "customMessageText",
    "customMessageLabel",
    "toolPendingBg",
    "toolSuccessBg",
    "toolErrorBg",
    "toolTitle",
    "toolOutput",
    "mdHeading",
    "mdLink",
    "mdLinkUrl",
    "mdCode",
    "mdCodeBlock",
    "mdCodeBlockBorder",
    "mdQuote",
    "mdQuoteBorder",
    "mdHr",
    "mdListBullet",
    "toolDiffAdded",
    "toolDiffRemoved",
    "toolDiffContext",
    "syntaxComment",
    "syntaxKeyword",
    "syntaxFunction",
    "syntaxVariable",
    "syntaxString",
    "syntaxNumber",
    "syntaxType",
    "syntaxOperator",
    "syntaxPunctuation",
    "thinkingOff",
    "thinkingMinimal",
    "thinkingLow",
    "thinkingMedium",
    "thinkingHigh",
    "thinkingXhigh",
    "bashMode",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Truecolor,
    Color256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalTheme {
    Dark,
    Light,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum ColorValue {
    String(String),
    Index(u8),
}

#[derive(Debug, Clone, Deserialize)]
struct ThemeJson {
    name: String,
    #[serde(default)]
    vars: BTreeMap<String, ColorValue>,
    colors: BTreeMap<String, ColorValue>,
    #[serde(rename = "export", default)]
    export_colors: BTreeMap<String, ColorValue>,
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub source_path: Option<PathBuf>,
    mode: ColorMode,
    fg_colors: BTreeMap<String, String>,
    bg_colors: BTreeMap<String, String>,
    export_colors: BTreeMap<String, ColorValue>,
}

impl Theme {
    pub fn from_json(label: &str, json: &str, mode: ColorMode) -> Result<Self, String> {
        let parsed: ThemeJson = serde_json::from_str(json)
            .map_err(|error| format!("Failed to parse theme {label}: {error}"))?;
        if parsed.name.contains('/') {
            return Err(format!(
                "Invalid theme name {:?}: theme names cannot contain /",
                parsed.name
            ));
        }
        let present: BTreeSet<_> = parsed.colors.keys().map(String::as_str).collect();
        let missing: Vec<_> = REQUIRED_COLORS
            .iter()
            .copied()
            .filter(|key| !present.contains(key))
            .collect();
        if !missing.is_empty() {
            return Err(format!(
                "Invalid theme \"{label}\": missing required color tokens: {}",
                missing.join(", ")
            ));
        }

        let mut resolved = BTreeMap::new();
        for (key, value) in &parsed.colors {
            resolved.insert(
                key.clone(),
                resolve(value, &parsed.vars, &mut BTreeSet::new())?,
            );
        }
        let mut fg_colors = BTreeMap::new();
        let mut bg_colors = BTreeMap::new();
        for (key, value) in resolved {
            let ansi = if BACKGROUNDS.contains(&key.as_str()) {
                bg_ansi(&value, mode)?
            } else {
                fg_ansi(&value, mode)?
            };
            if BACKGROUNDS.contains(&key.as_str()) {
                bg_colors.insert(key, ansi);
            } else {
                fg_colors.insert(key, ansi);
            }
        }
        Ok(Self {
            name: parsed.name,
            source_path: None,
            mode,
            fg_colors,
            bg_colors,
            export_colors: parsed.export_colors,
        })
    }

    pub fn from_path(path: impl AsRef<Path>, mode: ColorMode) -> Result<Self, String> {
        let path = path.as_ref();
        let json = fs::read_to_string(path)
            .map_err(|error| format!("Failed to read theme {}: {error}", path.display()))?;
        let mut theme = Self::from_json(&path.display().to_string(), &json, mode)?;
        theme.source_path = Some(path.to_path_buf());
        Ok(theme)
    }

    pub fn builtin(name: &str, mode: ColorMode) -> Result<Self, String> {
        match name {
            "dark" => Self::from_json("dark", DARK_THEME_JSON, mode),
            "light" => Self::from_json("light", LIGHT_THEME_JSON, mode),
            _ => Err(format!("Theme not found: {name}")),
        }
    }

    #[must_use]
    pub fn color_mode(&self) -> ColorMode {
        self.mode
    }
    pub fn fg(&self, color: &str, text: &str) -> Result<String, String> {
        self.fg_colors
            .get(color)
            .map(|ansi| format!("{ansi}{text}\x1b[39m"))
            .ok_or_else(|| format!("Unknown theme color: {color}"))
    }
    pub fn bg(&self, color: &str, text: &str) -> Result<String, String> {
        self.bg_colors
            .get(color)
            .map(|ansi| format!("{ansi}{text}\x1b[49m"))
            .ok_or_else(|| format!("Unknown theme background color: {color}"))
    }
    pub fn fg_ansi(&self, color: &str) -> Option<&str> {
        self.fg_colors.get(color).map(String::as_str)
    }
    pub fn bg_ansi(&self, color: &str) -> Option<&str> {
        self.bg_colors.get(color).map(String::as_str)
    }
    pub fn export_color(&self, color: &str) -> Option<&ColorValue> {
        self.export_colors.get(color)
    }
}

fn resolve(
    value: &ColorValue,
    vars: &BTreeMap<String, ColorValue>,
    visited: &mut BTreeSet<String>,
) -> Result<ColorValue, String> {
    match value {
        ColorValue::Index(value) => Ok(ColorValue::Index(*value)),
        ColorValue::String(value) if value.is_empty() || value.starts_with('#') => {
            Ok(ColorValue::String(value.clone()))
        }
        ColorValue::String(value) => {
            if !visited.insert(value.clone()) {
                return Err(format!("Circular variable reference detected: {value}"));
            }
            let result = resolve(
                vars.get(value)
                    .ok_or_else(|| format!("Variable reference not found: {value}"))?,
                vars,
                visited,
            );
            visited.remove(value);
            result
        }
    }
}

fn rgb(value: &str) -> Result<(u8, u8, u8), String> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    if hex.len() != 6 {
        return Err(format!("Invalid hex color: {value}"));
    }
    let parse = |range| {
        u8::from_str_radix(&hex[range], 16).map_err(|_| format!("Invalid hex color: {value}"))
    };
    Ok((parse(0..2)?, parse(2..4)?, parse(4..6)?))
}

fn ansi(value: &ColorValue, mode: ColorMode, background: bool) -> Result<String, String> {
    let base = if background { 48 } else { 38 };
    match value {
        ColorValue::String(value) if value.is_empty() => {
            Ok(format!("\x1b[{}m", if background { 49 } else { 39 }))
        }
        ColorValue::Index(index) => Ok(format!("\x1b[{base};5;{index}m")),
        ColorValue::String(value) if value.starts_with('#') => {
            let (r, g, b) = rgb(value)?;
            Ok(match mode {
                ColorMode::Truecolor => format!("\x1b[{base};2;{r};{g};{b}m"),
                ColorMode::Color256 => format!("\x1b[{base};5;{}m", rgb_to_256(r, g, b)),
            })
        }
        ColorValue::String(value) => Err(format!("Invalid color value: {value}")),
    }
}
fn fg_ansi(value: &ColorValue, mode: ColorMode) -> Result<String, String> {
    ansi(value, mode, false)
}
fn bg_ansi(value: &ColorValue, mode: ColorMode) -> Result<String, String> {
    ansi(value, mode, true)
}

fn rgb_to_256(r: u8, g: u8, b: u8) -> u8 {
    const CUBE: [i32; 6] = [0, 95, 135, 175, 215, 255];
    let closest = |v: u8| {
        (0..6)
            .min_by_key(|&i| (i32::from(v) - CUBE[i]).abs())
            .unwrap()
    };
    let (ri, gi, bi) = (closest(r), closest(g), closest(b));
    let cube = 16 + 36 * ri + 6 * gi + bi;
    let gray_value =
        (0.299 * f64::from(r) + 0.587 * f64::from(g) + 0.114 * f64::from(b)).round() as i32;
    let gray = (0..24)
        .min_by_key(|&i| (gray_value - (8 + i as i32 * 10)).abs())
        .unwrap();
    let distance = |rr: i32, gg: i32, bb: i32| {
        let dr = i32::from(r) - rr;
        let dg = i32::from(g) - gg;
        let db = i32::from(b) - bb;
        dr * dr * 299 + dg * dg * 587 + db * db * 114
    };
    let spread = r.max(g).max(b) - r.min(g).min(b);
    if spread < 10
        && distance(
            8 + gray as i32 * 10,
            8 + gray as i32 * 10,
            8 + gray as i32 * 10,
        ) < distance(CUBE[ri], CUBE[gi], CUBE[bi])
    {
        (232 + gray) as u8
    } else {
        cube as u8
    }
}

#[must_use]
pub fn available_themes() -> [&'static str; 2] {
    ["dark", "light"]
}

#[must_use]
pub fn parse_auto_theme_setting(setting: Option<&str>) -> Option<(String, String)> {
    let (light, dark) = setting?.split_once('/')?;
    if dark.contains('/') || light.trim().is_empty() || dark.trim().is_empty() {
        None
    } else {
        Some((light.trim().into(), dark.trim().into()))
    }
}

#[must_use]
pub fn resolve_theme_setting(setting: Option<&str>, terminal: TerminalTheme) -> Option<String> {
    if let Some((light, dark)) = parse_auto_theme_setting(setting) {
        return Some(if terminal == TerminalTheme::Light {
            light
        } else {
            dark
        });
    }
    setting
        .filter(|value| !value.contains('/'))
        .map(str::to_owned)
}

#[must_use]
pub fn terminal_theme_from_env(colorfgbg: Option<&str>) -> TerminalTheme {
    let background = colorfgbg
        .and_then(|value| value.rsplit(';').next())
        .and_then(|value| value.parse::<u8>().ok());
    if matches!(background, Some(0..=6 | 8)) {
        TerminalTheme::Dark
    } else if background.is_some() {
        TerminalTheme::Light
    } else {
        TerminalTheme::Dark
    }
}
