use std::{
    fs, io,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde_json::json;
use zedflow_coding_agent::{
    extensions::{
        ExtensionEventKind, ExtensionRunner, ExtensionRuntime, ExtensionSource,
        NativeExtensionInstall, ProviderConfig, RegisteredCommand, SessionActionResult,
        define_tool, receipt,
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
fn default_startup_loads_persisted_digest_bound_native_extension() {
    let manifest =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rust-extension/Cargo.toml");
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_zedflow-coding-agent"));
    let target = std::env::temp_dir().join("zedflow-interactive-native-extension-target");
    assert!(
        Command::new("cargo")
            .args(["build", "--manifest-path"])
            .arg(manifest)
            .args(["--target-dir"])
            .arg(&target)
            .status()
            .unwrap()
            .success()
    );
    let artifact = target.join("debug").join(format!(
        "{}zedflow_rust_extension_fixture{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    ));
    let root = std::env::temp_dir().join(format!(
        "zedflow-interactive-native-extension-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let extensions = root.join(".pi/extensions");
    let source = extensions.join("source");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("Cargo.toml"), "[package]\nname='native'\n").unwrap();
    let install = NativeExtensionInstall {
        source_dir: source.clone(),
        artifact: artifact.clone(),
        receipt: receipt(
            &ExtensionSource::Path(source.clone()),
            &source,
            &artifact,
            None,
        )
        .unwrap(),
    };
    install.persist(&extensions).unwrap();

    let mut child = Command::new(binary)
        .current_dir(&root)
        .env("PI_CODING_AGENT_DIR", root.join("agent"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(1);
    let mut running = true;
    while Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            running = false;
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    if running {
        child.kill().unwrap();
    }
    child.wait().unwrap();
    let _ = fs::remove_dir_all(root);

    assert!(
        running,
        "default startup rejected a persisted digest-bound native extension"
    );
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
