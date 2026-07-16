//! Google Vertex model catalog ported from Pi's `packages/ai/src/providers/google-vertex.models.ts`.

use crate::models::Model;

/// Pricing metadata for a Google Vertex model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GoogleVertexModelCost {
    /// Input token price in Pi's model catalog units.
    pub input: f64,
    /// Output token price in Pi's model catalog units.
    pub output: f64,
    /// Cache-read token price in Pi's model catalog units.
    pub cache_read: f64,
    /// Cache-write token price in Pi's model catalog units.
    pub cache_write: f64,
}

/// A `thinkingLevelMap` entry from Pi's Google Vertex model catalog.
pub type GoogleVertexThinkingLevelMap = &'static [(&'static str, Option<&'static str>)];

/// One Google Vertex model entry from Pi's generated model catalog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GoogleVertexModel {
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
    pub thinking_level_map: Option<GoogleVertexThinkingLevelMap>,
    /// Supported input modalities.
    pub input: &'static [&'static str],
    /// Model cost metadata.
    pub cost: GoogleVertexModelCost,
    /// Context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
}

const TEXT_IMAGE: &[&str] = &["text", "image"];
const BASE_URL: &str = "https://{location}-aiplatform.googleapis.com";
const THINKING_OFF: GoogleVertexThinkingLevelMap = &[("off", None)];
const THINKING_PRO_PREVIEW: GoogleVertexThinkingLevelMap = &[
    ("off", None),
    ("minimal", None),
    ("low", Some("LOW")),
    ("medium", None),
    ("high", Some("HIGH")),
];

/// Google Vertex models keyed by `id`, preserving Pi's generated catalog order and metadata.
pub const GOOGLE_VERTEX_MODELS: &[GoogleVertexModel] = &[
    GoogleVertexModel {
        id: "gemini-2.5-flash",
        name: "Gemini 2.5 Flash",
        api: "google-vertex",
        provider: "google-vertex",
        base_url: BASE_URL,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: GoogleVertexModelCost {
            input: 0.3,
            output: 2.5,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
    GoogleVertexModel {
        id: "gemini-2.5-flash-lite",
        name: "Gemini 2.5 Flash-Lite",
        api: "google-vertex",
        provider: "google-vertex",
        base_url: BASE_URL,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: GoogleVertexModelCost {
            input: 0.1,
            output: 0.4,
            cache_read: 0.01,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
    GoogleVertexModel {
        id: "gemini-2.5-pro",
        name: "Gemini 2.5 Pro",
        api: "google-vertex",
        provider: "google-vertex",
        base_url: BASE_URL,
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: GoogleVertexModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
    GoogleVertexModel {
        id: "gemini-3-flash-preview",
        name: "Gemini 3 Flash Preview",
        api: "google-vertex",
        provider: "google-vertex",
        base_url: BASE_URL,
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF),
        input: TEXT_IMAGE,
        cost: GoogleVertexModelCost {
            input: 0.5,
            output: 3.0,
            cache_read: 0.05,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
    GoogleVertexModel {
        id: "gemini-3.1-flash-lite",
        name: "Gemini 3.1 Flash Lite",
        api: "google-vertex",
        provider: "google-vertex",
        base_url: BASE_URL,
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF),
        input: TEXT_IMAGE,
        cost: GoogleVertexModelCost {
            input: 0.25,
            output: 1.5,
            cache_read: 0.025,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
    GoogleVertexModel {
        id: "gemini-3.1-pro-preview",
        name: "Gemini 3.1 Pro Preview",
        api: "google-vertex",
        provider: "google-vertex",
        base_url: BASE_URL,
        reasoning: true,
        thinking_level_map: Some(THINKING_PRO_PREVIEW),
        input: TEXT_IMAGE,
        cost: GoogleVertexModelCost {
            input: 2.0,
            output: 12.0,
            cache_read: 0.2,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
    GoogleVertexModel {
        id: "gemini-3.1-pro-preview-customtools",
        name: "Gemini 3.1 Pro Preview Custom Tools",
        api: "google-vertex",
        provider: "google-vertex",
        base_url: BASE_URL,
        reasoning: true,
        thinking_level_map: Some(THINKING_PRO_PREVIEW),
        input: TEXT_IMAGE,
        cost: GoogleVertexModelCost {
            input: 2.0,
            output: 12.0,
            cache_read: 0.2,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
    GoogleVertexModel {
        id: "gemini-3.5-flash",
        name: "Gemini 3.5 Flash",
        api: "google-vertex",
        provider: "google-vertex",
        base_url: BASE_URL,
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF),
        input: TEXT_IMAGE,
        cost: GoogleVertexModelCost {
            input: 1.5,
            output: 9.0,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
    GoogleVertexModel {
        id: "gemini-flash-latest",
        name: "Gemini Flash Latest",
        api: "google-vertex",
        provider: "google-vertex",
        base_url: BASE_URL,
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF),
        input: TEXT_IMAGE,
        cost: GoogleVertexModelCost {
            input: 1.5,
            output: 9.0,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
    GoogleVertexModel {
        id: "gemini-flash-lite-latest",
        name: "Gemini Flash-Lite Latest",
        api: "google-vertex",
        provider: "google-vertex",
        base_url: BASE_URL,
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF),
        input: TEXT_IMAGE,
        cost: GoogleVertexModelCost {
            input: 0.25,
            output: 1.5,
            cache_read: 0.025,
            cache_write: 0.0,
        },
        context_window: 1_048_576,
        max_tokens: 65_536,
    },
];

/// Returns the Google Vertex catalog as the crate's current minimal runtime model shape.
#[must_use]
pub fn google_vertex_models() -> Vec<Model> {
    GOOGLE_VERTEX_MODELS
        .iter()
        .map(|model| Model {
            provider: model.provider.to_owned(),
            id: model.id.to_owned(),
            api: model.api.to_owned(),
            ..Model::default()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_google_vertex_catalog_metadata() {
        assert_eq!(GOOGLE_VERTEX_MODELS.len(), 10);
        assert_eq!(GOOGLE_VERTEX_MODELS[0].id, "gemini-2.5-flash");
        assert_eq!(
            GOOGLE_VERTEX_MODELS[3].thinking_level_map,
            Some(THINKING_OFF)
        );
        assert_eq!(GOOGLE_VERTEX_MODELS[5].cost.input, 2.0);
        assert_eq!(GOOGLE_VERTEX_MODELS[5].cost.cache_read, 0.2);
        assert_eq!(
            GOOGLE_VERTEX_MODELS[5].thinking_level_map,
            Some(THINKING_PRO_PREVIEW)
        );
        assert_eq!(GOOGLE_VERTEX_MODELS[9].max_tokens, 65_536);
    }

    #[test]
    fn exposes_current_runtime_model_shape() {
        let models = google_vertex_models();
        assert_eq!(models.len(), GOOGLE_VERTEX_MODELS.len());
        assert!(models.iter().all(|model| model.provider == "google-vertex"));
        assert!(models.iter().all(|model| model.api == "google-vertex"));
    }
}
