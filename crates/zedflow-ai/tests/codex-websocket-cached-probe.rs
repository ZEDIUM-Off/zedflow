use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use zedflow_ai::api::openai_codex_responses::{
    Context, Model, OpenAICodexResponsesOptions, ReasoningEffort, Tool, Transport,
    close_openai_codex_websocket_sessions, get_openai_codex_websocket_debug_stats,
    reset_openai_codex_websocket_debug_stats, stream,
};

const DEFAULT_TURNS: usize = 20;
const DEFAULT_MAX_TOKENS: u32 = 64;
const BLOCKER: &str = "live OpenAI Codex Responses websocket-cached probe skipped; coding-agent AuthStorage, compat::get_model(openai-codex/gpt-5.5), and the Codex WebSocket/SSE transport remain PORT PLACEHOLDERs, and P1.T2 forbids live provider calls";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThinkingLevel {
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
}

impl From<ThinkingLevel> for ReasoningEffort {
    fn from(value: ThinkingLevel) -> Self {
        match value {
            ThinkingLevel::Minimal => Self::Minimal,
            ThinkingLevel::Low => Self::Low,
            ThinkingLevel::Medium => Self::Medium,
            ThinkingLevel::High => Self::High,
            ThinkingLevel::XHigh => Self::XHigh,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Args {
    turns: usize,
    transport: Transport,
    max_tokens: u32,
    reasoning: ThinkingLevel,
    session_id: String,
}

fn parse_args(argv: &[&str]) -> Result<Args, String> {
    let mut args = Args {
        turns: DEFAULT_TURNS,
        transport: Transport::WebSocketCached,
        max_tokens: DEFAULT_MAX_TOKENS,
        reasoning: ThinkingLevel::Low,
        session_id: default_session_id(),
    };

    let mut index = 0;
    while index < argv.len() {
        let arg = argv[index];
        match arg {
            "--turns" => {
                index += 1;
                args.turns = required(argv.get(index).copied(), arg)?
                    .parse()
                    .map_err(|_| format!("Invalid --turns: {}", argv[index]))?;
            }
            "--transport" => {
                index += 1;
                args.transport = match required(argv.get(index).copied(), arg)? {
                    "sse" => Transport::Sse,
                    "websocket" => Transport::WebSocket,
                    "websocket-cached" => Transport::WebSocketCached,
                    "auto" => Transport::Auto,
                    value => return Err(format!("Invalid --transport: {value}")),
                };
            }
            "--max-tokens" => {
                index += 1;
                args.max_tokens = required(argv.get(index).copied(), arg)?
                    .parse()
                    .map_err(|_| format!("Invalid --max-tokens: {}", argv[index]))?;
            }
            "--reasoning" => {
                index += 1;
                args.reasoning = match required(argv.get(index).copied(), arg)? {
                    "minimal" => ThinkingLevel::Minimal,
                    "low" => ThinkingLevel::Low,
                    "medium" => ThinkingLevel::Medium,
                    "high" => ThinkingLevel::High,
                    "xhigh" => ThinkingLevel::XHigh,
                    value => return Err(format!("Invalid --reasoning: {value}")),
                };
            }
            "--session-id" => {
                index += 1;
                args.session_id = required(argv.get(index).copied(), arg)?.to_owned();
            }
            "--help" => return Err(print_help()),
            _ => return Err(format!("Unknown argument: {arg}")),
        }
        index += 1;
    }

    Ok(args)
}

fn default_session_id() -> String {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());
    format!("pi-ai-codex-ws-cached-probe-{now_ms}")
}

fn required<'a>(value: Option<&'a str>, flag: &str) -> Result<&'a str, String> {
    value.ok_or_else(|| format!("Missing value for {flag}"))
}

fn print_help() -> String {
    format!(
        "Usage: node test/codex-websocket-cached-probe.ts [options]\n\nOptions:\n  --turns <n>          Number of user turns. Default: {DEFAULT_TURNS}\n  --transport <mode>   sse | websocket | websocket-cached | auto. Default: websocket-cached\n  --reasoning <level>  minimal | low | medium | high | xhigh. Default: low\n  --max-tokens <n>     Max output tokens per model request. Default: {DEFAULT_MAX_TOKENS}\n  --session-id <id>    Session id for websocket/cache state\n"
    )
}

fn build_prompt(turn: usize) -> String {
    let marker = format!("TURN-{turn:02}-MARKER-{}", (turn * 17 + 13) % 97);
    let mut lines = vec![
        "This is an automated OpenAI Codex Responses websocket cache probe.".to_owned(),
        format!(
            "Task for turn {turn}: call deterministic_probe exactly once before your final answer."
        ),
        format!("Use tool arguments: turn={turn}, marker={marker}"),
        format!("After the tool result arrives, reply exactly: TURN {turn} OK {marker}"),
        "The following repeated block is intentional benchmark padding.".to_owned(),
    ];
    for i in 1..=180 {
        lines.push(format!(
            "Turn {turn} synthetic record {i:03}: alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega."
        ));
    }
    lines.join("\n")
}

fn deterministic_probe_tool() -> Tool {
    Tool {
        name: "deterministic_probe".to_owned(),
        description: "Mandatory benchmark tool. Call exactly once with the turn and marker from the user prompt.".to_owned(),
        parameters: json!({
            "type": "object",
            "properties": {
                "turn": { "type": "number" },
                "marker": { "type": "string" }
            },
            "required": ["turn", "marker"]
        }),
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ToolCall {
    id: String,
    name: String,
    arguments: Value,
}

#[derive(Debug, Clone, PartialEq)]
struct ToolResultMessage {
    tool_call_id: String,
    tool_name: String,
    content: String,
    details: Value,
    is_error: bool,
}

fn execute_tool(call: &ToolCall) -> ToolResultMessage {
    ToolResultMessage {
        tool_call_id: call.id.clone(),
        tool_name: call.name.clone(),
        content: format!(
            "deterministic_probe_result {} fixed=OK",
            serde_json::to_string(&call.arguments).expect("tool arguments should serialize")
        ),
        details: json!({ "fixed": "OK" }),
        is_error: false,
    }
}

#[derive(Debug, Clone, PartialEq)]
enum AssistantBlock {
    Text(String),
    ToolCall(ToolCall),
}

#[derive(Debug, Clone, PartialEq)]
struct AssistantMessage {
    content: Vec<AssistantBlock>,
}

fn text_of(message: &AssistantMessage) -> String {
    message
        .content
        .iter()
        .filter_map(|block| match block {
            AssistantBlock::Text(text) => Some(text.as_str()),
            AssistantBlock::ToolCall(_) => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

fn average(values: &[u64]) -> f64 {
    let total: u64 = values.iter().sum();
    total as f64 / values.len().max(1) as f64
}

fn percentile(values: &[u64], p: u64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let index = ((p as f64 / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[index.saturating_sub(1).min(sorted.len() - 1)]
}

fn fake_account_token() -> String {
    "eyJhbGciOiJub25lIn0.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9hY2NvdW50X2lkIjoiYWNjdF90ZXN0In19.signature".to_owned()
}

fn codex_probe_model(max_tokens: u32) -> Model {
    Model {
        id: "gpt-5.5".to_owned(),
        provider: "openai-codex".to_owned(),
        base_url: None,
        reasoning: true,
        thinking_level_map: HashMap::new(),
        headers: HashMap::new(),
        max_tokens: Some(max_tokens),
    }
}

#[test]
fn codex_websocket_cached_probe_args_match_pi_defaults_and_overrides() {
    let defaults = parse_args(&[]).expect("default args should parse");
    assert_eq!(defaults.turns, DEFAULT_TURNS);
    assert_eq!(defaults.transport, Transport::WebSocketCached);
    assert_eq!(defaults.max_tokens, DEFAULT_MAX_TOKENS);
    assert_eq!(defaults.reasoning, ThinkingLevel::Low);
    assert!(
        defaults
            .session_id
            .starts_with("pi-ai-codex-ws-cached-probe-")
    );

    let args = parse_args(&[
        "--turns",
        "3",
        "--transport",
        "sse",
        "--max-tokens",
        "128",
        "--reasoning",
        "xhigh",
        "--session-id",
        "probe-session",
    ])
    .expect("explicit args should parse");
    assert_eq!(args.turns, 3);
    assert_eq!(args.transport, Transport::Sse);
    assert_eq!(args.max_tokens, 128);
    assert_eq!(args.reasoning, ThinkingLevel::XHigh);
    assert_eq!(args.session_id, "probe-session");

    assert_eq!(
        parse_args(&["--transport", "http"]).expect_err("invalid transport should fail"),
        "Invalid --transport: http"
    );
    assert_eq!(
        parse_args(&["--session-id"]).expect_err("missing value should fail"),
        "Missing value for --session-id"
    );
}

#[test]
fn codex_websocket_cached_probe_prompt_tool_and_text_helpers_match_pi() {
    let prompt = build_prompt(2);
    assert!(prompt.contains("Use tool arguments: turn=2, marker=TURN-02-MARKER-47"));
    assert!(
        prompt
            .contains("After the tool result arrives, reply exactly: TURN 2 OK TURN-02-MARKER-47")
    );
    assert_eq!(prompt.lines().count(), 185);

    let tool = deterministic_probe_tool();
    assert_eq!(tool.name, "deterministic_probe");
    assert_eq!(tool.parameters["required"], json!(["turn", "marker"]));

    let call = ToolCall {
        id: "call-1".to_owned(),
        name: "deterministic_probe".to_owned(),
        arguments: json!({ "turn": 2, "marker": "TURN-02-MARKER-47" }),
    };
    let result = execute_tool(&call);
    assert_eq!(result.tool_call_id, "call-1");
    assert_eq!(result.tool_name, "deterministic_probe");
    assert_eq!(
        result.content,
        "deterministic_probe_result {\"marker\":\"TURN-02-MARKER-47\",\"turn\":2} fixed=OK"
    );
    assert!(!result.is_error);

    let message = AssistantMessage {
        content: vec![
            AssistantBlock::Text(" first ".to_owned()),
            AssistantBlock::ToolCall(call),
            AssistantBlock::Text("second".to_owned()),
        ],
    };
    assert_eq!(text_of(&message), "first \nsecond");
}

#[test]
fn codex_websocket_cached_probe_timing_helpers_match_pi() {
    assert_eq!(average(&[]), 0.0);
    assert_eq!(average(&[100, 200, 300]), 200.0);
    assert_eq!(percentile(&[], 95), 0);
    assert_eq!(percentile(&[10, 20, 30, 40], 50), 20);
    assert_eq!(percentile(&[10, 20, 30, 40], 95), 40);
}

#[test]
#[ignore = "live provider/AuthStorage/websocket-cached transport parity is blocked; see BLOCKER"]
fn codex_websocket_cached_probe_live_loop_is_represented_as_ignored() {
    let args = parse_args(&[
        "--turns",
        "1",
        "--transport",
        "websocket-cached",
        "--reasoning",
        "low",
        "--max-tokens",
        "64",
        "--session-id",
        "pi-ai-codex-ws-cached-probe-rust",
    ])
    .expect("probe args should parse");
    let mut context = Context {
        system_prompt: Some(
            "You are participating in a benchmark. For each benchmark turn, call deterministic_probe exactly once before the final answer. Keep final answers minimal."
                .to_owned(),
        ),
        tools: vec![deterministic_probe_tool()],
        input: Vec::new(),
    };
    context
        .input
        .push(json!({ "role": "user", "content": build_prompt(1) }));

    reset_openai_codex_websocket_debug_stats(Some(&args.session_id));
    assert!(get_openai_codex_websocket_debug_stats(&args.session_id).is_none());

    let error = stream(
        &codex_probe_model(args.max_tokens),
        &context,
        Some(&OpenAICodexResponsesOptions {
            api_key: Some(fake_account_token()),
            session_id: Some(args.session_id.clone()),
            transport: Some(args.transport),
            reasoning_effort: Some(args.reasoning.into()),
            max_tokens: Some(args.max_tokens),
            ..OpenAICodexResponsesOptions::default()
        }),
    )
    .expect_err(BLOCKER)
    .to_string();

    assert!(error.contains("port placeholder for fetch/WebSocket/OpenAI Responses event stream"));
    close_openai_codex_websocket_sessions(Some(&args.session_id));
}
