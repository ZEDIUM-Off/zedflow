//! Port of Pi `packages/ai/test/stream.test.ts`.
//!
//! The source file is a provider E2E suite: it calls live Gemini, Vertex, OpenAI, Anthropic,
//! Azure OpenAI, xAI, Groq, Cerebras, Cloudflare, Hugging Face, Together, NVIDIA, OpenRouter,
//! Vercel AI Gateway, zAI, Mistral, MiniMax, Kimi, Xiaomi, Ant Ling, Bedrock, OAuth-backed
//! providers, OpenAI Codex transports, and a local Ollama server. P1.T2 forbids live provider
//! calls, and the Rust compat catalog/provider dispatch is still a documented parity blocker,
//! so the parity suite is represented as ignored until those blockers are removed.

use zedflow_ai::compat::get_model;

const BLOCKER: &str = "live stream E2E suite skipped; requires provider credentials/local Ollama plus completed compat::get_model, compat::complete, compat::stream, builtin provider dispatch, and provider streaming transports";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scenario {
    BasicTextGeneration,
    ToolCalling,
    Streaming,
    Thinking,
    MultiTurnWithThinkingAndTools,
    ImageInput,
    BedrockAdaptiveThinkingWithoutAnthropicBeta,
    BedrockRequestMetadataIncluded,
    BedrockRequestMetadataOmitted,
}

#[derive(Debug, Clone, Copy)]
struct ProviderCase {
    provider: &'static str,
    model: &'static str,
    scenarios: &'static [Scenario],
}

const BASIC_TOOL_STREAM: &[Scenario] = &[
    Scenario::BasicTextGeneration,
    Scenario::ToolCalling,
    Scenario::Streaming,
];
const BASIC_TOOL_STREAM_THINK_MULTI: &[Scenario] = &[
    Scenario::BasicTextGeneration,
    Scenario::ToolCalling,
    Scenario::Streaming,
    Scenario::Thinking,
    Scenario::MultiTurnWithThinkingAndTools,
];
const BASIC_TOOL_STREAM_IMAGE: &[Scenario] = &[
    Scenario::BasicTextGeneration,
    Scenario::ToolCalling,
    Scenario::Streaming,
    Scenario::ImageInput,
];
const BASIC_TOOL_STREAM_IMAGE_MULTI: &[Scenario] = &[
    Scenario::BasicTextGeneration,
    Scenario::ToolCalling,
    Scenario::Streaming,
    Scenario::ImageInput,
    Scenario::MultiTurnWithThinkingAndTools,
];
const THINK_MULTI: &[Scenario] = &[Scenario::Thinking, Scenario::MultiTurnWithThinkingAndTools];
const FULL: &[Scenario] = &[
    Scenario::BasicTextGeneration,
    Scenario::ToolCalling,
    Scenario::Streaming,
    Scenario::Thinking,
    Scenario::MultiTurnWithThinkingAndTools,
    Scenario::ImageInput,
];
const BEDROCK_INTERLEAVED: &[Scenario] = &[
    Scenario::BedrockAdaptiveThinkingWithoutAnthropicBeta,
    Scenario::BedrockRequestMetadataIncluded,
    Scenario::BedrockRequestMetadataOmitted,
];

const PROVIDER_CASES: &[ProviderCase] = &[
    ProviderCase {
        provider: "google",
        model: "gemini-2.5-flash",
        scenarios: FULL,
    },
    ProviderCase {
        provider: "google-vertex",
        model: "gemini-3-flash-preview",
        scenarios: FULL,
    },
    ProviderCase {
        provider: "openai",
        model: "gpt-4o-mini via openai-completions",
        scenarios: BASIC_TOOL_STREAM_IMAGE,
    },
    ProviderCase {
        provider: "deepseek",
        model: "deepseek-v4-flash",
        scenarios: BASIC_TOOL_STREAM_THINK_MULTI,
    },
    ProviderCase {
        provider: "openai",
        model: "gpt-5.4",
        scenarios: FULL,
    },
    ProviderCase {
        provider: "anthropic",
        model: "claude-haiku-4-5",
        scenarios: BASIC_TOOL_STREAM_IMAGE,
    },
    ProviderCase {
        provider: "azure-openai-responses",
        model: "gpt-4o-mini",
        scenarios: BASIC_TOOL_STREAM_IMAGE,
    },
    ProviderCase {
        provider: "xai",
        model: "grok-code-fast-1",
        scenarios: BASIC_TOOL_STREAM_THINK_MULTI,
    },
    ProviderCase {
        provider: "groq",
        model: "openai/gpt-oss-20b",
        scenarios: BASIC_TOOL_STREAM_THINK_MULTI,
    },
    ProviderCase {
        provider: "cerebras",
        model: "gpt-oss-120b",
        scenarios: BASIC_TOOL_STREAM_THINK_MULTI,
    },
    ProviderCase {
        provider: "cloudflare-workers-ai",
        model: "@cf/moonshotai/kimi-k2.6",
        scenarios: BASIC_TOOL_STREAM_THINK_MULTI,
    },
    ProviderCase {
        provider: "cloudflare-ai-gateway",
        model: "workers-ai/@cf/moonshotai/kimi-k2.6",
        scenarios: BASIC_TOOL_STREAM_THINK_MULTI,
    },
    ProviderCase {
        provider: "cloudflare-ai-gateway",
        model: "gpt-5.1 via OpenAI BYOK",
        scenarios: BASIC_TOOL_STREAM_THINK_MULTI,
    },
    ProviderCase {
        provider: "cloudflare-ai-gateway",
        model: "claude-sonnet-4-5 via Anthropic BYOK",
        scenarios: BASIC_TOOL_STREAM_THINK_MULTI,
    },
    ProviderCase {
        provider: "huggingface",
        model: "moonshotai/Kimi-K2.5",
        scenarios: BASIC_TOOL_STREAM_THINK_MULTI,
    },
    ProviderCase {
        provider: "together",
        model: "moonshotai/Kimi-K2.6",
        scenarios: FULL,
    },
    ProviderCase {
        provider: "nvidia",
        model: "nvidia/nemotron-3-super-120b-a12b",
        scenarios: BASIC_TOOL_STREAM_THINK_MULTI,
    },
    ProviderCase {
        provider: "openrouter",
        model: "z-ai/glm-4.5v",
        scenarios: FULL,
    },
    ProviderCase {
        provider: "vercel-ai-gateway",
        model: "google/gemini-2.5-flash",
        scenarios: BASIC_TOOL_STREAM_IMAGE_MULTI,
    },
    ProviderCase {
        provider: "vercel-ai-gateway",
        model: "anthropic/claude-opus-4.5",
        scenarios: BASIC_TOOL_STREAM_IMAGE_MULTI,
    },
    ProviderCase {
        provider: "vercel-ai-gateway",
        model: "openai/gpt-5.1-codex-max",
        scenarios: BASIC_TOOL_STREAM_IMAGE_MULTI,
    },
    ProviderCase {
        provider: "zai",
        model: "glm-5.1",
        scenarios: FULL,
    },
    ProviderCase {
        provider: "mistral",
        model: "devstral-medium-latest",
        scenarios: BASIC_TOOL_STREAM,
    },
    ProviderCase {
        provider: "mistral",
        model: "magistral-medium-latest",
        scenarios: THINK_MULTI,
    },
    ProviderCase {
        provider: "mistral",
        model: "pixtral-12b",
        scenarios: BASIC_TOOL_STREAM_IMAGE,
    },
    ProviderCase {
        provider: "minimax",
        model: "MiniMax-M2.7",
        scenarios: BASIC_TOOL_STREAM_THINK_MULTI,
    },
    ProviderCase {
        provider: "kimi-coding",
        model: "kimi-k2-thinking",
        scenarios: BASIC_TOOL_STREAM_THINK_MULTI,
    },
    ProviderCase {
        provider: "xiaomi",
        model: "mimo-v2.5-pro",
        scenarios: BASIC_TOOL_STREAM_THINK_MULTI,
    },
    ProviderCase {
        provider: "xiaomi-token-plan-cn",
        model: "mimo-v2.5-pro",
        scenarios: BASIC_TOOL_STREAM_THINK_MULTI,
    },
    ProviderCase {
        provider: "xiaomi-token-plan-ams",
        model: "mimo-v2.5-pro",
        scenarios: BASIC_TOOL_STREAM_THINK_MULTI,
    },
    ProviderCase {
        provider: "xiaomi-token-plan-sgp",
        model: "mimo-v2.5-pro",
        scenarios: BASIC_TOOL_STREAM_THINK_MULTI,
    },
    ProviderCase {
        provider: "ant-ling",
        model: "Ling-2.6-flash",
        scenarios: BASIC_TOOL_STREAM_THINK_MULTI,
    },
    ProviderCase {
        provider: "anthropic",
        model: "claude-sonnet-4-6 via OAuth",
        scenarios: FULL,
    },
    ProviderCase {
        provider: "anthropic",
        model: "claude-opus-4-6 adaptive thinking via OAuth",
        scenarios: FULL,
    },
    ProviderCase {
        provider: "github-copilot",
        model: "gpt-5.3-codex",
        scenarios: FULL,
    },
    ProviderCase {
        provider: "github-copilot",
        model: "claude-sonnet-4.6",
        scenarios: FULL,
    },
    ProviderCase {
        provider: "openai-codex",
        model: "gpt-5.4",
        scenarios: FULL,
    },
    ProviderCase {
        provider: "openai-codex",
        model: "gpt-5.5",
        scenarios: FULL,
    },
    ProviderCase {
        provider: "openai-codex",
        model: "gpt-5.5 via WebSocket",
        scenarios: FULL,
    },
    ProviderCase {
        provider: "amazon-bedrock",
        model: "global.anthropic.claude-sonnet-4-5-20250929-v1:0",
        scenarios: FULL,
    },
    ProviderCase {
        provider: "amazon-bedrock",
        model: "global.anthropic.claude-opus-4-6-v1",
        scenarios: BEDROCK_INTERLEAVED,
    },
    ProviderCase {
        provider: "ollama",
        model: "gpt-oss:20b via openai-completions",
        scenarios: BASIC_TOOL_STREAM_THINK_MULTI,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct Usage {
    input: u64,
    cache_read: u64,
    output: u64,
}

#[allow(
    dead_code,
    reason = "constructed only by capability-gated provider responses"
)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum ContentBlock {
    Text(String),
    Thinking(String),
    ToolCall {
        id: String,
        name: String,
        arguments: ToolArguments,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolArguments {
    a: i64,
    b: i64,
    operation: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AssistantResponse {
    role: &'static str,
    content: Vec<ContentBlock>,
    usage: Usage,
    stop_reason: &'static str,
    error_message: Option<String>,
}

fn text(response: &AssistantResponse) -> String {
    response
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn assert_successful_text_response(response: &AssistantResponse, expected: &str) {
    assert_eq!(response.role, "assistant");
    assert!(!response.content.is_empty());
    assert!(response.usage.input + response.usage.cache_read > 0);
    assert!(response.usage.output > 0);
    assert!(response.error_message.is_none());
    assert!(text(response).contains(expected));
}

fn run_live_stream_case(case: ProviderCase, scenario: Scenario) -> AssistantResponse {
    let _source_fixture = (case.provider, case.model, scenario);
    panic!("{BLOCKER}");
}

fn assert_basic_text_generation(case: ProviderCase) {
    let response = run_live_stream_case(case, Scenario::BasicTextGeneration);
    assert_successful_text_response(&response, "Hello test successful");

    let second_response = run_live_stream_case(case, Scenario::BasicTextGeneration);
    assert_successful_text_response(&second_response, "Goodbye test successful");
}

fn assert_tool_call(case: ProviderCase) {
    let response = run_live_stream_case(case, Scenario::ToolCalling);
    assert_eq!(response.stop_reason, "toolUse");
    let tool_call = response
        .content
        .iter()
        .find_map(|block| match block {
            ContentBlock::ToolCall {
                id,
                name,
                arguments,
            } => Some((id, name, arguments)),
            _ => None,
        })
        .expect("No tool call found in response");

    assert_eq!(tool_call.1, "math_operation");
    assert!(!tool_call.0.is_empty());
    assert_eq!(tool_call.2.a, 15);
    assert_eq!(tool_call.2.b, 27);
    assert!(["add", "subtract", "multiply", "divide"].contains(&tool_call.2.operation));
}

fn assert_streaming(case: ProviderCase) {
    let response = run_live_stream_case(case, Scenario::Streaming);
    assert!(!text(&response).is_empty());
    assert!(
        response
            .content
            .iter()
            .any(|block| matches!(block, ContentBlock::Text(_)))
    );
}

fn assert_thinking(case: ProviderCase) {
    let response = run_live_stream_case(case, Scenario::Thinking);
    assert_eq!(response.stop_reason, "stop", "{:?}", response.error_message);
    assert!(
        response
            .content
            .iter()
            .any(|block| matches!(block, ContentBlock::Thinking(text) if !text.is_empty()))
    );
}

fn assert_image_input(case: ProviderCase) {
    let response = run_live_stream_case(case, Scenario::ImageInput);
    assert!(!response.content.is_empty());
    let lower = text(&response).to_lowercase();
    assert!(lower.contains("red"));
    assert!(lower.contains("circle"));
}

fn assert_multi_turn(case: ProviderCase) {
    let response = run_live_stream_case(case, Scenario::MultiTurnWithThinkingAndTools);
    let has_thinking = response
        .content
        .iter()
        .any(|block| matches!(block, ContentBlock::Thinking(_)));
    let has_tool_calls = response
        .content
        .iter()
        .any(|block| matches!(block, ContentBlock::ToolCall { .. }));

    assert!(has_thinking || has_tool_calls);
    let all_text = text(&response);
    assert!(!all_text.is_empty());
    assert!(all_text.contains("714"));
    assert!(all_text.contains("887"));
}

fn assert_bedrock_payload(case: ProviderCase, scenario: Scenario) {
    let response = run_live_stream_case(case, scenario);
    assert_ne!(
        response.stop_reason, "error",
        "{:?}",
        response.error_message
    );
    assert!(response.error_message.is_none());
}

fn assert_scenario(case: ProviderCase, scenario: Scenario) {
    match scenario {
        Scenario::BasicTextGeneration => assert_basic_text_generation(case),
        Scenario::ToolCalling => assert_tool_call(case),
        Scenario::Streaming => assert_streaming(case),
        Scenario::Thinking => assert_thinking(case),
        Scenario::MultiTurnWithThinkingAndTools => assert_multi_turn(case),
        Scenario::ImageInput => assert_image_input(case),
        Scenario::BedrockAdaptiveThinkingWithoutAnthropicBeta
        | Scenario::BedrockRequestMetadataIncluded
        | Scenario::BedrockRequestMetadataOmitted => assert_bedrock_payload(case, scenario),
    }
}

#[test]
fn stream_e2e_source_uses_registered_compat_catalog() {
    let model = get_model("google", "gemini-2.5-flash")
        .expect("compat::get_model should read the registered builtin catalog");

    assert_eq!(model.provider, "google");
    assert_eq!(model.api, "google-generative-ai");
}

#[test]
#[ignore = "live provider/local Ollama E2E suite skipped; see BLOCKER"]
fn runs_generate_e2e_stream_provider_matrix() {
    assert!(!PROVIDER_CASES.is_empty());

    for case in PROVIDER_CASES {
        for scenario in case.scenarios {
            assert_scenario(*case, *scenario);
        }
    }
}
