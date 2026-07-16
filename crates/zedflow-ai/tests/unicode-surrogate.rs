//! Port of Pi `packages/ai/test/unicode-surrogate.test.ts`.
//!
//! The source suite is a live provider matrix gated by API keys/OAuth credentials. P1.T2 forbids
//! live provider calls, and Rust `compat::get_model`/`compat::complete` plus the full Pi `Context`
//! and tool-result message model are still `request-capture blocker`s, so parity tests are ignored until
//! those blockers are removed.

use zedflow_ai::types::StopReason;

const BLOCKER: &str = "live Unicode surrogate provider parity skipped; requires provider credentials/OAuth plus completed compat::get_model, compat::complete, provider streaming transports, and full Context/tool-result message ports";

const EMOJI_TOOL_RESULT_TEXT: &str = r#"Test with emoji 🙈 and other characters:
- Monkey emoji: 🙈
- Thumbs up: 👍
- Heart: ❤️
- Thinking face: 🤔
- Rocket: 🚀
- Mixed text: Mario Zechner wann? Wo? Bin grad äußersr eventuninformiert 🙈
- Japanese: こんにちは
- Chinese: 你好
- Mathematical symbols: ∑∫∂√
- Special quotes: "curly" 'quotes'"#;

const LINKEDIN_TOOL_RESULT_TEXT: &str = r#"Post: Hab einen "Generative KI für Nicht-Techniker" Workshop gebaut.
Unanswered Comments: 2

=> {
  "comments": [
    {
      "author": "Matthias Neumayer's  graphic link",
      "text": "Leider nehmen das viel zu wenige Leute ernst"
    },
    {
      "author": "Matthias Neumayer's  graphic link",
      "text": "Mario Zechner wann? Wo? Bin grad äußersr eventuninformiert 🙈"
    }
  ]
}"#;

const UNPAIRED_HIGH_SURROGATE_TOOL_RESULT_UTF16: &[u16] = &[
    b'T' as u16,
    b'e' as u16,
    b'x' as u16,
    b't' as u16,
    b' ' as u16,
    b'w' as u16,
    b'i' as u16,
    b't' as u16,
    b'h' as u16,
    b' ' as u16,
    b'u' as u16,
    b'n' as u16,
    b'p' as u16,
    b'a' as u16,
    b'i' as u16,
    b'r' as u16,
    b'e' as u16,
    b'd' as u16,
    b' ' as u16,
    b's' as u16,
    b'u' as u16,
    b'r' as u16,
    b'r' as u16,
    b'o' as u16,
    b'g' as u16,
    b'a' as u16,
    b't' as u16,
    b'e' as u16,
    b':' as u16,
    b' ' as u16,
    0xd83d,
    b' ' as u16,
    b'<' as u16,
    b'-' as u16,
    b' ' as u16,
    b's' as u16,
    b'h' as u16,
    b'o' as u16,
    b'u' as u16,
    b'l' as u16,
    b'd' as u16,
    b' ' as u16,
    b'b' as u16,
    b'e' as u16,
    b' ' as u16,
    b's' as u16,
    b'a' as u16,
    b'n' as u16,
    b'i' as u16,
    b't' as u16,
    b'i' as u16,
    b'z' as u16,
    b'e' as u16,
    b'd' as u16,
];

#[derive(Debug, Clone, Copy)]
struct ProviderCase {
    suite: &'static str,
    provider: &'static str,
    model: &'static str,
    credential_gate: CredentialGate,
    options: &'static [CaseOption],
}

#[allow(
    dead_code,
    reason = "fields are consumed only by capability-gated live dispatch"
)]
#[derive(Debug, Clone, Copy)]
enum CredentialGate {
    Env(&'static str),
    AzureOpenAi,
    Bedrock,
    CloudflareAiGateway,
    CloudflareWorkersAi,
    OAuth(&'static str),
}

#[allow(
    dead_code,
    reason = "fields are consumed only by capability-gated live dispatch"
)]
#[derive(Debug, Clone, Copy)]
enum CaseOption {
    ApiKeyFromOAuth(&'static str),
    AzureDeploymentName,
    ReasoningEffortHigh,
}

#[derive(Debug, Clone, Copy)]
enum ProbeFixture {
    EmojiInToolResults,
    RealWorldLinkedInData,
    UnpairedHighSurrogate,
}

#[derive(Debug, Clone, Copy)]
enum FixturePayload {
    Utf8(&'static str),
    Utf16(&'static [u16]),
}

impl FixturePayload {
    fn len(self) -> usize {
        match self {
            Self::Utf8(text) => text.len(),
            Self::Utf16(units) => units.len(),
        }
    }
}

#[allow(
    dead_code,
    reason = "constructed only by capability-gated live responses"
)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum ContentBlock {
    Text(String),
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProviderResponse {
    stop_reason: StopReason,
    error_message: Option<String>,
    content: Vec<ContentBlock>,
}

const ANTHROPIC_OAUTH: &[CaseOption] = &[CaseOption::ApiKeyFromOAuth("anthropic")];
const AZURE_OPTIONS: &[CaseOption] = &[CaseOption::AzureDeploymentName];
const OPENAI_CODEX_OAUTH: &[CaseOption] = &[CaseOption::ApiKeyFromOAuth("openai-codex")];
const TOGETHER_OPTIONS: &[CaseOption] = &[CaseOption::ReasoningEffortHigh];
const GITHUB_COPILOT_OAUTH: &[CaseOption] = &[CaseOption::ApiKeyFromOAuth("github-copilot")];

const PROVIDER_CASES: &[ProviderCase] = &[
    ProviderCase {
        suite: "Google Provider Unicode Handling",
        provider: "google",
        model: "gemini-2.5-flash",
        credential_gate: CredentialGate::Env("GEMINI_API_KEY"),
        options: &[],
    },
    ProviderCase {
        suite: "OpenAI Completions Provider Unicode Handling",
        provider: "openai",
        model: "gpt-4o-mini",
        credential_gate: CredentialGate::Env("OPENAI_API_KEY"),
        options: &[],
    },
    ProviderCase {
        suite: "OpenAI Responses Provider Unicode Handling",
        provider: "openai",
        model: "gpt-5-mini",
        credential_gate: CredentialGate::Env("OPENAI_API_KEY"),
        options: &[],
    },
    ProviderCase {
        suite: "Azure OpenAI Responses Provider Unicode Handling",
        provider: "azure-openai-responses",
        model: "gpt-4o-mini",
        credential_gate: CredentialGate::AzureOpenAi,
        options: AZURE_OPTIONS,
    },
    ProviderCase {
        suite: "Anthropic Provider Unicode Handling",
        provider: "anthropic",
        model: "claude-haiku-4-5",
        credential_gate: CredentialGate::Env("ANTHROPIC_API_KEY"),
        options: &[],
    },
    ProviderCase {
        suite: "Anthropic OAuth Provider Unicode Handling",
        provider: "anthropic",
        model: "claude-haiku-4-5",
        credential_gate: CredentialGate::OAuth("anthropic"),
        options: ANTHROPIC_OAUTH,
    },
    ProviderCase {
        suite: "GitHub Copilot Provider Unicode Handling",
        provider: "github-copilot",
        model: "claude-haiku-4.5",
        credential_gate: CredentialGate::OAuth("github-copilot"),
        options: GITHUB_COPILOT_OAUTH,
    },
    ProviderCase {
        suite: "GitHub Copilot Provider Unicode Handling",
        provider: "github-copilot",
        model: "claude-sonnet-4.6",
        credential_gate: CredentialGate::OAuth("github-copilot"),
        options: GITHUB_COPILOT_OAUTH,
    },
    ProviderCase {
        suite: "xAI Provider Unicode Handling",
        provider: "xai",
        model: "grok-3",
        credential_gate: CredentialGate::Env("XAI_API_KEY"),
        options: &[],
    },
    ProviderCase {
        suite: "Groq Provider Unicode Handling",
        provider: "groq",
        model: "openai/gpt-oss-20b",
        credential_gate: CredentialGate::Env("GROQ_API_KEY"),
        options: &[],
    },
    ProviderCase {
        suite: "Cerebras Provider Unicode Handling",
        provider: "cerebras",
        model: "gpt-oss-120b",
        credential_gate: CredentialGate::Env("CEREBRAS_API_KEY"),
        options: &[],
    },
    ProviderCase {
        suite: "Cloudflare Workers AI Provider Unicode Handling",
        provider: "cloudflare-workers-ai",
        model: "@cf/moonshotai/kimi-k2.6",
        credential_gate: CredentialGate::CloudflareWorkersAi,
        options: &[],
    },
    ProviderCase {
        suite: "Cloudflare AI Gateway Provider Unicode Handling",
        provider: "cloudflare-ai-gateway",
        model: "workers-ai/@cf/moonshotai/kimi-k2.6",
        credential_gate: CredentialGate::CloudflareAiGateway,
        options: &[],
    },
    ProviderCase {
        suite: "Hugging Face Provider Unicode Handling",
        provider: "huggingface",
        model: "moonshotai/Kimi-K2.5",
        credential_gate: CredentialGate::Env("HF_TOKEN"),
        options: &[],
    },
    ProviderCase {
        suite: "Together AI Provider Unicode Handling",
        provider: "together",
        model: "moonshotai/Kimi-K2.6",
        credential_gate: CredentialGate::Env("TOGETHER_API_KEY"),
        options: TOGETHER_OPTIONS,
    },
    ProviderCase {
        suite: "zAI Provider Unicode Handling",
        provider: "zai",
        model: "glm-4.5-air",
        credential_gate: CredentialGate::Env("ZAI_API_KEY"),
        options: &[],
    },
    ProviderCase {
        suite: "Mistral Provider Unicode Handling",
        provider: "mistral",
        model: "devstral-medium-latest",
        credential_gate: CredentialGate::Env("MISTRAL_API_KEY"),
        options: &[],
    },
    ProviderCase {
        suite: "MiniMax Provider Unicode Handling",
        provider: "minimax",
        model: "MiniMax-M2.7",
        credential_gate: CredentialGate::Env("MINIMAX_API_KEY"),
        options: &[],
    },
    ProviderCase {
        suite: "Xiaomi MiMo (API billing) Provider Unicode Handling",
        provider: "xiaomi",
        model: "mimo-v2.5-pro",
        credential_gate: CredentialGate::Env("XIAOMI_API_KEY"),
        options: &[],
    },
    ProviderCase {
        suite: "Xiaomi MiMo Token Plan (CN) Provider Unicode Handling",
        provider: "xiaomi-token-plan-cn",
        model: "mimo-v2.5-pro",
        credential_gate: CredentialGate::Env("XIAOMI_TOKEN_PLAN_CN_API_KEY"),
        options: &[],
    },
    ProviderCase {
        suite: "Xiaomi MiMo Token Plan (AMS) Provider Unicode Handling",
        provider: "xiaomi-token-plan-ams",
        model: "mimo-v2.5-pro",
        credential_gate: CredentialGate::Env("XIAOMI_TOKEN_PLAN_AMS_API_KEY"),
        options: &[],
    },
    ProviderCase {
        suite: "Xiaomi MiMo Token Plan (SGP) Provider Unicode Handling",
        provider: "xiaomi-token-plan-sgp",
        model: "mimo-v2.5-pro",
        credential_gate: CredentialGate::Env("XIAOMI_TOKEN_PLAN_SGP_API_KEY"),
        options: &[],
    },
    ProviderCase {
        suite: "Kimi For Coding Provider Unicode Handling",
        provider: "kimi-coding",
        model: "kimi-k2-thinking",
        credential_gate: CredentialGate::Env("KIMI_API_KEY"),
        options: &[],
    },
    ProviderCase {
        suite: "Vercel AI Gateway Provider Unicode Handling",
        provider: "vercel-ai-gateway",
        model: "google/gemini-2.5-flash",
        credential_gate: CredentialGate::Env("AI_GATEWAY_API_KEY"),
        options: &[],
    },
    ProviderCase {
        suite: "Amazon Bedrock Provider Unicode Handling",
        provider: "amazon-bedrock",
        model: "global.anthropic.claude-sonnet-4-5-20250929-v1:0",
        credential_gate: CredentialGate::Bedrock,
        options: &[],
    },
    ProviderCase {
        suite: "OpenAI Codex Provider Unicode Handling",
        provider: "openai-codex",
        model: "gpt-5.5",
        credential_gate: CredentialGate::OAuth("openai-codex"),
        options: OPENAI_CODEX_OAUTH,
    },
];

fn run_live_unicode_probe(case: ProviderCase, fixture: ProbeFixture) -> ProviderResponse {
    let payload = match fixture {
        ProbeFixture::EmojiInToolResults => FixturePayload::Utf8(EMOJI_TOOL_RESULT_TEXT),
        ProbeFixture::RealWorldLinkedInData => FixturePayload::Utf8(LINKEDIN_TOOL_RESULT_TEXT),
        ProbeFixture::UnpairedHighSurrogate => {
            FixturePayload::Utf16(UNPAIRED_HIGH_SURROGATE_TOOL_RESULT_UTF16)
        }
    };
    let _source_fixture = (
        case.suite,
        case.provider,
        case.model,
        case.credential_gate,
        case.options,
        payload.len(),
    );

    panic!("{BLOCKER}");
}

fn assert_not_error(response: &ProviderResponse) {
    assert_ne!(
        response.stop_reason,
        StopReason::Error,
        "{:?}",
        response.error_message
    );
    assert!(response.error_message.is_none());
}

#[test]
#[ignore = "live provider call skipped; see BLOCKER"]
fn handles_emoji_in_tool_results() {
    for case in PROVIDER_CASES {
        let response = run_live_unicode_probe(*case, ProbeFixture::EmojiInToolResults);

        assert_not_error(&response);
        assert!(!response.content.is_empty());
    }
}

#[test]
#[ignore = "live provider call skipped; see BLOCKER"]
fn handles_real_world_linkedin_comment_data_with_emoji() {
    for case in PROVIDER_CASES {
        let response = run_live_unicode_probe(*case, ProbeFixture::RealWorldLinkedInData);

        assert_not_error(&response);
        assert!(
            response
                .content
                .iter()
                .any(|block| matches!(block, ContentBlock::Text(_)))
        );
    }
}

#[test]
#[ignore = "live provider call skipped; Rust cannot construct JS lone-surrogate strings and compat/tool-result context is incomplete; see BLOCKER"]
fn handles_unpaired_high_surrogate_in_tool_results() {
    assert_eq!(UNPAIRED_HIGH_SURROGATE_TOOL_RESULT_UTF16[30], 0xd83d);

    for case in PROVIDER_CASES {
        let response = run_live_unicode_probe(*case, ProbeFixture::UnpairedHighSurrogate);

        assert_not_error(&response);
        assert!(!response.content.is_empty());
    }
}
