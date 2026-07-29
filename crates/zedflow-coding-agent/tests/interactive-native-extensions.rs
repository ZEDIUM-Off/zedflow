use std::{
    io::Cursor,
    sync::{Arc, Mutex},
};

use zedflow_tui::ProcessTerminal;

use serde_json::json;
use zedflow_coding_agent::{
    core::resource_loader::DefaultResourceLoader,
    extensions::{
        ExtensionEventKind, ExtensionRunner, ExtensionRuntime, RegisteredCommand, define_tool,
    },
    modes::InteractiveMode,
};

#[test]
fn default_interactive_harness_retains_one_runner_for_extension_operations() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut runtime = ExtensionRuntime::default();
    runtime.register_tool(
        define_tool("native-tool", "test"),
        Arc::new(|arguments, _| Ok(arguments)),
    );
    runtime.register_command(
        RegisteredCommand {
            name: "native-command".into(),
            description: "test".into(),
        },
        Arc::new(|_, _| Ok(Default::default())),
    );
    let mut runner = ExtensionRunner::with_runtime(Vec::new(), runtime);
    for kind in [
        ExtensionEventKind::SessionStart,
        ExtensionEventKind::Input,
        ExtensionEventKind::BeforeProviderRequest,
        ExtensionEventKind::SessionShutdown,
    ] {
        let events = Arc::clone(&events);
        runner.on(
            "native",
            kind,
            Arc::new(move |event, _| {
                events.lock().unwrap().push(event.kind.clone());
                Ok(if event.kind == ExtensionEventKind::Input {
                    Some(json!({"replacement":"rewritten"}))
                } else {
                    None
                })
            }),
        );
    }

    let mut mode = InteractiveMode::with_terminal_and_extensions(
        ProcessTerminal::with_reader_and_writer(
            Box::new(Cursor::new(Vec::new())),
            Box::new(Vec::new()),
        ),
        runner,
    );
    // This is the default resource configuration path: no unconfigured native
    // artifact can enter the host, but the runner is still retained by it.
    let loader = DefaultResourceLoader::new(".", "./.pi/agent");
    assert!(loader.native_extension_runner().is_ok());
    mode.run().unwrap();
    mode.queue_user_input("original");
    assert_eq!(mode.get_user_input(), Some("rewritten".into()));
    assert_eq!(
        mode.invoke_extension_tool("native-tool", json!({"x":1}))
            .unwrap(),
        json!({"x":1})
    );
    assert!(
        !mode
            .invoke_extension_command("native-command", &[])
            .unwrap()
            .cancelled
    );
    mode.emit_provider_event(ExtensionEventKind::BeforeProviderRequest, json!({}));
    drop(mode);

    let events = events.lock().unwrap();
    assert!(events.contains(&ExtensionEventKind::SessionStart));
    assert!(events.contains(&ExtensionEventKind::Input));
    assert!(events.contains(&ExtensionEventKind::BeforeProviderRequest));
    assert!(events.contains(&ExtensionEventKind::SessionShutdown));
}
