use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use futures::executor::block_on;
use zedflow_ai::api::anthropic_messages;
use zedflow_ai::api::openai_completions;
use zedflow_ai::api::openai_responses;
use zedflow_ai::providers::anthropic::anthropic_provider;
use zedflow_ai::providers::opencode_go_models::OPENCODE_GO_MODELS;
use zedflow_ai::providers::opencode_models::OPENCODE_MODELS;
use zedflow_ai::types as shared_types;

fn responses_model() -> openai_responses::Model {
    openai_responses::Model {
        id: "gpt-4o-mini".to_string(),
        api: "openai-responses".to_string(),
        provider: "openai".to_string(),
        base_url: "https://api.openai.com/v1".to_string(),
        reasoning: false,
        thinking_level_map: HashMap::new(),
        headers: openai_responses::ProviderHeaders::new(),
        compat: None,
    }
}

fn completions_model(
    compat: Option<openai_completions::OpenAICompletionsCompat>,
) -> openai_completions::Model {
    openai_completions::Model {
        id: "test-model".to_string(),
        api: "openai-completions".to_string(),
        provider: "test-openai-completions".to_string(),
        base_url: "https://my-proxy.example.com/v1".to_string(),
        input: vec![openai_completions::ModelInput::Text],
        reasoning: false,
        thinking_level_map: HashMap::new(),
        headers: openai_completions::ProviderHeaders::new(),
        max_tokens: 4096,
        context_window: None,
        compat,
    }
}

#[test]
fn anthropic_uses_default_cache_ttl_when_pi_cache_retention_is_not_set() {
    assert_eq!(
        anthropic_messages::resolve_cache_retention(None, None),
        shared_types::CacheRetention::Short
    );
    assert_eq!(
        anthropic_messages::cache_control(shared_types::CacheRetention::Short, true),
        Some(anthropic_messages::CacheControlEphemeral {
            r#type: "ephemeral",
            ttl: None,
        })
    );
}

#[test]
fn anthropic_uses_one_hour_cache_ttl_when_pi_cache_retention_is_long() {
    let mut env = shared_types::ProviderEnv::new();
    env.insert("PI_CACHE_RETENTION".to_string(), "long".to_string());

    let retention = anthropic_messages::resolve_cache_retention(None, Some(&env));

    assert_eq!(retention, shared_types::CacheRetention::Long);
    assert_eq!(
        anthropic_messages::cache_control(retention, true),
        Some(anthropic_messages::CacheControlEphemeral {
            r#type: "ephemeral",
            ttl: Some("1h"),
        })
    );
}

#[test]
fn anthropic_adds_ttl_for_long_retention_by_default() {
    assert_eq!(
        anthropic_messages::cache_control(shared_types::CacheRetention::Long, true),
        Some(anthropic_messages::CacheControlEphemeral {
            r#type: "ephemeral",
            ttl: Some("1h"),
        })
    );
}

#[test]
fn anthropic_omits_ttl_when_long_cache_retention_is_unsupported() {
    assert_eq!(
        anthropic_messages::cache_control(shared_types::CacheRetention::Long, false),
        Some(anthropic_messages::CacheControlEphemeral {
            r#type: "ephemeral",
            ttl: None,
        })
    );
}

#[test]
fn anthropic_omits_cache_control_when_cache_retention_is_none() {
    assert_eq!(
        anthropic_messages::cache_control(shared_types::CacheRetention::None, true),
        None
    );
}

#[test]
fn anthropic_adds_cache_control_to_string_user_messages() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind capture server");
    let address = listener.local_addr().expect("capture server address");
    let server = thread::spawn(move || {
        let (mut socket, _) = listener.accept().expect("accept request");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let read = socket.read(&mut buffer).expect("read request");
            request.extend_from_slice(&buffer[..read]);
            let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n")
            else {
                continue;
            };
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if request.len() >= header_end + 4 + length {
                break;
            }
        }
        socket.write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").expect("write response");
        String::from_utf8(request).expect("request UTF-8")
    });
    let model = shared_types::Model {
        id: "claude-haiku-4-5".to_owned(),
        name: "Claude Haiku 4.5".to_owned(),
        api: "anthropic-messages".to_owned(),
        provider: "anthropic".to_owned(),
        base_url: format!("http://{address}"),
        reasoning: true,
        thinking_level_map: None,
        input: vec![shared_types::ModelInput::Text],
        cost: shared_types::ModelCost {
            input: 1.0,
            output: 5.0,
            cache_read: 0.1,
            cache_write: 1.25,
        },
        context_window: 200_000,
        max_tokens: 4096,
        headers: None,
        compat: None,
    };
    let context = shared_types::Context {
        system_prompt: None,
        messages: vec![shared_types::Message::User(shared_types::UserMessage {
            role: shared_types::UserMessageRole::User,
            content: shared_types::UserMessageContent::Text("Hello".to_owned()),
            timestamp: 0,
        })],
        tools: None,
    };

    let provider = anthropic_provider().expect("registered Anthropic provider");
    let stream = provider.stream(
        &model,
        &context,
        Some(&shared_types::StreamOptions {
            api_key: Some("test-key".to_owned()),
            ..shared_types::StreamOptions::default()
        }),
    );
    assert_eq!(
        block_on(stream.result()).stop_reason,
        shared_types::StopReason::Stop
    );
    let request = server.join().expect("capture server");
    let (_, body) = request.split_once("\r\n\r\n").expect("HTTP request");
    let payload: serde_json::Value = serde_json::from_str(body).expect("JSON request body");
    let last_block = payload["messages"][0]["content"]
        .as_array()
        .and_then(|blocks| blocks.last())
        .expect("last string user message becomes a cached text block");

    assert_eq!(
        last_block["cache_control"],
        serde_json::json!({ "type": "ephemeral" })
    );
}

#[test]
fn openai_responses_does_not_set_prompt_cache_retention_when_pi_cache_retention_is_not_set() {
    let env = openai_responses::ProviderEnv::new();
    let retention = openai_responses::resolve_cache_retention(None, &env);

    assert_eq!(retention, openai_responses::CacheRetention::Short);
    assert_eq!(
        openai_responses::prompt_cache_retention(
            openai_responses::get_compat(&responses_model()),
            retention
        ),
        None
    );
}

#[test]
fn openai_responses_sets_prompt_cache_retention_when_pi_cache_retention_is_long() {
    let mut env = openai_responses::ProviderEnv::new();
    env.insert("PI_CACHE_RETENTION".to_string(), "long".to_string());
    let retention = openai_responses::resolve_cache_retention(None, &env);

    assert_eq!(retention, openai_responses::CacheRetention::Long);
    assert_eq!(
        openai_responses::prompt_cache_retention(
            openai_responses::get_compat(&responses_model()),
            retention
        ),
        Some("24h")
    );
}

#[test]
fn openai_responses_sets_prompt_cache_retention_for_proxy_base_url_by_default() {
    let mut model = responses_model();
    model.base_url = "https://my-proxy.example.com/v1".to_string();

    assert_eq!(
        openai_responses::prompt_cache_retention(
            openai_responses::get_compat(&model),
            openai_responses::CacheRetention::Long,
        ),
        Some("24h")
    );
}

#[test]
fn openai_responses_omits_prompt_cache_retention_when_unsupported() {
    let mut model = responses_model();
    model.compat = Some(openai_responses::OpenAIResponsesCompat {
        supports_long_cache_retention: Some(false),
        ..openai_responses::OpenAIResponsesCompat::default()
    });

    assert_eq!(
        openai_responses::prompt_cache_retention(
            openai_responses::get_compat(&model),
            openai_responses::CacheRetention::Long,
        ),
        None
    );
}

#[test]
fn openai_responses_omits_prompt_cache_key_when_cache_retention_is_none() {
    assert_eq!(
        openai_responses::prompt_cache_key(
            openai_responses::CacheRetention::None,
            Some("session-1")
        ),
        None
    );
    assert_eq!(
        openai_responses::prompt_cache_retention(
            openai_responses::get_compat(&responses_model()),
            openai_responses::CacheRetention::None,
        ),
        None
    );
}

#[test]
fn openai_responses_sets_prompt_cache_key_and_retention_when_cache_retention_is_long() {
    assert_eq!(
        openai_responses::prompt_cache_key(
            openai_responses::CacheRetention::Long,
            Some("session-2")
        ),
        Some("session-2".to_string())
    );
    assert_eq!(
        openai_responses::prompt_cache_retention(
            openai_responses::get_compat(&responses_model()),
            openai_responses::CacheRetention::Long,
        ),
        Some("24h")
    );
}

#[test]
fn openai_completions_sets_prompt_cache_retention_for_proxy_base_url_by_default() {
    let model = completions_model(None);
    let compat = openai_completions::get_compat(&model);

    assert_eq!(
        openai_completions::prompt_cache_key(
            &model,
            &compat,
            openai_completions::CacheRetention::Long,
            Some("session-completions"),
        ),
        Some("session-completions".to_string())
    );
    assert_eq!(
        openai_completions::prompt_cache_retention(
            &compat,
            openai_completions::CacheRetention::Long,
        ),
        Some("24h")
    );
}

#[test]
fn openai_completions_omits_prompt_cache_retention_when_unsupported() {
    let model = completions_model(Some(openai_completions::OpenAICompletionsCompat {
        supports_long_cache_retention: Some(false),
        ..openai_completions::OpenAICompletionsCompat::default()
    }));
    let compat = openai_completions::get_compat(&model);

    assert_eq!(
        openai_completions::prompt_cache_key(
            &model,
            &compat,
            openai_completions::CacheRetention::Long,
            Some("session-completions-false"),
        ),
        None
    );
    assert_eq!(
        openai_completions::prompt_cache_retention(
            &compat,
            openai_completions::CacheRetention::Long,
        ),
        None
    );
}

#[test]
fn openai_completions_omits_long_cache_retention_for_opencode_models() {
    for id in [
        "deepseek-v4-flash",
        "deepseek-v4-pro",
        "kimi-k2.5",
        "kimi-k2.6",
        "minimax-m2.7",
    ] {
        let metadata = OPENCODE_MODELS
            .iter()
            .find(|model| model.id == id)
            .unwrap_or_else(|| panic!("missing opencode model {id}"));
        assert_eq!(
            metadata
                .compat
                .and_then(|compat| compat.supports_long_cache_retention),
            Some(false),
            "{}/{} should not support long cache retention",
            metadata.provider,
            metadata.id
        );
    }

    let metadata = OPENCODE_GO_MODELS
        .iter()
        .find(|model| model.id == "kimi-k2.6")
        .expect("missing opencode-go kimi-k2.6 model");
    assert_eq!(
        metadata
            .compat
            .and_then(|compat| compat.supports_long_cache_retention),
        Some(false),
        "{}/{} should not support long cache retention",
        metadata.provider,
        metadata.id
    );
}
