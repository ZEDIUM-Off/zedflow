use std::collections::HashMap;

use serde_json::{Value, json};
use zedflow_ai::compat::get_model;
use zedflow_ai::types::Model;
use zedflow_ai::types::{
    AssistantContentBlock, AssistantMessage, Context, Message, StopReason, TextContent,
    TextContentType, ThinkingLevel, Tool, ToolCall, ToolResultContentBlock, ToolResultMessage,
    ToolResultMessageRole, UserMessage, UserMessageContent, UserMessageRole,
};

const BLOCKER: &str = "live Bedrock/Anthropic interleaved-thinking parity test skipped; requires provider credentials/network, completeSimple, and real provider streaming";

fn calculator_tool() -> Tool {
    Tool {
        name: "calculator".to_owned(),
        description: "Perform basic arithmetic operations".to_owned(),
        parameters: json!({
            "type": "object",
            "properties": {
                "a": { "type": "number", "description": "First number" },
                "b": { "type": "number", "description": "Second number" },
                "operation": {
                    "type": "string",
                    "enum": ["add", "subtract", "multiply", "divide"],
                    "description": "The operation to perform."
                }
            },
            "required": ["a", "b", "operation"]
        }),
    }
}

fn make_context() -> Context {
    Context {
        system_prompt: Some(
            [
                "You are a helpful assistant that must use tools for arithmetic.",
                "Always think before every tool call, not just the first one.",
                "Do not answer with plain text when a tool call is required.",
            ]
            .join(" "),
        ),
        messages: vec![Message::User(UserMessage {
            role: UserMessageRole::User,
            content: UserMessageContent::Text(
                [
                    "Use calculator to calculate 328 * 29.",
                    "You must call the calculator tool exactly once.",
                    "Provide the final answer based on the best guess given the tool result, even if it seems unreliable.",
                    "Start by thinking about the steps you will take to solve the problem.",
                ]
                .join(" "),
            ),
            timestamp: 0,
        })],
        tools: Some(vec![calculator_tool()]),
    }
}

fn get_live_model(provider: &str, id: &str) -> Result<Model, String> {
    get_model(provider, id).ok_or_else(|| format!("{BLOCKER}: unknown model {provider}/{id}"))
}

fn complete_simple_interleaved(
    _llm: &Model,
    _context: &Context,
    _reasoning: ThinkingLevel,
) -> Result<AssistantMessage, String> {
    Err(BLOCKER.to_owned())
}

fn has_thinking(response: &AssistantMessage) -> bool {
    response
        .content
        .iter()
        .any(|block| matches!(block, AssistantContentBlock::Thinking(_)))
}

fn has_tool_call(response: &AssistantMessage) -> bool {
    response
        .content
        .iter()
        .any(|block| matches!(block, AssistantContentBlock::ToolCall(_)))
}

fn has_text(response: &AssistantMessage) -> bool {
    response
        .content
        .iter()
        .any(|block| matches!(block, AssistantContentBlock::Text(_)))
}

fn first_tool_call(response: &AssistantMessage) -> Option<ToolCall> {
    response.content.iter().find_map(|block| match block {
        AssistantContentBlock::ToolCall(tool_call) => Some(tool_call.clone()),
        _ => None,
    })
}

fn number_argument(arguments: &HashMap<String, Value>, key: &str) -> Result<f64, String> {
    arguments
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| "Invalid calculator arguments".to_owned())
}

fn operation_argument(arguments: &HashMap<String, Value>) -> Result<&str, String> {
    let operation = arguments
        .get("operation")
        .and_then(Value::as_str)
        .ok_or_else(|| "Invalid calculator arguments".to_owned())?;

    match operation {
        "add" | "subtract" | "multiply" | "divide" => Ok(operation),
        _ => Err("Invalid calculator arguments".to_owned()),
    }
}

fn evaluate_calculator_call(tool_call: &ToolCall) -> Result<f64, String> {
    let a = number_argument(&tool_call.arguments, "a")?;
    let b = number_argument(&tool_call.arguments, "b")?;

    Ok(match operation_argument(&tool_call.arguments)? {
        "add" => a + b,
        "subtract" => a - b,
        "multiply" => a * b,
        "divide" => a / b,
        _ => unreachable!("operation_argument accepts only calculator operations"),
    })
}

fn assert_second_tool_call_with_interleaved_thinking(
    llm: Model,
    reasoning: ThinkingLevel,
) -> Result<(), String> {
    let mut context = make_context();

    let first_response = complete_simple_interleaved(&llm, &context, reasoning)?;

    assert_eq!(
        first_response.stop_reason,
        StopReason::ToolUse,
        "Error: {:?}",
        first_response.error_message
    );
    assert!(has_thinking(&first_response));
    assert!(has_tool_call(&first_response));

    let first_tool_call = first_tool_call(&first_response)
        .ok_or_else(|| "Expected first response to include a tool call".to_owned())?;

    context.messages.push(Message::Assistant(first_response));

    let correct_answer = evaluate_calculator_call(&first_tool_call)?;
    context
        .messages
        .push(Message::ToolResult(ToolResultMessage {
            role: ToolResultMessageRole::ToolResult,
            tool_call_id: first_tool_call.id.clone(),
            tool_name: first_tool_call.name.clone(),
            content: vec![ToolResultContentBlock::Text(TextContent {
                content_type: TextContentType::Text,
                text: format!(
                    "The answer is {correct_answer} or {}.",
                    correct_answer * 2.0
                ),
                text_signature: None,
            })],
            details: None,
            is_error: false,
            timestamp: 0,
        }));

    let second_response = complete_simple_interleaved(&llm, &context, reasoning)?;

    assert_eq!(
        second_response.stop_reason,
        StopReason::Stop,
        "Error: {:?}",
        second_response.error_message
    );
    assert!(has_thinking(&second_response));
    assert!(has_text(&second_response));

    Ok(())
}

#[test]
#[ignore = "live Bedrock provider call skipped; requires credentials/network and completeSimple/provider streaming parity not yet ported"]
fn bedrock_interleaved_thinking_on_claude_opus_4_5() {
    let llm = get_live_model(
        "amazon-bedrock",
        "global.anthropic.claude-opus-4-5-20251101-v1:0",
    )
    .expect("getModel should return Bedrock Claude Opus 4.5");

    assert_second_tool_call_with_interleaved_thinking(llm, ThinkingLevel::High)
        .expect("Claude Opus 4.5 should do interleaved thinking");
}

#[test]
#[ignore = "live Bedrock provider call skipped; requires credentials/network and completeSimple/provider streaming parity not yet ported"]
fn bedrock_interleaved_thinking_on_claude_opus_4_6() {
    let llm = get_live_model("amazon-bedrock", "global.anthropic.claude-opus-4-6-v1")
        .expect("getModel should return Bedrock Claude Opus 4.6");

    assert_second_tool_call_with_interleaved_thinking(llm, ThinkingLevel::High)
        .expect("Claude Opus 4.6 should do interleaved thinking");
}

#[test]
#[ignore = "live Anthropic provider call skipped; requires credentials/network and completeSimple/provider streaming parity not yet ported"]
fn anthropic_interleaved_thinking_on_claude_opus_4_5() {
    let llm = get_live_model("anthropic", "claude-opus-4-5")
        .expect("getModel should return Anthropic Claude Opus 4.5");

    assert_second_tool_call_with_interleaved_thinking(llm, ThinkingLevel::High)
        .expect("Claude Opus 4.5 should do interleaved thinking");
}

#[test]
#[ignore = "live Anthropic provider call skipped; requires credentials/network and completeSimple/provider streaming parity not yet ported"]
fn anthropic_interleaved_thinking_on_claude_opus_4_6() {
    let llm = get_live_model("anthropic", "claude-opus-4-6")
        .expect("getModel should return Anthropic Claude Opus 4.6");

    assert_second_tool_call_with_interleaved_thinking(llm, ThinkingLevel::High)
        .expect("Claude Opus 4.6 should do interleaved thinking");
}
