use serde_json::json;
use zedflow_ai::api::openai_responses_shared::{
    AssistantContent, AssistantMessage, AssistantMessageEvent, Model, Response, ResponseOutputItem,
    ResponseStatus, ResponseStreamEvent, StopReason, Usage, process_responses_stream,
};

#[test]
fn output_item_done_persists_only_final_tool_arguments() {
    let model = Model {
        id: "gpt-5-mini".into(),
        api: "openai-responses".into(),
        provider: "openai".into(),
        reasoning: true,
        input: vec!["text".into()],
        cost: Default::default(),
        compat: None,
    };
    let mut output = AssistantMessage {
        content: vec![],
        api: model.api.clone(),
        provider: model.provider.clone(),
        model: model.id.clone(),
        response_id: None,
        usage: Usage::default(),
        stop_reason: StopReason::Stop,
    };
    let arguments = r#"{"path":"README.md","content":"updated"}"#;
    let function = |arguments: &str| ResponseOutputItem::FunctionCall {
        id: Some("fc_test".into()),
        call_id: "call_test".into(),
        name: "edit".into(),
        arguments: arguments.into(),
    };
    let mut events = vec![];
    process_responses_stream(
        vec![
            ResponseStreamEvent::ResponseOutputItemAdded {
                output_index: 0,
                item: function(""),
            },
            ResponseStreamEvent::ResponseFunctionCallArgumentsDelta {
                output_index: 0,
                delta: r#"{"path":"README.md""#.into(),
            },
            ResponseStreamEvent::ResponseFunctionCallArgumentsDelta {
                output_index: 0,
                delta: r#", "content":"updated"}"#.into(),
            },
            ResponseStreamEvent::ResponseFunctionCallArgumentsDone {
                output_index: 0,
                arguments: arguments.into(),
            },
            ResponseStreamEvent::ResponseOutputItemDone {
                output_index: 0,
                item: function(arguments),
            },
            ResponseStreamEvent::ResponseCompleted {
                response: Response {
                    id: Some("resp_test".into()),
                    status: Some(ResponseStatus::Completed),
                    ..Response::default()
                },
            },
        ],
        &mut output,
        &mut events,
        &model,
        None,
    )
    .expect("complete stream");
    let AssistantContent::ToolCall(tool) = &output.content[0] else {
        panic!("tool call")
    };
    assert_eq!(
        tool.arguments,
        json!({"path":"README.md","content":"updated"})
    );
    assert!(
        serde_json::to_value(tool)
            .unwrap()
            .get("partialJson")
            .is_none()
    );
    let ended = events
        .iter()
        .find_map(|event| match event {
            AssistantMessageEvent::ToolCallEnd { tool_call, .. } => Some(tool_call),
            _ => None,
        })
        .expect("tool end");
    assert_eq!(ended, tool);
}
