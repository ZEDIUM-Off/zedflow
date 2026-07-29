use std::sync::{Arc, Mutex};

use serde_json::json;
use zedflow_coding_agent::extensions::{
    Extension, ExtensionError, ExtensionEvent, ExtensionEventKind, ExtensionRunner, define_tool,
};

fn runner() -> ExtensionRunner {
    ExtensionRunner::new(Vec::<Extension>::new())
}

#[test]
fn event_errors_do_not_stop_later_handlers() {
    let mut runner = runner();
    let errors = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let errors_out = Arc::clone(&errors);
    runner.set_error_listener(Arc::new(move |error| {
        errors_out.lock().unwrap().push(error.message)
    }));
    runner.on(
        "broken",
        ExtensionEventKind::AgentStart,
        Arc::new(|_, _| {
            Err(ExtensionError {
                message: "boom".into(),
                source: None,
            })
        }),
    );
    let seen_out = Arc::clone(&seen);
    runner.on(
        "next",
        ExtensionEventKind::AgentStart,
        Arc::new(move |_, _| {
            seen_out.lock().unwrap().push("ran");
            Ok(Some(json!("ok")))
        }),
    );

    assert_eq!(
        runner.emit(ExtensionEvent {
            kind: ExtensionEventKind::AgentStart,
            data: json!({})
        }),
        vec![json!("ok")]
    );
    assert_eq!(*seen.lock().unwrap(), vec!["ran"]);
    assert_eq!(*errors.lock().unwrap(), vec!["broken: boom"]);
}

#[test]
fn tools_use_current_context_and_reject_stale_contexts() {
    let mut runner = runner();
    runner.runtime.register_tool(
        define_tool("cwd", "return cwd"),
        Arc::new(|_, ctx| Ok(json!(ctx.cwd))),
    );
    runner.set_context(
        zedflow_coding_agent::extensions::ExtensionMode::Tui,
        "/work",
        true,
    );
    assert_eq!(
        runner.invoke_tool("cwd", json!({})).unwrap(),
        json!("/work")
    );
    runner.invalidate_context();
    assert_eq!(
        runner.invoke_tool("cwd", json!({})).unwrap_err().message,
        "extension context is stale"
    );
}
