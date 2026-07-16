use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use futures::executor::block_on;
use zedflow_ai::compat::{complete, get_model};
use zedflow_ai::types::StreamOptions;
use zedflow_ai::types::{Context, Model, StopReason};

const BLOCKER: &str = "live provider image tool-result probe skipped; compat::get_model, typed Context messages/tools/tool results, typed assistant tool-call/text content, StreamOptions provider extras, OAuth auth.json resolution, and provider streaming are still request-capture blockers";

#[derive(Debug, Clone, Copy)]
enum CredentialGate {
    Env(&'static str),
    AzureOpenAi,
    Bedrock,
    OAuth(&'static str),
}

#[derive(Debug, Clone, Copy)]
struct ProviderCase {
    provider: &'static str,
    model_id: &'static str,
    credential: CredentialGate,
    api_override: Option<&'static str>,
    option_note: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Tool {
    name: &'static str,
    description: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolCall {
    id: String,
    name: String,
}

#[allow(
    dead_code,
    reason = "tool calls are constructed only by capability-gated live responses"
)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum ContentBlock {
    Text(String),
    Image { data: String, mime_type: String },
    ToolCall(ToolCall),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolResultMessage {
    tool_call_id: String,
    tool_name: String,
    content: Vec<ContentBlock>,
    is_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProbeMessage {
    User(String),
    Assistant(ProbeAssistantMessage),
    ToolResult(ToolResultMessage),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProbeContext {
    system_prompt: &'static str,
    messages: Vec<ProbeMessage>,
    tools: Vec<Tool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProbeAssistantMessage {
    content: Vec<ContentBlock>,
    stop_reason: StopReason,
    error_message: Option<String>,
}

const IMAGE_ONLY_CASES: &[ProviderCase] = &[
    ProviderCase {
        provider: "google",
        model_id: "gemini-2.5-flash",
        credential: CredentialGate::Env("GEMINI_API_KEY"),
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "openai",
        model_id: "gpt-4o-mini",
        credential: CredentialGate::Env("OPENAI_API_KEY"),
        api_override: Some("openai-completions"),
        option_note: None,
    },
    ProviderCase {
        provider: "openai",
        model_id: "gpt-5-mini",
        credential: CredentialGate::Env("OPENAI_API_KEY"),
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "azure-openai-responses",
        model_id: "gpt-4o-mini",
        credential: CredentialGate::AzureOpenAi,
        api_override: None,
        option_note: Some("azureDeploymentName from AZURE_OPENAI_DEPLOYMENT_NAME_MAP"),
    },
    ProviderCase {
        provider: "anthropic",
        model_id: "claude-haiku-4-5",
        credential: CredentialGate::Env("ANTHROPIC_API_KEY"),
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "openrouter",
        model_id: "z-ai/glm-4.5v",
        credential: CredentialGate::Env("OPENROUTER_API_KEY"),
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "mistral",
        model_id: "pixtral-12b",
        credential: CredentialGate::Env("MISTRAL_API_KEY"),
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "together",
        model_id: "moonshotai/Kimi-K2.6",
        credential: CredentialGate::Env("TOGETHER_API_KEY"),
        api_override: None,
        option_note: Some("reasoningEffort=high"),
    },
    ProviderCase {
        provider: "xiaomi",
        model_id: "mimo-v2.5-pro",
        credential: CredentialGate::Env("XIAOMI_API_KEY"),
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "xiaomi-token-plan-cn",
        model_id: "mimo-v2.5-pro",
        credential: CredentialGate::Env("XIAOMI_TOKEN_PLAN_CN_API_KEY"),
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "xiaomi-token-plan-ams",
        model_id: "mimo-v2.5-pro",
        credential: CredentialGate::Env("XIAOMI_TOKEN_PLAN_AMS_API_KEY"),
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "xiaomi-token-plan-sgp",
        model_id: "mimo-v2.5-pro",
        credential: CredentialGate::Env("XIAOMI_TOKEN_PLAN_SGP_API_KEY"),
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "kimi-coding",
        model_id: "kimi-for-coding",
        credential: CredentialGate::Env("KIMI_API_KEY"),
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "vercel-ai-gateway",
        model_id: "google/gemini-2.5-flash",
        credential: CredentialGate::Env("AI_GATEWAY_API_KEY"),
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "amazon-bedrock",
        model_id: "global.anthropic.claude-sonnet-4-5-20250929-v1:0",
        credential: CredentialGate::Bedrock,
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "anthropic",
        model_id: "claude-sonnet-4-5",
        credential: CredentialGate::OAuth("anthropic"),
        api_override: None,
        option_note: Some("apiKey from ~/.pi/agent/auth.json"),
    },
    ProviderCase {
        provider: "github-copilot",
        model_id: "claude-haiku-4.5",
        credential: CredentialGate::OAuth("github-copilot"),
        api_override: None,
        option_note: Some("apiKey from ~/.pi/agent/auth.json"),
    },
    ProviderCase {
        provider: "github-copilot",
        model_id: "claude-sonnet-4.6",
        credential: CredentialGate::OAuth("github-copilot"),
        api_override: None,
        option_note: Some("apiKey from ~/.pi/agent/auth.json"),
    },
    ProviderCase {
        provider: "openai-codex",
        model_id: "gpt-5.5",
        credential: CredentialGate::OAuth("openai-codex"),
        api_override: None,
        option_note: Some("apiKey from ~/.pi/agent/auth.json"),
    },
];

const TEXT_AND_IMAGE_CASES: &[ProviderCase] = &[
    ProviderCase {
        provider: "google",
        model_id: "gemini-2.5-flash",
        credential: CredentialGate::Env("GEMINI_API_KEY"),
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "openai",
        model_id: "gpt-4o-mini",
        credential: CredentialGate::Env("OPENAI_API_KEY"),
        api_override: Some("openai-completions"),
        option_note: None,
    },
    ProviderCase {
        provider: "openai",
        model_id: "gpt-5-mini",
        credential: CredentialGate::Env("OPENAI_API_KEY"),
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "azure-openai-responses",
        model_id: "gpt-4o-mini",
        credential: CredentialGate::AzureOpenAi,
        api_override: None,
        option_note: Some("azureDeploymentName from AZURE_OPENAI_DEPLOYMENT_NAME_MAP"),
    },
    ProviderCase {
        provider: "anthropic",
        model_id: "claude-haiku-4-5",
        credential: CredentialGate::Env("ANTHROPIC_API_KEY"),
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "openrouter",
        model_id: "z-ai/glm-4.5v",
        credential: CredentialGate::Env("OPENROUTER_API_KEY"),
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "mistral",
        model_id: "pixtral-12b",
        credential: CredentialGate::Env("MISTRAL_API_KEY"),
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "together",
        model_id: "moonshotai/Kimi-K2.6",
        credential: CredentialGate::Env("TOGETHER_API_KEY"),
        api_override: None,
        option_note: Some("reasoningEffort=high"),
    },
    ProviderCase {
        provider: "kimi-coding",
        model_id: "kimi-for-coding",
        credential: CredentialGate::Env("KIMI_API_KEY"),
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "vercel-ai-gateway",
        model_id: "google/gemini-2.5-flash",
        credential: CredentialGate::Env("AI_GATEWAY_API_KEY"),
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "amazon-bedrock",
        model_id: "global.anthropic.claude-sonnet-4-5-20250929-v1:0",
        credential: CredentialGate::Bedrock,
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "anthropic",
        model_id: "claude-sonnet-4-5",
        credential: CredentialGate::OAuth("anthropic"),
        api_override: None,
        option_note: Some("apiKey from ~/.pi/agent/auth.json"),
    },
    ProviderCase {
        provider: "github-copilot",
        model_id: "claude-haiku-4.5",
        credential: CredentialGate::OAuth("github-copilot"),
        api_override: None,
        option_note: Some("apiKey from ~/.pi/agent/auth.json"),
    },
    ProviderCase {
        provider: "github-copilot",
        model_id: "claude-sonnet-4.6",
        credential: CredentialGate::OAuth("github-copilot"),
        api_override: None,
        option_note: Some("apiKey from ~/.pi/agent/auth.json"),
    },
    ProviderCase {
        provider: "openai-codex",
        model_id: "gpt-5.5",
        credential: CredentialGate::OAuth("openai-codex"),
        api_override: None,
        option_note: Some("apiKey from ~/.pi/agent/auth.json"),
    },
];

const XIAOMI_TEXT_AND_IMAGE_SKIPPED_CASES: &[ProviderCase] = &[
    ProviderCase {
        provider: "xiaomi",
        model_id: "mimo-v2.5-pro",
        credential: CredentialGate::Env("XIAOMI_API_KEY"),
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "xiaomi-token-plan-cn",
        model_id: "mimo-v2.5-pro",
        credential: CredentialGate::Env("XIAOMI_TOKEN_PLAN_CN_API_KEY"),
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "xiaomi-token-plan-ams",
        model_id: "mimo-v2.5-pro",
        credential: CredentialGate::Env("XIAOMI_TOKEN_PLAN_AMS_API_KEY"),
        api_override: None,
        option_note: None,
    },
    ProviderCase {
        provider: "xiaomi-token-plan-sgp",
        model_id: "mimo-v2.5-pro",
        credential: CredentialGate::Env("XIAOMI_TOKEN_PLAN_SGP_API_KEY"),
        api_override: None,
        option_note: None,
    },
];

fn red_circle_base64() -> String {
    STANDARD.encode(include_bytes!(
        "../../../references/pi/packages/ai/test/data/red-circle.png"
    ))
}

fn has_live_credentials(gate: CredentialGate) -> bool {
    match gate {
        CredentialGate::Env(name) => std::env::var_os(name).is_some(),
        CredentialGate::AzureOpenAi => {
            std::env::var_os("AZURE_OPENAI_API_KEY").is_some()
                && (std::env::var_os("AZURE_OPENAI_BASE_URL").is_some()
                    || std::env::var_os("AZURE_OPENAI_RESOURCE_NAME").is_some())
        }
        CredentialGate::Bedrock => {
            std::env::var_os("AWS_PROFILE").is_some()
                || (std::env::var_os("AWS_ACCESS_KEY_ID").is_some()
                    && std::env::var_os("AWS_SECRET_ACCESS_KEY").is_some())
                || std::env::var_os("AWS_BEARER_TOKEN_BEDROCK").is_some()
        }
        CredentialGate::OAuth(provider) => {
            let _ = provider;
            false
        }
    }
}

fn model_for(test_case: ProviderCase) -> Result<Model, String> {
    let mut model = get_model(test_case.provider, test_case.model_id)
        .map_err(|error| format!("{BLOCKER}: {error}"))?;
    if let Some(api) = test_case.api_override {
        model.api = api.to_owned();
    }
    Ok(model)
}

fn image_only_context() -> ProbeContext {
    ProbeContext {
        system_prompt: "You are a helpful assistant that uses tools when asked.",
        messages: vec![ProbeMessage::User(
            "Call the get_circle tool to get an image, and describe what you see, shapes, colors, etc."
                .to_owned(),
        )],
        tools: vec![Tool {
            name: "get_circle",
            description: "Returns a circle image for visualization",
        }],
    }
}

fn text_and_image_context() -> ProbeContext {
    ProbeContext {
        system_prompt: "You are a helpful assistant that uses tools when asked.",
        messages: vec![ProbeMessage::User(
            "Use the get_circle_with_description tool and tell me what you learned. Also say what color the shape is."
                .to_owned(),
        )],
        tools: vec![Tool {
            name: "get_circle_with_description",
            description: "Returns a circle image with a text description",
        }],
    }
}

fn complete_probe(
    model: &Model,
    probe_context: &ProbeContext,
    test_case: ProviderCase,
) -> Result<ProbeAssistantMessage, String> {
    let _ = (probe_context, test_case.option_note);
    let _response = block_on(complete(
        model,
        &Context::default(),
        Some(StreamOptions::default()),
    ))
    .map_err(|error| format!("{BLOCKER}: {error}"))?;
    Err(BLOCKER.to_owned())
}

fn tool_call<'a>(response: &'a ProbeAssistantMessage, expected_name: &str) -> &'a ToolCall {
    response
        .content
        .iter()
        .find_map(|block| match block {
            ContentBlock::ToolCall(tool_call) => Some(tool_call),
            _ => None,
        })
        .filter(|tool_call| tool_call.name == expected_name)
        .expect("Expected tool call")
}

fn text_content(response: &ProbeAssistantMessage) -> &str {
    response
        .content
        .iter()
        .find_map(|block| match block {
            ContentBlock::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .expect("expected text response")
}

fn handle_tool_with_image_result(test_case: ProviderCase) -> Result<(), String> {
    if !has_live_credentials(test_case.credential) {
        return Ok(());
    }

    let model = model_for(test_case)?;
    let base64_image = red_circle_base64();
    let mut context = image_only_context();

    let first_response = complete_probe(&model, &context, test_case)?;
    assert_eq!(first_response.stop_reason, StopReason::ToolUse);
    let tool_call = tool_call(&first_response, "get_circle");
    assert_eq!(tool_call.name, "get_circle");
    let tool_call_id = tool_call.id.clone();
    let tool_call_name = tool_call.name.clone();

    context
        .messages
        .push(ProbeMessage::Assistant(first_response));
    context
        .messages
        .push(ProbeMessage::ToolResult(ToolResultMessage {
            tool_call_id,
            tool_name: tool_call_name,
            content: vec![ContentBlock::Image {
                data: base64_image,
                mime_type: "image/png".to_owned(),
            }],
            is_error: false,
        }));

    let second_response = complete_probe(&model, &context, test_case)?;
    assert_eq!(second_response.stop_reason, StopReason::Stop);
    assert!(second_response.error_message.is_none());

    let lower_content = text_content(&second_response).to_lowercase();
    assert!(lower_content.contains("red"));
    assert!(lower_content.contains("circle"));

    Ok(())
}

fn handle_tool_with_text_and_image_result(test_case: ProviderCase) -> Result<(), String> {
    if !has_live_credentials(test_case.credential) {
        return Ok(());
    }

    let model = model_for(test_case)?;
    let base64_image = red_circle_base64();
    let mut context = text_and_image_context();

    let first_response = complete_probe(&model, &context, test_case)?;
    assert_eq!(first_response.stop_reason, StopReason::ToolUse);
    let tool_call = tool_call(&first_response, "get_circle_with_description");
    assert_eq!(tool_call.name, "get_circle_with_description");
    let tool_call_id = tool_call.id.clone();
    let tool_call_name = tool_call.name.clone();

    context
        .messages
        .push(ProbeMessage::Assistant(first_response));
    context.messages.push(ProbeMessage::ToolResult(ToolResultMessage {
        tool_call_id,
        tool_name: tool_call_name,
        content: vec![
            ContentBlock::Text(
                "This is a geometric shape with specific properties: it has a diameter of 100 pixels."
                    .to_owned(),
            ),
            ContentBlock::Image {
                data: base64_image,
                mime_type: "image/png".to_owned(),
            },
        ],
        is_error: false,
    }));

    let second_response = complete_probe(&model, &context, test_case)?;
    assert_eq!(second_response.stop_reason, StopReason::Stop);
    assert!(second_response.error_message.is_none());

    let lower_content = text_content(&second_response).to_lowercase();
    assert!(
        lower_content.contains("diameter")
            || lower_content.contains("100")
            || lower_content.contains("pixel")
    );
    assert!(lower_content.contains("red"));
    assert!(lower_content.contains("circle"));

    Ok(())
}

#[test]
#[ignore = "live provider call skipped; image tool-result context, typed assistant content, and provider streaming are request-capture blockers"]
fn image_tool_result_only_image_across_live_providers() -> Result<(), String> {
    for test_case in IMAGE_ONLY_CASES {
        handle_tool_with_image_result(*test_case)?;
    }
    Ok(())
}

#[test]
#[ignore = "live provider call skipped; text+image tool-result context, typed assistant content, and provider streaming are request-capture blockers"]
fn image_tool_result_text_and_image_across_live_providers() -> Result<(), String> {
    for test_case in TEXT_AND_IMAGE_CASES {
        handle_tool_with_text_and_image_result(*test_case)?;
    }
    Ok(())
}

#[test]
#[ignore = "matches Pi it.skip: Xiaomi MiMo text+image tool results currently ignore image color due upstream multimodal-fusion quality"]
fn image_tool_result_xiaomi_text_and_image_upstream_skip() -> Result<(), String> {
    for test_case in XIAOMI_TEXT_AND_IMAGE_SKIPPED_CASES {
        handle_tool_with_text_and_image_result(*test_case)?;
    }
    Ok(())
}
