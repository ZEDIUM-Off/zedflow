//! Azure OpenAI Responses model catalog ported from Pi.

use crate::models::Model;

/// Pricing metadata for an Azure OpenAI Responses model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AzureOpenAIResponsesModelCost {
    /// Input token price in Pi's model catalog units.
    pub input: f64,
    /// Output token price in Pi's model catalog units.
    pub output: f64,
    /// Cache-read token price in Pi's model catalog units.
    pub cache_read: f64,
    /// Cache-write token price in Pi's model catalog units.
    pub cache_write: f64,
}

/// A `thinkingLevelMap` entry from Pi's Azure OpenAI Responses model catalog.
pub type AzureOpenAIResponsesThinkingLevelMap = &'static [(&'static str, Option<&'static str>)];

/// One Azure OpenAI Responses model entry from Pi's generated model catalog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AzureOpenAIResponsesModel {
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
    pub thinking_level_map: Option<AzureOpenAIResponsesThinkingLevelMap>,
    /// Supported input modalities.
    pub input: &'static [&'static str],
    /// Model cost metadata.
    pub cost: AzureOpenAIResponsesModelCost,
    /// Context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
}

const TEXT: &[&str] = &["text"];
const TEXT_IMAGE: &[&str] = &["text", "image"];
const THINKING_OFF_UNSUPPORTED: AzureOpenAIResponsesThinkingLevelMap = &[("off", None)];
const THINKING_OFF_UNSUPPORTED_XHIGH: AzureOpenAIResponsesThinkingLevelMap =
    &[("off", None), ("xhigh", Some("xhigh"))];
const THINKING_OFF_MINIMAL_LOW_UNSUPPORTED_XHIGH: AzureOpenAIResponsesThinkingLevelMap = &[
    ("off", None),
    ("xhigh", Some("xhigh")),
    ("minimal", None),
    ("low", None),
];

/// Azure OpenAI Responses models keyed by `id`, preserving Pi's generated catalog order and metadata.
pub const AZURE_OPENAI_RESPONSES_MODELS: &[AzureOpenAIResponsesModel] = &[
    AzureOpenAIResponsesModel {
        id: "gpt-4",
        name: "GPT-4",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: false,
        thinking_level_map: None,
        input: TEXT,
        cost: AzureOpenAIResponsesModelCost {
            input: 30.0,
            output: 60.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 8_192,
        max_tokens: 8_192,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-4-turbo",
        name: "GPT-4 Turbo",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 10.0,
            output: 30.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 4_096,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-4.1",
        name: "GPT-4.1",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 2.0,
            output: 8.0,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        context_window: 1_047_576,
        max_tokens: 32_768,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-4.1-mini",
        name: "GPT-4.1 mini",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 0.4,
            output: 1.6,
            cache_read: 0.1,
            cache_write: 0.0,
        },
        context_window: 1_047_576,
        max_tokens: 32_768,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-4.1-nano",
        name: "GPT-4.1 nano",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 0.1,
            output: 0.4,
            cache_read: 0.025,
            cache_write: 0.0,
        },
        context_window: 1_047_576,
        max_tokens: 32_768,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-4o",
        name: "GPT-4o",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 2.5,
            output: 10.0,
            cache_read: 1.25,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 16_384,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-4o-2024-05-13",
        name: "GPT-4o (2024-05-13)",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 5.0,
            output: 15.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 4_096,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-4o-2024-08-06",
        name: "GPT-4o (2024-08-06)",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 2.5,
            output: 10.0,
            cache_read: 1.25,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 16_384,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-4o-2024-11-20",
        name: "GPT-4o (2024-11-20)",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 2.5,
            output: 10.0,
            cache_read: 1.25,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 16_384,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-4o-mini",
        name: "GPT-4o mini",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: false,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.075,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 16_384,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5",
        name: "GPT-5",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 400_000,
        max_tokens: 128_000,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5-chat-latest",
        name: "GPT-5 Chat Latest",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: false,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 16_384,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5-codex",
        name: "GPT-5-Codex",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 400_000,
        max_tokens: 128_000,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5-mini",
        name: "GPT-5 Mini",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 0.25,
            output: 2.0,
            cache_read: 0.025,
            cache_write: 0.0,
        },
        context_window: 400_000,
        max_tokens: 128_000,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5-nano",
        name: "GPT-5 Nano",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 0.05,
            output: 0.4,
            cache_read: 0.005,
            cache_write: 0.0,
        },
        context_window: 400_000,
        max_tokens: 128_000,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5-pro",
        name: "GPT-5 Pro",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 15.0,
            output: 120.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 400_000,
        max_tokens: 128_000,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5.1",
        name: "GPT-5.1",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 400_000,
        max_tokens: 128_000,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5.1-chat-latest",
        name: "GPT-5.1 Chat",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 16_384,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5.1-codex",
        name: "GPT-5.1 Codex",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 400_000,
        max_tokens: 128_000,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5.1-codex-max",
        name: "GPT-5.1 Codex Max",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 400_000,
        max_tokens: 128_000,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5.1-codex-mini",
        name: "GPT-5.1 Codex mini",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 0.25,
            output: 2.0,
            cache_read: 0.025,
            cache_write: 0.0,
        },
        context_window: 400_000,
        max_tokens: 128_000,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5.2",
        name: "GPT-5.2",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED_XHIGH),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 400_000,
        max_tokens: 128_000,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5.2-chat-latest",
        name: "GPT-5.2 Chat",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED_XHIGH),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 16_384,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5.2-codex",
        name: "GPT-5.2 Codex",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED_XHIGH),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 400_000,
        max_tokens: 128_000,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5.2-pro",
        name: "GPT-5.2 Pro",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED_XHIGH),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 21.0,
            output: 168.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 400_000,
        max_tokens: 128_000,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5.3-chat-latest",
        name: "GPT-5.3 Chat (latest)",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: false,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED_XHIGH),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 16_384,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5.3-codex",
        name: "GPT-5.3 Codex",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED_XHIGH),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 400_000,
        max_tokens: 128_000,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5.3-codex-spark",
        name: "GPT-5.3 Codex Spark",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED_XHIGH),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 32_000,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5.4",
        name: "GPT-5.4",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED_XHIGH),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 2.5,
            output: 15.0,
            cache_read: 0.25,
            cache_write: 0.0,
        },
        context_window: 1_050_000,
        max_tokens: 128_000,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5.4-mini",
        name: "GPT-5.4 mini",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED_XHIGH),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 0.75,
            output: 4.5,
            cache_read: 0.075,
            cache_write: 0.0,
        },
        context_window: 400_000,
        max_tokens: 128_000,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5.4-nano",
        name: "GPT-5.4 nano",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED_XHIGH),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 0.2,
            output: 1.25,
            cache_read: 0.02,
            cache_write: 0.0,
        },
        context_window: 400_000,
        max_tokens: 128_000,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5.4-pro",
        name: "GPT-5.4 Pro",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED_XHIGH),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 30.0,
            output: 180.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_050_000,
        max_tokens: 128_000,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5.5",
        name: "GPT-5.5",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_UNSUPPORTED_XHIGH),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 5.0,
            output: 30.0,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        context_window: 1_050_000,
        max_tokens: 128_000,
    },
    AzureOpenAIResponsesModel {
        id: "gpt-5.5-pro",
        name: "GPT-5.5 Pro",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: Some(THINKING_OFF_MINIMAL_LOW_UNSUPPORTED_XHIGH),
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 30.0,
            output: 180.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_050_000,
        max_tokens: 128_000,
    },
    AzureOpenAIResponsesModel {
        id: "o1",
        name: "o1",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 15.0,
            output: 60.0,
            cache_read: 7.5,
            cache_write: 0.0,
        },
        context_window: 200_000,
        max_tokens: 100_000,
    },
    AzureOpenAIResponsesModel {
        id: "o1-pro",
        name: "o1-pro",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 150.0,
            output: 600.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 200_000,
        max_tokens: 100_000,
    },
    AzureOpenAIResponsesModel {
        id: "o3",
        name: "o3",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 2.0,
            output: 8.0,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        context_window: 200_000,
        max_tokens: 100_000,
    },
    AzureOpenAIResponsesModel {
        id: "o3-deep-research",
        name: "o3-deep-research",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 10.0,
            output: 40.0,
            cache_read: 2.5,
            cache_write: 0.0,
        },
        context_window: 200_000,
        max_tokens: 100_000,
    },
    AzureOpenAIResponsesModel {
        id: "o3-mini",
        name: "o3-mini",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: None,
        input: TEXT,
        cost: AzureOpenAIResponsesModelCost {
            input: 1.1,
            output: 4.4,
            cache_read: 0.55,
            cache_write: 0.0,
        },
        context_window: 200_000,
        max_tokens: 100_000,
    },
    AzureOpenAIResponsesModel {
        id: "o3-pro",
        name: "o3-pro",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 20.0,
            output: 80.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 200_000,
        max_tokens: 100_000,
    },
    AzureOpenAIResponsesModel {
        id: "o4-mini",
        name: "o4-mini",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 1.1,
            output: 4.4,
            cache_read: 0.275,
            cache_write: 0.0,
        },
        context_window: 200_000,
        max_tokens: 100_000,
    },
    AzureOpenAIResponsesModel {
        id: "o4-mini-deep-research",
        name: "o4-mini-deep-research",
        api: "azure-openai-responses",
        provider: "azure-openai-responses",
        base_url: "",
        reasoning: true,
        thinking_level_map: None,
        input: TEXT_IMAGE,
        cost: AzureOpenAIResponsesModelCost {
            input: 2.0,
            output: 8.0,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        context_window: 200_000,
        max_tokens: 100_000,
    },
];

/// Returns the Azure OpenAI Responses catalog as the crate's current minimal runtime model shape.
#[must_use]
pub fn azure_openai_responses_models() -> Vec<Model> {
    AZURE_OPENAI_RESPONSES_MODELS
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
    fn preserves_azure_openai_responses_catalog_metadata() {
        assert_eq!(AZURE_OPENAI_RESPONSES_MODELS.len(), 42);
        assert_eq!(AZURE_OPENAI_RESPONSES_MODELS[0].id, "gpt-4");

        let pro = AZURE_OPENAI_RESPONSES_MODELS
            .iter()
            .find(|model| model.id == "gpt-5.5-pro")
            .expect("gpt-5.5-pro is present");
        assert_eq!(pro.name, "GPT-5.5 Pro");
        assert_eq!(pro.context_window, 1_050_000);
        assert_eq!(pro.max_tokens, 128_000);
        assert_eq!(
            pro.thinking_level_map.expect("thinking map"),
            THINKING_OFF_MINIMAL_LOW_UNSUPPORTED_XHIGH,
        );
    }

    #[test]
    fn exposes_current_runtime_model_shape() {
        let models = azure_openai_responses_models();
        assert_eq!(models.len(), AZURE_OPENAI_RESPONSES_MODELS.len());
        assert!(
            models
                .iter()
                .all(|model| model.provider == "azure-openai-responses")
        );
        assert!(
            models
                .iter()
                .all(|model| model.api == "azure-openai-responses")
        );
    }
}
