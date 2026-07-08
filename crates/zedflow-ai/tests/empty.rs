//! Port of Pi `packages/ai/test/empty.test.ts`.
//!
//! The source test exercises live providers through `compat::getModel` and `compat::complete`.
//! Keep these parity tests ignored until the Rust provider catalog, transports, credential
//! resolution, and OAuth token loading are implemented.

use zedflow_ai::api::lazy::{AssistantMessage, StopReason};

const BLOCKER: &str = "requires live provider transports, compat::get_model catalog parity, compat::complete dispatch, credentials, and OAuth token loading";

#[derive(Debug, Clone, Copy)]
struct ProviderEmptyCase {
    name: &'static str,
    provider: &'static str,
    model: &'static str,
    credential_gate: &'static str,
    options: &'static str,
}

const PROVIDER_EMPTY_CASES: &[ProviderEmptyCase] = &[
    ProviderEmptyCase {
        name: "Google Provider Empty Messages",
        provider: "google",
        model: "gemini-2.5-flash",
        credential_gate: "GEMINI_API_KEY",
        options: "default",
    },
    ProviderEmptyCase {
        name: "OpenAI Completions Provider Empty Messages",
        provider: "openai",
        model: "gpt-4o-mini",
        credential_gate: "OPENAI_API_KEY",
        options: "default",
    },
    ProviderEmptyCase {
        name: "OpenAI Responses Provider Empty Messages",
        provider: "openai",
        model: "gpt-5-mini",
        credential_gate: "OPENAI_API_KEY",
        options: "default",
    },
    ProviderEmptyCase {
        name: "Azure OpenAI Responses Provider Empty Messages",
        provider: "azure-openai-responses",
        model: "gpt-4o-mini",
        credential_gate: "hasAzureOpenAICredentials()",
        options: "azureDeploymentName when resolved",
    },
    ProviderEmptyCase {
        name: "Anthropic Provider Empty Messages",
        provider: "anthropic",
        model: "claude-haiku-4-5",
        credential_gate: "ANTHROPIC_API_KEY",
        options: "default",
    },
    ProviderEmptyCase {
        name: "xAI Provider Empty Messages",
        provider: "xai",
        model: "grok-3",
        credential_gate: "XAI_API_KEY",
        options: "default",
    },
    ProviderEmptyCase {
        name: "Groq Provider Empty Messages",
        provider: "groq",
        model: "openai/gpt-oss-20b",
        credential_gate: "GROQ_API_KEY",
        options: "default",
    },
    ProviderEmptyCase {
        name: "Cerebras Provider Empty Messages",
        provider: "cerebras",
        model: "gpt-oss-120b",
        credential_gate: "CEREBRAS_API_KEY",
        options: "default",
    },
    ProviderEmptyCase {
        name: "Cloudflare Workers AI Provider Empty Messages",
        provider: "cloudflare-workers-ai",
        model: "@cf/moonshotai/kimi-k2.6",
        credential_gate: "hasCloudflareWorkersAICredentials()",
        options: "default",
    },
    ProviderEmptyCase {
        name: "Cloudflare AI Gateway Provider Empty Messages",
        provider: "cloudflare-ai-gateway",
        model: "workers-ai/@cf/moonshotai/kimi-k2.6",
        credential_gate: "hasCloudflareAiGatewayCredentials()",
        options: "default",
    },
    ProviderEmptyCase {
        name: "Hugging Face Provider Empty Messages",
        provider: "huggingface",
        model: "moonshotai/Kimi-K2.5",
        credential_gate: "HF_TOKEN",
        options: "default",
    },
    ProviderEmptyCase {
        name: "Together AI Provider Empty Messages",
        provider: "together",
        model: "moonshotai/Kimi-K2.6",
        credential_gate: "TOGETHER_API_KEY",
        options: "default",
    },
    ProviderEmptyCase {
        name: "zAI Provider Empty Messages",
        provider: "zai",
        model: "glm-4.5-air",
        credential_gate: "ZAI_API_KEY",
        options: "default",
    },
    ProviderEmptyCase {
        name: "Mistral Provider Empty Messages",
        provider: "mistral",
        model: "devstral-medium-latest",
        credential_gate: "MISTRAL_API_KEY",
        options: "default",
    },
    ProviderEmptyCase {
        name: "MiniMax Provider Empty Messages",
        provider: "minimax",
        model: "MiniMax-M2.7",
        credential_gate: "MINIMAX_API_KEY",
        options: "default",
    },
    ProviderEmptyCase {
        name: "Xiaomi MiMo (API billing) Provider Empty Messages",
        provider: "xiaomi",
        model: "mimo-v2.5-pro",
        credential_gate: "XIAOMI_API_KEY",
        options: "default",
    },
    ProviderEmptyCase {
        name: "Xiaomi MiMo Token Plan (CN) Provider Empty Messages",
        provider: "xiaomi-token-plan-cn",
        model: "mimo-v2.5-pro",
        credential_gate: "XIAOMI_TOKEN_PLAN_CN_API_KEY",
        options: "default",
    },
    ProviderEmptyCase {
        name: "Xiaomi MiMo Token Plan (AMS) Provider Empty Messages",
        provider: "xiaomi-token-plan-ams",
        model: "mimo-v2.5-pro",
        credential_gate: "XIAOMI_TOKEN_PLAN_AMS_API_KEY",
        options: "default",
    },
    ProviderEmptyCase {
        name: "Xiaomi MiMo Token Plan (SGP) Provider Empty Messages",
        provider: "xiaomi-token-plan-sgp",
        model: "mimo-v2.5-pro",
        credential_gate: "XIAOMI_TOKEN_PLAN_SGP_API_KEY",
        options: "default",
    },
    ProviderEmptyCase {
        name: "Kimi For Coding Provider Empty Messages",
        provider: "kimi-coding",
        model: "kimi-k2-thinking",
        credential_gate: "KIMI_API_KEY",
        options: "default",
    },
    ProviderEmptyCase {
        name: "Vercel AI Gateway Provider Empty Messages",
        provider: "vercel-ai-gateway",
        model: "google/gemini-2.5-flash",
        credential_gate: "AI_GATEWAY_API_KEY",
        options: "default",
    },
    ProviderEmptyCase {
        name: "Amazon Bedrock Provider Empty Messages",
        provider: "amazon-bedrock",
        model: "global.anthropic.claude-sonnet-4-5-20250929-v1:0",
        credential_gate: "hasBedrockCredentials()",
        options: "default",
    },
    ProviderEmptyCase {
        name: "Anthropic OAuth Provider Empty Messages",
        provider: "anthropic",
        model: "claude-haiku-4-5",
        credential_gate: "resolveApiKey(\"anthropic\")",
        options: "apiKey from Anthropic OAuth token",
    },
    ProviderEmptyCase {
        name: "GitHub Copilot Provider Empty Messages / claude-haiku-4.5",
        provider: "github-copilot",
        model: "claude-haiku-4.5",
        credential_gate: "resolveApiKey(\"github-copilot\")",
        options: "apiKey from GitHub Copilot OAuth token",
    },
    ProviderEmptyCase {
        name: "GitHub Copilot Provider Empty Messages / claude-sonnet-4.6",
        provider: "github-copilot",
        model: "claude-sonnet-4.6",
        credential_gate: "resolveApiKey(\"github-copilot\")",
        options: "apiKey from GitHub Copilot OAuth token",
    },
    ProviderEmptyCase {
        name: "OpenAI Codex Provider Empty Messages",
        provider: "openai-codex",
        model: "gpt-5.5",
        credential_gate: "resolveApiKey(\"openai-codex\")",
        options: "apiKey from OpenAI Codex OAuth token",
    },
];

#[derive(Debug, Clone, Copy)]
enum EmptyUserInput {
    ContentArray,
    EmptyString,
    WhitespaceOnly,
}

#[test]
#[ignore = "live provider parity test; see BLOCKER"]
fn provider_empty_content_array_live_parity() {
    for case in PROVIDER_EMPTY_CASES {
        let response = run_live_empty_user_message(*case, EmptyUserInput::ContentArray);
        assert_empty_user_response(response);
    }
}

#[test]
#[ignore = "live provider parity test; see BLOCKER"]
fn provider_empty_string_content_live_parity() {
    for case in PROVIDER_EMPTY_CASES {
        let response = run_live_empty_user_message(*case, EmptyUserInput::EmptyString);
        assert_empty_user_response(response);
    }
}

#[test]
#[ignore = "live provider parity test; see BLOCKER"]
fn provider_whitespace_only_content_live_parity() {
    for case in PROVIDER_EMPTY_CASES {
        let response = run_live_empty_user_message(*case, EmptyUserInput::WhitespaceOnly);
        assert_empty_user_response(response);
    }
}

#[test]
#[ignore = "live provider parity test; see BLOCKER"]
fn provider_empty_assistant_message_in_conversation_live_parity() {
    for case in PROVIDER_EMPTY_CASES {
        let response = run_live_empty_assistant_message(*case);
        assert_empty_assistant_response(response);
    }
}

fn assert_empty_user_response(response: AssistantMessage) {
    assert_eq!(response.role, "assistant");
    if response.stop_reason == StopReason::Error {
        assert!(response.error_message.is_some());
    } else {
        let _content_is_defined = &response.content;
    }
}

fn assert_empty_assistant_response(response: AssistantMessage) {
    assert_eq!(response.role, "assistant");
    if response.stop_reason == StopReason::Error {
        assert!(response.error_message.is_some());
    } else {
        assert!(!response.content.is_empty());
    }
}

fn run_live_empty_user_message(case: ProviderEmptyCase, input: EmptyUserInput) -> AssistantMessage {
    panic!(
        "{BLOCKER}: {} provider={} model={} credential_gate={} options={} input={input:?}",
        case.name, case.provider, case.model, case.credential_gate, case.options
    )
}

fn run_live_empty_assistant_message(case: ProviderEmptyCase) -> AssistantMessage {
    panic!(
        "{BLOCKER}: {} provider={} model={} credential_gate={} options={}",
        case.name, case.provider, case.model, case.credential_gate, case.options
    )
}
