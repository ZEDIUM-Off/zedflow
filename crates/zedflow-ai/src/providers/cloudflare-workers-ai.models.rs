//! Cloudflare Workers AI model catalog ported from Pi's `packages/ai/src/providers/cloudflare-workers-ai.models.ts`.

/// Pricing metadata for a Cloudflare Workers AI model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudflareWorkersAiModelCost {
    /// Input token price in Pi's model catalog units.
    pub input: f64,
    /// Output token price in Pi's model catalog units.
    pub output: f64,
    /// Cache-read token price in Pi's model catalog units.
    pub cache_read: f64,
    /// Cache-write token price in Pi's model catalog units.
    pub cache_write: f64,
}

/// Compatibility metadata for a Cloudflare Workers AI model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CloudflareWorkersAiModelCompat {
    /// Whether the model supports the OpenAI store option.
    pub supports_store: bool,
    /// Whether the model supports the developer role.
    pub supports_developer_role: bool,
    /// Whether the model supports long cache retention.
    pub supports_long_cache_retention: bool,
    /// Whether Pi sends session affinity headers for the model.
    pub send_session_affinity_headers: bool,
}

/// One Cloudflare Workers AI model entry from Pi's generated model catalog.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudflareWorkersAiModel {
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
    /// Compatibility flags.
    pub compat: CloudflareWorkersAiModelCompat,
    /// Whether the model supports reasoning.
    pub reasoning: bool,
    /// Supported input modalities.
    pub input: &'static [&'static str],
    /// Model cost metadata.
    pub cost: CloudflareWorkersAiModelCost,
    /// Context window in tokens.
    pub context_window: u32,
    /// Maximum output tokens.
    pub max_tokens: u32,
}

const TEXT: &[&str] = &["text"];
const TEXT_IMAGE: &[&str] = &["text", "image"];
const CLOUDFLARE_WORKERS_AI_COMPAT: CloudflareWorkersAiModelCompat =
    CloudflareWorkersAiModelCompat {
        supports_store: false,
        supports_developer_role: false,
        supports_long_cache_retention: false,
        send_session_affinity_headers: true,
    };

/// Cloudflare Workers AI models keyed by `id`, preserving Pi's generated catalog order and metadata.
pub const CLOUDFLARE_WORKERS_AI_MODELS: &[CloudflareWorkersAiModel] = &[
    CloudflareWorkersAiModel {
        id: "@cf/google/gemma-4-26b-a4b-it",
        name: "Gemma 4 26B A4B IT",
        api: "openai-completions",
        provider: "cloudflare-workers-ai",
        base_url: "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1",
        compat: CLOUDFLARE_WORKERS_AI_COMPAT,
        reasoning: true,
        input: TEXT_IMAGE,
        cost: CloudflareWorkersAiModelCost {
            input: 0.1,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000,
        max_tokens: 16_384,
    },
    CloudflareWorkersAiModel {
        id: "@cf/ibm-granite/granite-4.0-h-micro",
        name: "Granite 4.0 H Micro",
        api: "openai-completions",
        provider: "cloudflare-workers-ai",
        base_url: "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1",
        compat: CLOUDFLARE_WORKERS_AI_COMPAT,
        reasoning: false,
        input: TEXT,
        cost: CloudflareWorkersAiModelCost {
            input: 0.017,
            output: 0.112,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_000,
        max_tokens: 131_000,
    },
    CloudflareWorkersAiModel {
        id: "@cf/meta/llama-3.3-70b-instruct-fp8-fast",
        name: "Llama 3.3 70B Instruct fp8 Fast",
        api: "openai-completions",
        provider: "cloudflare-workers-ai",
        base_url: "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1",
        compat: CLOUDFLARE_WORKERS_AI_COMPAT,
        reasoning: false,
        input: TEXT,
        cost: CloudflareWorkersAiModelCost {
            input: 0.293,
            output: 2.253,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 24_000,
        max_tokens: 24_000,
    },
    CloudflareWorkersAiModel {
        id: "@cf/meta/llama-4-scout-17b-16e-instruct",
        name: "Llama 4 Scout 17B 16E Instruct",
        api: "openai-completions",
        provider: "cloudflare-workers-ai",
        base_url: "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1",
        compat: CLOUDFLARE_WORKERS_AI_COMPAT,
        reasoning: false,
        input: TEXT_IMAGE,
        cost: CloudflareWorkersAiModelCost {
            input: 0.27,
            output: 0.85,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_000,
        max_tokens: 16_384,
    },
    CloudflareWorkersAiModel {
        id: "@cf/mistralai/mistral-small-3.1-24b-instruct",
        name: "Mistral Small 3.1 24B Instruct",
        api: "openai-completions",
        provider: "cloudflare-workers-ai",
        base_url: "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1",
        compat: CLOUDFLARE_WORKERS_AI_COMPAT,
        reasoning: false,
        input: TEXT,
        cost: CloudflareWorkersAiModelCost {
            input: 0.351,
            output: 0.555,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 128_000,
    },
    CloudflareWorkersAiModel {
        id: "@cf/moonshotai/kimi-k2.6",
        name: "Kimi K2.6",
        api: "openai-completions",
        provider: "cloudflare-workers-ai",
        base_url: "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1",
        compat: CLOUDFLARE_WORKERS_AI_COMPAT,
        reasoning: true,
        input: TEXT_IMAGE,
        cost: CloudflareWorkersAiModelCost {
            input: 0.95,
            output: 4.0,
            cache_read: 0.16,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 256_000,
    },
    CloudflareWorkersAiModel {
        id: "@cf/moonshotai/kimi-k2.7-code",
        name: "Kimi K2.7 Code",
        api: "openai-completions",
        provider: "cloudflare-workers-ai",
        base_url: "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1",
        compat: CLOUDFLARE_WORKERS_AI_COMPAT,
        reasoning: true,
        input: TEXT_IMAGE,
        cost: CloudflareWorkersAiModelCost {
            input: 0.95,
            output: 4.0,
            cache_read: 0.19,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 262_144,
    },
    CloudflareWorkersAiModel {
        id: "@cf/nvidia/nemotron-3-120b-a12b",
        name: "Nemotron 3 Super 120B",
        api: "openai-completions",
        provider: "cloudflare-workers-ai",
        base_url: "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1",
        compat: CLOUDFLARE_WORKERS_AI_COMPAT,
        reasoning: true,
        input: TEXT,
        cost: CloudflareWorkersAiModelCost {
            input: 0.5,
            output: 1.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000,
        max_tokens: 256_000,
    },
    CloudflareWorkersAiModel {
        id: "@cf/openai/gpt-oss-120b",
        name: "GPT OSS 120B",
        api: "openai-completions",
        provider: "cloudflare-workers-ai",
        base_url: "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1",
        compat: CLOUDFLARE_WORKERS_AI_COMPAT,
        reasoning: true,
        input: TEXT,
        cost: CloudflareWorkersAiModelCost {
            input: 0.35,
            output: 0.75,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 16_384,
    },
    CloudflareWorkersAiModel {
        id: "@cf/openai/gpt-oss-20b",
        name: "GPT OSS 20B",
        api: "openai-completions",
        provider: "cloudflare-workers-ai",
        base_url: "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1",
        compat: CLOUDFLARE_WORKERS_AI_COMPAT,
        reasoning: true,
        input: TEXT,
        cost: CloudflareWorkersAiModelCost {
            input: 0.2,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000,
        max_tokens: 16_384,
    },
    CloudflareWorkersAiModel {
        id: "@cf/qwen/qwen3-30b-a3b-fp8",
        name: "Qwen3 30B A3b fp8",
        api: "openai-completions",
        provider: "cloudflare-workers-ai",
        base_url: "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1",
        compat: CLOUDFLARE_WORKERS_AI_COMPAT,
        reasoning: true,
        input: TEXT,
        cost: CloudflareWorkersAiModelCost {
            input: 0.0509,
            output: 0.335,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_768,
        max_tokens: 32_768,
    },
    CloudflareWorkersAiModel {
        id: "@cf/zai-org/glm-4.7-flash",
        name: "GLM-4.7-Flash",
        api: "openai-completions",
        provider: "cloudflare-workers-ai",
        base_url: "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1",
        compat: CLOUDFLARE_WORKERS_AI_COMPAT,
        reasoning: true,
        input: TEXT,
        cost: CloudflareWorkersAiModelCost {
            input: 0.0605,
            output: 0.4,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072,
        max_tokens: 131_072,
    },
    CloudflareWorkersAiModel {
        id: "@cf/zai-org/glm-5.2",
        name: "Glm 5.2",
        api: "openai-completions",
        provider: "cloudflare-workers-ai",
        base_url: "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1",
        compat: CLOUDFLARE_WORKERS_AI_COMPAT,
        reasoning: true,
        input: TEXT,
        cost: CloudflareWorkersAiModelCost {
            input: 1.4,
            output: 4.4,
            cache_read: 0.26,
            cache_write: 0.0,
        },
        context_window: 262_144,
        max_tokens: 262_144,
    },
];
