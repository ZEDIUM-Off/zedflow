use serde_json::json;
use zedflow_ai::api::google_shared::{
    AssistantContent, Context, Message, Model, ModelInput, StopReason, convert_messages,
};

fn model(api: &str, provider: &str, id: &str) -> Model {
    Model {
        id: id.into(),
        api: api.into(),
        provider: provider.into(),
        input: vec![ModelInput::Text],
    }
}

fn context(source: &Model, signature: Option<&str>) -> Context {
    Context {
        messages: vec![
            Message::User {
                content: zedflow_ai::api::google_shared::UserContent::Text("Hi".into()),
            },
            Message::Assistant {
                content: vec![
                    AssistantContent::ToolCall {
                        id: "call_1".into(),
                        name: "bash".into(),
                        arguments: Some(json!({"command":"echo hi"})),
                        thought_signature: signature.map(str::to_owned),
                    },
                    AssistantContent::ToolCall {
                        id: "call_2".into(),
                        name: "bash".into(),
                        arguments: Some(json!({"command":"ls"})),
                        thought_signature: None,
                    },
                ],
                api: source.api.clone(),
                provider: source.provider.clone(),
                model: source.id.clone(),
                stop_reason: StopReason::ToolUse,
            },
        ],
    }
}

#[test]
fn unsigned_calls_never_gain_a_signature_or_skip_validator_marker() {
    for (api, provider) in [
        ("google-generative-ai", "google"),
        ("google-vertex", "google-vertex"),
    ] {
        let target = model(api, provider, "gemini-3-pro-preview");
        let source = model(api, provider, "other-model");
        let converted = convert_messages(&target, &context(&source, None));
        let turn = converted
            .iter()
            .find(|turn| turn.role == "model")
            .expect("model turn");
        assert_eq!(
            turn.parts
                .iter()
                .filter(|part| part.function_call.is_some())
                .count(),
            2
        );
        assert!(
            turn.parts
                .iter()
                .all(|part| part.thought_signature.is_none())
        );
        assert!(
            !serde_json::to_string(turn)
                .unwrap()
                .contains("skip_thought_signature_validator")
        );
    }
}

#[test]
fn valid_same_model_signature_is_preserved_but_not_synthesized() {
    let target = model("google-generative-ai", "google", "gemini-3-pro-preview");
    let converted = convert_messages(&target, &context(&target, Some("AAAAAAAAAAAAAAAAAAAAAA==")));
    let calls = converted
        .iter()
        .find(|turn| turn.role == "model")
        .unwrap()
        .parts
        .iter()
        .filter(|part| part.function_call.is_some())
        .collect::<Vec<_>>();
    assert_eq!(
        calls[0].thought_signature.as_deref(),
        Some("AAAAAAAAAAAAAAAAAAAAAA==")
    );
    assert!(calls[1].thought_signature.is_none());
}
