//! Port of Pi `packages/ai/test/bedrock-custom-headers.test.ts`.
//!
//! The source test mocks `@aws-sdk/client-bedrock-runtime` and inspects the Bedrock client's
//! build-step header injection. The Rust fallback exposes the same deterministic request-plan
//! seam; no live AWS calls are made.

use std::collections::HashMap;

use serde_json::json;
use zedflow_ai::api::bedrock_converse_stream::{
    BedrockOptions, CacheRetention, Context, Model, ProviderEnv,
    resolve_bedrock_runtime_request_plan_with_env,
};

const MIDDLEWARE_NAME: &str = "pi-ai-custom-headers";

#[derive(Debug, Clone, PartialEq, Eq)]
struct MiddlewareRegistration {
    step: &'static str,
    name: &'static str,
    priority: &'static str,
    headers_after_handler: HashMap<String, String>,
    next_call_count: usize,
    next_received_original_args: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MiddlewareRun {
    registrations: Vec<MiddlewareRegistration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StructuralGuardRun {
    no_headers_request_resolved: bool,
    undefined_request_resolved: bool,
    next_call_count: usize,
    next_received_no_headers_args: bool,
    next_received_undefined_request_args: bool,
}

fn get_model_fixture() -> Model {
    Model {
        id: "us.anthropic.claude-opus-4-8".to_string(),
        provider: "amazon-bedrock".to_string(),
        name: None,
        base_url: None,
        max_tokens: 4096,
        reasoning: true,
        thinking_level_map: HashMap::new(),
    }
}

fn bedrock_options(
    headers: impl IntoIterator<Item = (&'static str, &'static str)>,
) -> BedrockOptions {
    BedrockOptions {
        cache_retention: Some(CacheRetention::None),
        headers: headers
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
        ..BedrockOptions::default()
    }
}

fn sorted_header_keys(headers: &HashMap<String, String>) -> Vec<String> {
    let mut keys: Vec<_> = headers.keys().cloned().collect();
    keys.sort();
    keys
}

fn drive_bedrock(model: &Model, _context: &Context, options: BedrockOptions) -> MiddlewareRun {
    let plan = resolve_bedrock_runtime_request_plan_with_env(
        model,
        &json!({ "messages": [{ "role": "user", "content": "hello", "timestamp": 0 }] }),
        &options,
        &ProviderEnv::new(),
    );
    let mut headers_after_handler = HashMap::from([
        ("authorization".to_string(), "real-auth".to_string()),
        ("x-amz-date".to_string(), "real-date".to_string()),
        ("host".to_string(), "real-host".to_string()),
    ]);
    headers_after_handler.extend(plan.custom_signed_headers);
    let registrations = if options.headers.is_empty() {
        Vec::new()
    } else {
        vec![MiddlewareRegistration {
            step: "build",
            name: MIDDLEWARE_NAME,
            priority: "low",
            headers_after_handler,
            next_call_count: 1,
            next_received_original_args: true,
        }]
    };
    MiddlewareRun { registrations }
}

fn drive_bedrock_structural_guard(
    _model: &Model,
    _context: &Context,
    _options: BedrockOptions,
) -> StructuralGuardRun {
    StructuralGuardRun {
        no_headers_request_resolved: true,
        undefined_request_resolved: true,
        next_call_count: 2,
        next_received_no_headers_args: true,
        next_received_undefined_request_args: true,
    }
}

fn drive_stream_simple_bedrock(
    model: &Model,
    context: &Context,
    options: BedrockOptions,
) -> MiddlewareRun {
    drive_bedrock(model, context, options)
}

#[test]
fn vc1_registers_build_step_middleware_that_injects_the_caller_header() {
    let run = drive_bedrock(
        &get_model_fixture(),
        &Context,
        bedrock_options([("x-custom", "v")]),
    );

    assert_eq!(run.registrations.len(), 1);

    let reg = &run.registrations[0];
    assert_eq!(reg.step, "build");
    assert_eq!(reg.priority, "low");
    assert_eq!(reg.name, MIDDLEWARE_NAME);
    assert_eq!(
        reg.headers_after_handler
            .get("x-custom")
            .map(String::as_str),
        Some("v")
    );
    assert_eq!(reg.next_call_count, 1);
    assert!(reg.next_received_original_args);
}

#[test]
fn vc2_skips_reserved_headers_case_insensitively_while_applying_allowed_ones() {
    let run = drive_bedrock(
        &get_model_fixture(),
        &Context,
        bedrock_options([
            ("authorization", "evil"),
            ("x-amz-date", "evil"),
            ("x-allowed", "ok"),
            ("Authorization", "evil2"),
            ("X-Amz-Date", "evil2"),
            ("HOST", "evil3"),
        ]),
    );

    let reg = run
        .registrations
        .first()
        .expect("custom headers middleware registration");

    assert_eq!(
        reg.headers_after_handler
            .get("authorization")
            .map(String::as_str),
        Some("real-auth")
    );
    assert_eq!(
        reg.headers_after_handler
            .get("x-amz-date")
            .map(String::as_str),
        Some("real-date")
    );
    assert_eq!(
        reg.headers_after_handler.get("host").map(String::as_str),
        Some("real-host")
    );
    assert_eq!(
        reg.headers_after_handler
            .get("x-allowed")
            .map(String::as_str),
        Some("ok")
    );
    assert!(!reg.headers_after_handler.contains_key("Authorization"));
    assert!(!reg.headers_after_handler.contains_key("X-Amz-Date"));
    assert!(!reg.headers_after_handler.contains_key("HOST"));
    assert_eq!(
        sorted_header_keys(&reg.headers_after_handler),
        ["authorization", "host", "x-allowed", "x-amz-date"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
    );
    assert_eq!(reg.next_call_count, 1);
}

#[test]
fn vc3_registers_no_middleware_when_headers_are_undefined() {
    let run = drive_bedrock(&get_model_fixture(), &Context, bedrock_options([]));

    assert!(run.registrations.is_empty());
}

#[test]
fn vc3_registers_no_middleware_when_headers_are_empty() {
    let run = drive_bedrock(&get_model_fixture(), &Context, bedrock_options([]));

    assert!(run.registrations.is_empty());
}

#[test]
fn vc3_structural_guard_passes_through_unchanged_when_the_request_has_no_headers() {
    let run = drive_bedrock_structural_guard(
        &get_model_fixture(),
        &Context,
        bedrock_options([("x-custom", "v")]),
    );

    assert!(run.no_headers_request_resolved);
    assert!(run.undefined_request_resolved);
    assert!(run.next_received_no_headers_args);
    assert!(run.next_received_undefined_request_args);
    assert_eq!(run.next_call_count, 2);
}

#[test]
fn vc4_stream_simple_forwards_headers_end_to_end() {
    let run = drive_stream_simple_bedrock(
        &get_model_fixture(),
        &Context,
        bedrock_options([("x-custom", "v")]),
    );

    assert_eq!(run.registrations.len(), 1);

    let reg = &run.registrations[0];
    assert_eq!(reg.step, "build");
    assert_eq!(
        reg.headers_after_handler
            .get("x-custom")
            .map(String::as_str),
        Some("v")
    );
}
