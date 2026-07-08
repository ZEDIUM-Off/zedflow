//! NVIDIA model catalog ported from Pi's `packages/ai/src/providers/nvidia.models.ts`.

/// Pricing metadata for an NVIDIA model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NvidiaModelCost {
    /// Input token price in Pi's model catalog units.
    pub input: f64,
    /// Output token price in Pi's model catalog units.
    pub output: f64,
    /// Cache-read token price in Pi's model catalog units.
    pub cache_read: f64,
    /// Cache-write token price in Pi's model catalog units.
    pub cache_write: f64,
}

/// OpenAI-compatible feature flags for an NVIDIA model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NvidiaModelCompat {
    /// Whether the model supports the OpenAI store option.
    pub supports_store: bool,
    /// Whether the model supports the developer role.
    pub supports_developer_role: bool,
    /// Whether the model supports the reasoning-effort parameter.
    pub supports_reasoning_effort: bool,
    /// Name of the field Pi uses for the maximum output token limit.
    pub max_tokens_field: &'static str,
    /// Whether the model supports strict JSON schema mode.
    pub supports_strict_mode: bool,
    /// Whether the model supports long prompt-cache retention.
    pub supports_long_cache_retention: bool,
}

/// Request headers from Pi's NVIDIA model catalog.
pub type NvidiaHeaders = &'static [(&'static str, &'static str)];

/// One NVIDIA model entry from Pi's generated model catalog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NvidiaModel {
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
    /// Request headers.
    pub headers: NvidiaHeaders,
    /// OpenAI-compatible feature flags.
    pub compat: NvidiaModelCompat,
    /// Whether the model supports reasoning.
    pub reasoning: bool,
    /// Supported input modalities.
    pub input: &'static [&'static str],
    /// Model cost metadata.
    pub cost: NvidiaModelCost,
    /// Context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
}

const TEXT: &[&str] = &["text"];
const TEXT_IMAGE: &[&str] = &["text", "image"];
const NVIDIA_HEADERS: NvidiaHeaders = &[("NVCF-POLL-SECONDS", "3600")];
const NVIDIA_COMPAT: NvidiaModelCompat = NvidiaModelCompat {
    supports_store: false,
    supports_developer_role: false,
    supports_reasoning_effort: false,
    max_tokens_field: "max_tokens",
    supports_strict_mode: false,
    supports_long_cache_retention: false,
};

/// NVIDIA models keyed by `id`, preserving Pi's generated catalog order and metadata.
pub const NVIDIA_MODELS: &[NvidiaModel] = &[
    NvidiaModel {
        id: "meta/llama-3.1-70b-instruct",
        name: "Llama 3.1 70b Instruct",
        api: "openai-completions",
        provider: "nvidia",
        base_url: "https://integrate.api.nvidia.com/v1",
        headers: NVIDIA_HEADERS,
        compat: NVIDIA_COMPAT,
        reasoning: false,
        input: TEXT,
        cost: NvidiaModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 4_096,
    },
    NvidiaModel {
        id: "meta/llama-3.1-8b-instruct",
        name: "Llama 3.1 8B Instruct",
        api: "openai-completions",
        provider: "nvidia",
        base_url: "https://integrate.api.nvidia.com/v1",
        headers: NVIDIA_HEADERS,
        compat: NVIDIA_COMPAT,
        reasoning: false,
        input: TEXT,
        cost: NvidiaModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 16_000,
        max_tokens: 4_096,
    },
    NvidiaModel {
        id: "meta/llama-3.2-11b-vision-instruct",
        name: "Llama 3.2 11b Vision Instruct",
        api: "openai-completions",
        provider: "nvidia",
        base_url: "https://integrate.api.nvidia.com/v1",
        headers: NVIDIA_HEADERS,
        compat: NVIDIA_COMPAT,
        reasoning: false,
        input: TEXT_IMAGE,
        cost: NvidiaModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 4_096,
    },
    NvidiaModel {
        id: "meta/llama-3.2-90b-vision-instruct",
        name: "Llama-3.2-90B-Vision-Instruct",
        api: "openai-completions",
        provider: "nvidia",
        base_url: "https://integrate.api.nvidia.com/v1",
        headers: NVIDIA_HEADERS,
        compat: NVIDIA_COMPAT,
        reasoning: false,
        input: TEXT_IMAGE,
        cost: NvidiaModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 8_192,
    },
    NvidiaModel {
        id: "meta/llama-3.3-70b-instruct",
        name: "Llama 3.3 70b Instruct",
        api: "openai-completions",
        provider: "nvidia",
        base_url: "https://integrate.api.nvidia.com/v1",
        headers: NVIDIA_HEADERS,
        compat: NVIDIA_COMPAT,
        reasoning: false,
        input: TEXT,
        cost: NvidiaModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 4_096,
    },
    NvidiaModel {
        id: "minimaxai/minimax-m3",
        name: "MiniMax-M3",
        api: "openai-completions",
        provider: "nvidia",
        base_url: "https://integrate.api.nvidia.com/v1",
        headers: NVIDIA_HEADERS,
        compat: NVIDIA_COMPAT,
        reasoning: true,
        input: TEXT_IMAGE,
        cost: NvidiaModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_000_000,
        max_tokens: 16_384,
    },
    NvidiaModel {
        id: "mistralai/mistral-large-3-675b-instruct-2512",
        name: "Mistral Large 3 675B Instruct 2512",
        api: "openai-completions",
        provider: "nvidia",
        base_url: "https://integrate.api.nvidia.com/v1",
        headers: NVIDIA_HEADERS,
        compat: NVIDIA_COMPAT,
        reasoning: false,
        input: TEXT_IMAGE,
        cost: NvidiaModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 262_144,
    },
    NvidiaModel {
        id: "mistralai/mistral-small-4-119b-2603",
        name: "mistral-small-4-119b-2603",
        api: "openai-completions",
        provider: "nvidia",
        base_url: "https://integrate.api.nvidia.com/v1",
        headers: NVIDIA_HEADERS,
        compat: NVIDIA_COMPAT,
        reasoning: true,
        input: TEXT_IMAGE,
        cost: NvidiaModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 8_192,
    },
    NvidiaModel {
        id: "moonshotai/kimi-k2.6",
        name: "Kimi K2.6",
        api: "openai-completions",
        provider: "nvidia",
        base_url: "https://integrate.api.nvidia.com/v1",
        headers: NVIDIA_HEADERS,
        compat: NVIDIA_COMPAT,
        reasoning: true,
        input: TEXT_IMAGE,
        cost: NvidiaModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 262_144,
    },
    NvidiaModel {
        id: "nvidia/nemotron-3-nano-30b-a3b",
        name: "nemotron-3-nano-30b-a3b",
        api: "openai-completions",
        provider: "nvidia",
        base_url: "https://integrate.api.nvidia.com/v1",
        headers: NVIDIA_HEADERS,
        compat: NVIDIA_COMPAT,
        reasoning: true,
        input: TEXT,
        cost: NvidiaModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072,
        max_tokens: 131_072,
    },
    NvidiaModel {
        id: "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning",
        name: "Nemotron 3 Nano Omni",
        api: "openai-completions",
        provider: "nvidia",
        base_url: "https://integrate.api.nvidia.com/v1",
        headers: NVIDIA_HEADERS,
        compat: NVIDIA_COMPAT,
        reasoning: true,
        input: TEXT_IMAGE,
        cost: NvidiaModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000,
        max_tokens: 65_536,
    },
    NvidiaModel {
        id: "nvidia/nemotron-3-super-120b-a12b",
        name: "Nemotron 3 Super",
        api: "openai-completions",
        provider: "nvidia",
        base_url: "https://integrate.api.nvidia.com/v1",
        headers: NVIDIA_HEADERS,
        compat: NVIDIA_COMPAT,
        reasoning: true,
        input: TEXT,
        cost: NvidiaModelCost {
            input: 0.2,
            output: 0.8,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 262_144,
    },
    NvidiaModel {
        id: "nvidia/nemotron-3-ultra-550b-a55b",
        name: "Nemotron 3 Ultra 550B A55B",
        api: "openai-completions",
        provider: "nvidia",
        base_url: "https://integrate.api.nvidia.com/v1",
        headers: NVIDIA_HEADERS,
        compat: NVIDIA_COMPAT,
        reasoning: true,
        input: TEXT,
        cost: NvidiaModelCost {
            input: 0.5,
            output: 2.5,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 1_000_000,
        max_tokens: 65_536,
    },
    NvidiaModel {
        id: "nvidia/nvidia-nemotron-nano-9b-v2",
        name: "nvidia-nemotron-nano-9b-v2",
        api: "openai-completions",
        provider: "nvidia",
        base_url: "https://integrate.api.nvidia.com/v1",
        headers: NVIDIA_HEADERS,
        compat: NVIDIA_COMPAT,
        reasoning: true,
        input: TEXT,
        cost: NvidiaModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072,
        max_tokens: 131_072,
    },
    NvidiaModel {
        id: "openai/gpt-oss-120b",
        name: "GPT-OSS-120B",
        api: "openai-completions",
        provider: "nvidia",
        base_url: "https://integrate.api.nvidia.com/v1",
        headers: NVIDIA_HEADERS,
        compat: NVIDIA_COMPAT,
        reasoning: true,
        input: TEXT,
        cost: NvidiaModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 8_192,
    },
    NvidiaModel {
        id: "openai/gpt-oss-20b",
        name: "GPT OSS 20B",
        api: "openai-completions",
        provider: "nvidia",
        base_url: "https://integrate.api.nvidia.com/v1",
        headers: NVIDIA_HEADERS,
        compat: NVIDIA_COMPAT,
        reasoning: true,
        input: TEXT,
        cost: NvidiaModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072,
        max_tokens: 32_768,
    },
    NvidiaModel {
        id: "qwen/qwen3.5-122b-a10b",
        name: "Qwen3.5 122B-A10B",
        api: "openai-completions",
        provider: "nvidia",
        base_url: "https://integrate.api.nvidia.com/v1",
        headers: NVIDIA_HEADERS,
        compat: NVIDIA_COMPAT,
        reasoning: true,
        input: TEXT_IMAGE,
        cost: NvidiaModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 65_536,
    },
    NvidiaModel {
        id: "stepfun-ai/step-3.5-flash",
        name: "Step 3.5 Flash",
        api: "openai-completions",
        provider: "nvidia",
        base_url: "https://integrate.api.nvidia.com/v1",
        headers: NVIDIA_HEADERS,
        compat: NVIDIA_COMPAT,
        reasoning: true,
        input: TEXT,
        cost: NvidiaModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000,
        max_tokens: 16_384,
    },
    NvidiaModel {
        id: "stepfun-ai/step-3.7-flash",
        name: "Step 3.7 Flash",
        api: "openai-completions",
        provider: "nvidia",
        base_url: "https://integrate.api.nvidia.com/v1",
        headers: NVIDIA_HEADERS,
        compat: NVIDIA_COMPAT,
        reasoning: true,
        input: TEXT_IMAGE,
        cost: NvidiaModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000,
        max_tokens: 16_384,
    },
    NvidiaModel {
        id: "z-ai/glm-5.2",
        name: "GLM-5.2",
        api: "openai-completions",
        provider: "nvidia",
        base_url: "https://integrate.api.nvidia.com/v1",
        headers: NVIDIA_HEADERS,
        compat: NVIDIA_COMPAT,
        reasoning: true,
        input: TEXT,
        cost: NvidiaModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_000_000,
        max_tokens: 131_072,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_nvidia_catalog_metadata() {
        assert_eq!(NVIDIA_MODELS.len(), 20);
        assert_eq!(NVIDIA_MODELS[0].id, "meta/llama-3.1-70b-instruct");
        assert_eq!(NVIDIA_MODELS[0].headers, NVIDIA_HEADERS);
        assert_eq!(NVIDIA_MODELS[0].compat, NVIDIA_COMPAT);
        assert_eq!(NVIDIA_MODELS[5].input, TEXT_IMAGE);
        assert_eq!(NVIDIA_MODELS[12].cost.cache_read, 0.15);
        assert_eq!(NVIDIA_MODELS[19].id, "z-ai/glm-5.2");
        assert_eq!(NVIDIA_MODELS[19].context_window, 1_000_000);
        assert_eq!(NVIDIA_MODELS[19].max_tokens, 131_072);
    }
}
