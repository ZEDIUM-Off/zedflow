use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::{Arc, Mutex},
};

use serde_json::json;
use sha2::{Digest, Sha256};
use zedflow_coding_agent::extensions::loader::load_native_extensions;
use zedflow_coding_agent::extensions::{
    ABI_V1, Extension, ExtensionError, ExtensionEvent, ExtensionEventKind, ExtensionRunner,
    JsonEnvelope, NativeExtensionArtifact, define_tool,
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

#[test]
fn native_artifacts_activate_into_the_production_runner() {
    let manifest =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rust-extension/Cargo.toml");
    let target = std::env::temp_dir().join("zedflow-native-runner-fixture-target");
    assert!(
        Command::new("cargo")
            .args(["build", "--manifest-path"])
            .arg(manifest)
            .args(["--target-dir"])
            .arg(&target)
            .status()
            .expect("build native fixture")
            .success()
    );

    let path = target.join("debug").join(format!(
        "{}zedflow_rust_extension_fixture{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    ));
    let artifact = NativeExtensionArtifact {
        sha256: format!(
            "{:x}",
            Sha256::digest(fs::read(&path).expect("read native fixture"))
        ),
        path,
        trusted: true,
    };
    let mut runner = load_native_extensions(
        &[artifact],
        &JsonEnvelope {
            version: ABI_V1,
            payload: json!({"start": true}),
        },
    )
    .expect("load and activate native fixture");
    runner.set_context(
        zedflow_coding_agent::extensions::ExtensionMode::Tui,
        "/work",
        true,
    );

    assert_eq!(runner.extensions.len(), 1);
    assert_eq!(
        runner
            .invoke_tool("fixture-tool", json!({"answer": 42}))
            .unwrap(),
        json!({"echo": {"arguments": {"answer": 42}, "context": {"cwd": "/work", "generation": 0, "hasUi": true}, "kind": "tool", "name": "fixture-tool"}})
    );
}
