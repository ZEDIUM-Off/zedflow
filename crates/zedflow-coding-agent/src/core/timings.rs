use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::Instant,
};

#[derive(Default)]
struct Namespace {
    timings: Vec<(String, u128)>,
    last_time: Option<Instant>,
}
static TIMINGS: OnceLock<Mutex<HashMap<&'static str, Namespace>>> = OnceLock::new();
fn enabled() -> bool {
    std::env::var("PI_TIMING").as_deref() == Ok("1")
}
fn timings() -> &'static Mutex<HashMap<&'static str, Namespace>> {
    TIMINGS.get_or_init(Default::default)
}

pub fn reset_timings(namespace: &'static str) {
    if enabled() {
        timings().lock().unwrap().insert(
            namespace,
            Namespace {
                timings: vec![],
                last_time: Some(Instant::now()),
            },
        );
    }
}

pub fn time(label: impl Into<String>, namespace: &'static str) {
    if !enabled() {
        return;
    }
    let now = Instant::now();
    let mut all = timings().lock().unwrap();
    let group = all.entry(namespace).or_insert_with(|| Namespace {
        timings: vec![],
        last_time: Some(now),
    });
    group.timings.push((
        label.into(),
        group
            .last_time
            .map_or(0, |last| now.duration_since(last).as_millis()),
    ));
    group.last_time = Some(now);
}

pub fn format_timings() -> String {
    if !enabled() {
        return String::new();
    }
    let mut output = String::new();
    for (namespace, group) in timings().lock().unwrap().iter() {
        if group.timings.is_empty() {
            continue;
        }
        let title = format!("Startup Timings: {namespace}");
        output.push_str(&format!("\n--- {title} ---\n"));
        for (label, ms) in &group.timings {
            output.push_str(&format!("  {label}: {ms}ms\n"));
        }
        output.push_str(&format!(
            "  TOTAL: {}ms\n{}\n",
            group.timings.iter().map(|(_, ms)| ms).sum::<u128>(),
            "-".repeat(title.len() + 8)
        ));
    }
    output
}

pub fn print_timings() {
    let output = format_timings();
    if !output.is_empty() {
        eprint!("{output}");
    }
}
