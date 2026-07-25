#![forbid(unsafe_code)]

//! Zedflow ai crate.

/// Error and placeholder conventions for the Pi AI package port.
pub mod error;

/// CLI helpers ported from Pi's `packages/ai/src/cli.ts`.
pub mod cli;

#[path = "bedrock-provider.rs"]
pub mod bedrock_provider;

#[path = "env-api-keys.rs"]
pub mod env_api_keys;

/// API modules ported from Pi's `packages/ai/src/api` tree.
pub mod api {
    #[path = "anthropic-messages.rs"]
    pub mod anthropic_messages;
    #[path = "anthropic-messages.lazy.rs"]
    pub mod anthropic_messages_lazy;
    #[path = "azure-openai-responses.rs"]
    pub mod azure_openai_responses;
    #[path = "azure-openai-responses.lazy.rs"]
    pub mod azure_openai_responses_lazy;
    #[path = "bedrock-converse-stream.rs"]
    pub mod bedrock_converse_stream;
    #[path = "bedrock-converse-stream.lazy.rs"]
    pub mod bedrock_converse_stream_lazy;
    pub mod cloudflare;
    #[path = "github-copilot-headers.rs"]
    pub mod github_copilot_headers;
    #[path = "google-generative-ai.rs"]
    pub mod google_generative_ai;
    #[path = "google-generative-ai.lazy.rs"]
    pub mod google_generative_ai_lazy;
    #[path = "google-shared.rs"]
    pub mod google_shared;
    #[path = "google-vertex.rs"]
    pub mod google_vertex;
    #[path = "google-vertex.lazy.rs"]
    pub mod google_vertex_lazy;
    pub mod lazy;
    #[path = "mistral-conversations.rs"]
    pub mod mistral_conversations;
    #[path = "mistral-conversations.lazy.rs"]
    pub mod mistral_conversations_lazy;
    #[path = "openai-codex-responses.rs"]
    pub mod openai_codex_responses;
    #[path = "openai-codex-responses.lazy.rs"]
    pub mod openai_codex_responses_lazy;
    #[path = "openai-completions.rs"]
    pub mod openai_completions;
    #[path = "openai-completions.lazy.rs"]
    pub mod openai_completions_lazy;
    #[path = "openai-prompt-cache.rs"]
    pub mod openai_prompt_cache;
    #[path = "openai-responses.rs"]
    pub mod openai_responses;
    #[path = "openai-responses.lazy.rs"]
    pub mod openai_responses_lazy;
    #[path = "openai-responses-shared.rs"]
    pub mod openai_responses_shared;
    #[path = "openrouter-images.rs"]
    pub mod openrouter_images;
    #[path = "openrouter-images.lazy.rs"]
    pub mod openrouter_images_lazy;
    #[path = "simple-options.rs"]
    pub mod simple_options;
    #[path = "transform-messages.rs"]
    pub mod transform_messages;
}

/// Auth modules ported from Pi's `packages/ai/src/auth` tree.
pub mod auth {
    pub mod context;
    #[path = "credential-store.rs"]
    pub mod credential_store;
    pub mod helpers;
    pub mod resolve;
    pub mod types;
}

/// Compatibility entrypoint ported from Pi's `packages/ai/src/compat.ts`.
pub mod compat;

#[path = "image-models.generated.rs"]
/// Generated image model catalog ported from Pi's `packages/ai/src/image-models.generated.ts`.
pub mod image_models_generated;

#[path = "image-models.rs"]
/// Image model registry helpers ported from Pi's `packages/ai/src/image-models.ts`.
pub mod image_models;

#[path = "images-api-registry.rs"]
/// Image API provider registry ported from Pi's `packages/ai/src/images-api-registry.ts`.
pub mod images_api_registry;

#[path = "images-models.rs"]
/// Image provider collection helpers ported from Pi's `packages/ai/src/images-models.ts`.
pub mod images_models;

/// Image generation entrypoint ported from Pi's `packages/ai/src/images.ts`.
pub mod images;

/// Core package entrypoint marker ported from Pi's `packages/ai/src/index.ts`.
pub mod index;

/// Utility modules ported from Pi's `packages/ai/src/utils` tree.
pub mod utils {
    #[path = "abort-signals.rs"]
    pub mod abort_signals;
    /// Assistant-message diagnostic helpers ported from Pi's `packages/ai/src/utils/diagnostics.ts`.
    pub mod diagnostics;
    /// Error-body helpers ported from Pi's `packages/ai/src/utils/error-body.ts`.
    #[path = "error-body.rs"]
    pub mod error_body;
    /// Context token estimation ported from Pi's `packages/ai/src/utils/estimate.ts`.
    pub mod estimate;
    /// Event stream helpers ported from Pi's `packages/ai/src/utils/event-stream.ts`.
    #[path = "event-stream.rs"]
    pub mod event_stream;
    /// Deterministic short hashing ported from Pi's `packages/ai/src/utils/hash.ts`.
    pub mod hash;
    /// HTTP header conversion helpers ported from Pi's `packages/ai/src/utils/headers.ts`.
    pub mod headers;
    /// JSON repair and streaming parse helpers ported from Pi's `packages/ai/src/utils/json-parse.ts`.
    #[path = "json-parse.rs"]
    pub mod json_parse;
    /// Node-style HTTP proxy environment resolution ported from Pi's `packages/ai/src/utils/node-http-proxy.ts`.
    #[path = "node-http-proxy.rs"]
    pub mod node_http_proxy;
    /// Context-overflow detection ported from Pi's `packages/ai/src/utils/overflow.ts`.
    pub mod overflow;
    /// Provider environment lookup ported from Pi's `packages/ai/src/utils/provider-env.ts`.
    #[path = "provider-env.rs"]
    pub mod provider_env;
    /// Retry classification helpers ported from Pi's `packages/ai/src/utils/retry.ts`.
    pub mod retry;
    /// Crate-private Tokio worker support for synchronous stream entrypoints.
    pub(crate) mod runtime;
    /// Unicode sanitization helpers ported from Pi's `packages/ai/src/utils/sanitize-unicode.ts`.
    #[path = "sanitize-unicode.rs"]
    pub mod sanitize_unicode;
    /// TypeBox helper schemas ported from Pi's `packages/ai/src/utils/typebox-helpers.ts`.
    #[path = "typebox-helpers.rs"]
    pub mod typebox_helpers;
    /// Tool argument validation helpers ported from Pi's `packages/ai/src/utils/validation.ts`.
    pub mod validation;
    /// OAuth helpers ported from Pi's `packages/ai/src/utils/oauth` tree.
    pub mod oauth {
        /// Anthropic OAuth flow ported from Pi's `packages/ai/src/utils/oauth/anthropic.ts`.
        pub mod anthropic;
        #[path = "device-code.rs"]
        /// OAuth device-code polling ported from Pi's `packages/ai/src/utils/oauth/device-code.ts`.
        pub mod device_code;
        #[path = "github-copilot.rs"]
        /// GitHub Copilot OAuth flow ported from Pi's `packages/ai/src/utils/oauth/github-copilot.ts`.
        pub mod github_copilot;
        /// OAuth registry entrypoint ported from Pi's `packages/ai/src/utils/oauth/index.ts`.
        pub mod index;
        /// Lazy OAuth module loaders ported from Pi's `packages/ai/src/utils/oauth/load.ts`.
        pub mod load;
        #[path = "oauth-page.rs"]
        /// OAuth callback HTML pages ported from Pi's `packages/ai/src/utils/oauth/oauth-page.ts`.
        pub mod oauth_page;
        #[path = "openai-codex.rs"]
        /// OpenAI Codex OAuth flow ported from Pi's `packages/ai/src/utils/oauth/openai-codex.ts`.
        pub mod openai_codex;
        /// PKCE helpers ported from Pi's `packages/ai/src/utils/oauth/pkce.ts`.
        pub mod pkce;
        /// Shared OAuth type definitions ported from Pi's `packages/ai/src/utils/oauth/types.ts`.
        pub mod types;
    }
}

#[path = "legacy-api-aliases.rs"]
/// Deprecated stream aliases ported from Pi's `packages/ai/src/legacy-api-aliases.ts`.
pub mod legacy_api_aliases;

#[path = "models.generated.rs"]
/// Generated chat model catalog keys ported from Pi's `packages/ai/src/models.generated.ts`.
pub mod models_generated;

/// Runtime chat model collection ported from Pi's `packages/ai/src/models.ts`.
pub mod models;

/// OAuth public entrypoint ported from Pi's `packages/ai/src/oauth.ts`.
pub mod oauth;

/// Shared AI types ported from Pi's `packages/ai/src/types.ts`.
pub mod types;

/// Session-scoped cleanup registry ported from Pi's `packages/ai/src/session-resources.ts`.
#[path = "session-resources.rs"]
pub mod session_resources;

/// Provider helpers ported from Pi's `packages/ai/src/providers` tree.
pub mod providers {
    pub mod all;
    #[path = "amazon-bedrock.rs"]
    pub mod amazon_bedrock;
    #[path = "amazon-bedrock.models.rs"]
    pub mod amazon_bedrock_models;
    #[path = "ant-ling.rs"]
    pub mod ant_ling;
    #[path = "ant-ling.models.rs"]
    pub mod ant_ling_models;
    pub mod anthropic;
    #[path = "anthropic.models.rs"]
    pub mod anthropic_models;
    #[path = "azure-openai-responses.rs"]
    pub mod azure_openai_responses;
    #[path = "azure-openai-responses.models.rs"]
    pub mod azure_openai_responses_models;
    /// Cerebras provider factory ported from Pi's `packages/ai/src/providers/cerebras.ts`.
    pub mod cerebras;
    #[path = "cerebras.models.rs"]
    /// Cerebras model catalog ported from Pi's `packages/ai/src/providers/cerebras.models.ts`.
    pub mod cerebras_models;
    #[path = "cloudflare-ai-gateway.rs"]
    /// Cloudflare AI Gateway provider factory ported from Pi's `packages/ai/src/providers/cloudflare-ai-gateway.ts`.
    pub mod cloudflare_ai_gateway;
    #[path = "cloudflare-ai-gateway.models.rs"]
    /// Cloudflare AI Gateway model catalog ported from Pi's `packages/ai/src/providers/cloudflare-ai-gateway.models.ts`.
    pub mod cloudflare_ai_gateway_models;
    #[path = "cloudflare-auth.rs"]
    /// Cloudflare auth helpers ported from Pi's `packages/ai/src/providers/cloudflare-auth.ts`.
    pub mod cloudflare_auth;
    #[path = "cloudflare-workers-ai.rs"]
    /// Cloudflare Workers AI provider factory ported from Pi's `packages/ai/src/providers/cloudflare-workers-ai.ts`.
    pub mod cloudflare_workers_ai;
    #[path = "cloudflare-workers-ai.models.rs"]
    /// Cloudflare Workers AI model catalog ported from Pi's `packages/ai/src/providers/cloudflare-workers-ai.models.ts`.
    pub mod cloudflare_workers_ai_models;
    /// DeepSeek provider factory ported from Pi's `packages/ai/src/providers/deepseek.ts`.
    pub mod deepseek;
    #[path = "deepseek.models.rs"]
    /// DeepSeek model catalog ported from Pi's `packages/ai/src/providers/deepseek.models.ts`.
    pub mod deepseek_models;
    /// Faux provider helpers ported from Pi's `packages/ai/src/providers/faux.ts`.
    pub mod faux;
    /// Fireworks provider factory ported from Pi's `packages/ai/src/providers/fireworks.ts`.
    pub mod fireworks;
    #[path = "fireworks.models.rs"]
    /// Fireworks model catalog ported from Pi's `packages/ai/src/providers/fireworks.models.ts`.
    pub mod fireworks_models;
    #[path = "github-copilot.rs"]
    /// GitHub Copilot provider factory ported from Pi's `packages/ai/src/providers/github-copilot.ts`.
    pub mod github_copilot;
    #[path = "github-copilot.models.rs"]
    /// GitHub Copilot model catalog ported from Pi's `packages/ai/src/providers/github-copilot.models.ts`.
    pub mod github_copilot_models;
    /// Google provider factory ported from Pi's `packages/ai/src/providers/google.ts`.
    pub mod google;
    #[path = "google.models.rs"]
    /// Google model catalog ported from Pi's `packages/ai/src/providers/google.models.ts`.
    pub mod google_models;
    #[path = "google-vertex.rs"]
    /// Google Vertex provider factory ported from Pi's `packages/ai/src/providers/google-vertex.ts`.
    pub mod google_vertex;
    #[path = "google-vertex.models.rs"]
    /// Google Vertex model catalog ported from Pi's `packages/ai/src/providers/google-vertex.models.ts`.
    pub mod google_vertex_models;
    /// Groq provider factory ported from Pi's `packages/ai/src/providers/groq.ts`.
    pub mod groq;
    #[path = "groq.models.rs"]
    /// Groq model catalog ported from Pi's `packages/ai/src/providers/groq.models.ts`.
    pub mod groq_models;
    /// Hugging Face provider factory ported from Pi's `packages/ai/src/providers/huggingface.ts`.
    pub mod huggingface;
    #[path = "huggingface.models.rs"]
    /// Hugging Face model catalog ported from Pi's `packages/ai/src/providers/huggingface.models.ts`.
    pub mod huggingface_models;
    /// Image provider registration helpers ported from Pi's `packages/ai/src/providers/images` tree.
    pub mod images {
        #[path = "register-builtins.rs"]
        /// Built-in image provider registration ported from Pi's `packages/ai/src/providers/images/register-builtins.ts`.
        pub mod register_builtins;
    }
    #[path = "kimi-coding.rs"]
    pub mod kimi_coding;
    #[path = "kimi-coding.models.rs"]
    pub mod kimi_coding_models;
    pub mod minimax;
    #[path = "minimax-cn.rs"]
    pub mod minimax_cn;
    #[path = "minimax-cn.models.rs"]
    pub mod minimax_cn_models;
    #[path = "minimax.models.rs"]
    /// MiniMax model catalog ported from Pi's `packages/ai/src/providers/minimax.models.ts`.
    pub mod minimax_models;
    pub mod mistral;
    #[path = "mistral.models.rs"]
    /// Mistral model catalog ported from Pi's `packages/ai/src/providers/mistral.models.ts`.
    pub mod mistral_models;
    pub mod moonshotai;
    #[path = "moonshotai-cn.rs"]
    pub mod moonshotai_cn;
    #[path = "moonshotai-cn.models.rs"]
    /// Moonshot AI CN model catalog ported from Pi's `packages/ai/src/providers/moonshotai-cn.models.ts`.
    pub mod moonshotai_cn_models;
    #[path = "moonshotai.models.rs"]
    /// Moonshot AI model catalog ported from Pi's `packages/ai/src/providers/moonshotai.models.ts`.
    pub mod moonshotai_models;
    /// NVIDIA provider factory ported from Pi's `packages/ai/src/providers/nvidia.ts`.
    pub mod nvidia;
    #[path = "nvidia.models.rs"]
    /// NVIDIA model catalog ported from Pi's `packages/ai/src/providers/nvidia.models.ts`.
    pub mod nvidia_models;
    /// OpenAI provider factory ported from Pi's `packages/ai/src/providers/openai.ts`.
    pub mod openai;
    #[path = "openai-codex.rs"]
    /// OpenAI Codex provider factory ported from Pi's `packages/ai/src/providers/openai-codex.ts`.
    pub mod openai_codex;
    #[path = "openai-codex.models.rs"]
    /// OpenAI Codex model catalog ported from Pi's `packages/ai/src/providers/openai-codex.models.ts`.
    pub mod openai_codex_models;
    #[path = "openai.models.rs"]
    /// OpenAI model catalog ported from Pi's `packages/ai/src/providers/openai.models.ts`.
    pub mod openai_models;
    /// OpenCode Zen provider factory ported from Pi's `packages/ai/src/providers/opencode.ts`.
    pub mod opencode;
    #[path = "opencode-go.rs"]
    /// OpenCode Zen Go provider factory ported from Pi's `packages/ai/src/providers/opencode-go.ts`.
    pub mod opencode_go;
    #[path = "opencode-go.models.rs"]
    /// OpenCode Zen Go model catalog ported from Pi's `packages/ai/src/providers/opencode-go.models.ts`.
    pub mod opencode_go_models;
    #[path = "opencode.models.rs"]
    /// OpenCode model catalog ported from Pi's `packages/ai/src/providers/opencode.models.ts`.
    pub mod opencode_models;
    /// OpenRouter provider factory ported from Pi's `packages/ai/src/providers/openrouter.ts`.
    pub mod openrouter;
    #[path = "openrouter-images.rs"]
    /// OpenRouter image provider factory ported from Pi's `packages/ai/src/providers/openrouter-images.ts`.
    pub mod openrouter_images;
    #[path = "openrouter.models.rs"]
    /// OpenRouter model catalog ported from Pi's `packages/ai/src/providers/openrouter.models.ts`.
    pub mod openrouter_models;
    pub(crate) mod static_catalog;
    /// Together provider factory ported from Pi's `packages/ai/src/providers/together.ts`.
    pub mod together;
    #[path = "together.models.rs"]
    /// Together model catalog ported from Pi's `packages/ai/src/providers/together.models.ts`.
    pub mod together_models;
    #[path = "vercel-ai-gateway.rs"]
    /// Vercel AI Gateway provider factory ported from Pi's `packages/ai/src/providers/vercel-ai-gateway.ts`.
    pub mod vercel_ai_gateway;
    #[path = "vercel-ai-gateway.models.rs"]
    /// Vercel AI Gateway model catalog ported from Pi's `packages/ai/src/providers/vercel-ai-gateway.models.ts`.
    pub mod vercel_ai_gateway_models;
    /// xAI provider factory ported from Pi's `packages/ai/src/providers/xai.ts`.
    pub mod xai;
    #[path = "xai.models.rs"]
    /// xAI model catalog ported from Pi's `packages/ai/src/providers/xai.models.ts`.
    pub mod xai_models;
    /// Xiaomi provider factory ported from Pi's `packages/ai/src/providers/xiaomi.ts`.
    pub mod xiaomi;
    #[path = "xiaomi.models.rs"]
    /// Xiaomi model catalog ported from Pi's `packages/ai/src/providers/xiaomi.models.ts`.
    pub mod xiaomi_models;
    #[path = "xiaomi-token-plan-ams.rs"]
    /// Xiaomi Token Plan AMS provider factory ported from Pi's `packages/ai/src/providers/xiaomi-token-plan-ams.ts`.
    pub mod xiaomi_token_plan_ams;
    #[path = "xiaomi-token-plan-ams.models.rs"]
    /// Xiaomi Token Plan AMS model catalog ported from Pi's `packages/ai/src/providers/xiaomi-token-plan-ams.models.ts`.
    pub mod xiaomi_token_plan_ams_models;
    #[path = "xiaomi-token-plan-cn.rs"]
    /// Xiaomi Token Plan CN provider factory ported from Pi's `packages/ai/src/providers/xiaomi-token-plan-cn.ts`.
    pub mod xiaomi_token_plan_cn;
    #[path = "xiaomi-token-plan-cn.models.rs"]
    /// Xiaomi Token Plan CN model catalog ported from Pi's `packages/ai/src/providers/xiaomi-token-plan-cn.models.ts`.
    pub mod xiaomi_token_plan_cn_models;
    #[path = "xiaomi-token-plan-sgp.rs"]
    /// Xiaomi Token Plan SGP provider factory ported from Pi's `packages/ai/src/providers/xiaomi-token-plan-sgp.ts`.
    pub mod xiaomi_token_plan_sgp;
    #[path = "xiaomi-token-plan-sgp.models.rs"]
    /// Xiaomi Token Plan SGP model catalog ported from Pi's `packages/ai/src/providers/xiaomi-token-plan-sgp.models.ts`.
    pub mod xiaomi_token_plan_sgp_models;
    /// Z.AI provider factory ported from Pi's `packages/ai/src/providers/zai.ts`.
    pub mod zai;
    #[path = "zai-coding-cn.rs"]
    /// Z.AI Coding CN provider factory ported from Pi's `packages/ai/src/providers/zai-coding-cn.ts`.
    pub mod zai_coding_cn;
    #[path = "zai-coding-cn.models.rs"]
    /// Z.AI Coding CN model catalog ported from Pi's `packages/ai/src/providers/zai-coding-cn.models.ts`.
    pub mod zai_coding_cn_models;
    #[path = "zai.models.rs"]
    /// Z.AI model catalog ported from Pi's `packages/ai/src/providers/zai.models.ts`.
    pub mod zai_models;
}

/// Pi-compatible root facade exports from `packages/ai/src/index.ts`.
pub use crate::index::*;

/// Crate identity, useful while the clean workspace skeleton is being filled.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
