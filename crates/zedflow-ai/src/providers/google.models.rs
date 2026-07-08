//! Google model catalog ported from Pi's `packages/ai/src/providers/google.models.ts`.

/// Pricing metadata for a Google model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GoogleModelCost {
    /// Input token price in Pi's model catalog units.
    pub input: f64,
    /// Output token price in Pi's model catalog units.
    pub output: f64,
    /// Cache-read token price in Pi's model catalog units.
    pub cache_read: f64,
    /// Cache-write token price in Pi's model catalog units.
    pub cache_write: f64,
}

/// A `thinkingLevelMap` entry from Pi's Google model catalog.
pub type GoogleThinkingLevelMap = &'static [(&'static str, Option<&'static str>)];

/// One Google model entry from Pi's generated model catalog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GoogleModel {
    /// Model id and catalog key.
    pub id: &'static str,
    /// Human-readable model name.
    pub name: &'static str,
    /// API implementation id.
    pub api: &'static str,
    /// Provider id.
    pub provider: &'static str,
    /// Base API URL.
    pub base_url: &'static str,
    /// Whether the model supports reasoning.
    pub reasoning: bool,
    /// Optional Pi thinking-level mapping.
    pub thinking_level_map: Option<GoogleThinkingLevelMap>,
    /// Supported input modalities.
    pub input: &'static [&'static str],
    /// Model cost metadata.
    pub cost: GoogleModelCost,
    /// Context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
}

const TEXT_IMAGE: &[&str] = &["text", "image"];
const THINKING_OFF: GoogleThinkingLevelMap = &[("off", None)];
const THINKING_PRO: GoogleThinkingLevelMap = &[
    ("off", None),
    ("minimal", None),
    ("low", Some("LOW")),
    ("medium", None),
    ("high", Some("HIGH")),
];
const THINKING_GEMMA: GoogleThinkingLevelMap = &[
    ("off", None),
    ("minimal", Some("MINIMAL")),
    ("low", None),
    ("medium", None),
    ("high", Some("HIGH")),
];

/// Google models keyed by `id`, preserving Pi's generated catalog order and metadata.
pub const GOOGLE_MODELS: &[GoogleModel] = &[
    GoogleModel {
        id: "gemini-2.0-flash",
        name: "Gemini 2.0 Flash",
        api: "google-generative-ai",
        provider: "google",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: GoogleModelCost {
            input: 0.1,
            output: 0.4,
            cache_read: 0.025,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 8_192,
    },
    GoogleModel {
        id: "gemini-2.0-flash-lite",
        name: "Gemini 2.0 Flash-Lite",
        api: "google-generative-ai",
        provider: "google",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: GoogleModelCost {
            input: 0.075,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 8_192,
    },
    GoogleModel {
        id: "gemini-2.5-flash",
        name: "Gemini 2.5 Flash",
        api: "google-generative-ai",
        provider: "google",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: GoogleModelCost {
            input: 0.3,
            output: 2.5,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
    GoogleModel {
        id: "gemini-2.5-flash-lite",
        name: "Gemini 2.5 Flash-Lite",
        api: "google-generative-ai",
        provider: "google",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: GoogleModelCost {
            input: 0.1,
            output: 0.4,
            cache_read: 0.01,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
    GoogleModel {
        id: "gemini-2.5-pro",
        name: "Gemini 2.5 Pro",
        api: "google-generative-ai",
        provider: "google",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: GoogleModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
    GoogleModel {
        id: "gemini-3-flash-preview",
        name: "Gemini 3 Flash Preview",
        api: "google-generative-ai",
        provider: "google",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF),
        input: TEXT_IMAGE,
        cost: GoogleModelCost {
            input: 0.5,
            output: 3.0,
            cache_read: 0.05,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
    GoogleModel {
        id: "gemini-3-pro-preview",
        name: "Gemini 3 Pro Preview",
        api: "google-generative-ai",
        provider: "google",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        reasoning: true,
        thinking_level_map: Some(THINKING_PRO),
        input: TEXT_IMAGE,
        cost: GoogleModelCost {
            input: 2.0,
            output: 12.0,
            cache_read: 0.2,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
    GoogleModel {
        id: "gemini-3.1-flash-lite",
        name: "Gemini 3.1 Flash Lite",
        api: "google-generative-ai",
        provider: "google",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF),
        input: TEXT_IMAGE,
        cost: GoogleModelCost {
            input: 0.25,
            output: 1.5,
            cache_read: 0.025,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
    GoogleModel {
        id: "gemini-3.1-flash-lite-preview",
        name: "Gemini 3.1 Flash Lite Preview",
        api: "google-generative-ai",
        provider: "google",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF),
        input: TEXT_IMAGE,
        cost: GoogleModelCost {
            input: 0.25,
            output: 1.5,
            cache_read: 0.025,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
    GoogleModel {
        id: "gemini-3.1-pro-preview",
        name: "Gemini 3.1 Pro Preview",
        api: "google-generative-ai",
        provider: "google",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        reasoning: true,
        thinking_level_map: Some(THINKING_PRO),
        input: TEXT_IMAGE,
        cost: GoogleModelCost {
            input: 2.0,
            output: 12.0,
            cache_read: 0.2,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
    GoogleModel {
        id: "gemini-3.1-pro-preview-customtools",
        name: "Gemini 3.1 Pro Preview Custom Tools",
        api: "google-generative-ai",
        provider: "google",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        reasoning: true,
        thinking_level_map: Some(THINKING_PRO),
        input: TEXT_IMAGE,
        cost: GoogleModelCost {
            input: 2.0,
            output: 12.0,
            cache_read: 0.2,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
    GoogleModel {
        id: "gemini-3.5-flash",
        name: "Gemini 3.5 Flash",
        api: "google-generative-ai",
        provider: "google",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF),
        input: TEXT_IMAGE,
        cost: GoogleModelCost {
            input: 1.5,
            output: 9.0,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
    GoogleModel {
        id: "gemini-flash-latest",
        name: "Gemini Flash Latest",
        api: "google-generative-ai",
        provider: "google",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF),
        input: TEXT_IMAGE,
        cost: GoogleModelCost {
            input: 1.5,
            output: 9.0,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
    GoogleModel {
        id: "gemini-flash-lite-latest",
        name: "Gemini Flash-Lite Latest",
        api: "google-generative-ai",
        provider: "google",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF),
        input: TEXT_IMAGE,
        cost: GoogleModelCost {
            input: 0.25,
            output: 1.5,
            cache_read: 0.025,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
    GoogleModel {
        id: "gemma-4-26b-a4b-it",
        name: "Gemma 4 26B A4B IT",
        api: "google-generative-ai",
        provider: "google",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        reasoning: true,
        thinking_level_map: Some(THINKING_GEMMA),
        input: TEXT_IMAGE,
        cost: GoogleModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 32_768,
    },
    GoogleModel {
        id: "gemma-4-31b-it",
        name: "Gemma 4 31B IT",
        api: "google-generative-ai",
        provider: "google",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        reasoning: true,
        thinking_level_map: Some(THINKING_GEMMA),
        input: TEXT_IMAGE,
        cost: GoogleModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 32_768,
    },
];
