use serde_json::json;
use zedflow_ai::api::transform_messages::{
    AssistantContent, AssistantMessage, InputContent, Message, Model, StopReason, TextContent,
    ToolCall, UserContent, UserMessage, transform_messages,
};

const BLOCKER: &str = "live provider parity suite requires getModel/complete provider dispatch, OAuth token resolution, and network credentials; Rust compat get_model and builtin provider registration are still request-capture blockers";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProviderCase {
    provider: &'static str,
    model: &'static str,
    api_override: Option<&'static str>,
    option: Option<&'static str>,
}

const PROVIDER_CASES: &[ProviderCase] = &[
    ProviderCase {
        provider: "google",
        model: "gemini-2.5-flash",
        api_override: None,
        option: None,
    },
    ProviderCase {
        provider: "openai",
        model: "gpt-4o-mini",
        api_override: Some("openai-completions"),
        option: None,
    },
    ProviderCase {
        provider: "openai",
        model: "gpt-5-mini",
        api_override: None,
        option: None,
    },
    ProviderCase {
        provider: "azure-openai-responses",
        model: "gpt-4o-mini",
        api_override: None,
        option: Some("azureDeploymentName"),
    },
    ProviderCase {
        provider: "anthropic",
        model: "claude-haiku-4-5",
        api_override: None,
        option: None,
    },
    ProviderCase {
        provider: "xai",
        model: "grok-3-fast",
        api_override: None,
        option: None,
    },
    ProviderCase {
        provider: "groq",
        model: "openai/gpt-oss-20b",
        api_override: None,
        option: None,
    },
    ProviderCase {
        provider: "cerebras",
        model: "gpt-oss-120b",
        api_override: None,
        option: None,
    },
    ProviderCase {
        provider: "cloudflare-workers-ai",
        model: "@cf/moonshotai/kimi-k2.6",
        api_override: None,
        option: None,
    },
    ProviderCase {
        provider: "cloudflare-ai-gateway",
        model: "workers-ai/@cf/moonshotai/kimi-k2.6",
        api_override: None,
        option: None,
    },
    ProviderCase {
        provider: "huggingface",
        model: "moonshotai/Kimi-K2.5",
        api_override: None,
        option: None,
    },
    ProviderCase {
        provider: "together",
        model: "moonshotai/Kimi-K2.6",
        api_override: None,
        option: Some("reasoningEffort=high"),
    },
    ProviderCase {
        provider: "zai",
        model: "glm-4.5-air",
        api_override: None,
        option: None,
    },
    ProviderCase {
        provider: "mistral",
        model: "devstral-medium-latest",
        api_override: None,
        option: None,
    },
    ProviderCase {
        provider: "minimax",
        model: "MiniMax-M2.7",
        api_override: None,
        option: None,
    },
    ProviderCase {
        provider: "xiaomi",
        model: "mimo-v2.5-pro",
        api_override: None,
        option: None,
    },
    ProviderCase {
        provider: "xiaomi-token-plan-cn",
        model: "mimo-v2.5-pro",
        api_override: None,
        option: None,
    },
    ProviderCase {
        provider: "xiaomi-token-plan-ams",
        model: "mimo-v2.5-pro",
        api_override: None,
        option: None,
    },
    ProviderCase {
        provider: "xiaomi-token-plan-sgp",
        model: "mimo-v2.5-pro",
        api_override: None,
        option: None,
    },
    ProviderCase {
        provider: "kimi-coding",
        model: "kimi-k2-thinking",
        api_override: None,
        option: None,
    },
    ProviderCase {
        provider: "vercel-ai-gateway",
        model: "google/gemini-2.5-flash",
        api_override: None,
        option: None,
    },
    ProviderCase {
        provider: "amazon-bedrock",
        model: "global.anthropic.claude-sonnet-4-5-20250929-v1:0",
        api_override: None,
        option: None,
    },
    ProviderCase {
        provider: "anthropic",
        model: "claude-haiku-4-5",
        api_override: None,
        option: Some("apiKey=anthropicOAuthToken"),
    },
    ProviderCase {
        provider: "github-copilot",
        model: "claude-haiku-4.5",
        api_override: None,
        option: Some("apiKey=githubCopilotToken"),
    },
    ProviderCase {
        provider: "github-copilot",
        model: "claude-sonnet-4.6",
        api_override: None,
        option: Some("apiKey=githubCopilotToken"),
    },
    ProviderCase {
        provider: "openai-codex",
        model: "gpt-5.5",
        api_override: None,
        option: Some("apiKey=openaiCodexToken"),
    },
];

fn destination_model() -> Model {
    Model {
        id: "gpt-5-mini".to_owned(),
        api: "openai-responses".to_owned(),
        provider: "openai".to_owned(),
        input: vec!["text".to_owned()],
    }
}

fn user(content: &str) -> Message {
    Message::User(UserMessage {
        content: UserContent::Text(content.to_owned()),
        timestamp: 0,
    })
}

fn assistant_tool_call() -> Message {
    Message::Assistant(AssistantMessage {
        content: vec![AssistantContent::ToolCall(ToolCall {
            id: "call_calculate".to_owned(),
            name: "calculate".to_owned(),
            arguments: json!({ "expression": "25 * 18" }),
            thought_signature: None,
        })],
        api: "openai-responses".to_owned(),
        provider: "openai".to_owned(),
        model: "gpt-5-mini".to_owned(),
        stop_reason: StopReason::ToolUse,
        timestamp: 0,
        ..AssistantMessage::default()
    })
}

#[test]
fn inserts_missing_tool_result_before_followup_user_message() {
    let first_response = assistant_tool_call();
    let Message::Assistant(first_assistant) = &first_response else {
        panic!("expected assistant response");
    };
    let has_tool_call = first_assistant
        .content
        .iter()
        .any(|block| matches!(block, AssistantContent::ToolCall(_)));
    assert!(has_tool_call);

    let result = transform_messages(
        &[
            user("Please calculate 25 * 18 using the calculate tool."),
            first_response,
            user("Never mind, just tell me what is 2+2?"),
        ],
        &destination_model(),
        None,
    );

    assert_eq!(result.len(), 4);

    let Message::ToolResult(synthetic) = &result[2] else {
        panic!("expected synthetic tool result before follow-up user message");
    };
    assert_eq!(synthetic.tool_call_id, "call_calculate");
    assert_eq!(synthetic.tool_name, "calculate");
    assert!(synthetic.is_error);
    assert_eq!(
        synthetic.content,
        vec![InputContent::Text(TextContent {
            text: "No result provided".to_owned(),
            text_signature: None,
        })]
    );

    let Message::User(second_user) = &result[3] else {
        panic!("expected second user message after synthetic result");
    };
    assert_eq!(
        second_user.content,
        UserContent::Text("Never mind, just tell me what is 2+2?".to_owned())
    );
}

#[test]
#[ignore = "live provider parity suite needs compat getModel/complete and network credentials; see BLOCKER"]
fn live_provider_tool_call_without_result_suite_is_represented() {
    assert_eq!(PROVIDER_CASES.len(), 26);
    assert!(
        PROVIDER_CASES
            .iter()
            .any(|case| case.api_override == Some("openai-completions"))
    );
    assert!(
        PROVIDER_CASES
            .iter()
            .any(|case| case.option == Some("reasoningEffort=high"))
    );

    panic!("{BLOCKER}");
}

#[test]
fn live_provider_case_manifest_matches_source_suite() {
    assert_eq!(PROVIDER_CASES.len(), 26);
    assert!(PROVIDER_CASES.contains(&ProviderCase {
        provider: "google",
        model: "gemini-2.5-flash",
        api_override: None,
        option: None,
    }));
    assert!(PROVIDER_CASES.contains(&ProviderCase {
        provider: "openai-codex",
        model: "gpt-5.5",
        api_override: None,
        option: Some("apiKey=openaiCodexToken"),
    }));
}
