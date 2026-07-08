//! Port of Pi `packages/ai/test/openrouter-cache-write-repro.test.ts`.
//!
//! The source test is a live OpenRouter request gated by `OPENROUTER_API_KEY`. P1.T2 forbids live
//! provider calls, and Rust `compat::get_model` plus the OpenAI-completions streaming transport are
//! still documented port placeholders, so the parity E2E assertion is ignored until those blockers
//! are removed.

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const BLOCKER: &str = "live OpenRouter cache_write repro skipped; requires OPENROUTER_API_KEY plus completed compat::get_model/complete_simple and OpenAI-completions streaming transport ports";
const PROVIDER: &str = "openrouter";
const MODEL: &str = "google/gemini-2.5-flash";
const USER_PROMPT: &str = "Reply with exactly: OK";
const CACHE_PROBE_TEXT: &str = "Prompt-caching probe content. Keep this exact text stable across requests so the provider can reuse prefix tokens and report cache read and cache write usage.";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Usage {
    cache_write: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Completion {
    stop_reason: &'static str,
    error_message: Option<String>,
    usage: Usage,
}

fn create_long_system_prompt() -> String {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());
    format!(
        "You are a concise assistant.\nCache nonce: {nonce}-{}\n\n{}",
        std::process::id(),
        std::iter::repeat_n(CACHE_PROBE_TEXT, 80)
            .collect::<Vec<_>>()
            .join("\n\n")
    )
}

fn mark_last_user_message_cache_control(payload: &mut Value) {
    let Some(messages) = payload.get_mut("messages").and_then(Value::as_array_mut) else {
        return;
    };

    for message in messages.iter_mut().rev() {
        if message.get("role").and_then(Value::as_str) != Some("user") {
            continue;
        }

        let Some(content) = message.get_mut("content") else {
            break;
        };
        if let Some(text) = content.as_str() {
            *content =
                json!([{ "type": "text", "text": text, "cache_control": { "type": "ephemeral" } }]);
            break;
        }

        if let Some(parts) = content.as_array_mut() {
            if let Some(part) = parts
                .iter_mut()
                .rev()
                .find(|part| part.get("type").and_then(Value::as_str) == Some("text"))
            {
                part["cache_control"] = json!({ "type": "ephemeral" });
            }
        }
        break;
    }
}

fn run_live_openrouter_cache_write_probe() -> (Completion, Completion) {
    let mut payload = json!({
        "model": MODEL,
        "messages": [
            { "role": "system", "content": create_long_system_prompt() },
            { "role": "user", "content": USER_PROMPT, "timestamp": 0_u64 }
        ],
        "max_tokens": 32,
        "temperature": 0
    });
    mark_last_user_message_cache_control(&mut payload);

    let _source_fixture = (PROVIDER, MODEL, "OPENROUTER_API_KEY", payload);
    panic!("{BLOCKER}");
}

#[test]
fn marks_last_user_message_with_ephemeral_cache_control() {
    let mut payload = json!({
        "messages": [
            { "role": "user", "content": "first" },
            { "role": "assistant", "content": "ignored" },
            { "role": "user", "content": [{ "type": "text", "text": "last" }] }
        ]
    });

    mark_last_user_message_cache_control(&mut payload);

    assert_eq!(
        payload["messages"][2]["content"][0]["cache_control"],
        json!({ "type": "ephemeral" })
    );
    assert!(payload["messages"][0]["content"].as_str().is_some());
}

#[test]
#[ignore = "live OpenRouter provider call skipped; see BLOCKER"]
fn regression_preserves_cache_write_tokens_on_openai_completions_stream_path() {
    let (first, second) = run_live_openrouter_cache_write_probe();

    assert_eq!(first.stop_reason, "stop", "{:?}", first.error_message);
    assert_eq!(second.stop_reason, "stop", "{:?}", second.error_message);

    // Regression expectation: cache_write_tokens from provider usage must be preserved.
    // With the cache_control marker above, at least one of the two calls should create cache.
    let has_cache_write = first.usage.cache_write > 0 || second.usage.cache_write > 0;
    assert!(has_cache_write);
}
