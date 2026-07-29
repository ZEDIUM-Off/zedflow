use std::{
    io,
    sync::{Arc, Mutex},
};

use serde_json::json;
use zedflow_coding_agent::{
    extensions::{
        ExtensionEventKind, ExtensionRunner, ExtensionRuntime, ProviderConfig, RegisteredCommand,
        SessionActionResult, define_tool,
    },
    modes::interactive::InteractiveMode,
};
use zedflow_tui::ProcessTerminal;

#[derive(Clone)]
struct Writer(Arc<Mutex<Vec<u8>>>);
impl io::Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn interactive_host_uses_one_runner_for_lifecycle_input_tools_commands_and_providers() {
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
        Arc::new(|_, _| Ok(SessionActionResult { cancelled: true })),
    );
    runtime.register_provider(ProviderConfig {
        name: "native-provider".into(),
        config: json!({"native": true}),
    });
    let mut runner = ExtensionRunner::with_runtime(Vec::new(), runtime);
    let events = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&events);
    runner.on(
        "native",
        ExtensionEventKind::SessionStart,
        Arc::new(move |event, _| {
            seen.lock().unwrap().push(event.kind.clone());
            Ok(None)
        }),
    );
    let terminal = ProcessTerminal::with_writer(Box::new(Writer(Arc::new(Mutex::new(Vec::new())))));
    let mut mode = InteractiveMode::with_extension_runner(terminal, runner);

    mode.run().unwrap();
    mode.queue_user_input("hello");
    assert_eq!(mode.get_user_input().as_deref(), Some("hello"));
    assert_eq!(
        mode.invoke_extension_tool("native-tool", json!({"ok": true}))
            .unwrap(),
        json!({"ok": true})
    );
    assert!(
        mode.invoke_extension_command("native-command", &[])
            .unwrap()
            .cancelled
    );
    assert_eq!(mode.extension_providers()[0].name, "native-provider");
    assert_eq!(
        *events.lock().unwrap(),
        vec![ExtensionEventKind::SessionStart]
    );
    mode.stop().unwrap();
}
