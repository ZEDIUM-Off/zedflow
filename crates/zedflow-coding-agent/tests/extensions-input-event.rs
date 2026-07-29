use std::sync::Arc;

use serde_json::json;
use zedflow_coding_agent::extensions::{ExtensionEventKind, ExtensionRunner, InputEvent};

#[test]
fn input_replacement_composes_until_consumed() {
    let mut runner = ExtensionRunner::new(vec![]);
    runner.on(
        "replace",
        ExtensionEventKind::Input,
        Arc::new(|_, _| Ok(Some(json!({"replacement":"rewritten"})))),
    );
    runner.on(
        "consume",
        ExtensionEventKind::Input,
        Arc::new(|_, _| Ok(Some(json!({"consume":true})))),
    );
    runner.on(
        "never",
        ExtensionEventKind::Input,
        Arc::new(|_, _| panic!("consumed input propagated")),
    );
    assert_eq!(
        runner.emit_input(InputEvent::Text("original".into())),
        zedflow_coding_agent::extensions::InputEventResult {
            consumed: true,
            replacement: Some("rewritten".into())
        }
    );
}
