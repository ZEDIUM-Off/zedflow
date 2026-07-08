use std::collections::HashMap;

use zedflow_ai::api::anthropic_messages;
use zedflow_ai::api::openai_completions;
use zedflow_ai::api::openai_responses;
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
#[ignore = "anthropic_messages::stream does not build request payloads until the Anthropic SDK/HTTP-SSE dependency is selected"]
fn anthropic_adds_cache_control_to_string_user_messages() {
    panic!("Pi asserts the last string user-message block carries cache_control");
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
