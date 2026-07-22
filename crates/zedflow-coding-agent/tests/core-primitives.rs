use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use zedflow_coding_agent::{
    event_bus::create_event_bus, provider_display_names::provider_display_name, session_cwd::*,
    timings,
};

struct Source {
    cwd: PathBuf,
    file: Option<PathBuf>,
}
impl SessionCwdSource for Source {
    fn cwd(&self) -> &Path {
        &self.cwd
    }
    fn session_file(&self) -> Option<&Path> {
        self.file.as_deref()
    }
}

#[test]
fn event_bus_unsubscribes_and_clears() {
    let bus = create_event_bus();
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let off = bus.on("channel", move |_| {
        count.fetch_add(1, Ordering::SeqCst);
    });
    bus.emit("channel", Arc::new(1_u8));
    off();
    bus.emit("channel", Arc::new(2_u8));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let count = calls.clone();
    let _off = bus.on("channel", move |_| {
        count.fetch_add(1, Ordering::SeqCst);
    });
    bus.clear();
    bus.emit("channel", Arc::new(3_u8));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn provider_names_include_fallback() {
    assert_eq!(provider_display_name("google"), "Google Gemini");
    assert_eq!(provider_display_name("custom"), "custom");
}

#[test]
fn timings_capture_enablement_and_preserve_namespace_order() {
    // SAFETY: this test is the only timing/environment user in this test binary.
    unsafe { std::env::set_var("PI_TIMING", "1") };
    timings::reset_timings("extensions");
    timings::reset_timings("main");
    timings::reset_timings("extensions");
    unsafe { std::env::remove_var("PI_TIMING") };
    timings::time("extension ready", "extensions");
    timings::time("main ready", "main");

    let output = timings::format_timings();
    assert!(output.contains("extension ready"));
    assert!(
        output.find("Startup Timings: extensions").unwrap()
            < output.find("Startup Timings: main").unwrap()
    );
}

#[test]
fn missing_session_cwd_is_reported() {
    let missing = std::env::temp_dir().join("zedflow-definitely-missing-session-cwd");
    let source = Source {
        cwd: missing.clone(),
        file: Some(PathBuf::from("session.jsonl")),
    };
    let issue = get_missing_session_cwd_issue(&source, Path::new("fallback")).unwrap();
    assert_eq!(
        format_missing_session_cwd_prompt(&issue),
        format!(
            "cwd from session file does not exist\n{}\n\ncontinue in current cwd\nfallback",
            missing.display()
        )
    );
    assert!(assert_session_cwd_exists(&source, Path::new("fallback")).is_err());
}
